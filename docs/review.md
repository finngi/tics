# Editorial review

`tics review` locates lexical and structural cues for a human or separate LLM to judge.
It does not classify every writing issue, approve replacements, call a service or modify files.
Its additional `guided-tour-variants` cue can match valid teaching and invitations; the
conditional hint asks whether the framing helps the reader.

## Commands and exit status

```sh
tics review draft.md --format pretty
tics review docs/ --json > review.json
tics review docs/ --format jsonl > review.jsonl
tics review docs/ --format ci > review.md
tics instructions
```

Exit `0` means no candidates, `1` means at least one candidate, and `2` means a usage or IO
error. Shell and CI callers must handle `1` as a completed scan, rather than discard its
output as an execution failure. An input error produces no partial report.

Terminal review output shows counts, passage hotspots and selected findings. Piped review
output defaults to JSONL. `--select`, `--ignore` and `--severity` filter candidates. A positive
`--min-confidence` is rejected: legacy authorship scores are not edit probabilities.
`--format jsonl` and `--format ci` on the main scan command also select review mode.
`--json` and `--format` are mutually exclusive.

The existing `tics --json`, compact scan output, `tics score` and `tics rules` retain their
0.2 contracts. `tics rules` lists the 30 legacy rules; the additional review-only cue is
`guided-tour-variants`. Review deduplicates overlapping input paths by canonical file path.

## Machine contract

The schema identifier is `tics.review.v1`. JSON is an object containing `schema`,
`instructions`, `summary` and `files`. JSONL emits, in order:

1. One `instructions` record with the schema, coordinate convention and embedded skill.
2. A `file` record for each input, followed by that file's `candidate` records.
3. One `summary` record.

A file has `path`, `format`, `words`, `needs_review`, `status`, `passages` and, only when
flagged, the complete `source`. JSON includes its `candidates` array; JSONL instead includes
`candidate_count` and separate candidate records carrying `path`.

- `status: "candidates_found"` means review is needed, not that an error was confirmed.
- `status: "no_candidates_unreviewed"` is a legacy status string meaning no selected
  patterns matched. The source body is omitted; passage metadata can still be present. Tics
  does not record a file's external editor or LLM review history.
- Summary fields are `files`, `files_needing_review`, `files_without_candidates`, `candidates`
  and `words`.

Each candidate contains `id`, `rule`, `category`, `range`, full verbatim `original`,
`crosses_code_or_html`, optional `passage_id`, `review_hint` and `status: "needs_judgment"`.
Categories are rule families, not confirmed editorial diagnoses. Candidate IDs (`f1`, …) and
passage IDs (`p1`, …) are local to a file and report; join them with `path`, and do not reuse
them as persistent IDs after the source changes.

Passages contain a `range`, `kind` (`heading` or `prose`), `heading_id`, `previous_id`,
`next_id` and `candidate_count`. These are parser-derived blocks, not guaranteed grammatical
sentences or a replacement-unit contract. Nearby and earlier source can be necessary to
judge a recap, contrast or claim.

### Exact coordinates

Every range has `start_byte`, `end_byte`, `start_line`, `start_column`, `end_line` and
`end_column`. Byte ranges are half-open UTF-8 offsets into the unchanged original source,
without newline normalization. End positions are exclusive; lines and Unicode scalar
columns are one-based. Initial UTF-8 BOM bytes count toward byte offsets, but the BOM does
not consume a display column. Columns are not terminal display widths or UTF-16 indices.

A consumer can verify each JSON candidate in Python:

```python
import json

with open("review.json", encoding="utf-8") as stream:
    report = json.load(stream)
for file in report["files"]:
    if not file["needs_review"]:
        continue
    source = file["source"].encode("utf-8")
    for candidate in file["candidates"]:
        r = candidate["range"]
        assert source[r["start_byte"]:r["end_byte"]].decode("utf-8") == candidate["original"]
```

The complete `original` differs from the shortened legacy `matched` display. It can verify
an anchor, but it is not necessarily the appropriate extent for an edit. Revalidate against
the current file before applying any downstream changes.

## Context and protection limits

Markdown scanning masks code, HTML, URLs, link destinations and other nonprose as in the
legacy scanner, and additionally masks dollar-delimited math (`$…$`, `$$…$$`) in review
mode. Plain text and other equation delimiters remain visible. Quoted prose and link labels
may be scanned; a match does not authorize changing a quotation, equation or linked text.

A regex can bridge masked regions. `crosses_code_or_html: true` signals that the source cue
contains masked material; despite the historical field name, it is a general masked-span
warning. Keep that material intact and refine the proposed edit rather than replace the
entire candidate mechanically. Source fidelity and masking are not a complete edit-safety
validator.

Use the embedded instructions or [portable skill](../skills/tics-review/SKILL.md) to dismiss
acceptable uses and assess context. Downstream reviewers should preserve facts, scientific
qualifications, useful orientation and voice; compose all proposed changes and check the
resulting grammar. This CLI provides neither automatic patch application nor a guarantee
that a model follows those instructions.

## Evaluation history

The [initial 32-file evaluation](0.3-evaluation.md) and
[expanded regression investigation](0.3-regression-investigation.md) are retained as
historical evidence, including negative results. They have not demonstrated a correction-
efficiency gain without quality loss. These development sets and model-reviewed judgments
are not population accuracy estimates.

When comparing a gated workflow with direct LLM review, count every source file, including
skips and their missed issues. Assess actual corrected copies for useful issue removal,
unnecessary edits, grammar and meaning, alongside total latency and model usage. Fewer calls
or more detected cues alone do not establish a better correction workflow.

## Review configuration

Pass an explicit JSON file with `tics review --config .tics.json`; configuration is not
loaded automatically and is not accepted by legacy scan or score commands. For example:

```json
{
  "select": ["ai-vocabulary", "house-phrase"],
  "ignore": [],
  "allow": ["a robust standard"],
  "severity": "info",
  "custom_rules": [
    {
      "id": "house-phrase",
      "title": "Check house phrase",
      "pattern": "(?i)world.class",
      "hint": "Does the claim have specific support?",
      "severity": "warn"
    }
  ]
}
```

Every top-level field is optional. `select` limits the rules, including custom rules; an
explicit empty array selects none. `ignore` removes rules from that selection. A CLI
`--select` replaces the config selection, and CLI `--ignore` adds to config ignores. The
explicit CLI `--severity` takes precedence over config `severity`; otherwise the default is
`info`. Allowed severities are `info` and `warn`. Unknown selected or ignored IDs, unknown
fields, duplicate or built-in custom IDs, invalid patterns and malformed JSON are errors.

Custom rules require all five fields shown. IDs must match `[a-z][a-z0-9_-]*` so they can
be selected on the command line. Patterns use Rust regex syntax and its normal
case-sensitive default; add `(?i)` for case-insensitive matching. They match visible text
within a review passage; code, HTML and Markdown math receive the same protection as other
review candidates, and a custom match cannot cross masked material. Candidates carry the
configured title and hint and participate in passage counts and exit status. Review custom
matches in context. No configuration field executes commands.

`allow` contains literal, case-insensitive phrases. A finding is suppressed only when its
whole source range is inside a matching phrase in visible source; protected regions do not
supply allow matches. Candidate `title` is a display string for both built-in and custom
rules; `rule` remains the stable rule identifier.
