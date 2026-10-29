//! Renders findings, the summary and the rule list as human-readable text or JSON.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::engine::Finding;
use crate::rules::{Category, Rule, Severity};

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
            "{}:{}:{}: {}: {} «{}»",
            finding.path,
            finding.line,
            finding.column,
            finding.rule,
            finding.message,
            finding.matched
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
    source: &'static str,
    description: &'static str,
}

pub(crate) fn rules_human(rules: &[Rule]) -> String {
    let width = rules.iter().map(|rule| rule.id.len()).max().unwrap_or(0);
    let mut out = String::new();
    for rule in rules {
        let _ = writeln!(
            out,
            "{:<width$}  {:<10}  {:<4}  {}  {}  {}",
            rule.id,
            rule.category.as_str(),
            rule.severity.as_str(),
            rule.since,
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

    use super::{Summary, density, human, json};
    use crate::engine::Finding;
    use crate::rules::{Category, Severity};

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
            "a.md:2:5: em-dash: possible AI-writing tic: em dash in prose «—»\n"
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
}
