//! `tics`: flag possible AI-writing tics in Markdown and plain-text prose.

use std::io::Write as _;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser as _;

mod cli;
mod document;
mod engine;
mod inputs;
mod output;
mod rules;
mod score;
mod sentences;

#[cfg(test)]
mod rule_tests;

use cli::{Cli, Command, Format};
use rules::{RULES, Rule};

/// Usage and IO errors; clap uses the same code for argument errors.
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("tics: {error:#}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

fn run(cli: &Cli) -> Result<ExitCode> {
    if matches!(cli.command, Some(Command::Rules)) {
        let text = match cli.format {
            Format::Human => output::rules_human(RULES),
            Format::Json => output::rules_json(RULES)?,
        };
        write_stdout(&text)?;
        return Ok(ExitCode::SUCCESS);
    }
    let selected = select_rules(cli)?;
    let compiled = engine::compile(&selected)?;
    let paths = match &cli.command {
        Some(Command::Score { paths, .. }) => paths,
        _ => &cli.paths,
    };
    let mut reports = Vec::new();
    for input in inputs::collect(paths)? {
        let (path, text) = inputs::read(&input)?;
        let report = engine::scan(&path, &text, inputs::mode_of(&input), &compiled);
        reports.push((path, report));
    }
    if let Some(Command::Score { sections, .. }) = cli.command {
        let text = match cli.format {
            Format::Human => output::scores_human(&reports, sections),
            Format::Json => output::scores_json(&reports)?,
        };
        write_stdout(&text)?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut findings = Vec::new();
    let mut words = 0;
    for (_, report) in reports {
        words += report.words;
        findings.extend(
            report
                .findings
                .into_iter()
                .filter(|finding| finding.confidence >= cli.min_confidence),
        );
    }
    let summary = cli.summary.then(|| output::Summary::new(&findings, words));
    let text = match cli.format {
        Format::Human => output::human(&findings, summary.as_ref()),
        Format::Json => output::json(&findings, summary.as_ref())?,
    };
    write_stdout(&text)?;
    Ok(if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Applies `--select`, `--ignore` and `--severity`; unknown ids are usage errors.
fn select_rules(cli: &Cli) -> Result<Vec<&'static Rule>> {
    for id in cli.select.iter().chain(&cli.ignore) {
        if rules::by_id(id).is_none() {
            bail!("unknown rule id `{id}` (run `tics rules` to list them)");
        }
    }
    Ok(RULES
        .iter()
        .filter(|rule| cli.select.is_empty() || cli.select.iter().any(|id| id == rule.id))
        .filter(|rule| !cli.ignore.iter().any(|id| id == rule.id))
        .filter(|rule| rule.severity >= cli.severity)
        .collect())
}

/// Writes everything at once; a closed pipe (`tics … | head`) is not an error.
fn write_stdout(text: &str) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        result => result.context("writing to stdout"),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::Parser as _;

    use super::select_rules;
    use crate::cli::Cli;
    use crate::rules::{RULES, Severity};

    fn ids(args: &[&str]) -> Result<Vec<&'static str>> {
        let cli = Cli::try_parse_from(args)?;
        Ok(select_rules(&cli)?.iter().map(|rule| rule.id).collect())
    }

    #[test]
    fn default_selection_is_every_rule() -> Result<()> {
        assert_eq!(ids(&["tics"])?.len(), RULES.len());
        Ok(())
    }

    #[test]
    fn select_ignore_and_severity_narrow_the_rules() -> Result<()> {
        assert_eq!(
            ids(&[
                "tics",
                "--select",
                "em-dash,rather-than",
                "--ignore",
                "rather-than"
            ])?,
            ["em-dash"]
        );
        let warn_only = ids(&["tics", "--severity", "warn"])?;
        assert!(!warn_only.contains(&"rather-than"));
        assert_eq!(
            warn_only.len(),
            RULES
                .iter()
                .filter(|rule| rule.severity == Severity::Warn)
                .count()
        );
        Ok(())
    }

    #[test]
    fn unknown_rule_id_is_an_error() -> Result<()> {
        let cli = Cli::try_parse_from(["tics", "--ignore", "nope"])?;
        assert!(select_rules(&cli).is_err());
        Ok(())
    }
}
