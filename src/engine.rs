//! Compiles the selected rules once and runs them over a parsed document.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use anyhow::{Context, Result};
use regex::{Captures, Regex, RegexBuilder};
use serde::Serialize;

use crate::document::{self, Directive, DirectiveKind, Document, Mode, SegmentKind};
use crate::rules::{Category, Detector, Rule, Severity};
use crate::score::{self, Unit, Units};
use crate::sentences;

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
    /// Opportunities each kind of rule had: sentences, list items, headings, blocks.
    pub(crate) units: Units,
    /// Per rule, how many opportunities hold at least one hit (the model's `k`).
    pub(crate) occupied: BTreeMap<&'static str, usize>,
    /// Confidence that the whole file is AI-style text (0 to 1); none for a file with no prose.
    pub(crate) score: Option<f64>,
    /// The file split at each heading, each with its own score.
    pub(crate) sections: Vec<Section>,
}

/// The text from one heading to the next (or the text before the first heading).
#[derive(Debug, Serialize)]
pub(crate) struct Section {
    pub(crate) line: usize,
    pub(crate) title: Option<String>,
    pub(crate) units: Units,
    pub(crate) findings: usize,
    pub(crate) score: Option<f64>,
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
    Block(Regex),
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
                Detector::Block(pattern) => Matcher::Block(build(pattern)?),
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
            units: Units::default(),
            occupied: BTreeMap::new(),
            score: None,
            sections: Vec::new(),
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
    // Ascending by offset, like `hits`, so ranges of it can be found by binary search.
    let mut kept: Vec<(&'static Rule, usize)> = Vec::new();
    for (rule, span) in hits {
        let (line, column) = cursor.position(span.start);
        if is_suppressed(&document.directives, &lines, rule.id, line) {
            continue;
        }
        kept.push((rule, span.start));
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
    let selected: Vec<&'static Rule> = rules.iter().map(|compiled| compiled.rule).collect();
    let layout = Layout::new(&document, &blocks);
    let whole = 0..source.len();
    let units = layout.units(&whole);
    let occupied = layout.occupied(&kept);
    // One confidence per rule: it depends only on the rule's counts in the file.
    let confidence: BTreeMap<&str, f64> = selected
        .iter()
        .map(|rule| {
            let count = occupied.get(rule.id).copied().unwrap_or(0);
            (rule.id, score::confidence(rule, count, &units))
        })
        .collect();
    for finding in &mut findings {
        finding.confidence = confidence.get(finding.rule).copied().unwrap_or(0.0);
    }
    // A file with no prose says nothing either way, so it gets no score.
    let score = units
        .has_prose()
        .then(|| score::score(&occupied, &units, &selected));
    let sections = sections(&document, &lines, &layout, &kept, &findings, &selected);
    FileReport {
        findings,
        words,
        units,
        occupied,
        score,
        sections,
    }
}

/// Where each kind of opportunity starts, as ascending byte offsets.
struct Layout {
    /// Every sentence, with whether it is long enough (three words) to count as an opportunity.
    sentences: Vec<(usize, bool)>,
    /// `counted[i]`: how many of the first `i` sentences count as opportunities.
    counted: Vec<usize>,
    list_items: Vec<usize>,
    headings: Vec<usize>,
    blocks: Vec<usize>,
}

/// Sentences shorter than this (table cells, numbers, labels) are not opportunities.
const MIN_SENTENCE_WORDS: usize = 3;

impl Layout {
    fn new(document: &Document, blocks: &[Vec<Sentence<'_>>]) -> Self {
        let mut sentences: Vec<(usize, bool)> = blocks
            .iter()
            .flatten()
            .map(|sentence| {
                (
                    sentence.start,
                    count_words(sentence.text) >= MIN_SENTENCE_WORDS,
                )
            })
            .collect();
        sentences.sort_unstable();
        let mut headings: Vec<usize> = document
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Heading)
            .map(|segment| segment.start)
            .collect();
        headings.sort_unstable();
        let mut counted = Vec::with_capacity(sentences.len() + 1);
        counted.push(0);
        for (_, counts) in &sentences {
            counted.push(counted.last().copied().unwrap_or(0) + usize::from(*counts));
        }
        Self {
            sentences,
            counted,
            list_items: document.list_items.clone(),
            headings,
            blocks: document.blocks.clone(),
        }
    }

    /// Opportunities that start inside `range`, by binary search over the sorted starts.
    fn units(&self, range: &Range<usize>) -> Units {
        let within = |starts: &[usize]| {
            starts.partition_point(|start| *start < range.end)
                - starts.partition_point(|start| *start < range.start)
        };
        let first = self
            .sentences
            .partition_point(|(start, _)| *start < range.start);
        let last = self
            .sentences
            .partition_point(|(start, _)| *start < range.end);
        Units {
            sentences: self.counted[last] - self.counted[first],
            list_items: within(&self.list_items),
            headings: within(&self.headings),
            blocks: within(&self.blocks),
        }
    }

    /// Index of the opportunity of kind `unit` that holds `offset`.
    fn index_of(&self, unit: Unit, offset: usize) -> usize {
        let position = match unit {
            Unit::Sentence => self
                .sentences
                .partition_point(|(start, _)| *start <= offset),
            Unit::ListItem => self.list_items.partition_point(|start| *start <= offset),
            Unit::Heading => self.headings.partition_point(|start| *start <= offset),
            Unit::Block => self.blocks.partition_point(|start| *start <= offset),
        };
        position.saturating_sub(1)
    }

    /// Per rule, how many distinct opportunities hold at least one of `kept`'s hits. Three
    /// em dashes in one sentence occupy one sentence.
    fn occupied(&self, kept: &[(&'static Rule, usize)]) -> BTreeMap<&'static str, usize> {
        let mut seen: BTreeSet<(&'static str, usize)> = BTreeSet::new();
        for (rule, offset) in kept {
            seen.insert((rule.id, self.index_of(rule.unit(), *offset)));
        }
        let mut counts = BTreeMap::new();
        for (id, _) in seen {
            *counts.entry(id).or_insert(0) += 1;
        }
        counts
    }
}

/// Splits the file at each heading and scores each part on its own.
fn sections(
    document: &Document,
    lines: &LineIndex,
    layout: &Layout,
    kept: &[(&'static Rule, usize)],
    findings: &[Finding],
    selected: &[&'static Rule],
) -> Vec<Section> {
    let mut starts: Vec<(usize, Option<String>)> = document
        .segments
        .iter()
        .filter(|segment| segment.kind == SegmentKind::Heading)
        // A section starts at its heading's line, so the heading belongs to its own section.
        .map(|segment| {
            let line_start = lines.start_of(lines.line_of(segment.start));
            (line_start, Some(segment.text.trim().to_owned()))
        })
        .collect();
    if starts.first().is_none_or(|(start, _)| *start > 0) {
        starts.insert(0, (0, None));
    }
    let mut sections = Vec::new();
    for (index, (start, title)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(usize::MAX, |(next, _)| *next);
        let range = *start..end;
        let units = layout.units(&range);
        if title.is_none() && units == Units::default() {
            continue;
        }
        let from = kept.partition_point(|(_, offset)| *offset < range.start);
        let to = kept.partition_point(|(_, offset)| *offset < range.end);
        let occupied = layout.occupied(&kept[from..to]);
        // Findings are sorted by line; a section runs from its heading line to the next one.
        let first = lines.line_of(*start);
        let next = if end == usize::MAX {
            usize::MAX
        } else {
            lines.line_of(end)
        };
        let count = findings.partition_point(|finding| finding.line < next)
            - findings.partition_point(|finding| finding.line < first);
        sections.push(Section {
            line: first,
            title: title.clone(),
            units,
            findings: count,
            score: units
                .has_prose()
                .then(|| score::score(&occupied, &units, selected)),
        });
    }
    sections
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
            Matcher::Block(pattern) => {
                spans.extend(pattern.find_iter(&document.visible).map(|hit| hit.range()));
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
