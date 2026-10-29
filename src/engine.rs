//! Compiles the selected rules once and runs them over a parsed document.

use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::{Context, Result};
use regex::{Captures, Regex, RegexBuilder};
use serde::Serialize;

use crate::document::{self, Directive, DirectiveKind, Document, Mode, SegmentKind};
use crate::rules::{Category, Detector, Rule, Severity};
use crate::{score, sentences};

/// Longest `matched` excerpt, in characters.
const MATCHED_MAX_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Finding {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) rule: &'static str,
    pub(crate) category: Category,
    pub(crate) severity: Severity,
    pub(crate) message: String,
    pub(crate) matched: String,
    /// How strongly this rule's hits in this file suggest AI-style text (0 to 1).
    pub(crate) confidence: f64,
}

/// The result of scanning one file.
#[derive(Debug)]
pub(crate) struct FileReport {
    pub(crate) findings: Vec<Finding>,
    pub(crate) words: usize,
    /// Confidence that the whole file is AI-style text (0 to 1).
    pub(crate) score: f64,
}

enum Matcher {
    Text {
        find: Regex,
        reject: Option<Regex>,
        check: Option<fn(&str) -> bool>,
    },
    Pair {
        first: Regex,
        first_reject: Option<Regex>,
        second: Regex,
        same_subject: bool,
    },
    ListItem(Regex),
    Heading(fn(&str) -> bool),
    Raw(Regex),
}

pub(crate) struct Compiled {
    rule: &'static Rule,
    matcher: Matcher,
}

pub(crate) fn compile(rules: &[&'static Rule]) -> Result<Vec<Compiled>> {
    rules
        .iter()
        .map(|rule| {
            let matcher = match &rule.detector {
                Detector::Text {
                    find,
                    reject,
                    check,
                } => Matcher::Text {
                    find: build(find)?,
                    reject: reject.map(build).transpose()?,
                    check: *check,
                },
                Detector::Pair {
                    first,
                    first_reject,
                    second,
                    same_subject,
                } => Matcher::Pair {
                    first: build(first)?,
                    first_reject: first_reject.map(build).transpose()?,
                    second: build(second)?,
                    same_subject: *same_subject,
                },
                Detector::ListItem(pattern) => Matcher::ListItem(build(pattern)?),
                Detector::Heading(check) => Matcher::Heading(*check),
                Detector::Raw(pattern) => Matcher::Raw(build(pattern)?),
            };
            Ok(Compiled { rule, matcher })
        })
        .collect::<Result<_>>()
        .context("compiling rule patterns")
}

fn build(pattern: &str) -> Result<Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .ignore_whitespace(true)
        .build()
        .with_context(|| format!("invalid pattern {pattern}"))
}

/// Scans one source. `path` is only copied into the findings.
pub(crate) fn scan(path: &str, source: &str, mode: Mode, rules: &[Compiled]) -> FileReport {
    let document = document::parse(source, mode);
    let words = document
        .segments
        .iter()
        .map(|segment| count_words(&segment.text))
        .sum();
    if document
        .directives
        .iter()
        .any(|directive| directive.kind == DirectiveKind::File)
    {
        return FileReport {
            findings: Vec::new(),
            words,
            score: score::file_score(&BTreeMap::new(), words),
        };
    }
    let lines = LineIndex::new(source);
    let blocks: Vec<Vec<Sentence>> = document
        .segments
        .iter()
        .map(|segment| {
            sentences::split(&segment.text)
                .into_iter()
                .map(|range| Sentence {
                    start: segment.start + range.start,
                    text: &segment.text[range],
                })
                .collect()
        })
        .collect();
    let mut hits: Vec<(&'static Rule, Range<usize>)> = Vec::new();
    for compiled in rules {
        let mut spans = Vec::new();
        compiled.find(&document, &blocks, &mut spans);
        hits.extend(spans.into_iter().map(|span| (compiled.rule, span)));
    }
    // Ascending offsets let the cursor count each line's characters once in total.
    hits.sort_by_key(|(_, span)| span.start);
    let mut cursor = Cursor::new(source, &lines);
    let mut findings: Vec<Finding> = Vec::new();
    for (rule, span) in hits {
        let (line, column) = cursor.position(span.start);
        if is_suppressed(&document.directives, &lines, rule.id, line) {
            continue;
        }
        findings.push(Finding {
            path: path.to_owned(),
            line,
            column,
            rule: rule.id,
            category: rule.category,
            severity: rule.severity,
            message: format!("possible AI-writing tic: {}", rule.description),
            matched: excerpt(source.get(span).unwrap_or_default()),
            confidence: 0.0,
        });
    }
    findings.sort_by(|a, b| (a.line, a.column, a.rule).cmp(&(b.line, b.column, b.rule)));
    findings.dedup();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for finding in &findings {
        *counts.entry(finding.rule).or_insert(0) += 1;
    }
    for finding in &mut findings {
        finding.confidence = score::confidence(finding.rule, counts[finding.rule], words);
    }
    let score = score::file_score(&counts, words);
    FileReport {
        findings,
        words,
        score,
    }
}

/// A sentence of masked prose and the byte offset of its first byte in the source.
struct Sentence<'a> {
    start: usize,
    text: &'a str,
}

