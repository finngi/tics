//! Splits a source file into prose segments without moving a single byte.
//!
//! Every segment's `text` is the source slice it covers with all non-prose bytes (markup, code
//! spans, link destinations, URLs) replaced by spaces and newlines kept. A byte offset in
//! `text` plus `start` is therefore the exact byte offset in the original file, which is what
//! makes line and column reporting exact.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Markdown,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SegmentKind {
    Heading,
    Prose,
}

/// One block of prose: a paragraph, heading, table cell or list-item line run.
#[derive(Debug)]
pub(crate) struct Segment {
    pub(crate) start: usize,
    pub(crate) text: String,
    pub(crate) kind: SegmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DirectiveKind {
    File,
    NextLine,
    Rules(Vec<String>),
}

/// A `<!-- tics-… -->` comment; `offset` is the byte offset of its closing `-->`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Directive {
    pub(crate) offset: usize,
    pub(crate) kind: DirectiveKind,
}

#[derive(Debug)]
pub(crate) struct Document {
    pub(crate) segments: Vec<Segment>,
    pub(crate) directives: Vec<Directive>,
    /// The source with front matter and code blanked; URLs and HTML are kept.
    pub(crate) code_free: String,
    /// Byte offset of each Markdown list item's marker.
    pub(crate) list_items: Vec<usize>,
    /// The source with front matter, code and HTML (comments included) blanked, offsets kept:
    /// what a reader sees as text, still with Markdown markup such as `**`.
    pub(crate) visible: String,
    /// Byte offset of the first line of each block a reader would see as one: a paragraph,
    /// heading or list item. Table rows and blank-line runs are not blocks.
    pub(crate) blocks: Vec<usize>,
}

pub(crate) fn parse(source: &str, mode: Mode) -> Document {
    match mode {
        Mode::Markdown => parse_markdown(source),
        Mode::Plain => parse_plain(source),
    }
}

fn parse_markdown(source: &str) -> Document {
    // Front matter is blanked by hand: pulldown-cmark's metadata-block option also accepts
    // `---` blocks in the middle of a document, which would hide prose between thematic breaks.
    let front_matter = 0..front_matter_len(source);
    let body = mask(
        source,
        0..source.len(),
        std::slice::from_ref(&front_matter),
        Keep::Outside,
    );
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let mut collector = Collector::default();
    let mut opaque_depth = 0_usize;
    let mut in_heading = false;
    let mut code: Vec<Range<usize>> = Vec::new();
    let mut directives = Vec::new();
    let mut list_items = Vec::new();
    let mut html: Vec<Range<usize>> = Vec::new();
    let mut tables: Vec<Range<usize>> = Vec::new();
    for (event, range) in Parser::new_ext(&body, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if is_block(tag.to_end()) {
                    collector.flush(&body, in_heading);
                }
                match tag {
                    Tag::CodeBlock(_) => {
                        code.push(range);
                        opaque_depth += 1;
                    }
                    Tag::MetadataBlock(_)
                    | Tag::Link {
                        link_type: LinkType::Autolink | LinkType::Email,
                        ..
                    } => opaque_depth += 1,
                    Tag::Heading { .. } => in_heading = true,
                    Tag::Item => list_items.push(range.start),
                    Tag::Table(_) => tables.push(range),
                    _ => {}
                }
            }
            Event::End(end) => {
                if is_block(end) {
                    collector.flush(&body, in_heading);
                }
                match end {
                    TagEnd::CodeBlock | TagEnd::MetadataBlock(_) => {
                        opaque_depth = opaque_depth.saturating_sub(1);
                    }
                    TagEnd::Link if is_autolink(&body, &range) => {
                        opaque_depth = opaque_depth.saturating_sub(1);
                    }
                    TagEnd::Heading(_) => in_heading = false,
                    _ => {}
                }
            }
            Event::Text(_) if opaque_depth == 0 => collector.ranges.push(range),
            Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => code.push(range),
            Event::Html(text) | Event::InlineHtml(text) => {
                directives.extend(parse_directives(&text, range.start));
                html.push(range);
            }
            _ => {}
        }
    }
    collector.flush(&body, in_heading);
    code.sort_by_key(|range| range.start);
    let code_free = mask(&body, 0..body.len(), &code, Keep::Outside);
    let mut hidden = code;
    hidden.extend(html);
    hidden.sort_by_key(|range| range.start);
    let visible = mask(&body, 0..body.len(), &merge(hidden), Keep::Outside);
    let blocks = block_starts(&visible, &tables);
    Document {
        segments: collector.segments,
        directives,
        code_free,
        list_items,
        visible,
        blocks,
    }
}

