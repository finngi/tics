# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Rule ids are stable; a rule's `since` version is the release that added it.

## [Unreleased]

## [0.3.0] - 2026-11-06

### Added

- `tics review`: advisory passage hotspots, full source-exact candidates, JSON, JSONL and
  CI Markdown. Machine reports include review instructions and context links; source bodies
  are omitted for files with no candidates, which are explicitly marked unreviewed.
- `tics instructions` and a portable `tics-review` skill for a separate human or LLM reviewer.
- Reusable GitHub Action for changed prose, an updated PR conversation comment and a job
  summary. Findings are advisory; read-only fork tokens fall back to the summary.
- Review configuration for rule selection, exclusions, allowed phrases and custom patterns;
  the Action accepts matching YAML inputs. PR reports show match counts and collapsed
  source-linked findings.
- The Action exposes a complete JSON report path and match counts for downstream agent
  steps. Shared source packages also support a local Action reference before publication.
- One review-only `guided-tour-variants` cue, with conditional guidance for legitimate uses.
  Review findings carry no authorship confidence or automatic edit approval.

### Review contract

- Half-open UTF-8 byte ranges and complete original matches, including initial BOM bytes;
  one-based Unicode scalar display columns ignore the initial BOM.
- Markdown math masking and canonical-path input deduplication in review mode.
- Existing 0.2 scan rules, score calibration, JSON and compact output remain unchanged.
  Tics does not rewrite files or invoke an LLM.

Evaluation reports are retained in [docs/review.md](docs/review.md#evaluation-history);
they do not establish improved downstream correction quality.

## [0.2.0] - 2026-10-29

### Added

- A confidence from 0 to 1 on every finding, in human and JSON output, from a beta-binomial
  likelihood-ratio model fitted on a corpus of human and AI text (DESIGN.md, "Calibration").
  A rule's hits are counted as occupied opportunities (sentences, list items, headings or
  Markdown blocks), so repetition and file length are handled by the model.
- `--min-confidence N` to report only findings at or above `N`.
- `tics score [PATH]...`: one score per file with the rules behind it, `--sections` for one
  score per heading section, and `units`, `occupied` and `sections` in JSON. Files with no
  prose get no score.
- A signed strength per rule in `tics rules` and the README table.
- Rules `bold-label-line` (a paragraph led by a bold label, or a bold line used as a heading)
  and `stock-section-label` (`Why it matters`, `How it works`, `Short answer:`, `TL;DR`).
- `calibration/`: scripts that build the corpus and fit the model, with their reports.

- Readable terminal output (`--format pretty`, the default in a terminal): file percentages,
  counts by rule, findings in file order with fixes, and a compact files overview. Short files
  are marked as having too little text to judge. `--color auto|always|never` respects `NO_COLOR`.
- `--json` as a shorthand for `--format json`.

### Changed

- Directory walks respect parent and nested `.gitignore` files. Explicit file arguments
  still scan ignored files.
- The one-line-per-finding output is now `--format compact` (`--format human` still works)
  and is the default when output is piped. Each line ends with `(confidence N)`, and JSON
  findings gain a `confidence` field.

## [0.1.0] - 2026-10-29

### Added

- `tics [OPTIONS] [PATH]...` for Markdown (`.md`, `.markdown`, `.mdx`) and plain-text
  (`.txt`) files, directories and stdin, with exact 1-based line and column.
- Markdown-aware scanning that skips code, URLs, link destinations, HTML, front matter and
  table delimiter rows.
- Suppression comments: `tics-ignore-next-line`, `tics-ignore <rule-id>`, `tics-ignore-file`.
- Output as `human` or `json`; `--summary` with per-rule counts, words scanned and hits per
  1,000 words; `--select`, `--ignore` and `--severity`.
- A leading UTF-8 byte order mark is ignored.
- Scanning 1 MiB of Markdown takes well under a second, checked by a release-build test.
- `tics rules` (human or JSON).
- 28 rules:
  - contrast: `negative-parallelism`, `not-x-but-y`, `not-x-its-y-split`, `x-not-y`,
    `does-x-does-not-y`, `rather-than`, `not-sentence-start`;
  - rhetoric: `trailing-ing-analysis`, `significance-puffing`, `importance-hedge`,
    `vague-attribution`, `summary-opener`, `signposting`, `rhetorical-reveal`,
    `whether-youre`, `todays-world`, `transition-opener`, `rule-of-three`;
  - vocabulary: `ai-vocabulary`, `promotional`, `copula-avoidance`, `canned-assurance`;
  - chatbot: `chatbot-residue`, `placeholder`, `leaked-markup`;
  - formatting: `em-dash`, `bold-lead-in`, `title-case-heading`.

[Unreleased]: https://github.com/finngi/tics/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/finngi/tics/compare/v0.1.0...v0.3.0
[0.2.0]: https://github.com/finngi/tics/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/finngi/tics/releases/tag/v0.1.0