impl Compiled {
    /// Pushes the byte spans (in the original source) of every hit of this rule.
    fn find(
        &self,
        document: &Document,
        blocks: &[Vec<Sentence<'_>>],
        spans: &mut Vec<Range<usize>>,
    ) {
        match &self.matcher {
            Matcher::Text {
                find,
                reject,
                check,
            } => {
                for sentence in blocks.iter().flatten() {
                    for captures in find.captures_iter(sentence.text) {
                        let Some(whole) = captures.get(0) else {
                            continue;
                        };
                        let tail = &sentence.text[whole.start()..];
                        let rejected = reject.as_ref().is_some_and(|reject| reject.is_match(tail));
                        let checked = check.is_none_or(|check| check(whole.as_str()));
                        if !rejected && checked {
                            spans.push(shift(reported(&captures), sentence.start));
                        }
                    }
                }
            }
            Matcher::Pair {
                first,
                first_reject,
                second,
                same_subject,
            } => {
                for pair in blocks.iter().flat_map(|block| block.windows(2)) {
                    let [one, two] = pair else { continue };
                    let Some(opening) = first.captures(one.text) else {
                        continue;
                    };
                    if first_reject
                        .as_ref()
                        .is_some_and(|reject| reject.is_match(one.text))
                    {
                        continue;
                    }
                    let Some(closing) = second.captures(two.text) else {
                        continue;
                    };
                    if *same_subject && subject(&opening) != subject(&closing) {
                        continue;
                    }
                    let start = one.start + reported(&opening).start;
                    let end = two.start + reported(&closing).end;
                    spans.push(start..end);
                }
            }
            Matcher::ListItem(pattern) => {
                for &start in &document.list_items {
                    let line = document.code_free[start..]
                        .split('\n')
                        .next()
                        .unwrap_or_default();
                    if let Some(captures) = pattern.captures(line) {
                        spans.push(shift(reported(&captures), start));
                    }
                }
            }
            Matcher::Heading(check) => {
                for segment in &document.segments {
                    let trimmed = segment.text.trim();
                    if segment.kind == SegmentKind::Heading && check(trimmed) {
                        let lead = segment.text.len() - segment.text.trim_start().len();
                        let start = segment.start + lead;
                        spans.push(start..start + trimmed.len());
                    }
                }
            }
            Matcher::Raw(pattern) => {
                spans.extend(
                    pattern
                        .find_iter(&document.code_free)
                        .map(|hit| hit.range()),
                );
            }
        }
    }
}

/// The span to report: the named group `m` when the pattern has one, else the whole match.
fn reported(captures: &Captures<'_>) -> Range<usize> {
    captures
        .name("m")
        .or_else(|| captures.get(0))
        .map_or(0..0, |hit| hit.range())
}

fn shift(range: Range<usize>, base: usize) -> Range<usize> {
    base + range.start..base + range.end
}

fn subject(captures: &Captures<'_>) -> Option<String> {
    captures.name("subj").map(|subject| {
        subject
            .as_str()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    })
}

fn is_suppressed(directives: &[Directive], lines: &LineIndex, rule: &str, line: usize) -> bool {
    directives.iter().any(|directive| {
        lines.line_of(directive.offset) + 1 == line
            && match &directive.kind {
                DirectiveKind::NextLine => true,
                DirectiveKind::Rules(ids) => ids.iter().any(|id| id == rule),
                DirectiveKind::File => false,
            }
    })
}

/// Collapses whitespace and caps the length so one finding stays on one line.
fn excerpt(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MATCHED_MAX_CHARS {
        return collapsed;
    }
    let mut short: String = collapsed.chars().take(MATCHED_MAX_CHARS - 1).collect();
    short.push('…');
    short
}

/// Words are whitespace-separated tokens that contain a letter.
fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(char::is_alphabetic))
        .count()
}

/// Maps byte offsets to 1-based line and column (columns count Unicode scalar values).
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        Self { starts }
    }

    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset)
    }

    fn start_of(&self, line: usize) -> usize {
        self.starts
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or(0)
    }
}

/// Turns byte offsets into 1-based line and column. Given ascending offsets it counts each
/// character once, so a 1 MiB line with thousands of hits is not recounted per hit.
struct Cursor<'a> {
    source: &'a [u8],
    lines: &'a LineIndex,
    line: usize,
    offset: usize,
    column: usize,
}

impl<'a> Cursor<'a> {
    fn new(source: &'a str, lines: &'a LineIndex) -> Self {
        Self {
            source: source.as_bytes(),
            lines,
            line: 1,
            offset: 0,
            column: 1,
        }
    }

