//! Source-exact editorial candidates. Source text is included once per flagged file.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::Result;
use serde::Serialize;

use crate::document::{self, DirectiveKind, Mode, SegmentKind};
use crate::engine::FileReport;
use crate::rules::Severity;
use crate::rules::{Category, Rule};
use crate::{cli::Cli, config::Config};

pub(crate) const INSTRUCTIONS: &str = include_str!("../skills/tics-review/SKILL.md");
const SCHEMA: &str = "tics.review.v1";

#[derive(Debug, Serialize)]
pub(crate) struct SourceRange {
    /// Half-open UTF-8 byte offsets in the original source, without newline normalization.
    start_byte: usize,
    end_byte: usize,
    /// One-based lines and Unicode scalar columns. End position is exclusive.
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Passage {
    id: String,
    range: SourceRange,
    kind: &'static str,
    heading_id: Option<String>,
    previous_id: Option<String>,
    next_id: Option<String>,
    candidate_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Candidate {
    id: String,
    rule: String,
    title: String,
    category: String,
    range: SourceRange,
    /// Full source text, never the shortened legacy `matched` display.
    original: String,
    /// Masking can join prose across code/HTML. This is a cue, never an approved edit range.
    crosses_code_or_html: bool,
    passage_id: Option<String>,
    review_hint: String,
    status: &'static str,
}

fn review_hint(rule: &Rule) -> &'static str {
    match rule.id {
        "summary-opener" => {
            "Does this recap repeat visible earlier material, or add a useful answer, implication or action?"
        }
        "importance-hedge" | "signposting" => {
            "Does this framing add useful orientation, a warning or an action?"
        }
        "guided-tour-variants" => rule.hint,
        _ => match rule.category {
            Category::Contrast => {
                "Does this contrast convey a real distinction the reader needs? Preserve necessary negation."
            }
            Category::Vocabulary => {
                "Is this wording inflated or vague in this context, or a precise, supported use?"
            }
            Category::Formatting => {
                "Is this repeated structure distracting here? A punctuation mark or heading alone does not justify an edit."
            }
            Category::Chatbot => {
                "Is this unwanted assistant residue in the author's prose, or a quoted example or literal placeholder?"
            }
            Category::Rhetoric => {
                "Does this rhetorical construction add useful meaning in context?"
            }
        },
    }
}

/// A masked non-ASCII code character occupies several spaces in the prose view.
/// Enclose it completely if a regex ends inside it; the candidate remains review-only.
fn source_span(source: &str, mut span: std::ops::Range<usize>) -> std::ops::Range<usize> {
    while !source.is_char_boundary(span.start) {
        span.start -= 1;
    }
    while !source.is_char_boundary(span.end) {
        span.end += 1;
    }
    span
}

#[derive(Debug, Serialize)]
pub(crate) struct ReviewFile {
    path: String,
    format: &'static str,
    words: usize,
    needs_review: bool,
    status: &'static str,
    /// Omitted for zero-candidate files so piping a tree does not send their bodies to an LLM.
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    passages: Vec<Passage>,
    candidates: Vec<Candidate>,
}

struct Locations {
    prefix_bytes: usize,
    starts: Vec<usize>,
    continuations: Vec<usize>,
}

impl Locations {
    fn new(source: &str) -> Self {
        let body = source.strip_prefix('\u{feff}').unwrap_or(source);
        Self {
            prefix_bytes: source.len() - body.len(),
            starts: std::iter::once(0)
                .chain(body.match_indices('\n').map(|(i, _)| i + 1))
                .collect(),
            continuations: body
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| (b & 0xC0 == 0x80).then_some(i))
                .collect(),
        }
    }

