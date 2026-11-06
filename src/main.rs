//! `tics`: flag possible AI-writing tics in Markdown and plain-text prose.

use std::io::{IsTerminal as _, Write as _};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser as _;

mod cli;
mod config;
mod document;
mod engine;
mod inputs;
mod output;
mod review;
mod review_rules;
mod rules;
mod score;
mod sentences;

#[cfg(test)]
mod rule_tests;

use cli::{Cli, ColorChoice, Command, Format};
use rules::{RULES, Rule, Severity};

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

#[allow(clippy::too_many_lines)]
fn run(cli: &Cli) -> Result<ExitCode> {
    let terminal = std::io::stdout().is_terminal();
    if matches!(cli.command, Some(Command::Instructions)) {
        write_stdout(review::INSTRUCTIONS)?;
        return Ok(ExitCode::SUCCESS);
    }
    let reviewing = matches!(cli.command, Some(Command::Review { .. }))
        || matches!(cli.format, Some(Format::Jsonl | Format::Ci));
    let format = output_format(cli, terminal, reviewing);
    if matches!(cli.command, Some(Command::Score { .. }))
        && matches!(format, Format::Jsonl | Format::Ci)
    {
        bail!("use --format json for file scores");
    }
    if matches!(cli.command, Some(Command::Rules)) {
        let text = match format {
            Format::Pretty | Format::Compact => output::rules_human(RULES),
            Format::Json => output::rules_json(RULES)?,
            Format::Jsonl | Format::Ci => bail!("use --format json for the rule catalog"),
        };
        write_stdout(&text)?;
        return Ok(ExitCode::SUCCESS);
    }
    let available: Vec<_> = RULES.iter().chain(review_rules::REVIEW_RULES).collect();
    let config = match &cli.command {
        Some(Command::Review {
            config: Some(path), ..
        }) => config::Config::load(path, &available)?,
        _ => config::Config::default(),
    };
    let selected = if reviewing {
        select_review_rules(cli, &config, &available)?
    } else {
        select_rules(cli)?
    };
    let allows = config.allows()?;
    let compiled = engine::compile(&selected)?;
    let paths = match &cli.command {
        Some(Command::Score { paths, .. } | Command::Review { paths, .. }) => paths,
        _ => &cli.paths,
    };
    let mut reports = Vec::new();
    let mut review_files = Vec::new();
    if reviewing && cli.min_confidence > 0.0 {
        bail!("--min-confidence applies to legacy authorship-style scores, not review candidates");
    }
    let sources = if reviewing {
        inputs::collect_review(paths)?
    } else {
        inputs::collect(paths)?
    };
    for input in sources {
        let (path, report, review_file) = scan_input(
            &input, &compiled, &selected, reviewing, &config, &allows, cli,
        )?;
        if let Some(file) = review_file {
            review_files.push(file);
        }
        reports.push((path, report));
    }
    if let Some(Command::Score { sections, .. }) = cli.command {
        let text = match format {
            Format::Pretty | Format::Compact => output::scores_human(&reports, sections),
            Format::Json => output::scores_json(&reports)?,
            Format::Jsonl | Format::Ci => bail!("use --format json for file scores"),
        };
        write_stdout(&text)?;
        return Ok(ExitCode::SUCCESS);
    }
    if reviewing {
        return write_review(&review_files, format);
    }
    for (_, report) in &mut reports {
        report
            .findings
            .retain(|finding| finding.confidence >= cli.min_confidence);
    }
    let words = reports.iter().map(|(_, report)| report.words).sum();
    let findings: Vec<_> = reports
        .iter()
        .flat_map(|(_, report)| report.findings.iter().cloned())
        .collect();
    let summary = cli.summary.then(|| output::Summary::new(&findings, words));
    let text = match format {
        Format::Pretty => {
            let style = output::Style {
                color: match cli.color {
                    ColorChoice::Always => true,
                    ColorChoice::Never => false,
                    ColorChoice::Auto => {
                        terminal
                            && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                    }
                },
                width: terminal_width(),
            };
            output::pretty(&reports, summary.as_ref(), &style)
        }
        Format::Compact => output::human(&findings, summary.as_ref()),
        Format::Json => output::json(&findings, summary.as_ref())?,
        Format::Jsonl | Format::Ci => unreachable!("review output was handled above"),
    };
    write_stdout(&text)?;
    Ok(if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn scan_input(
    input: &inputs::Input,
    compiled: &[engine::Compiled],
    selected: &[&Rule],
    reviewing: bool,
    config: &config::Config,
    allows: &[regex::Regex],
    cli: &Cli,
) -> Result<(String, engine::FileReport, Option<review::ReviewFile>)> {
    let (path, text) = if reviewing {
        inputs::read_raw(input)?
    } else {
        inputs::read(input)?
    };
    let body = if reviewing {
        text.strip_prefix('\u{feff}').unwrap_or(&text)
    } else {
        text.as_str()
    };
    let report = if reviewing {
        engine::scan_review(&path, body, inputs::mode_of(input), compiled)
    } else {
        engine::scan(&path, body, inputs::mode_of(input), compiled)
    };
    let file = reviewing.then(|| {
        review::file(
            &path,
            &text,
            inputs::mode_of(input),
            &report,
            selected,
            config,
            allows,
            cli,
        )
    });
    Ok((path, report, file))
}

fn output_format(cli: &Cli, terminal: bool, reviewing: bool) -> Format {
    if cli.json {
        return Format::Json;
    }
    cli.format.unwrap_or(if reviewing && !terminal {
        Format::Jsonl
    } else if terminal {
        Format::Pretty
    } else {
        Format::Compact
    })
}

fn write_review(files: &[review::ReviewFile], format: Format) -> Result<ExitCode> {
    let text = match format {
        Format::Json => review::json(files)?,
        Format::Jsonl => review::jsonl(files)?,
        Format::Ci => review::ci(files)?,
        Format::Pretty | Format::Compact => review::human(files),
    };
    write_stdout(&text)?;
    Ok(if review::has_candidates(files) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Columns available for pretty output: `COLUMNS` when set, else 100.
fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|columns| columns.parse().ok())
        .unwrap_or(100)
}

/// Applies `--select`, `--ignore` and `--severity`; unknown ids are usage errors.
fn select_rules(cli: &Cli) -> Result<Vec<&'static Rule>> {
    select_rules_for(cli, false)
}

fn select_rules_for(cli: &Cli, reviewing: bool) -> Result<Vec<&'static Rule>> {
    let available: Vec<_> = RULES
        .iter()
        .chain(if reviewing {
            review_rules::REVIEW_RULES
        } else {
            &[]
        })
        .collect();
    for id in cli.select.iter().chain(&cli.ignore) {
        if !available.iter().any(|rule| rule.id == id) {
            bail!("unknown rule id `{id}` (run `tics rules` to list them)");
        }
    }
    Ok(available
        .into_iter()
        .filter(|rule| cli.select.is_empty() || cli.select.iter().any(|id| id == rule.id))
        .filter(|rule| !cli.ignore.iter().any(|id| id == rule.id))
        .filter(|rule| rule.severity >= cli.severity.unwrap_or(Severity::Info))
        .collect())
}

fn select_review_rules<'a>(
    cli: &Cli,
    config: &config::Config,
    available: &[&'a Rule],
) -> Result<Vec<&'a Rule>> {
    let select = if cli.select.is_empty() {
        config.select.as_deref()
    } else {
        Some(cli.select.as_slice())
    };
    // Validate configuration even when a CLI override replaces its selection.
    for id in cli
        .select
        .iter()
        .chain(&cli.ignore)
        .chain(config.select.iter().flatten())
        .chain(&config.ignore)
    {
        if !available.iter().any(|r| r.id == id) && !config.custom_rules.iter().any(|r| r.id == *id)
        {
            bail!("unknown rule id `{id}` (run `tics rules` to list built-in rules)");
        }
    }
    let severity = cli.severity.or(config.severity).unwrap_or(Severity::Info);
    Ok(available
        .iter()
        .copied()
        .filter(|r| {
            select.is_none_or(|ids| ids.iter().any(|id| id == r.id))
                && !cli.ignore.iter().chain(&config.ignore).any(|id| id == r.id)
                && r.severity >= severity
        })
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
