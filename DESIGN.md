# Design

`tics` is one binary crate (about 3,100 lines of Rust, roughly half of them tests). The pipeline:

```text
inputs.rs    paths -> files (sorted walk) or stdin
document.rs  source -> prose segments (masked, offsets unchanged) + directives + code-free text
sentences.rs segment -> sentence ranges
engine.rs    compiled rules x sentences/segments -> findings with line/column, suppression
score.rs     per-rule counts in a file -> confidence per finding and a score per file
output.rs    human | JSON | summary | scores | rule list
rules.rs     the rule table (data) and the two non-regex heuristics
```

## Dependencies

Versions checked with `cargo info` against crates.io on 2026-10-29; APIs read in
`~/.cargo/registry/src`. 33 crates in the normal dependency tree, most of them clap's and
serde's proc-macro stack.

| Crate | Version | Why |
|-------|---------|-----|
| `pulldown-cmark` | 0.13.5, `default-features = false` | CommonMark + GFM tables parser with `into_offset_iter()` (byte range per event). Default features pull in `getopts` and the HTML renderer, neither needed. |
| `regex` | 1.13.1 | Linear-time matching, Unicode word boundaries, case-insensitive and verbose (`x`) modes; no backtracking blow-ups on hostile input. |
| `clap` | 4.6.7, `derive` | Argument parsing, `--help`/`--version`, exit code 2 on usage errors, value enums for `--format`/`--severity`. |
| `serde` + `serde_json` | 1.0.229 / 1.0.154 | Correct JSON escaping for `--format json`; derive keeps output structs and JSON in sync. clap already brings the proc-macro stack, so the extra build cost is small. |
| `anyhow` | 1.0.104 | Application error chains with context (`reading docs/a.md as UTF-8: …`). |

Considered and not used: `walkdir`/`ignore` (a sorted `read_dir` recursion is 25 lines and the
skip rules are three names), `once_cell`/`LazyLock` (rules are compiled once per run in
`run()` and passed by reference, so no global and no `expect`), `assert_cmd`/`tempfile`
(end-to-end tests use `CARGO_BIN_EXE_tics` and `CARGO_TARGET_TMPDIR`).

## Skipping Markdown with exact positions

**Decision: pulldown-cmark plus a byte-preserving mask.** For each prose block the engine sees
the original source slice with every non-prose character replaced by as many spaces as it
has bytes (newlines kept). An offset in the masked text is the offset in the file, so
line/column come from one binary search over line starts; there is no mapping table to get
wrong.

Evidence (a spike printing `(range, source[range], event)` for a test document):

- Text ranges exclude container syntax: blockquote `> `, list markers and indentation, table
  pipes. Table delimiter rows and link reference definitions emit no events.
- Link labels are `Text` inside `Link`; destinations and titles are outside every `Text`
  range, so they are masked for free. Autolinks are `Link { link_type: Autolink }` with the
  URL as `Text`: skipped by tracking that link type.
- `\*` yields a `Text` range that starts after the backslash; `&amp;` yields a `Text` whose
  range covers the entity. Both map back to real source bytes.