    fn range(&self, start: usize, end: usize) -> SourceRange {
        let position = |offset| {
            let line = self.starts.partition_point(|&a| a <= offset);
            let begin = self.starts[line - 1];
            let extra = self.continuations.partition_point(|&p| p < offset)
                - self.continuations.partition_point(|&p| p < begin);
            (line, offset - begin - extra + 1)
        };
        let (start_line, start_column) = position(start);
        let (end_line, end_column) = position(end);
        SourceRange {
            start_byte: start + self.prefix_bytes,
            end_byte: end + self.prefix_bytes,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn file(
    path: &str,
    source: &str,
    mode: Mode,
    report: &FileReport,
    rules: &[&Rule],
    config: &Config,
    allows: &[regex::Regex],
    cli: &Cli,
) -> ReviewFile {
    let body = source.strip_prefix('\u{feff}').unwrap_or(source);
    let document = document::parse_review(body, mode);
    let locations = Locations::new(source);
    let mut heading = None;
    let mut passages: Vec<_> = document
        .segments
        .iter()
        .enumerate()
        .map(|(i, segment)| {
            let id = format!("p{}", i + 1);
            if segment.kind == SegmentKind::Heading {
                heading = Some(id.clone());
            }
            Passage {
                id,
                range: locations.range(segment.start, segment.start + segment.text.len()),
                kind: if segment.kind == SegmentKind::Heading {
                    "heading"
                } else {
                    "prose"
                },
                heading_id: heading.clone(),
                previous_id: (i > 0).then(|| format!("p{i}")),
                next_id: (i + 1 < document.segments.len()).then(|| format!("p{}", i + 2)),
                candidate_count: 0,
            }
        })
        .collect();
    let allow_spans: Vec<_> = document
        .segments
        .iter()
        .flat_map(|segment| {
            allows.iter().flat_map(move |regex| {
                regex.find_iter(&segment.text).filter_map(move |m| {
                    let span = (segment.start + m.start())..(segment.start + m.end());
                    (body.get(span.clone()) == Some(&segment.text[m.range()])).then_some(span)
                })
            })
        })
        .collect();
    let mut hits: Vec<(std::ops::Range<usize>, String, String, String, String)> = report
        .findings
        .iter()
        .map(|finding| {
            let rule = rules.iter().find(|r| r.id == finding.rule);
            let description = rule
                .map_or(finding.message.as_str(), |r| r.description)
                .split(" (`")
                .next()
                .unwrap_or_default();
            let title = match finding.rule {
                "negative-parallelism" => "Contrast wording".to_owned(),
                "importance-hedge" => "Importance framing".to_owned(),
                "summary-opener" => "Summary opener".to_owned(),
                "ai-vocabulary" => "Broad descriptive wording".to_owned(),
                _ => {
                    let mut chars = description.chars();
                    chars
                        .next()
                        .map(|c| c.to_uppercase().chain(chars).collect())
                        .unwrap_or_default()
                }
            };
            (
                finding.span.clone(),
                finding.rule.to_owned(),
                title,
                finding.category.as_str().to_owned(),
                rule.map_or("Judge this wording in context.", |r| review_hint(r))
                    .to_owned(),
            )
        })
        .collect();
    let selection = if cli.select.is_empty() {
        config.select.as_deref()
    } else {
        Some(cli.select.as_slice())
    };
    let severity = cli.severity.or(config.severity).unwrap_or(Severity::Info);
    if !document
        .directives
        .iter()
        .any(|d| d.kind == DirectiveKind::File)
    {
        for rule in &config.custom_rules {
            if selection.is_some_and(|ids| !ids.contains(&rule.id))
                || cli
                    .ignore
                    .iter()
                    .chain(&config.ignore)
                    .any(|id| id == &rule.id)
                || rule.severity < severity
            {
                continue;
            }
            if let Some(regex) = &rule.regex {
                for segment in &document.segments {
                    for m in regex.find_iter(&segment.text) {
                        let span = (segment.start + m.start())..(segment.start + m.end());
                        if span.is_empty()
                            || body.get(span.clone()) != Some(&segment.text[m.range()])
                        {
                            continue;
                        }
                        let line = body[..span.start].bytes().filter(|&b| b == b'\n').count() + 1;
                        let suppressed = document.directives.iter().any(|d| {
                            let directive_line =
                                body[..d.offset].bytes().filter(|&b| b == b'\n').count() + 1;
                            directive_line + 1 == line
                                && match &d.kind {
                                    DirectiveKind::NextLine => true,
                                    DirectiveKind::Rules(ids) => ids.contains(&rule.id),
                                    DirectiveKind::File => false,
                                }
                        });
                        if !suppressed {
                            hits.push((
                                span,
                                rule.id.clone(),
                                rule.title.clone(),
                                "custom".to_owned(),
                                rule.hint.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }
    hits.retain(|(span, _, _, _, _)| {
        !allow_spans
            .iter()
            .any(|a| a.start <= span.start && a.end >= span.end)
    });
    hits.sort_by_key(|(span, _, _, _, _)| span.start);
    let candidates = hits
        .into_iter()
        .enumerate()
        .map(|(i, (span, rule, title, category, hint))| {
            let span = source_span(body, span);
            let start = span.start + locations.prefix_bytes;
            let end = span.end + locations.prefix_bytes;
            let next = passages.partition_point(|p| p.range.start_byte <= start);
            let passage = next
                .checked_sub(1)
                .filter(|&j| start < passages[j].range.end_byte)
                .or_else(|| {
                    (next < passages.len() && passages[next].range.start_byte < end).then_some(next)
                });
            if let Some(index) = passage {
                passages[index].candidate_count += 1;
            }
            Candidate {
                id: format!("f{}", i + 1),
                rule,
                title,
                category,
                range: locations.range(span.start, span.end),
                original: body[span.clone()].to_owned(),
                crosses_code_or_html: document.visible[span.clone()] != body[span],
                passage_id: passage.map(|j| passages[j].id.clone()),
                review_hint: hint,
                status: "needs_judgment",
            }
        })
        .collect::<Vec<_>>();
    let needs_review = !candidates.is_empty();
    ReviewFile {
        path: path.to_owned(),
        format: if mode == Mode::Plain {
            "txt"
        } else {
            "markdown"
        },
        words: report.words,
        needs_review,
        status: if needs_review {
            "candidates_found"
        } else {
            "no_candidates_unreviewed"
        },
        source: needs_review.then(|| source.to_owned()),
        passages,
        candidates,
    }
}

#[derive(Serialize)]
struct Summary {
    files: usize,
    files_needing_review: usize,
    files_without_candidates: usize,
    candidates: usize,
    words: usize,
}

fn summary(files: &[ReviewFile]) -> Summary {
    let needs_review = files.iter().filter(|f| f.needs_review).count();
    Summary {
        files: files.len(),
        files_needing_review: needs_review,
        files_without_candidates: files.len() - needs_review,
        candidates: files.iter().map(|f| f.candidates.len()).sum(),
        words: files.iter().map(|f| f.words).sum(),
    }
}

pub(crate) fn has_candidates(files: &[ReviewFile]) -> bool {
    files.iter().any(|f| f.needs_review)
}

pub(crate) fn json(files: &[ReviewFile]) -> Result<String> {
    #[derive(Serialize)]
    struct Report<'a> {
        schema: &'static str,
        instructions: &'static str,
        summary: Summary,
        files: &'a [ReviewFile],
    }
    Ok(serde_json::to_string_pretty(&Report {
        schema: SCHEMA,
        instructions: INSTRUCTIONS,
        summary: summary(files),
        files,
    })? + "\n")
}

pub(crate) fn jsonl(files: &[ReviewFile]) -> Result<String> {
    let mut out = serde_json::to_string(
        &serde_json::json!({"type":"instructions","schema":SCHEMA,"instructions":INSTRUCTIONS,"coordinates":"half-open UTF-8 bytes; one-based Unicode scalar line/column; exclusive end"}),
    )? + "\n";
    for file in files {
        // File and source appear once; candidates are separate records for stream consumers.
        let mut value = serde_json::to_value(file)?;
        value
            .as_object_mut()
            .expect("serialized struct")
            .remove("candidates");
        value["type"] = "file".into();
        value["candidate_count"] = file.candidates.len().into();
        out += &serde_json::to_string(&value)?;
        out.push('\n');
        for candidate in &file.candidates {
            let mut value = serde_json::to_value(candidate)?;
            value["type"] = "candidate".into();
            value["path"] = file.path.clone().into();
            out += &serde_json::to_string(&value)?;
            out.push('\n');
        }
    }
    out += &serde_json::to_string(&serde_json::json!({"type":"summary","summary":summary(files)}))?;
    out.push('\n');
    Ok(out)
}

pub(crate) fn human(files: &[ReviewFile]) -> String {
    let s = summary(files);
    let mut out = format!(
        "Tics editorial review\nFiles {} · Need review {} · No candidates {} · Signals {}\n",
        s.files, s.files_needing_review, s.files_without_candidates, s.candidates
    );
    out.push_str(
        "Review these matches in context.\n\nHotspots (· none, ░ one, ▒ two, ▓ three or more):\n",
    );
    for file in files {
        let heat: String = file
            .passages
            .iter()
            .map(|p| ['·', '░', '▒', '▓'][p.candidate_count.min(3)])
            .collect();
        let _ = writeln!(
            out,
            "  {}  {}  {} candidates",
            file.path,
            heat,
            file.candidates.len()
        );
    }
    let mut groups = Vec::new();
    for file in files {
        let mut by_passage: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for candidate in &file.candidates {
            by_passage
                .entry(candidate.passage_id.as_deref())
                .or_default()
                .push(candidate);
        }
        for (passage, candidates) in by_passage {
            groups.push((file, passage, candidates));
        }
    }
    groups.sort_by_key(|(f, p, c)| (std::cmp::Reverse(c.len()), f.path.as_str(), *p));
    if !groups.is_empty() {
        out.push_str("\nInteresting findings:\n");
    }
    for (file, passage, candidates) in groups.iter().take(5) {
        let rules: Vec<_> = candidates.iter().map(|c| c.rule.as_str()).collect();
        let _ = writeln!(
            out,
            "  {} · {} · {}",
            file.path,
            passage.unwrap_or("source"),
            rules.join(", ")
        );
        let excerpt: String = candidates[0]
            .original
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(180)
            .collect();
        let _ = writeln!(out, "    {excerpt}");
    }
    out
}

pub(crate) fn ci(files: &[ReviewFile]) -> Result<String> {
    // The skill is already above the payload; include it once for the model reader.
    let payload = serde_json::to_string_pretty(
        &serde_json::json!({"schema":SCHEMA,"summary":summary(files),"files":files}),
    )? + "\n";
    let longest = payload.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    Ok(format!(
        "# Tics editorial review\n\n{INSTRUCTIONS}\n\n{}\n<details><summary>Exact candidates and source context</summary>\n\n{fence}json\n{payload}{fence}\n\n</details>\n",
        human(files)
    ))
}
