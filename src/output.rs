//! Renders findings, the summary and the rule list as human-readable text or JSON.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::engine::{FileReport, Finding, Section};
use crate::rules::{Category, Rule, Severity};
use crate::score::{self, Unit, Units};

#[derive(Debug, Serialize)]
pub(crate) struct Summary {
    /// Hits per rule id, sorted by id.
    pub(crate) rules: BTreeMap<&'static str, usize>,
    pub(crate) total: usize,
    pub(crate) words: usize,
    pub(crate) per_1000_words: f64,
}

impl Summary {
    pub(crate) fn new(findings: &[Finding], words: usize) -> Self {
        let mut rules = BTreeMap::new();
        for finding in findings {
            *rules.entry(finding.rule).or_insert(0) += 1;
        }
        Self {
            rules,
            total: findings.len(),
            words,
            per_1000_words: density(findings.len(), words),
        }
    }
}

/// Hits per 1,000 words, rounded to two decimals; 0 when nothing was scanned.
fn density(hits: usize, words: usize) -> f64 {
    if words == 0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts stay far below 2^52, where f64 is exact"
    )]
    let per_thousand = hits as f64 * 1000.0 / words as f64;
    (per_thousand * 100.0).round() / 100.0
}

pub(crate) fn human(findings: &[Finding], summary: Option<&Summary>) -> String {
    let mut out = String::new();
    for finding in findings {
        let _ = writeln!(
            out,
            "{}:{}:{}: {}: {} «{}» (confidence {:.2})",
            finding.path,
            finding.line,
            finding.column,
            finding.rule,
            finding.message,
            finding.matched,
            finding.confidence
        );
    }
    if let Some(summary) = summary {
        let _ = writeln!(
            out,
            "summary: {} possible AI-writing tics in {} words ({:.2} per 1,000 words)",
            summary.total, summary.words, summary.per_1000_words
        );
        let mut counts: Vec<(&&str, &usize)> = summary.rules.iter().collect();
        counts.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let width = counts.iter().map(|(id, _)| id.len()).max().unwrap_or(0);
        for (id, count) in counts {
            let _ = writeln!(out, "  {id:<width$}  {count}");
        }
    }
    out
}

#[derive(Serialize)]
struct JsonReport<'a> {
    findings: &'a [Finding],
    summary: &'a Summary,
}

/// A JSON array of findings; with a summary, an object `{findings, summary}`.
pub(crate) fn json(findings: &[Finding], summary: Option<&Summary>) -> Result<String> {
    let mut text = match summary {
        Some(summary) => serde_json::to_string_pretty(&JsonReport { findings, summary }),
        None => serde_json::to_string_pretty(findings),
    }
    .context("serialising findings as JSON")?;
    text.push('\n');
    Ok(text)
}

#[derive(Serialize)]
struct RuleInfo {
    id: &'static str,
    category: Category,
    severity: Severity,
    since: &'static str,
    /// What one opportunity is for this rule.
    unit: Unit,
    /// How well the rule separates AI from human text (signed Jeffreys divergence over ten
    /// opportunities, see `score.rs`); negative means people use the pattern more.
    strength: f64,
    source: &'static str,
    description: &'static str,
}

pub(crate) fn rules_human(rules: &[Rule]) -> String {
    let width = rules.iter().map(|rule| rule.id.len()).max().unwrap_or(0);
    let mut out = String::new();
    for rule in rules {
        let _ = writeln!(
            out,
            "{:<width$}  {:<10}  {:<4}  {}  {:+.2}  {}  {}",
            rule.id,
            rule.category.as_str(),
            rule.severity.as_str(),
            rule.since,
            score::strength(rule.id),
            rule.description,
            rule.source
        );
    }
    out
}

