# tics

A small, deterministic command-line tool that flags AI-writing tics in Markdown and plain-text
prose: negative parallelisms (`not X, but Y`), stock openers, puffed-up vocabulary, chatbot
residue and formatting habits.

Same input, same output. No network, no models, no randomness.

> **A hit is a signal and never proof on its own.** People wrote `delve` and em dashes long before chatbots
> did. A single hit means little; look at the density in `--summary` (hits per 1,000 words)
> and read the text. Wikipedia makes the same point about its own list of signs.

## Install

```sh
cargo install --locked --git https://github.com/finngi/tics --tag v0.1.0
```

Needs a stable Rust toolchain (edition 2024, Rust 1.88 or newer).

## Usage

```sh
tics README.md docs/          # files and directories (recursive)
cat draft.md | tics           # stdin (also `tics -`)
tics --summary docs/          # per-rule counts, words scanned, hits per 1,000 words
tics --format json docs/      # machine-readable findings
tics --select em-dash,x-not-y docs/
tics --ignore rule-of-three --severity warn docs/
tics --min-confidence 0.6 docs/   # hide weak findings, such as one bold label in a long file
tics score docs/              # one 0-1 score per file (also `--format json`)
tics rules                    # list the rules (also `--format json`)
```

Output, one finding per line:

```text
docs/intro.md:12:41: x-not-y: possible AI-writing tic: appositive contrast (`X, not Y`) «, not inside the VM» (confidence 0.53)
```

The column counts Unicode characters from 1. JSON output is an array of objects with `path`,
`line`, `column`, `rule`, `category`, `severity`, `message`, `matched` and `confidence`. With `--summary`,
JSON output becomes an object `{"findings": [...], "summary": {...}}`.

Exit codes: `0` no findings, `1` findings, `2` usage or IO error. `tics score` exits `0`
unless there is an error.

### Confidence

Each finding has a confidence from 0 to 1: how likely it is, from this rule's hits in this file
alone, that the file is AI-styled text, with even odds to start. `tics score` combines every
rule into one number per file.

Counts matter more than single hits. One bold label in a long page is something people write
too; six in a short answer is not:

```text
$ tics score notes.md answer.md
notes.md: 0.51 (1 finding in 475 words): bold-lead-in ×1
answer.md: 0.96 (6 findings in 52 words): bold-lead-in ×6
```

The weights come from 146 human texts written before ChatGPT and 438 answers from eleven
Claude and Codex models (see [DESIGN.md](DESIGN.md#calibration)). Rules that people use as
often as models, such as `not-x-but-y`, get no weight and stay at the floor of 0.29. On
held-out files, 3% of human texts scored 0.8 or more, against 86-90% of answers from Claude
Opus and Sonnet and 0-22% from the Codex models, whose answers often have no tics at all. A
score is a reason to reread a draft. It is not proof of who wrote it.

### What is checked

Directories are walked in sorted order for `.md`, `.markdown`, `.mdx` and `.txt` files,
skipping hidden directories, `target/` and `node_modules/`. Files named on the command line
are checked whatever their extension. `.txt` files are plain text; everything else, stdin
included, is Markdown.

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
| Rule | Category | Severity | Since | Description | Source |
|------|----------|----------|-------|-------------|--------|
| `negative-parallelism` | contrast | warn | 0.1.0 | negative parallelism (`not only X but also Y`, `isn't just X`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `not-x-but-y` | contrast | warn | 0.1.0 | contrast inside one sentence (`not X, but Y`, `isn't X; it's Y`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `not-x-its-y-split` | contrast | warn | 0.1.0 | contrast split over two sentences (`It isn't X. It's Y.`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `x-not-y` | contrast | warn | 0.1.0 | appositive contrast (`X, not Y`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `does-x-does-not-y` | contrast | warn | 0.1.0 | same subject affirmed then denied (`It does X. It does not Y.`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `rather-than` | contrast | info | 0.1.0 | contrast framing (`rather than`, `instead of`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `not-sentence-start` | contrast | warn | 0.1.0 | sentence opening with a negation (`Not a…`, `Not just…`, `Not because…`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `trailing-ing-analysis` | rhetoric | warn | 0.1.0 | trailing participle analysis (`, highlighting…`, `, underscoring…`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `significance-puffing` | rhetoric | warn | 0.1.0 | puffed-up significance (`pivotal role`, `is a testament`, `evolving landscape`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `importance-hedge` | rhetoric | warn | 0.1.0 | editorialising hedge (`it's worth noting`, `it should be noted`) | [Manual of Style/Words to watch](https://en.wikipedia.org/wiki/Wikipedia:Manual_of_Style/Words_to_watch) |
| `vague-attribution` | rhetoric | warn | 0.1.0 | vague attribution (`experts argue`, `critics say`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `summary-opener` | rhetoric | warn | 0.1.0 | summary opener (`In summary,`, `Overall,`, `Ultimately,`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `signposting` | rhetoric | warn | 0.1.0 | chatty signposting (`Here's the thing`, `Let's dive in`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `rhetorical-reveal` | rhetoric | warn | 0.1.0 | rhetorical reveal (`The result?`, `The catch?`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `whether-youre` | rhetoric | warn | 0.1.0 | audience sweep (`Whether you're X or Y`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `todays-world` | rhetoric | warn | 0.1.0 | era cliché (`In today's fast-paced world`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `transition-opener` | rhetoric | warn | 0.1.0 | stock transition opener (`Additionally,`, `Moreover,`, `Furthermore,`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `rule-of-three` | rhetoric | info | 0.1.0 | list of three or more adjectives or abstract nouns (`fast, reliable, and secure`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `ai-vocabulary` | vocabulary | warn | 0.1.0 | overused AI vocabulary (`delve`, `tapestry`, `robust`, `seamless`, …) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `promotional` | vocabulary | warn | 0.1.0 | promotional buzzwords (`boasts a`, `cutting-edge`, `best-in-class`) | [Marketing buzzspeak](https://en.wikipedia.org/wiki/Wikipedia:Marketing_buzzspeak) |
| `copula-avoidance` | vocabulary | warn | 0.1.0 | copula avoidance (`serves as a`, `stands as the`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `canned-assurance` | vocabulary | warn | 0.1.0 | canned assurance (`ensure that`, `in compliance with`, `streamline`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `chatbot-residue` | chatbot | warn | 0.1.0 | chatbot residue (`Certainly!`, `I hope this helps`, `as an AI`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `placeholder` | chatbot | warn | 0.1.0 | unfilled placeholder (`[Insert …]`, `INSERT_`, `YYYY-XX-XX`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `leaked-markup` | chatbot | warn | 0.1.0 | leaked chatbot markup (`contentReference[oaicite`, `utm_source=chatgpt.com`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `em-dash` | formatting | warn | 0.1.0 | em dash in prose | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `bold-lead-in` | formatting | warn | 0.1.0 | list item led by a bold label and colon (`- **Label:** text`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `bold-label-line` | formatting | warn | 0.2.0 | paragraph led by a bold label, or a bold line used as a heading (`**Why:** …`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `stock-section-label` | formatting | warn | 0.2.0 | stock section label (`Why it matters`, `How it works`, `Short answer:`, `TL;DR`) | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
| `title-case-heading` | formatting | info | 0.1.0 | heading in Title Case | [Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) |
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