/// Sorted ranges with overlaps joined, as `mask` expects.
fn merge(ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Offsets of lines that start a block: non-blank lines after a blank line (or at the top),
/// plus list-item and heading lines, outside tables. Wrapping a paragraph adds no blocks.
fn block_starts(visible: &str, tables: &[Range<usize>]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut previous_blank = true;
    let mut offset = 0;
    for line in visible.split_inclusive('\n') {
        let text = line.trim();
        let in_table = tables.iter().any(|table| table.contains(&offset));
        if !text.is_empty() && !in_table {
            let marker = text.starts_with('#')
                || text.starts_with(['-', '*', '+'])
                    && text.chars().nth(1).is_some_and(char::is_whitespace)
                || text.split_once(['.', ')']).is_some_and(|(number, _)| {
                    !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
                });
            if previous_blank || marker {
                starts.push(offset);
            }
        }
        previous_blank = text.is_empty();
        offset += line.len();
    }
    starts
}

/// Autolink ends carry the `<url>` range; an inline link's range starts with `[`.
fn is_autolink(body: &str, range: &Range<usize>) -> bool {
    body.as_bytes().get(range.start) == Some(&b'<')
}

/// Span-level tags continue a segment; every other tag starts or ends one.
fn is_block(end: TagEnd) -> bool {
    !matches!(
        end,
        TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::Link
            | TagEnd::Image
    )
}

#[derive(Default)]
struct Collector {
    segments: Vec<Segment>,
    ranges: Vec<Range<usize>>,
}

impl Collector {
    fn flush(&mut self, body: &str, in_heading: bool) {
        let (Some(first), Some(last)) = (self.ranges.first(), self.ranges.last()) else {
            return;
        };
        let span = first.start..last.end;
        let ranges = std::mem::take(&mut self.ranges);
        let text = mask(body, span.clone(), &ranges, Keep::Inside);
        self.segments.push(Segment {
            start: span.start,
            text: mask_urls(&text),
            kind: if in_heading {
                SegmentKind::Heading
            } else {
                SegmentKind::Prose
            },
        });
    }
}

fn parse_plain(source: &str) -> Document {
    let mut segments = Vec::new();
    let mut directives = Vec::new();
    let mut paragraph: Option<Range<usize>> = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        directives.extend(parse_directives(line, offset));
        let end = offset + line.len();
        if line.trim().is_empty() {
            if let Some(span) = paragraph.take() {
                segments.push(plain_segment(source, span));
            }
        } else {
            paragraph = Some(paragraph.map_or(offset..end, |span| span.start..end));
        }
        offset = end;
    }
    if let Some(span) = paragraph {
        segments.push(plain_segment(source, span));
    }
    let blocks = block_starts(source, &[]);
    Document {
        segments,
        directives,
        code_free: source.to_owned(),
        list_items: Vec::new(),
        visible: source.to_owned(),
        blocks,
    }
}

fn plain_segment(source: &str, span: Range<usize>) -> Segment {
    Segment {
        start: span.start,
        text: mask_urls(&source[span]),
        kind: SegmentKind::Prose,
    }
}

/// Byte length of YAML (`---`) or TOML (`+++`) front matter at the very start, or 0.
fn front_matter_len(source: &str) -> usize {
    let mut lines = source.split_inclusive('\n');
    let Some(opening) = lines.next().map(str::trim_end) else {
        return 0;
    };
    let closers: &[&str] = match opening {
        "---" => &["---", "..."],
        "+++" => &["+++"],
        _ => return 0,
    };
    let mut length = source.find('\n').map_or(source.len(), |index| index + 1);
    for line in lines {
        length += line.len();
        if closers.contains(&line.trim_end()) {
            return length;
        }
    }
    0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    Inside,
    Outside,
}

/// Copies `source[span]`, keeping the characters inside (or outside) `ranges` and turning
/// every other character into as many spaces as it has bytes (newlines stay), so offsets
/// never move. `ranges` must be sorted and non-overlapping, as parser events are.
fn mask(source: &str, span: Range<usize>, ranges: &[Range<usize>], keep: Keep) -> String {
    let base = span.start;
    let mut out = String::with_capacity(span.len());
    let mut cursor = 0;
    for (index, character) in source[span].char_indices() {
        let offset = base + index;
        while ranges.get(cursor).is_some_and(|range| range.end <= offset) {
            cursor += 1;
        }
        let inside = ranges
            .get(cursor)
            .is_some_and(|range| range.contains(&offset));
        if character == '\n' || inside == (keep == Keep::Inside) {
            out.push(character);
        } else {
            out.extend(std::iter::repeat_n(' ', character.len_utf8()));
        }
    }
    out
}

/// Blanks bare URLs (`http://…`, `https://…`, `www.…`) up to whitespace, leaving trailing
/// sentence punctuation so sentence splitting still works.
fn mask_urls(text: &str) -> String {
    let mut out = text.to_owned();
    let mut search = 0;
    while let Some(start) = next_url(text, search) {
        let rest = &text[start..];
        let length = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
            .unwrap_or(rest.len());
        let trimmed = rest[..length].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '\'']);
        out.replace_range(start..start + trimmed.len(), &" ".repeat(trimmed.len()));
        search = start + length.max(1);
    }
    out
}

fn next_url(text: &str, from: usize) -> Option<usize> {
    ["http://", "https://", "www."]
        .iter()
        .filter_map(|prefix| {
            let mut at = from;
            while let Some(found) = text.get(at..)?.find(prefix) {
                let index = at + found;
                let starts_word = text[..index]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_alphanumeric());
                if starts_word {
                    return Some(index);
                }
                at = index + prefix.len();
            }
            None
        })
        .min()
}

