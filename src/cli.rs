//! Command-line definition (clap derive).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::rules::Severity;

/// Flag possible AI-writing tics in Markdown and plain-text prose.
///
/// Tics are signals, not proof: read the density in `--summary`, not single hits.
#[derive(Debug, Parser)]
#[command(version, about, args_conflicts_with_subcommands = true)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,

    /// Files or directories to check (`.md`, `.markdown`, `.mdx`, `.txt`); `-` or none reads stdin.
    #[arg(value_name = "PATH")]
    pub(crate) paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Human, global = true)]
    pub(crate) format: Format,

    /// Print per-rule counts, total, words scanned and hits per 1,000 words.
    #[arg(long)]
    pub(crate) summary: bool,

    /// Only run these rules (comma-separated ids).
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub(crate) select: Vec<String>,

    /// Skip these rules (comma-separated ids).
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub(crate) ignore: Vec<String>,

    /// Minimum severity to report.
    #[arg(long, value_enum, default_value_t = Severity::Info)]
    pub(crate) severity: Severity,

    /// Only report findings at or above this confidence (0 to 1). A lone hit in a long file
    /// scores low; many hits of the same kind score high.
    #[arg(long, value_name = "N", default_value_t = 0.0, value_parser = parse_confidence, global = true)]
    pub(crate) min_confidence: f64,
}

fn parse_confidence(text: &str) -> Result<f64, String> {
    match text.parse::<f64>() {
        Ok(value) if (0.0..=1.0).contains(&value) => Ok(value),
        _ => Err(format!("`{text}` is not a number from 0 to 1")),
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// List the rules with id, category, severity, since, source and description.
    Rules,
    /// Score each file from 0 to 1 for how AI-styled its tics make it look.
    Score {
        /// Files or directories to score; `-` or none reads stdin.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
        /// Also score each heading section on its own.
        #[arg(long)]
        sections: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    Human,
    Json,
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, Format};
    use crate::rules::Severity;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn defaults_read_stdin_in_human_format_at_info() -> Result<()> {
        let cli = Cli::try_parse_from(["tics"])?;
        assert!(cli.paths.is_empty() && cli.command.is_none() && !cli.summary);
        assert_eq!((cli.format, cli.severity), (Format::Human, Severity::Info));
        Ok(())
    }

    #[test]
    fn select_and_ignore_split_on_commas() -> Result<()> {
        let cli = Cli::try_parse_from(["tics", "--select", "em-dash,rather-than", "a.md"])?;
        assert_eq!(cli.select, ["em-dash", "rather-than"]);
        assert_eq!(cli.paths.len(), 1);
        Ok(())
    }

    #[test]
    fn rules_subcommand_accepts_format_after_it() -> Result<()> {
        let cli = Cli::try_parse_from(["tics", "rules", "--format", "json"])?;
        assert!(matches!(cli.command, Some(Command::Rules)));
        assert_eq!(cli.format, Format::Json);
        Ok(())
    }

    #[test]
    fn unknown_severity_is_rejected() {
        assert!(Cli::try_parse_from(["tics", "--severity", "error"]).is_err());
    }
}
