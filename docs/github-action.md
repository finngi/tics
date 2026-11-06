# GitHub Action

The reusable Action reviews changed `.md`, `.markdown` and `.txt` files in a pull request.
It maintains one advisory conversation comment and writes a job summary. The comment shows
counts and findings collapsed by file and passage with links to source.
Review the matches in context. The Action makes no editorial decisions or file edits.

Use `finngi/tics@v0.3.0` for this release. Pin the Action and checkout to verified commit
SHAs when adopting a release.

## Workflow

```yaml
name: Prose review
on:
  pull_request:

concurrency:
  group: tics-${{ github.event.pull_request.number }}
  cancel-in-progress: true

permissions:
  contents: read
  pull-requests: write

jobs:
  tics:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          ref: ${{ github.event.pull_request.head.sha }}
          persist-credentials: false
      - uses: finngi/tics@v0.3.0
        with:
          config: .tics.json
          ignore: em-dash,rule-of-three
```

Omit `with` to use all built-in rules. Inputs accept these settings:

| Input | Meaning |
| --- | --- |
| `config` | Repository-relative JSON config path; loaded only when specified. |
| `select` | Enable only these rule IDs, separated by commas or newlines. Replaces the config selection. |
| `ignore` | Disable these rule IDs, separated by commas or newlines. Adds to config exclusions. |
| `severity` | Minimum `info` or `warn`; overrides the config value when supplied. |

For example, `select: importance-hedge,summary-opener` restricts the scan to those two rules.
Rules in `ignore` are excluded from the resulting selection. Custom rule IDs participate
in the same selection, so include them when using `select`.

## Custom patterns and exceptions

Save this as `.tics.json` for the workflow above:

```json
{
  "allow": ["seamless pipe"],
  "custom_rules": [
    {
      "id": "team-marketing",
      "title": "Marketing claim",
      "pattern": "(?i)world-class|best-in-class",
      "hint": "Give the specific comparison or evidence for this claim.",
      "severity": "warn"
    }
  ]
}
```

This adds a case-insensitive custom pattern alongside the built-in rules. An allowed phrase
suppresses findings whose entire matched range falls inside that phrase. Allowed phrases are
literal and case-insensitive; this example permits the technical phrase `seamless pipe`.
To disable the added rule, use `ignore: team-marketing`, or remove it and its references from
the config. To replace a built-in rule, disable its ID and add a custom rule with a new ID.

The config also accepts `select`, `ignore` and `severity`. Custom IDs use
`[a-z][a-z0-9_-]*` and must be unique. All five custom-rule fields shown above are required.
Patterns use Rust regex syntax; `(?i)` enables case-insensitive matching. Patterns apply to
visible prose within a passage, with Markdown code, HTML and math protected. Invalid config,
patterns and unknown rule IDs fail the scan. See the [complete configuration contract](review.md#review-configuration).

For a one-off exception, the existing `<!-- tics-ignore-next-line -->` and
`<!-- tics-ignore team-marketing -->` comments suppress the next line; the second form
suppresses just the named rule.

## Reading the comment

See a [generated example comment](github-action-example.md).

Each file has a linked name and match count. Counts reflect
the enabled rules and exceptions.

Open a file dropdown, then a passage dropdown, to read the original context, matched wording
and review guidance. Source links point to the scanned PR commit and line. Display limits
omit complete passage groups and state how many were omitted; the count still includes all
matches from the scan. The overview shows up to 50 files. Larger job-summary and smaller
PR-comment limits can produce different numbers of displayed details.

## Runtime and permissions

The Action requires Rust 1.88 or newer and Node.js 18 or newer. GitHub's hosted Ubuntu runner
provides both. It builds the Action's locked Rust source from `GITHUB_ACTION_PATH`, then scans
the PR head checkout as data. It uses `github.token`; posting or updating comments requires
`pull-requests: write` or `issues: write`. A read-only fork token retains the report in the job
summary when commenting is denied.

Findings are advisory and the Action succeeds when they are found. Usage, config, scan and
unexpected API errors fail the Action. Removed files are skipped; symlink paths are rejected.
The complete JSON scan output is limited to 16 MiB; larger reports fail the scan.
The PR files API is capped at 3,000 files, and the Action fails at that threshold to expose an
incomplete listing. Use the `pull_request` event as shown above. The Action verifies that the
checkout and current PR head agree before scanning.

## Using the report in later steps

The Action exposes `report-path`, `candidates`, and `files-needing-review` outputs. The
path points to the complete `tics.review.v1` JSON report in a temporary directory on the
same runner; unlike the comment and summary, it is not truncated for display. The report
contains source text for files with candidates, so handle it as repository content. See the
[machine contract](review.md#machine-contract) for its fields and coordinates.

Give the Action an ID and use its outputs in an adjacent step, for example:

```yaml
- uses: finngi/tics@v0.3.0
  id: tics
- name: Process candidates
  if: steps.tics.outputs.candidates != '0'
  env:
    TICS_REPORT: ${{ steps.tics.outputs.report-path }}
  run: |
    echo "Review report: $TICS_REPORT"
    # Pass "$TICS_REPORT" to your review agent or other consumer here.
```

The temporary path is available only while the job's runner exists. Upload the report as an
artifact if a later job needs it, subject to your repository's data-handling practices.
