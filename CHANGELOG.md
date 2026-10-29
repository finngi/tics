# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Rule ids are stable; a rule's `since` version is the release that added it.

## [Unreleased]

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

[Unreleased]: https://github.com/finngi/tics/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/finngi/tics/releases/tag/v0.1.0
