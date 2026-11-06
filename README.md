# tics

A small, deterministic local tool for spotting writing patterns worth reviewing in Markdown
and plain text: stock framing, inflated vocabulary, empty contrasts, chatbot residue and
repeated formatting.

`tics review` gives a human or a downstream LLM exact candidates and surrounding context.
Review the matches in context.
Tics itself makes no network requests, runs no models and never rewrites files.

## Install 0.3.0

With Rust 1.88 or newer:

```sh
cargo install --git https://github.com/finngi/tics --tag v0.3.0 --locked --force
tics --version
```

Prebuilt executables for macOS and Linux are available on the
[0.3.0 release](https://github.com/finngi/tics/releases/tag/v0.3.0).
See [installation and CI options](docs/distribution.md) for the shared source package.

To install a local checkout, run from its repository root:

```sh
cargo install --path . --locked --force
tics --version
```

This replaces the `tics` binary in Cargo's bin directory. To try the checkout separately, use
`cargo build --release --locked` and run `./target/release/tics`.

## Editorial review

```sh
tics review draft.md                  # passage hotspots and findings in a terminal
tics review docs/ --json              # exact candidates and source context
tics review docs/ --format jsonl      # records for a downstream consumer
tics review docs/ --format ci         # Markdown report with review instructions
tics instructions                    # print the companion review skill
```

Review defaults to JSONL when piped. Its 31 cues include the existing 30 rules and the
review-only `guided-tour-variants` cue. Conditional hints help the reviewer retain legitimate
teaching, useful distinctions, technical claims and deliberate voice. Hotspot counts indicate
where patterns occur, not where edits are necessarily needed.

Exit `1` means candidates were found; `0` means none were found; `2` means a usage or input
error. Exit `0` records that no selected patterns matched.
Zero-candidate file records omit the source body. Never treat exit `2` as a clean result.

See [the review contract and examples](docs/review.md) and the portable
[review skill](skills/tics-review/SKILL.md). Downstream edits need their own source validation,
meaning and grammar review. Local experiments have **not demonstrated a correction-efficiency
gain without quality loss**; [evaluation history](docs/review.md#evaluation-history) preserves
those results.

## GitHub pull requests

The reusable Action scans changed Markdown and text files, posts one advisory PR comment
and updates it on later runs. It also writes a job summary; fork PRs retain that summary
when their token cannot post comments. See the [complete workflow example](docs/github-action.md).

```yaml
- uses: finngi/tics@v0.3.0
```

The Action requires checkout of the PR head and comment permissions, as shown in the example.
Use its `select`, `ignore`, `severity` and `config` inputs to choose rules or load a JSON
config with custom patterns and allowed phrases. The PR comment shows match counts and
collapsed findings linked to source lines.

## Existing scan and score commands

```sh
tics README.md docs/          # files and directories (recursive)
cat draft.md | tics           # stdin (also `tics -`)
tics --summary docs/          # per-rule counts, words scanned, hits per 1,000 words
tics --json docs/             # machine-readable findings (also `--format json`)
tics --select em-dash,x-not-y docs/
tics --ignore rule-of-three --severity warn docs/
tics --min-confidence 0.6 docs/   # hide weak findings, such as one bold label in a long file
tics score docs/              # one 0-1 score per file (also `--format json`)
tics rules                    # list the rules (also `--format json`)
```

In a terminal, files with findings show their overall AI-style percentage and counts by rule.
Findings follow in file order, with the match underlined and a fix. The pattern percentage uses
that rule alone; below 50% means its observed rate is more typical of human text. The files
overview lists up to ten files by score. Files with fewer than 80 words are marked as too short
to judge:

```text
tests/fixtures/notes.md
  AI-style: 62% · 1 finding
  Tics: bold-lead-in ×1

  10:3 List item led by a bold label and colon · 81% AI-leaning pattern
  │ - **Note:** the cache now lives in `target/shared`, so delete the old one by hand.
  │   ^^^^^^^^^
  ╰ Write the items as plain sentences, or keep bold labels few. (bold-lead-in)

Files (AI-style)
  62%  tests/fixtures/notes.md  1 finding

● 1 possible tic in 1 of 1 file (168 words)
  Pattern below 50%: more typical of human text.
```

Piped into another program, or with `--format compact`, it prints one finding per line:

```text
tests/fixtures/tics.md:11:48: x-not-y: possible AI-writing tic: appositive contrast (`X, not Y`) «, not inside the VM» (confidence 0.82)
```

Colour follows `--color auto|always|never`; `auto` turns it off for pipes and when `NO_COLOR`
is set. The column counts Unicode characters from 1. JSON output is an array of objects with `path`,
`line`, `column`, `rule`, `category`, `severity`, `message`, `matched` and `confidence`. With `--summary`,
JSON output becomes an object `{"findings": [...], "summary": {...}}`.

Exit codes: `0` no findings, `1` findings, `2` usage or IO error. `tics score` exits `0`
unless there is an error.

### Legacy confidence

These scores describe the fitted authorship-style corpus, not the probability that a phrase
needs editing. Review output does not use them.

Each finding has a confidence from 0 to 1: how likely it is, from this rule's hits in this file
alone and with even odds to start, that the file is AI-styled. Below 0.5 means the pattern, at
this rate, is more typical of people. `tics score` combines every rule into one number per
file, and `--sections` adds one per heading. A file or section with no prose (empty, only
code, only a table) gets `n/a`.

How often a pattern repeats matters more than a single hit, because a model that picks a
pattern tends to keep copying it. Both files below use `- **Label:**` list items:

```text
$ tics score tests/fixtures/notes.md tests/fixtures/answer.md
tests/fixtures/notes.md: 0.62 (1 finding in 168 words): bold-lead-in ×1
tests/fixtures/answer.md: 0.91 (6 findings in 52 words): bold-lead-in ×6
```

The model is fitted on 146 human texts from before ChatGPT and 438 answers to the same
prompts from eleven Claude and Codex models; [DESIGN.md](DESIGN.md#calibration) has the method
and its limits. On held-out files, 3% of the human texts scored 0.8 or more, against 70% of
Claude answers and 10% of Codex answers. Codex answers are short and often have no tics at
all. Part of what separates this corpus is length and layout, which tics does not judge, so
treat a score as a reason to reread a draft. It is not proof of who wrote it.

### What is checked

Directories are walked in sorted order for `.md`, `.markdown`, `.mdx` and `.txt` files,
respecting `.gitignore` rules in the directory and its parents, including nested overrides.
This works outside Git repositories too. Hidden directories, `target/` and `node_modules/`
are always skipped during a walk. Files named on the command line are checked whatever their
extension or ignore status. `.txt` files are plain text; everything else, stdin included,
is Markdown.

In Markdown, tics ignores fenced and indented code blocks, inline code, autolinks, bare URLs,
link destinations, HTML (including comments), YAML or TOML front matter at the top of the
file, and table delimiter rows. It checks the text of headings, paragraphs, lists, tables,
blockquotes and link labels.

### Suppressing findings

```markdown
<!-- tics-ignore-next-line -->
<!-- tics-ignore rule-id -->       (next line, only that rule; several ids may be listed)
<!-- tics-ignore-file -->
```

A directive must be a single-line HTML comment. Directives inside code blocks do nothing.

## Rules

Every rule has a stable id and the version it appeared in, so rule changes show up in the
[changelog](CHANGELOG.md). `info` rules are noisier heuristics; hide them with
`--severity warn`.

<!-- rules:start -->
| Rule | Category | Severity | Since | Strength | Description | Source |
|------|----------|----------|-------|----------|-------------|--------|
| `negative-parallelism` | contrast | warn | 0.1.0 | -0.01 | negative parallelism (`not only X but also Y`, `isn't just X`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `not-x-but-y` | contrast | warn | 0.1.0 | -0.05 | contrast inside one sentence (`not X, but Y`, `isn't X; it's Y`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `not-x-its-y-split` | contrast | warn | 0.1.0 | -0.00 | contrast split over two sentences (`It isn't X. It's Y.`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `x-not-y` | contrast | warn | 0.1.0 | +0.16 | appositive contrast (`X, not Y`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `does-x-does-not-y` | contrast | warn | 0.1.0 | +0.00 | same subject affirmed then denied (`It does X. It does not Y.`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `rather-than` | contrast | info | 0.1.0 | +0.02 | contrast framing (`rather than`, `instead of`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `not-sentence-start` | contrast | warn | 0.1.0 | +0.00 | sentence opening with a negation (`Not a…`, `Not just…`, `Not because…`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `trailing-ing-analysis` | rhetoric | warn | 0.1.0 | +0.00 | trailing participle analysis (`, highlighting…`, `, underscoring…`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `significance-puffing` | rhetoric | warn | 0.1.0 | +0.00 | puffed-up significance (`pivotal role`, `is a testament`, `evolving landscape`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `importance-hedge` | rhetoric | warn | 0.1.0 | -0.01 | editorialising hedge (`it's worth noting`, `it should be noted`) | [Manual of Style/Words to watch](https://en.wikipedia.org/wiki/Wikipedia:Manual_of_Style/Words_to_watch) |
| `vague-attribution` | rhetoric | warn | 0.1.0 | +0.00 | vague attribution (`experts argue`, `critics say`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `summary-opener` | rhetoric | warn | 0.1.0 | +0.00 | summary opener (`In summary,`, `Overall,`, `Ultimately,`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `signposting` | rhetoric | warn | 0.1.0 | +0.00 | chatty signposting (`Here's the thing`, `Let's dive in`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `rhetorical-reveal` | rhetoric | warn | 0.1.0 | +0.00 | rhetorical reveal (`The result?`, `The catch?`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `whether-youre` | rhetoric | warn | 0.1.0 | +0.00 | audience sweep (`Whether you're X or Y`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `todays-world` | rhetoric | warn | 0.1.0 | +0.00 | era cliché (`In today's fast-paced world`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `transition-opener` | rhetoric | warn | 0.1.0 | -0.04 | stock transition opener (`Additionally,`, `Moreover,`, `Furthermore,`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `rule-of-three` | rhetoric | info | 0.1.0 | +0.00 | list of three or more adjectives or abstract nouns (`fast, reliable, and secure`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `ai-vocabulary` | vocabulary | warn | 0.1.0 | +0.00 | overused AI vocabulary (`delve`, `tapestry`, `robust`, `seamless`, …) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `promotional` | vocabulary | warn | 0.1.0 | +0.00 | promotional buzzwords (`boasts a`, `cutting-edge`, `best-in-class`) | [Marketing buzzspeak](https://en.wikipedia.org/wiki/Wikipedia:Marketing_buzzspeak) |
| `copula-avoidance` | vocabulary | warn | 0.1.0 | -0.01 | copula avoidance (`serves as a`, `stands as the`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `canned-assurance` | vocabulary | warn | 0.1.0 | -0.00 | canned assurance (`ensure that`, `in compliance with`, `streamline`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `chatbot-residue` | chatbot | warn | 0.1.0 | +0.00 | chatbot residue (`Certainly!`, `I hope this helps`, `as an AI`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `placeholder` | chatbot | warn | 0.1.0 | +0.00 | unfilled placeholder (`[Insert …]`, `INSERT_`, `YYYY-XX-XX`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `leaked-markup` | chatbot | warn | 0.1.0 | +0.00 | leaked chatbot markup (`contentReference[oaicite`, `utm_source=chatgpt.com`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `em-dash` | formatting | warn | 0.1.0 | +0.14 | em dash in prose | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `bold-lead-in` | formatting | warn | 0.1.0 | +1.03 | list item led by a bold label and colon (`- **Label:** text`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `bold-label-line` | formatting | warn | 0.2.0 | +0.74 | paragraph led by a bold label, or a bold line used as a heading (`**Why:** …`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `stock-section-label` | formatting | warn | 0.2.0 | +0.42 | stock section label (`Why it matters`, `How it works`, `Short answer:`, `TL;DR`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `title-case-heading` | formatting | info | 0.1.0 | +0.03 | heading in Title Case | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
<!-- rules:end -->

This table is checked against `tics rules` by the test suite.

## False positives

The patterns are tuned on technical prose, and each rule's tests include realistic sentences it
must leave alone (`unlocks the mutex`, `landscape mode`, `Do not just restart the daemon`).
Some constructions are ambiguous by nature: `serves as the entry point` or `Use tabs, not
spaces.` are fine sentences that match a tic's shape. Use the suppression comments for
deliberate uses and `--ignore` for rules that do not suit your writing. See
[DESIGN.md](DESIGN.md) for the trade-offs.

## Sources

- [Wikipedia: Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing)
- [Wikipedia: Manual of Style/Words to watch](https://en.wikipedia.org/wiki/Wikipedia:Manual_of_Style/Words_to_watch)
- [Wikipedia: Marketing buzzspeak](https://en.wikipedia.org/wiki/Wikipedia:Marketing_buzzspeak)

Read in October 2026.

## Licence

Apache-2.0. See [LICENSE](LICENSE).