pub(crate) fn rules_json(rules: &[Rule]) -> Result<String> {
    let infos: Vec<RuleInfo> = rules
        .iter()
        .map(|rule| RuleInfo {
            id: rule.id,
            category: rule.category,
            severity: rule.severity,
            since: rule.since,
            unit: rule.unit(),
            strength: score::strength(rule.id),
            source: rule.source,
            description: rule.description,
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&infos).context("serialising rules as JSON")?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{Summary, density, human, json, scores_human, scores_json};
    use crate::engine::{FileReport, Finding};
    use crate::rules::{Category, Severity};
    use crate::score::Units;
    use std::collections::BTreeMap;

    fn finding(rule: &'static str) -> Finding {
        Finding {
            path: "a.md".into(),
            line: 2,
            column: 5,
            rule,
            category: Category::Formatting,
            severity: Severity::Warn,
            message: "possible AI-writing tic: em dash in prose".into(),
            matched: "—".into(),
            confidence: 0.56,
        }
    }

    #[test]
    fn density_is_hits_per_thousand_words_rounded() {
        assert!((density(3, 1500) - 2.0).abs() < f64::EPSILON);
        assert!((density(1, 3) - 333.33).abs() < 1e-9);
        assert!(density(5, 0).abs() < f64::EPSILON);
    }

    #[test]
    fn human_line_matches_the_spec_shape() {
        assert_eq!(
            human(&[finding("em-dash")], None),
            "a.md:2:5: em-dash: possible AI-writing tic: em dash in prose «—» (confidence 0.56)\n"
        );
    }

    #[test]
    fn human_summary_lists_counts_most_frequent_first() {
        let findings = [
            finding("em-dash"),
            finding("rather-than"),
            finding("rather-than"),
        ];
        let text = human(&findings, Some(&Summary::new(&findings, 600)));
        assert!(
            text.contains(
                "summary: 3 possible AI-writing tics in 600 words (5.00 per 1,000 words)\n"
            )
        );
        assert!(text.ends_with("  rather-than  2\n  em-dash      1\n"));
    }

    #[test]
    fn json_is_an_array_without_summary_and_an_object_with_it() -> Result<()> {
        let findings = [finding("em-dash")];
        let array: serde_json::Value = serde_json::from_str(&json(&findings, None)?)?;
        assert_eq!(array[0]["rule"], "em-dash");
        assert_eq!(array[0]["severity"], "warn");
        assert_eq!(array[0]["category"], "formatting");
        let summary = Summary::new(&findings, 10);
        let object: serde_json::Value = serde_json::from_str(&json(&findings, Some(&summary))?)?;
        assert_eq!(object["summary"]["total"], 1);
        assert_eq!(object["summary"]["rules"]["em-dash"], 1);
        Ok(())
    }

    fn report(rules: &[&'static str]) -> FileReport {
        FileReport {
            findings: rules.iter().map(|rule| finding(rule)).collect(),
            words: 120,
            units: Units::default(),
            occupied: BTreeMap::new(),
            score: Some(0.87),
            sections: Vec::new(),
        }
    }

    #[test]
    fn score_line_names_the_top_rules() {
        let reports = vec![
            (
                "a.md".to_owned(),
                report(&["x-not-y", "em-dash", "em-dash"]),
            ),
            ("b.md".to_owned(), report(&[])),
        ];
        assert_eq!(
            scores_human(&reports, false),
            "a.md: 0.87 (3 findings in 120 words): em-dash ×2, x-not-y ×1\n\
             b.md: 0.87 (0 findings in 120 words)\n"
        );
    }

    #[test]
    fn score_json_has_one_object_per_file() -> Result<()> {
        let reports = vec![("a.md".to_owned(), report(&["em-dash"]))];
        let value: serde_json::Value = serde_json::from_str(&scores_json(&reports)?)?;
        assert_eq!(value[0]["path"], "a.md");
        assert_eq!(value[0]["score"], 0.87);
        assert_eq!(value[0]["rules"]["em-dash"], 1);
        Ok(())
    }
}

/// A score to two decimals, or `n/a` for a file or section with no prose.
fn shown(score: Option<f64>) -> String {
    score.map_or_else(|| "n/a".to_owned(), |score| format!("{score:.2}"))
}

/// Rules with the most hits in a report, most first, for the score line.
fn top_rules(report: &FileReport) -> Vec<(&'static str, usize)> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for finding in &report.findings {
        *counts.entry(finding.rule).or_insert(0) += 1;
    }
    let mut top: Vec<(&'static str, usize)> = counts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    top
}

/// One line per file: score, findings and words, and the rules behind the score. With
/// `sections`, an indented line per heading section follows.
pub(crate) fn scores_human(reports: &[(String, FileReport)], sections: bool) -> String {
    let mut out = String::new();
    for (path, report) in reports {
        let rules: Vec<String> = top_rules(report)
            .iter()
            .take(3)
            .map(|(id, count)| format!("{id} ×{count}"))
            .collect();
        let count = report.findings.len();
        let noun = if count == 1 { "finding" } else { "findings" };
        let _ = write!(
            out,
            "{path}: {} ({count} {noun} in {} words)",
            shown(report.score),
            report.words
        );
        if !rules.is_empty() {
            let _ = write!(out, ": {}", rules.join(", "));
        }
        out.push('\n');
        if sections {
            for section in &report.sections {
                let title = section
                    .title
                    .as_deref()
                    .unwrap_or("(before the first heading)");
                let _ = writeln!(
                    out,
                    "  {}: {} {title} ({} {})",
                    section.line,
                    shown(section.score),
                    section.findings,
                    if section.findings == 1 {
                        "finding"
                    } else {
                        "findings"
                    }
                );
            }
        }
    }
    out
}

#[derive(Serialize)]
struct FileScore<'a> {
    path: &'a str,
    /// `null` for a file with no prose.
    score: Option<f64>,
    words: usize,
    units: Units,
    findings: usize,
    rules: BTreeMap<&'static str, usize>,
    /// Per rule, opportunities holding at least one hit (the model's `k`).
    occupied: &'a BTreeMap<&'static str, usize>,
    sections: &'a [Section],
}

/// A JSON array with one object per file.
pub(crate) fn scores_json(reports: &[(String, FileReport)]) -> Result<String> {
    let scores: Vec<FileScore<'_>> = reports
        .iter()
        .map(|(path, report)| FileScore {
            path,
            score: report.score,
            words: report.words,
            units: report.units,
            occupied: &report.occupied,
            findings: report.findings.len(),
            rules: top_rules(report).into_iter().collect(),
            sections: &report.sections,
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&scores).context("serialising scores as JSON")?;
    text.push('\n');
    Ok(text)
}
