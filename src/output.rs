//! Renders findings, the summary and the rule list as human-readable text or JSON.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::engine::{FileReport, Finding, Section};
use crate::rules::{self, Category, Rule, Severity};
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

    use super::{Style, Summary, density, human, json, pretty, scores_human, scores_json, window};
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
            context: "Fast — and small.".into(),
            context_column: 5,
            width: 1,
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

    #[test]
    fn pretty_shows_scores_rules_context_and_hints() {
        let mut file = report(&["em-dash", "x-not-y", "em-dash"]);
        file.findings[1].confidence = 0.91;
        let reports = vec![("a.md".to_owned(), file)];
        let text = pretty(
            &reports,
            None,
            &Style {
                color: false,
                width: 100,
            },
        );
        assert!(
            text.starts_with(
                "a.md\n  AI-style: 87% · 3 findings\n  Tics: em-dash ×2, x-not-y ×1\n"
            ),
            "{text}"
        );
        assert!(
            text.find("Em dash in prose · 56% AI-leaning pattern")
                .unwrap_or(usize::MAX)
                < text
                    .find("Appositive contrast · 91% AI-leaning pattern")
                    .unwrap_or(0),
            "{text}"
        );
        assert!(text.contains("│ Fast — and small.\n"), "{text}");
        assert!(text.contains("│      ^\n"), "{text}");
        assert!(
            text.contains("╰ Use a comma, colon, parentheses or a new sentence."),
            "{text}"
        );
        assert!(
            text.contains("Pattern below 50%: more typical of human text."),
            "{text}"
        );
        assert!(!text.contains('\x1b'));
    }

    #[test]
    fn pretty_orders_findings_by_position_and_tics_by_count_then_confidence() {
        let mut file = report(&["x-not-y", "em-dash", "bold-lead-in", "em-dash"]);
        file.findings[0].line = 9;
        file.findings[0].confidence = 0.9;
        file.findings[1].line = 3;
        file.findings[1].column = 9;
        file.findings[2].line = 3;
        file.findings[2].column = 2;
        file.findings[2].confidence = 0.8;
        file.findings[3].line = 5;
        let text = pretty(
            &[("a.md".into(), file)],
            None,
            &Style {
                color: false,
                width: 100,
            },
        );
        assert!(
            text.contains("Tics: em-dash ×2, x-not-y ×1, bold-lead-in ×1"),
            "{text}"
        );
        let positions: Vec<_> = ["3:2", "3:9", "5:5", "9:5"]
            .iter()
            .map(|needle| text.find(needle).unwrap_or(usize::MAX))
            .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        assert!(
            text.contains("Em dash in prose · 56% AI-leaning pattern"),
            "{text}"
        );
        assert!(
            text.contains("Use a comma, colon, parentheses or a new sentence. (em-dash)"),
            "{text}"
        );
    }

    #[test]
    fn pretty_overview_orders_limits_and_marks_short_text() {
        let mut reports: Vec<_> = (0..11)
            .map(|i| {
                let mut file = report(&[]);
                file.score = Some(f64::from(i) / 10.0);
                (format!("file-{i:02}.md"), file)
            })
            .collect();
        reports[8].1.words = 79;
        reports.push((
            "no prose.md".into(),
            FileReport {
                score: None,
                words: 0,
                ..report(&[])
            },
        ));
        let text = pretty(
            &reports,
            None,
            &Style {
                color: false,
                width: 100,
            },
        );
        assert!(text.starts_with("Files (AI-style)\n  100%  file-10.md  0 findings\n  90%  file-09.md  0 findings\n  80% (too little text to judge)  file-08.md  0 findings\n"), "{text}");
        assert!(text.contains("  and 2 more\n"), "{text}");
        assert!(!text.contains("file-00.md"), "{text}");
        assert!(!text.contains("no prose.md"), "{text}");
        let mut short = report(&["em-dash"]);
        short.words = 79;
        let empty = FileReport {
            score: None,
            words: 0,
            ..report(&[])
        };
        let text = pretty(
            &[("short.md".into(), short), ("no  prose\n.md".into(), empty)],
            None,
            &Style {
                color: false,
                width: 100,
            },
        );
        assert!(
            text.contains("AI-style: 87% (too little text to judge) · 1 finding"),
            "{text}"
        );
        assert!(
            text.contains("  87% (too little text to judge)  short.md  1 finding"),
            "{text}"
        );
        assert!(
            text.contains("  n/a (too little text to judge)  no  prose\\n.md  0 findings"),
            "{text}"
        );
    }

    #[test]
    fn overview_ties_and_eighty_word_boundary_are_deterministic() {
        let threshold = || FileReport {
            words: 80,
            ..report(&[])
        };
        let no_prose = || FileReport {
            words: 0,
            score: None,
            ..report(&[])
        };
        let reports = vec![
            ("z.md".into(), threshold()),
            ("b.md".into(), no_prose()),
            ("a.md".into(), no_prose()),
            ("x.md".into(), threshold()),
        ];
        let text = pretty(
            &reports,
            None,
            &Style {
                color: false,
                width: 100,
            },
        );
        let paths: Vec<_> = ["x.md", "z.md", "a.md", "b.md"]
            .iter()
            .map(|path| text.find(path).unwrap_or(usize::MAX))
            .collect();
        assert!(paths.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
        assert!(text.contains("87%  x.md"), "{text}");
    }

    #[test]
    fn pretty_says_so_when_nothing_is_found() {
        let reports = vec![("a.md".to_owned(), report(&[]))];
        let style = Style {
            color: false,
            width: 100,
        };
        assert_eq!(
            pretty(&reports, None, &style),
            "Files (AI-style)\n  87%  a.md  0 findings\n\n✓ No tics found in 1 file (120 words).\n"
        );
    }

    #[test]
    fn pretty_wraps_long_paths_hints_and_counts_at_narrow_width() {
        let reports = vec![
            (
                "資料/averylongunbrokenfilename.md".to_owned(),
                report(&["em-dash", "bold-lead-in"]),
            ),
            ("b.md".to_owned(), report(&[])),
        ];
        let text = pretty(
            &reports,
            None,
            &Style {
                color: false,
                width: 20,
            },
        );
        for line in text.lines().filter(|line| !line.contains("│ …ast")) {
            let budget: usize = line.chars().map(super::column_budget).sum();
            assert!(budget <= 20, "line too wide: {line:?} ({budget})\n{text}");
        }
        assert!(text.contains("b.md"), "{text}");
        assert!(text.contains("AI-style:"), "{text}");
    }

    #[test]
    fn pretty_preserves_path_spaces_and_handles_narrow_underlines() {
        let mut file = report(&["em-dash"]);
        file.findings[0].context = "資料 — text".into();
        file.findings[0].context_column = 3;
        let reports = vec![("a  b.md".to_owned(), file)];
        let text = pretty(
            &reports,
            None,
            &Style {
                color: false,
                width: 100,
            },
        );
        assert!(text.starts_with("a  b.md\n"), "{text}");
        assert!(text.contains("│      ^"), "{text}");
        for width in [1, 5] {
            let text = pretty(
                &reports,
                None,
                &Style {
                    color: false,
                    width,
                },
            );
            for line in text.lines().filter(|line| line.contains('^')) {
                assert!(
                    line.chars().map(super::context_columns).sum::<usize>() <= width,
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn wide_context_keeps_the_match_underline_visible() {
        let mut file = report(&["em-dash"]);
        file.findings[0].context = "資料資料資料資料資料資料 — end".into();
        file.findings[0].context_column = 13;
        let text = pretty(
            &[("a.md".to_owned(), file)],
            None,
            &Style {
                color: false,
                width: 20,
            },
        );
        let underline = text
            .lines()
            .find(|line| line.contains('^'))
            .unwrap_or_default();
        assert!(!underline.is_empty(), "{text}");
        assert!(
            underline.chars().map(super::context_columns).sum::<usize>() <= 20,
            "{text}"
        );
    }

    #[test]
    fn long_lines_are_windowed_around_the_match() {
        let line = format!("{}target{}", "a".repeat(200), "b".repeat(200));
        let (text, offset, length) = window(&line, 200, 6, 60);
        assert!(text.starts_with('…') && text.ends_with('…'), "{text}");
        assert!(text.chars().count() <= 62);
        let shown: String = text.chars().skip(offset).take(length).collect();
        assert_eq!(shown, "target");
        let (text, offset, length) = window("short line", 6, 4, 60);
        assert_eq!((text.as_str(), offset, length), ("short line", 6, 4));
    }
}

/// How pretty output is drawn: colour on or off, and the terminal width in columns.
pub(crate) struct Style {
    pub(crate) color: bool,
    pub(crate) width: usize,
}

impl Style {
    /// Wraps `text` in an ANSI SGR code when colour is on.
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }
}

/// Writes text within the terminal width, breaking long words when necessary. Non-ASCII
/// characters are budgeted as two columns, which also accommodates wide characters without
/// requiring a terminal-width dependency. Source context is handled separately by `window`.
fn wrapped(out: &mut String, text: &str, indent: usize, style: &Style, code: &str) {
    let width = style.width.max(1);
    let indent = indent.min(width.saturating_sub(1));
    let prefix = " ".repeat(indent);
    let room = width - indent;
    let mut line = String::new();
    let mut used = 0;
    for word in text.split_whitespace() {
        let mut chars = word.chars().peekable();
        let word_width: usize = word.chars().map(column_budget).sum();
        if used > 0 && used + 1 + word_width > room {
            let _ = writeln!(out, "{prefix}{}", style.paint(code, &line));
            line.clear();
            used = 0;
        }
        if used > 0 {
            line.push(' ');
            used += 1;
        }
        while let Some(ch) = chars.next() {
            let columns = column_budget(ch).min(room);
            if used + columns > room {
                let _ = writeln!(out, "{prefix}{}", style.paint(code, &line));
                line.clear();
                used = 0;
            }
            line.push(if room == 1 && column_budget(ch) > 1 {
                '?'
            } else {
                ch
            });
            used += columns;
            if used == room && chars.peek().is_some() {
                let _ = writeln!(out, "{prefix}{}", style.paint(code, &line));
                line.clear();
                used = 0;
            }
        }
    }
    if !line.is_empty() {
        let _ = writeln!(out, "{prefix}{}", style.paint(code, &line));
    }
}

/// Preserve spaces in paths, while making control characters visible and safe to print.
fn wrapped_path(out: &mut String, path: &str, style: &Style) {
    let width = style.width.max(1);
    let mut line = String::new();
    let mut used = 0;
    for ch in path.chars().flat_map(|ch| {
        if ch.is_control() {
            ch.escape_default().collect::<String>()
        } else {
            ch.to_string()
        }
        .chars()
        .collect::<Vec<_>>()
    }) {
        let columns = column_budget(ch);
        if used > 0 && used + columns > width {
            let _ = writeln!(out, "{}", style.paint("1;4", &line));
            line.clear();
            used = 0;
        }
        if columns > width {
            line.push('?');
            used += 1;
        } else {
            line.push(ch);
            used += columns;
        }
    }
    if !line.is_empty() {
        let _ = writeln!(out, "{}", style.paint("1;4", &line));
    }
}

fn column_budget(ch: char) -> usize {
    if ch.is_ascii() { 1 } else { 2 }
}

/// Findings grouped by file, with the file score first and findings in source order.
pub(crate) fn pretty(
    reports: &[(String, FileReport)],
    summary: Option<&Summary>,
    style: &Style,
) -> String {
    let mut out = String::new();
    let mut total = 0;
    let mut flagged = 0;
    for (path, report) in reports {
        let count = report.findings.len();
        total += count;
        if count == 0 {
            continue;
        }
        if flagged > 0 {
            out.push('\n');
        }
        flagged += 1;
        wrapped_path(&mut out, path, style);
        let noun = if count == 1 { "finding" } else { "findings" };
        wrapped(
            &mut out,
            &format!("AI-style: {} · {count} {noun}", verdict(report)),
            2,
            style,
            "1",
        );

        render_tic_counts(&mut out, report, style);
        // The engine normally returns findings in source order; avoid sorting that common
        // case so rendering remains linear, while tolerating reports supplied out of order.
        if report
            .findings
            .is_sorted_by_key(|finding| (finding.line, finding.column))
        {
            for finding in &report.findings {
                render_finding(&mut out, finding, style);
            }
        } else {
            let mut findings: Vec<_> = report.findings.iter().collect();
            findings.sort_by_key(|finding| (finding.line, finding.column));
            for finding in findings {
                render_finding(&mut out, finding, style);
            }
        }
    }
    if flagged > 0 {
        out.push('\n');
    }
    render_overview(&mut out, reports, style);
    let files = reports.len();
    let words: usize = reports.iter().map(|(_, report)| report.words).sum();
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            one.to_owned()
        } else {
            many.to_owned()
        }
    };
    out.push('\n');
    if total == 0 {
        wrapped(
            &mut out,
            &format!(
                "✓ No tics found in {files} {} ({words} words).",
                plural(files, "file", "files")
            ),
            0,
            style,
            "32",
        );
    } else {
        wrapped(
            &mut out,
            &format!(
                "● {total} {} in {flagged} of {files} {} ({words} words)",
                plural(total, "possible tic", "possible tics"),
                plural(files, "file", "files")
            ),
            0,
            style,
            "33",
        );
        wrapped(
            &mut out,
            "Pattern below 50%: more typical of human text.",
            2,
            style,
            "2",
        );
    }
    if let Some(summary) = summary {
        render_summary(&mut out, summary, style);
    }
    out
}

fn render_tic_counts(out: &mut String, report: &FileReport, style: &Style) {
    // Confidence is the same for every occurrence of a rule in a file. The set of
    // rule IDs is fixed and small, so grouping stays linear in the findings.
    let mut groups: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
    for finding in &report.findings {
        let group = groups
            .entry(finding.rule)
            .or_insert((0, finding.confidence));
        group.0 += 1;
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by(|a, b| {
        b.1.0
            .cmp(&a.1.0)
            .then_with(|| b.1.1.total_cmp(&a.1.1))
            .then(a.0.cmp(b.0))
    });
    let counts = groups
        .iter()
        .map(|(id, (count, _))| format!("{id} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ");
    wrapped(out, &format!("Tics: {counts}"), 2, style, "2");
}

fn render_overview(out: &mut String, reports: &[(String, FileReport)], style: &Style) {
    wrapped(out, "Files (AI-style)", 0, style, "1");
    let mut overview: Vec<_> = reports.iter().collect();
    overview.sort_by(|a, b| match (a.1.score, b.1.score) {
        (Some(a_score), Some(b_score)) => b_score.total_cmp(&a_score).then(a.0.cmp(&b.0)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
    });
    for (path, report) in overview.iter().take(10) {
        let count = report.findings.len();
        let noun = if count == 1 { "finding" } else { "findings" };
        let safe_path: String = path
            .chars()
            .flat_map(|ch| {
                if ch.is_control() {
                    ch.escape_default().collect::<String>()
                } else {
                    ch.to_string()
                }
                .chars()
                .collect::<Vec<_>>()
            })
            .collect();
        let line = format!("  {}  {safe_path}  {count} {noun}", verdict(report));
        wrapped_preserving_spaces(out, &line, style);
    }
    if overview.len() > 10 {
        wrapped(
            out,
            &format!("and {} more", overview.len() - 10),
            2,
            style,
            "2",
        );
    }
}

fn verdict(report: &FileReport) -> String {
    let score = report.score.map_or_else(
        || "n/a".to_owned(),
        |score| format!("{:.0}%", score * 100.0),
    );
    if report.words < 80 {
        format!("{score} (too little text to judge)")
    } else {
        score
    }
}

/// Preserve spaces in overview paths, including when a narrow terminal forces wrapping.
fn wrapped_preserving_spaces(out: &mut String, text: &str, style: &Style) {
    let width = style.width.max(1);
    let mut line = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let columns = column_budget(ch);
        if used > 0 && used + columns > width {
            let _ = writeln!(out, "{line}");
            line.clear();
            used = 0;
        }
        if columns > width {
            line.push('?');
            used += 1;
        } else {
            line.push(ch);
            used += columns;
        }
    }
    if !line.is_empty() {
        let _ = writeln!(out, "{line}");
    }
}

fn render_summary(out: &mut String, summary: &Summary, style: &Style) {
    wrapped(
        out,
        &format!("{:.2} tics per 1,000 words", summary.per_1000_words),
        2,
        style,
        "2",
    );
    let mut counts: Vec<(&&str, &usize)> = summary.rules.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (id, count) in counts {
        wrapped(out, &format!("{id} {count}"), 2, style, "2");
    }
}

fn render_finding(out: &mut String, finding: &Finding, style: &Style) {
    out.push('\n');
    let rule = rules::by_id(finding.rule);
    let title = rule
        .map_or(finding.message.as_str(), |rule| rule.description)
        .split(" (`")
        .next()
        .unwrap_or_default();
    let mut chars = title.chars();
    let title: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    wrapped(
        out,
        &format!(
            "{}:{}  {title} · {}",
            finding.line,
            finding.column,
            lean_label(finding.confidence)
        ),
        2,
        style,
        "1",
    );
    let (text, offset, length) = window(
        &finding.context,
        finding.context_column,
        finding.width,
        style.width.saturating_sub(5).max(1),
    );
    let bar = style.paint("2", "│");
    let _ = writeln!(out, "  {bar} {text}");
    // Keep the underline within the terminal even if a window contains wide characters.
    if style.width >= 5 {
        let available = style.width - 4;
        let columns_before: usize = text.chars().take(offset).map(context_columns).sum();
        let columns_hit: usize = text
            .chars()
            .skip(offset)
            .take(length)
            .map(context_columns)
            .sum();
        if columns_before < available {
            let marks = format!(
                "{}{}",
                " ".repeat(columns_before),
                "^".repeat(columns_hit.max(1).min(available - columns_before))
            );
            let _ = writeln!(out, "  {bar} {}", style.paint("31", &marks));
        }
    }
    if let Some(rule) = rule {
        wrapped(
            out,
            &format!("╰ {}  ({})", rule.hint, finding.rule),
            2,
            style,
            "2",
        );
    }
}

// Common terminal-wide ranges; combining marks occupy no column.
fn context_columns(ch: char) -> usize {
    match ch as u32 {
        0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f => {
            0
        }
        0x1100..=0x115f
        | 0x2329..=0x232a
        | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe10..=0xfe19
        | 0xfe30..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6
        | 0x1f300..=0x1faff
        | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

/// How a finding's confidence reads in words: at or above 50% the pattern leans AI here;
/// below, people use it about as often, which the label says plainly.
fn lean_label(confidence: f64) -> String {
    let percent = confidence * 100.0;
    if confidence >= 0.5 {
        format!("{percent:.0}% AI-leaning pattern")
    } else {
        format!("{percent:.0}%, common in human writing too")
    }
}

/// The part of `line` to show in `room` columns around the hit at char `start` (`length` chars
/// long): the text, and where the underline starts and how long it is. Long lines are cut with
/// `…` on the side that is dropped.
fn window(line: &str, start: usize, length: usize, room: usize) -> (String, usize, usize) {
    let chars: Vec<char> = line
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .collect();
    let start = start.min(chars.len());
    if chars.iter().map(|&ch| context_columns(ch)).sum::<usize>() <= room {
        let length = length.clamp(1, chars.len().saturating_sub(start).max(1));
        return (chars.iter().collect(), start, length);
    }
    // Reserve room for the ellipses, and use a third of the remaining space before the hit.
    let ellipses = room >= 3;
    let content_room = if ellipses { room - 2 } else { room.max(1) };
    let mut first = start;
    let mut before = 0;
    while first > 0 {
        let width = context_columns(chars[first - 1]);
        if before + width > content_room / 3 {
            break;
        }
        before += width;
        first -= 1;
    }
    let mut last = first;
    let mut used = 0;
    while last < chars.len() {
        let width = context_columns(chars[last]);
        if used + width > content_room && last > start {
            break;
        }
        used += width;
        last += 1;
    }
    let mut text = String::new();
    let mut offset = start - first;
    if ellipses && first > 0 {
        text.push('…');
        offset += 1;
    }
    text.extend(&chars[first..last]);
    if ellipses && last < chars.len() {
        text.push('…');
    }
    let length = length.clamp(1, last.saturating_sub(start).max(1));
    (text, offset, length)
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