/// Finds `<!-- tics-ignore… -->` comments in an HTML chunk starting at byte `base`.
fn parse_directives(html: &str, base: usize) -> Vec<Directive> {
    let mut directives = Vec::new();
    let mut rest = 0;
    while let Some(open) = html[rest..].find("<!--") {
        let inner_start = rest + open + 4;
        let Some(close) = html[inner_start..].find("-->") else {
            break;
        };
        let inner = html[inner_start..inner_start + close].trim();
        let offset = base + inner_start + close;
        let kind = match inner {
            "tics-ignore-file" => Some(DirectiveKind::File),
            "tics-ignore-next-line" => Some(DirectiveKind::NextLine),
            _ => inner.strip_prefix("tics-ignore ").map(|ids| {
                DirectiveKind::Rules(
                    ids.split([',', ' ', '\t'])
                        .filter(|id| !id.is_empty())
                        .map(str::to_owned)
                        .collect(),
                )
            }),
        };
        if let Some(kind) = kind {
            directives.push(Directive { offset, kind });
        }
        rest = inner_start + close + 3;
    }
    directives
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};

    use super::{Directive, DirectiveKind, Mode, SegmentKind, parse};

    fn prose(source: &str) -> String {
        parse(source, Mode::Markdown)
            .segments
            .iter()
            .map(|segment| {
                segment
                    .text
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }

    #[test]
    fn fenced_code_blocks_are_skipped_for_backticks_tildes_and_long_fences() {
        let source =
            "a\n\n```rust\nfenced\n```\n\n~~~\ntilde\n~~~\n\n````md\n```\ninner\n```\n````\n\nb\n";
        assert_eq!(prose(source), "a | b");
    }

    #[test]
    fn indented_code_is_skipped_but_lazy_continuation_is_prose() {
        assert_eq!(prose("a\n\n    indented code\n\nb\n"), "a | b");
        assert_eq!(prose("a\n    continued\n"), "a continued");
    }

    #[test]
    fn inline_code_urls_and_link_destinations_are_blanked() {
        let source = "Use `delve()` at https://x.io/delve, <https://auto.delve> or [the docs](https://d.io/delve).\n";
        assert_eq!(prose(source), "Use at , or the docs .");
    }

    #[test]
    fn html_comments_front_matter_and_table_delimiters_are_skipped() {
        let source = "---\ntitle: delve\n---\n<!-- delve -->\n\n| a | b |\n|---|---|\n| c | d |\n";
        assert_eq!(prose(source), "a | b | c | d");
    }

    #[test]
    fn front_matter_is_only_recognised_at_the_start() {
        let source = "para one\n\n---\nkept prose\n---\n";
        assert!(prose(source).contains("kept prose"));
        assert_eq!(prose("+++\nx = 1\n+++\nbody\n"), "body");
        assert_eq!(prose("---\nunterminated\n"), "unterminated");
    }

    #[test]
    fn labels_headings_lists_and_blockquotes_are_prose() {
        let source = "# Head\n\n- item [label](u)\n\n> quoted\n";
        assert_eq!(prose(source), "Head | item label | quoted");
        let document = parse(source, Mode::Markdown);
        assert_eq!(document.segments[0].kind, SegmentKind::Heading);
        assert_eq!(document.list_items, [8]);
    }

    #[test]
    fn segment_offsets_point_into_the_original_source() -> Result<()> {
        let source = "> quote **bold** text\n";
        let document = parse(source, Mode::Markdown);
        let segment = document.segments.first().context("one segment")?;
        let found = segment.text.find("bold").context("bold kept")?;
        assert_eq!(&source[segment.start + found..][..4], "bold");
        Ok(())
    }

    #[test]
    fn code_free_keeps_urls_but_blanks_code() {
        let document = parse(
            "[x](https://a.io/?utm_source=chatgpt.com) `code`\n",
            Mode::Markdown,
        );
        assert!(document.code_free.contains("utm_source=chatgpt.com"));
        assert!(!document.code_free.contains("code"));
    }

    #[test]
    fn directives_are_read_from_html_comments_only() {
        let source = "<!-- tics-ignore-file -->\ntext <!-- tics-ignore em-dash, rule-of-three -->\n\n```\n<!-- tics-ignore-next-line -->\n```\n";
        let kinds: Vec<DirectiveKind> = parse(source, Mode::Markdown)
            .directives
            .into_iter()
            .map(|Directive { kind, .. }| kind)
            .collect();
        assert_eq!(
            kinds,
            [
                DirectiveKind::File,
                DirectiveKind::Rules(vec!["em-dash".into(), "rule-of-three".into()]),
            ]
        );
    }

    #[test]
    fn plain_text_splits_paragraphs_and_reads_directives() {
        let document = parse(
            "one\ntwo\n\n<!-- tics-ignore-next-line -->\nthree https://x.io\n",
            Mode::Plain,
        );
        assert_eq!(document.segments.len(), 2);
        assert_eq!(document.directives.len(), 1);
        assert!(!document.segments[1].text.contains("x.io"));
    }
}
