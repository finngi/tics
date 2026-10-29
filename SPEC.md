<!-- tics-ignore-file -->
# tics — spec v0.1

A small, deterministic Rust CLI that flags AI-writing tics in Markdown and plain-text prose.
It will live at https://github.com/finngi/tics (Apache-2.0) and be installed with
`cargo install --git https://github.com/finngi/tics --tag vX.Y.Z`.

## Principles
- Deterministic: same input → same output, no network, no models, no randomness.
- Small: one binary crate, few dependencies, each justified. Rust stable, edition 2024.
- Honest: tics are signals, not proof. Output says "possible AI-writing tic", and the summary
  reports density (hits per 1,000 words) because single hits are noise (Wikipedia's own caveat).
- Versioned: semver `version` in Cargo.toml, `tics --version`, CHANGELOG.md (Keep a Changelog).
  Each rule has a stable id and a `since` version, so rule changes are visible in the changelog.

## Input
- `tics [OPTIONS] [PATH]...` — files or directories (recursive; `.md`, `.markdown`, `.mdx`, `.txt`);
  `-` or no path = stdin. Skip hidden dirs, `target/`, `node_modules/`.
- Markdown-aware: ignore fenced code blocks (``` and ~~~, any info string, fence length rules),
  indented code blocks, inline code spans, autolinks/URLs and link destinations, HTML comments,
  YAML/TOML front matter, and table delimiter rows. Text inside link labels, headings, lists,
  tables and blockquotes IS checked. Report correct 1-based line and column of the original file.
- Suppression: `<!-- tics-ignore-next-line -->`, `<!-- tics-ignore-file -->`, and
  `<!-- tics-ignore rule-id -->` (next line, specific rule).

## Rules (initial set; ids are kebab-case, grouped by category)
Sources: https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing ,
https://en.wikipedia.org/wiki/Wikipedia:Manual_of_Style/Words_to_watch ,
https://en.wikipedia.org/wiki/Wikipedia:Marketing_buzzspeak (October 2026). Each rule records
its source URL. Case-insensitive unless stated.

Contrast / negation (the main target):
- `negative-parallelism`: "not only/just/merely X but (also) Y"; "isn't/doesn't just X".
- `not-x-but-y`: "not X, but Y" / "not X — it's Y" / "isn't X; it's Y" within a sentence.
- `not-x-its-y-split`: "It isn't X. It's Y." across adjacent sentences.
- `x-not-y`: "…X, not Y" appositive contrast ("enforced on your Mac, not inside the VM").
- `does-x-does-not-y`: "It does X. It does not Y." adjacent sentences with same subject.
- `rather-than`: "rather than" / "instead of" (severity info).
- `not-sentence-start`: sentence starting "Not a/an/the/just/only/because".
Rhetoric and structure:
- `trailing-ing-analysis`: ", highlighting/underscoring/emphasizing/reflecting/showcasing/fostering …".
- `significance-puffing`: crucial/pivotal/vital role; "is a testament/reminder"; "setting the stage";
  "evolving landscape"; "indelible mark".
- `importance-hedge`: "it's important/worth noting/remembering", "should be noted".
- `vague-attribution`: "experts/observers/critics argue/say/believe".
- `summary-opener`: sentence starting "In summary," "In conclusion," "Overall," "Ultimately," "In short,".
- `signposting`: "Here's the thing", "Let's dive in/delve/unpack", "In this section, we will".
- `rhetorical-reveal`: "The result?" / "The catch?" / "The answer?".
- `whether-youre`: "Whether you're X or Y".
- `todays-world`: "In today's fast-paced/ever-changing/digital …".
- `transition-opener`: sentence starting "Additionally,", "Moreover,", "Furthermore,", "Notably,".
- `rule-of-three` (info): a sentence with three or more comma-joined single-word or two-word
  adjectives/nouns ending "and/or X" — heuristic; keep false positives low.
Vocabulary:
- `ai-vocabulary`: delve, tapestry, testament, landscape, robust, leverage, underscore, pivotal,
  vibrant, showcase, garner, intricate, meticulous, seamless, holistic, synergy, paradigm, unlock,
  empower, elevate, realm, game-changer, effortless (word-boundary, inflections).
- `promotional`: boasts a/an, groundbreaking, cutting-edge, world-class, state-of-the-art,
  best-in-class.
- `copula-avoidance`: "serves as / stands as / functions as a/an/the".
- `canned-assurance`: "ensure that", "in compliance with", "streamline".
Chatbot residue:
- `chatbot-residue`: "Certainly!", "Of course!", "Great question", "I hope this helps",
  "let me know if", "as an AI".
- `placeholder`: "[Insert …]", "[Describe …]", "INSERT_", "YYYY-XX-XX" — NOT link labels like
  "[Your first sandbox](…)".
- `leaked-markup`: "contentReference[oaicite", "utm_source=chatgpt.com", "referrer=grok.com".
Formatting:
- `em-dash`: "—" in prose.
- `bold-lead-in`: list item starting with a bold label followed by a colon ("- **Label:** text").
- `title-case-heading` (info): heading where most words ≥4 letters are capitalised.

## Options and output
- `--format human|json` (default human). Human: `path:line:col: rule-id: message «matched text»`.
  JSON: one array of objects {path, line, column, rule, category, severity, message, matched}.
- `--summary`: per-rule counts, total, words scanned, hits per 1,000 words.
- `--select ID,...` / `--ignore ID,...`; `--severity warn|info` (minimum, default info).
- `tics rules`: list rules with id, category, severity, since, source, one-line description
  (`--format json` too).
- Exit codes: 0 no findings, 1 findings, 2 usage or IO error.
- No config file in v0.1 (YAGNI).

## Quality
- clippy pedantic clean with `-D warnings`, rustfmt, no `unwrap`/`expect` outside tests,
  `unsafe_code = "forbid"`.
- Tests: for every rule at least two positive and two negative examples (the negatives must be
  realistic technical prose); Markdown skipping (fences, inline code, URLs, front matter,
  comments, tables); line/column accuracy; suppression comments; JSON output; exit codes.
  End-to-end CLI tests with real files.
- README: what it is, install, usage, rule table (generated or checked against `tics rules`),
  caveat on false positives, sources, licence.
- Performance: compile regexes once; scanning 1 MB of Markdown takes well under a second.