    fn position(&mut self, offset: usize) -> (usize, usize) {
        let line = self.lines.line_of(offset);
        if line != self.line || offset < self.offset {
            self.line = line;
            self.offset = self.lines.start_of(line);
            self.column = 1;
        }
        let skipped = self.source.get(self.offset..offset).unwrap_or_default();
        // Columns count Unicode scalar values: every byte that is not a continuation byte.
        self.column += skipped.iter().filter(|&&byte| byte & 0xC0 != 0x80).count();
        self.offset = offset;
        (line, self.column)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};

    use super::{Cursor, FileReport, LineIndex, compile, excerpt, scan};
    use crate::document::Mode;
    use crate::rules::RULES;

    fn report(source: &str) -> Result<FileReport> {
        let all: Vec<_> = RULES.iter().collect();
        Ok(scan("t.md", source, Mode::Markdown, &compile(&all)?))
    }

    fn positions(source: &str, rule: &str) -> Result<Vec<(usize, usize)>> {
        Ok(report(source)?
            .findings
            .into_iter()
            .filter(|finding| finding.rule == rule)
            .map(|finding| (finding.line, finding.column))
            .collect())
    }

    #[test]
    fn all_rule_patterns_compile() -> Result<()> {
        let all: Vec<_> = RULES.iter().collect();
        assert_eq!(compile(&all)?.len(), RULES.len());
        Ok(())
    }

    #[test]
    fn cursor_reports_one_based_char_columns_in_any_order() {
        let source = "ab\néé—x\n";
        let lines = LineIndex::new(source);
        let mut cursor = Cursor::new(source, &lines);
        assert_eq!(cursor.position(0), (1, 1));
        assert_eq!(cursor.position(3), (2, 1));
        assert_eq!(cursor.position("ab\néé".len()), (2, 3));
        assert_eq!(cursor.position("ab\néé—".len()), (2, 4));
        assert_eq!(cursor.position("ab\né".len()), (2, 2));
        assert_eq!(cursor.position(1), (1, 2));
    }

    #[test]
    fn columns_are_exact_inside_containers() -> Result<()> {
        let source = "# Title\n\n> quoted — here\n\n- item — x\n\n| a — b | c |\n|---|---|\n| d | e — f |\n\nCafé — ok\n";
        assert_eq!(
            positions(source, "em-dash")?,
            [(3, 10), (5, 8), (7, 5), (9, 9), (11, 6)]
        );
        Ok(())
    }

    #[test]
    fn front_matter_keeps_line_numbers() -> Result<()> {
        let source = "---\ntitle: a — b\n---\n\nText — here.\n";
        assert_eq!(positions(source, "em-dash")?, [(5, 6)]);
        Ok(())
    }

    #[test]
    fn multi_line_paragraph_reports_the_line_of_the_hit() -> Result<()> {
        let source = "First line\nand then we delve\ninto it.\n";
        assert_eq!(positions(source, "ai-vocabulary")?, [(2, 13)]);
        Ok(())
    }

    #[test]
    fn ignore_next_line_suppresses_only_the_next_line() -> Result<()> {
        let source = "<!-- tics-ignore-next-line -->\nA — b.\n\nC — d.\n";
        assert_eq!(positions(source, "em-dash")?, [(4, 3)]);
        Ok(())
    }

    #[test]
    fn ignore_rule_suppresses_only_that_rule() -> Result<()> {
        let source = "<!-- tics-ignore em-dash -->\nWe delve — deeply.\n";
        assert_eq!(positions(source, "em-dash")?, Vec::<(usize, usize)>::new());
        assert_eq!(positions(source, "ai-vocabulary")?, [(2, 4)]);
        Ok(())
    }

    #[test]
    fn ignore_file_suppresses_everything_but_still_counts_words() -> Result<()> {
        let report = report("We delve — deeply.\n\n<!-- tics-ignore-file -->\n")?;
        assert_eq!(report.findings, Vec::new());
        assert_eq!(report.words, 3);
        Ok(())
    }

    #[test]
    fn directives_inside_code_do_not_suppress() -> Result<()> {
        let source = "```\n<!-- tics-ignore-file -->\n```\n\nA — b.\n";
        assert_eq!(positions(source, "em-dash")?, [(5, 3)]);
        Ok(())
    }

    #[test]
    fn excerpt_collapses_whitespace_and_truncates() -> Result<()> {
        assert_eq!(excerpt("a\n  b"), "a b");
        let long = excerpt(&"word ".repeat(40));
        assert_eq!(long.chars().count(), 80);
        assert!(long.ends_with('…'));
        let report = report("Not only fast\nbut also safe.\n")?;
        let finding = report.findings.first().context("one finding")?;
        assert_eq!(finding.matched, "Not only fast but also");
        Ok(())
    }
}