- A fence inside a list item (`1. ```bash`) is a `CodeBlock`; HTML comments arrive as `Html`
  (block, one event per line) or `InlineHtml`, so suppression directives inside code blocks
  are naturally ignored.

A hand-rolled scanner would need CommonMark's block rules to avoid both false positives and
false negatives: lazy paragraph continuation (an indented line after a paragraph is not
code), fences nested in list items, fence length and info-string rules, HTML block types,
reference definitions. That is where a scanner quietly goes wrong, and pulldown-cmark
already passes the CommonMark spec tests.

One exception, found in its source (`firstpass.rs`, `scan_metadata_block`): the
YAML metadata-block option accepts a `---` … `---` block at *any* block start, which would
swallow prose between two thematic breaks. Front matter is therefore blanked by hand, only at
byte 0 (`---`…`---`/`...`, or `+++`…`+++`), before parsing.

Bare URLs (`http://`, `https://`, `www.`) are masked in prose by a small scanner, leaving
trailing sentence punctuation. Inline code, code blocks and math are recorded separately to
build `code_free`, a copy of the source with only code blanked, used by `leaked-markup`,
which must see link destinations (`utm_source=chatgpt.com` lives there).

`.txt` files are plain text: paragraphs split on blank lines, URLs masked, directives read
from any line. Parsing them as Markdown would turn indented paragraphs into "code".

## Rules

`rules.rs` holds a `static RULES: &[Rule]`. A rule is data: id, category, severity, `since`,
source URL, one-line description and a `Detector`:

| Detector | Reads | Used by |
|----------|-------|---------|
| `Text { find, reject, check }` | each sentence (`\A` = sentence start) | most rules |
| `Pair { first, first_reject, second, same_subject }` | adjacent sentences in one block | `not-x-its-y-split`, `does-x-does-not-y` |
| `ListItem(pattern)` | first source line of each real list item | `bold-lead-in` |
| `Heading(fn)` | heading text | `title-case-heading` |
| `Raw(pattern)` | `code_free` | `leaked-markup` |

Patterns are compiled case-insensitive in verbose mode; `(?-i:…)` marks case-sensitive parts
(`INSERT_`, `YYYY-XX-XX`, the lower-case items of `rule-of-three`). A named group `m` narrows
the reported span. `reject` is matched against the sentence *from the hit onwards*, which lets
a rule veto known technical uses without lookarounds (the `regex` crate has none): `unlocks
the mutex`, `landscape mode`, `elevated privileges`, `not supported, but you can …`.
`check` is a plain function for the one heuristic regex cannot express (`rule-of-three`:
at least two of three or more items must be evaluative adjectives or abstract nouns by
suffix or a short list, so `git, curl, and make` passes).

`x-not-y` vetoes qualifiers that do not start a contrast (`, not much happens`, `, not yet`,
`, not necessarily`) and `, not only/just X but …`, which is `negative-parallelism`'s. A bare
`, not only Ollama` or `, not every config key` is the appositive contrast and is reported.

`title-case-heading` skips the first word and counts plain words of four or more letters
(acronyms and mixed-case names such as `GitHub` count neither way). It needs at least two
capitalised words and at least 75 % of them capitalised, so single-word headings (`Install`)
and sentence case with proper nouns (`Reach Ollama or LM Studio on your Mac`) pass.

`does-x-does-not-y` needs "same subject", a backreference; the `Pair` detector captures
`subj` in both sentences and compares them in code instead.

Sentence splitting is deliberately simple: `.`/`!`/`?`/`…` plus closing quotes, followed by
whitespace, except after a short abbreviation list or a single letter. An earlier version
also refused to split before a lower-case word; the self-check below showed that this merged
sentences starting with `microsandbox`, so it was removed.

Regexes are compiled once per run (`engine::compile`), never per file. Reject patterns are
bounded (`{0,80}`) so a hit's veto check is constant-time. Hits are sorted by offset and a
forward cursor turns offsets into line/column, so each character is counted once per file:
counting from the line start for every hit made a 1 MiB single line with 80,000 hits take
1.4 s. `tests/performance.rs` scans two generated 1 MiB files with the built binary
(structured Markdown with code, tables, links and about 18,000 hits; one 1 MiB line with about
91,000 hits). It asserts under 1 s in release builds and under 20 s in debug builds, so a slow
CI runner does not flake. Release, 2026-10-29, Apple Silicon: 0.06 s and 0.11 s.

A leading UTF-8 byte order mark is dropped on read, so front matter after it is recognised and
first-line columns match what an editor shows.

### Tests

- `src/rule_tests.rs`: a table with, per rule, sentences it must flag and realistic technical
  prose it must not flag. One test asserts every rule has ≥ 2 of each; another runs them all
  and reports every miss and false positive at once. 28 rules, 69 positive and 71 negative
  examples.
- Unit tests per module: masking (fences of both kinds and lengths, indented code vs lazy
  continuation, inline code, URLs, autolinks, link destinations, front matter position,
  HTML comments, table delimiters), sentence splitting, exact line/column in blockquotes,
  lists, tables, after front matter and after multi-byte characters, suppression directives
  (including ones inside code, which must not apply), rule metadata, CLI shape.
- `tests/cli.rs`: the built binary on real files and stdin: positions, JSON shape, summary,
  `--select`/`--ignore`/`--severity`, exit codes 0/1/2, directory walking (hidden,
  `target/`, `node_modules/`, extensions, order), symlinked directories not walked, no partial
  output when a later file fails, a byte order mark, `tics rules`, `--version`, and the README
  rule table compared row by row with `tics rules --format json`.
- `tests/performance.rs`: the 1 MiB timing check above.

## Trade-offs and choices the spec left open

- **Columns** count Unicode scalar values from 1, as rustc does.
- Matches can overlap across rules: `pivotal role` is both `ai-vocabulary` and
  `significance-puffing`; `Not only …` is both `negative-parallelism` and
  `not-sentence-start`. Each rule is independent; density counts both.
- **`--summary` with `--format json`** prints `{"findings": [...], "summary": {...}}`; without
  `--summary` the output is the plain array the spec asks for.
- Errors: any unreadable path or non-UTF-8 file stops the run with exit 2 before output, so
  output is never partial.
- **Directives** must be single-line comments; `tics-ignore` accepts several ids separated by
  spaces or commas. Unknown ids in a directive are ignored silently.
- **A file literally named `rules`** must be passed as `./rules` (it is the subcommand).
- Sources: each rule points at the Wikipedia page that best covers it. Read in
  October 2026, *Signs of AI writing* names most patterns, but not `whether-youre`,
  `todays-world`, `signposting` or `rhetorical-reveal` explicitly (they are filed under it as
  the closest page), and it now lists em dashes under "Historical indicators
  (2022–September 2026)". `em-dash` is kept as the spec requires.

## Self-check

Release binary, 2026-10-29. Messages shortened to `rule «matched»` where noted.

### `tics --summary SPEC.md` (exit 1)

```text
SPEC.md:1:8: em-dash: possible AI-writing tic: em dash in prose «—»
SPEC.md:10:27: x-not-y: possible AI-writing tic: appositive contrast (`X, not Y`) «, not proof»
SPEC.md:32:73: negative-parallelism: … «doesn't just X»
SPEC.md:33:19: not-x-but-y: … «not X, but Y»
SPEC.md:41:43: significance-puffing: … «vital role»
…
summary: 77 possible AI-writing tics in 589 words (130.73 per 1,000 words)
  ai-vocabulary          28
  chatbot-residue        6
  em-dash                6
  promotional            6
  significance-puffing   5
  placeholder            4
  x-not-y                4
  …
```

Almost every hit is the spec *quoting* a tic in plain double quotes, which tics cannot tell
from use. The README avoids this by putting examples in backticks. `SPEC.md:10` (`signals,
not proof`) is a genuine instance of the `x-not-y` shape.

### `tics --summary tests/fixtures/tics.md` (exit 1)

```text
tests/fixtures/tics.md:5:3: title-case-heading: possible AI-writing tic: heading in Title Case «Getting Started With The Platform»
tests/fixtures/tics.md:7:1: todays-world: … «In today's fast-paced»
tests/fixtures/tics.md:7:58: not-x-but-y: … «not a tool, but a»
tests/fixtures/tics.md:8:1: transition-opener: … «Additionally,»
tests/fixtures/tics.md:10:3: bold-lead-in: … «**Speed:**»
tests/fixtures/tics.md:11:48: x-not-y: … «, not inside the VM»
tests/fixtures/tics.md:13:1: not-x-its-y-split: … «It isn't a linter. It's a»
tests/fixtures/tics.md:13:49: rule-of-three: … «robust, seamless, and intuitive workflow»
tests/fixtures/tics.md:21:58: leaked-markup: … «utm_source=chatgpt.com»
…
summary: 21 possible AI-writing tics in 86 words (244.19 per 1,000 words)
```

Nothing is reported for the front matter, the fenced block, inline code (`delve()`) or the
line after `<!-- tics-ignore-next-line -->`.

### `tics --summary ~/dev/personal/msbx/docs/src` (read-only; exit 1)

```text
explanation/how-msbx-works.md:32:19: rather-than «instead of»
explanation/how-msbx-works.md:43:74: x-not-y «, not `PATH`»
explanation/security-model.md:20:66: x-not-y «, not only Ollama»
how-to/reach-host-models.md:38:57: x-not-y «, not only model servers»
reference/sandbox-defaults.md:14:35: x-not-y «, not a reservation»
rfcs/0001-git-safety.md:124:74: x-not-y «, not every config key»
rfcs/0005-detachable-sessions.md:115:169: does-x-does-not-y «We take "find the running session and rejoin it". We do not»
rfcs/0006-setup-script.md:81:3: does-x-does-not-y «It runs on launches through `msbx agent` and the TUI. It does not»
…
summary: 95 possible AI-writing tics in 13446 words (7.07 per 1,000 words)
  bold-lead-in       58
  x-not-y            22
  rather-than        12
  does-x-does-not-y  3
```

False positives seen and how they were handled:

- Fixed: `rfcs/0004-shared-logins.md:121` was reported as `not-x-its-y-split`. The
  splitter merged `… anyway. microsandbox cannot …` into one sentence (lower-case start), so a
  later `It is the …` sentence counted as adjacent. Lower-case words now start sentences; a
  regression assertion is in `sentences.rs`.
- Fixed: `Reach Ollama or LM Studio on your Mac` was flagged by `title-case-heading`
  (proper nouns look like Title Case). The bar was raised from half to 75 % of the counted words;
  the heading is now a negative example.
- Fixed (missed tics): `, not only Ollama`, `, not only model servers`, `, not every config
  key`, `, not just block the request` and `, not only its own` were vetoed as qualifiers. They
  are now reported unless a `but` follows.
- Not false positives by the spec's definition: the 58 `bold-lead-in` hits are RFC lists
  in `- **Label:** text` form; the 22 `x-not-y` hits are real `X, not Y` appositives (one is
  the spec's own example). Whether a house style that uses them on purpose wants these is a
  per-project choice: `--ignore` or a directive.
- Checked during development and fixed before this run: `underscore` (the character) vs
  `underscores the need`, `unlocks the mutex`, `landscape mode`, `elevated privileges`,
  `The function as a whole`, `World class`, `as an AIX package`, `[Your first sandbox](…)`
  link labels and `YYYY-MM-DD`; each is a negative example in the rule tests.

## Calibration

Confidence values are fitted, so they need data. `calibration/` holds the scripts; the corpus
itself is not committed, because it is CC BY-SA text and model output that anyone can rebuild.

### Corpus (built 2026-10-29)

| Source | Documents | What |
|---|---|---|
| Stack Exchange | 72 | Top-voted answers from 11 sites, written and last edited before 2022-11-30 |
| Wikipedia | 74 | Article lead sections from the last revision before 2022-11-01 |
| Codex models | 292 | `gpt-6.1-sol`, `gpt-6.1-terra`, `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna` (36-37 each) via `codex exec` |
| Claude models | 146 | Opus, Sonnet and Haiku (48-49 each) via `claude -p` with a plain assistant system prompt |

The human texts predate ChatGPT, so none of them can be AI-assisted. Each model answered the
same question as the human text (the Stack Exchange question, or "Write a short explanation of
X for a general reader"), so topic does not separate the two sides.

### What the corpus showed

Share of documents with at least one hit. Stack Exchange only, where people also write lists
and Markdown:

| Signal | Human | Claude | Codex |
|---|---|---|---|
| `x-not-y` | 8% | 39% | 21% |
| `bold-lead-in`, two or more | 4% | 63% | 29% |
| bold label on its own line | 7% | 68% | 3% |
| `em-dash` | 0% | 0% | 46% |
| `rather-than` | 16% | 28% | 19% |
| `not-x-but-y` | 8% | 3% | 1% |

Most of the vocabulary and rhetoric rules (`delve`, `pivotal role`, `Moreover,`) almost never
fire on these models: `ai-vocabulary` hit 1-3% of answers. Their tells are now structural
(bold labels, label-led lists, stock section names) plus `X, not Y` and, for Codex, the em
dash. Wikipedia leads use em dashes too (21%), which is why that rule needs a weight and not a
yes or no. `not-x-but-y` and `transition-opener` were more common in the human text, so they
get no weight. Two rules came out of this: `bold-label-line` (71% of Claude answers, 5% of
human texts) and `stock-section-label` (33% of Claude answers, no human texts).

The corpus also turned up patterns that are not rules, because people use them as much in
other genres: semicolons (54% of Codex Q&A answers, but 46% of Wikipedia leads) and phrases
such as `rule of thumb` or `in practice`.

### Model

For rule `r` with `tf` hits in a file of `words` words:

```text
x_r = tf * (k1 + 1) / (tf + k1 * (1 - b + b * words / avg_words))      (BM25 saturation)
p   = sigmoid(bias + sum_r w_r * x_r)
```

The weights come from a logistic regression with weights kept at zero or above, an L2 penalty
pulling each weight towards 1.0 (so a rule the corpus never saw keeps a middling weight), and
classes weighted to even odds. `k1` and `b` were picked by grouped 5-fold cross-validation,
where all answers to one question stay in the same fold: `k1 = 2.0`, `b = 0.75`.

Held-out results (AUC 0.73):

| Group | Score ≥ 0.5 | Score ≥ 0.8 |
|---|---|---|
| Human | 18% | 3% |
| Claude Opus / Sonnet | 96% / 96% | 86% / 90% |
| Claude Haiku | 73% | 52% |
| Codex models | 22-78% | 0-22% |

Codex answers are short (a median of 81 words, against 265 for the human texts) and often
have no tics at all, so tics cannot tell them from human text. A score measures tics; it cannot say who wrote a file.

To refit after changing rules, see `calibration/README.md`, then copy the constants from
`calibration/weights.json` into `src/score.rs`. A test fails until they match.

## Provenance

Two independent implementations were written from the same `SPEC.md`: one by a Claude agent
(A) and one by OpenAI Codex gpt-6-astra (B). Both were run on the msbx documentation and
compared; this crate is A with parts of B merged in.

- From A (the base): everything not listed below: the byte-preserving mask and exact
  positions, front matter only at byte 0, the rule table with reject patterns and the
  `rule-of-three` heuristic, `does-x-does-not-y`, the per-rule example table, the CLI and
  output shapes, and `--summary` JSON as `{findings, summary}`.
- From B: wider `x-not-y` coverage (`, not only X`, `, not just X`, `, not every X`,
  without B's unfiltered `, not <anything>`); the 1 MiB timing test, with its dense long-line
  case; one forward pass for line/column (B's design; A recounted from the line start per
  hit); dropping a leading byte order mark; end-to-end tests that symlinked directories are
  not walked and that a failing later file leaves no partial output.
- Not taken from B: its `title-case-heading` (flags single-word headings such as
  `Install`) and `rule-of-three` (flags plain lists such as `KVM, Homebrew, and Rust`), both
  kept as negative examples; the summary on stderr (A keeps stdout self-contained);
  deduplicating paths by canonical name and skipping hidden or build directories named
  explicitly on the command line (A scans what the user names); multi-line suppression
  comments; and the `tempfile` dev-dependency (`CARGO_TARGET_TMPDIR` suffices).
