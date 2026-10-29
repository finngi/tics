//! End-to-end tests: run the built `tics` binary on real files and stdin.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result};
use serde_json::Value;

fn tics(args: &[&str]) -> Result<Output> {
    tics_with_stdin(args, "")
}

fn tics_with_stdin(args: &[&str], stdin: &str) -> Result<Output> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tics"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning tics")?;
    child
        .stdin
        .take()
        .context("stdin pipe")?
        .write_all(stdin.as_bytes())?;
    child.wait_with_output().context("waiting for tics")
}

fn stdout(output: &Output) -> Result<String> {
    String::from_utf8(output.stdout.clone()).context("stdout is UTF-8")
}

#[test]
fn sample_file_reports_exact_positions_and_exits_one() -> Result<()> {
    let output = tics(&["tests/fixtures/tics.md"])?;
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output)?;
    for expected in [
        "tests/fixtures/tics.md:5:3: title-case-heading: possible AI-writing tic: heading in Title Case «Getting Started With The Platform»",
        "tests/fixtures/tics.md:7:58: not-x-but-y: ",
        "tests/fixtures/tics.md:10:3: bold-lead-in: ",
        "tests/fixtures/tics.md:10:39: em-dash: ",
        "tests/fixtures/tics.md:11:48: x-not-y: possible AI-writing tic: appositive contrast (`X, not Y`) «, not inside the VM»",
        "tests/fixtures/tics.md:13:1: not-x-its-y-split: ",
        "tests/fixtures/tics.md:21:58: leaked-markup: ",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    // Front matter (line 2), the code block (lines 16-19), inline code and the suppressed
    // line 23 produce nothing.
    for line in ["2", "16", "17", "18", "19", "23"] {
        let prefix = format!("tests/fixtures/tics.md:{line}:");
        assert!(
            !text.contains(&prefix),
            "unexpected finding on line {line}:\n{text}"
        );
    }
    assert!(
        !text.contains(":21:5: ai-vocabulary"),
        "inline code was checked:\n{text}"
    );
    Ok(())
}

#[test]
fn clean_file_prints_nothing_and_exits_zero() -> Result<()> {
    let output = tics(&["tests/fixtures/clean.md"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output)?, "");
    Ok(())
}

#[test]
fn plain_text_files_are_not_parsed_as_markdown() -> Result<()> {
    let text = stdout(&tics(&["tests/fixtures/notes.txt"])?)?;
    assert!(
        text.contains("tests/fixtures/notes.txt:3:40: em-dash: "),
        "{text}"
    );
    Ok(())
}

#[test]
fn json_output_is_an_array_of_finding_objects() -> Result<()> {
    let output = tics(&["--format", "json", "tests/fixtures/tics.md"])?;
    assert_eq!(output.status.code(), Some(1));
    let findings: Value = serde_json::from_str(&stdout(&output)?)?;
    let findings = findings.as_array().context("an array")?;
    let first = findings.first().context("at least one finding")?;
    let mut keys: Vec<&str> = first
        .as_object()
        .context("an object")?
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "category",
            "column",
            "confidence",
            "line",
            "matched",
            "message",
            "path",
            "rule",
            "severity"
        ]
    );
    assert_eq!(first["path"], "tests/fixtures/tics.md");
    assert_eq!(first["line"], 5);
    assert_eq!(first["column"], 3);
    assert_eq!(first["severity"], "info");
    Ok(())
}

#[test]
fn json_summary_wraps_findings_with_counts_and_density() -> Result<()> {
    let output = tics(&["--format", "json", "--summary", "tests/fixtures/tics.md"])?;
    let report: Value = serde_json::from_str(&stdout(&output)?)?;
    let total = report["findings"].as_array().context("findings")?.len();
    assert_eq!(report["summary"]["total"], total);
    assert!(report["summary"]["words"].as_u64().unwrap_or(0) > 50);
    assert!(report["summary"]["per_1000_words"].as_f64().unwrap_or(0.0) > 0.0);
    assert_eq!(report["summary"]["rules"]["ai-vocabulary"], 3);
    Ok(())
}

#[test]
fn human_summary_reports_density() -> Result<()> {
    let text = stdout(&tics(&["--summary", "tests/fixtures/tics.md"])?)?;
    assert!(
        text.lines()
            .any(|line| line.starts_with("summary: ") && line.ends_with(" per 1,000 words)")),
        "{text}"
    );
    Ok(())
}

#[test]
fn stdin_is_read_for_dash_and_for_no_paths() -> Result<()> {
    for args in [&["-"][..], &[][..]] {
        let output = tics_with_stdin(args, "Let's dive in.\n")?;
        assert_eq!(output.status.code(), Some(1));
        assert!(stdout(&output)?.starts_with("<stdin>:1:1: signposting: "));
    }
    Ok(())
}

#[test]
fn select_ignore_and_severity_filter_findings() -> Result<()> {
    let only = stdout(&tics(&["--select", "em-dash", "tests/fixtures/tics.md"])?)?;
    assert!(
        only.lines().all(|line| line.contains(": em-dash: ")),
        "{only}"
    );
    let ignored = stdout(&tics(&["--ignore", "em-dash", "tests/fixtures/tics.md"])?)?;
    assert!(!ignored.contains(": em-dash: "));
    let warn = stdout(&tics(&["--severity", "warn", "tests/fixtures/tics.md"])?)?;
    assert!(!warn.contains("title-case-heading") && !warn.contains("rule-of-three"));
    Ok(())
}

#[test]
fn usage_and_io_errors_exit_two() -> Result<()> {
    for args in [
        &["tests/fixtures/missing.md"][..],
        &["--select", "no-such-rule", "tests/fixtures/clean.md"][..],
        &["--format", "xml"][..],
    ] {
        let output = tics(args)?;
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
    Ok(())
}

#[test]
fn version_comes_from_cargo_toml() -> Result<()> {
    let output = tics(&["--version"])?;
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output)?,
        format!("tics {}\n", env!("CARGO_PKG_VERSION"))
    );
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// An empty directory under Cargo's per-test temp dir, recreated on each run.
fn fresh_dir(name: &str) -> Result<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn directories_are_walked_in_order_skipping_hidden_target_and_node_modules() -> Result<()> {
    let root = fresh_dir("walk")?;
    for name in [
        "b/two.markdown",
        "a.md",
        "c.mdx",
        "d.txt",
        "skip.rs",
        ".hidden/x.md",
        "target/x.md",
        "node_modules/x.md",
    ] {
        write(&root.join(name), "We delve.\n")?;
    }
    let root_arg = root.to_str().context("UTF-8 temp dir")?;
    let text = stdout(&tics(&["--select", "ai-vocabulary", root_arg])?)?;
    let files: Vec<&str> = text
        .lines()
        .filter_map(|line| line.split(':').next())
        .filter_map(|path| path.strip_prefix(root_arg))
        .collect();
    assert_eq!(files, ["/a.md", "/b/two.markdown", "/c.mdx", "/d.txt"]);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_directories_are_not_walked() -> Result<()> {
    let root = fresh_dir("symlink-loop")?;
    write(&root.join("a.md"), "We delve.\n")?;
    std::os::unix::fs::symlink(&root, root.join("loop"))?;
    let output = tics(&["--select", "ai-vocabulary", root.to_str().context("UTF-8")?])?;
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output)?.lines().count(), 1);
    Ok(())
}

#[test]
fn a_later_unreadable_file_leaves_no_partial_output() -> Result<()> {
    let root = fresh_dir("partial")?;
    write(&root.join("good.md"), "We delve.\n")?;
    std::fs::write(root.join("bad.md"), [0xff])?;
    let good = root.join("good.md");
    let bad = root.join("bad.md");
    let output = tics(&[
        "--format",
        "json",
        good.to_str().context("UTF-8")?,
        bad.to_str().context("UTF-8")?,
    ])?;
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output)?, "");
    Ok(())
}

#[test]
fn a_byte_order_mark_does_not_hide_front_matter_or_shift_columns() -> Result<()> {
    let output = tics_with_stdin(
        &[],
        "\u{feff}---\ntitle: robust\n---\nUse tabs, not spaces.\n",
    )?;
    assert_eq!(
        stdout(&output)?,
        "<stdin>:4:9: x-not-y: possible AI-writing tic: appositive contrast (`X, not Y`) «, not spaces» (confidence 0.83)\n"
    );
    Ok(())
}

#[test]
fn rules_lists_every_rule_in_both_formats() -> Result<()> {
    let human = stdout(&tics(&["rules"])?)?;
    let rules: Value = serde_json::from_str(&stdout(&tics(&["rules", "--format", "json"])?)?)?;
    let rules = rules.as_array().context("an array")?;
    assert_eq!(human.lines().count(), rules.len());
    for rule in rules {
        let id = rule["id"].as_str().context("id")?;
        assert!(human.lines().any(|line| line.starts_with(id)), "{id}");
        for field in [
            "category",
            "severity",
            "since",
            "unit",
            "source",
            "description",
        ] {
            assert!(rule[field].is_string(), "{id} lacks {field}");
        }
        assert!(rule["strength"].is_f64(), "{id} lacks strength");
    }
    Ok(())
}

/// The README rule table must match `tics rules` exactly, row for row.
#[test]
fn readme_rule_table_matches_tics_rules() -> Result<()> {
    let rules: Value = serde_json::from_str(&stdout(&tics(&["rules", "--format", "json"])?)?)?;
    let expected: Vec<String> = rules
        .as_array()
        .context("an array")?
        .iter()
        .map(|rule| {
            let field = |name: &str| rule[name].as_str().unwrap_or_default().to_owned();
            let source = field("source");
            let label = source
                .rsplit("Wikipedia:")
                .next()
                .unwrap_or_default()
                .replace('_', " ");
            format!(
                "| `{}` | {} | {} | {} | {:+.2} | {} | [{label}]({source}) |",
                field("id"),
                field("category"),
                field("severity"),
                field("since"),
                rule["strength"].as_f64().unwrap_or_default(),
                field("description"),
            )
        })
        .collect();
    let readme = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))?;
    let table = readme
        .split("<!-- rules:start -->")
        .nth(1)
        .and_then(|rest| rest.split("<!-- rules:end -->").next())
        .context("README has rules:start/end markers")?;
    let rows: Vec<&str> = table
        .lines()
        .filter(|line| line.starts_with("| `"))
        .collect();
    assert_eq!(rows, expected);
    Ok(())
}

#[test]
fn min_confidence_hides_weak_findings_and_sets_the_exit_code() -> Result<()> {
    let weak = "One sentence uses tabs, not spaces, in a long and otherwise plain paragraph about \
                build settings, compiler flags, cache directories, network timeouts, retry \
                limits, log rotation, file permissions and the order in which services start.\n";
    let all = tics_with_stdin(&[], weak)?;
    assert_eq!(all.status.code(), Some(1));
    let strict = tics_with_stdin(&["--min-confidence", "0.9"], weak)?;
    assert_eq!(strict.status.code(), Some(0));
    assert_eq!(stdout(&strict)?, "");
    let bad = tics_with_stdin(&["--min-confidence", "2"], weak)?;
    assert_eq!(bad.status.code(), Some(2));
    Ok(())
}

#[test]
fn score_rates_each_file_and_exits_zero() -> Result<()> {
    let output = tics(&["score", "tests/fixtures/tics.md", "tests/fixtures/clean.md"])?;
    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output)?;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].starts_with("tests/fixtures/tics.md: 0.97 (21 findings in 86 words)"),
        "{text}"
    );
    assert!(
        lines[1].starts_with("tests/fixtures/clean.md: 0.43 (0 findings"),
        "{text}"
    );
    let json: serde_json::Value = serde_json::from_str(&stdout(&tics(&[
        "score",
        "--format",
        "json",
        "tests/fixtures/tics.md",
    ])?)?)?;
    assert!(json[0]["score"].as_f64().is_some_and(|score| score > 0.9));
    assert_eq!(json[0]["units"]["list_items"], 2);
    assert_eq!(
        json[0]["sections"][0]["title"],
        "Getting Started With The Platform"
    );
    let sections = stdout(&tics(&["score", "--sections", "tests/fixtures/tics.md"])?)?;
    assert!(
        sections.contains("\n  5: 0.97 Getting Started With The Platform (21 findings)\n"),
        "{sections}"
    );
    Ok(())
}

#[test]
fn hidden_or_proseless_text_gets_no_findings_and_no_score() -> Result<()> {
    for input in [
        "<!--\n**Why:** because\nShort answer: yes\n-->\n",
        "",
        "```\ncode only\n```\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n",
    ] {
        let findings = tics_with_stdin(&[], input)?;
        assert_eq!(findings.status.code(), Some(0), "{input:?}");
        let score = stdout(&tics_with_stdin(&["score"], input)?)?;
        assert!(score.starts_with("<stdin>: n/a"), "{input:?}: {score}");
    }
    Ok(())
}

#[test]
fn wrapping_a_paragraph_does_not_change_its_score() -> Result<()> {
    let sentences: Vec<String> = (0..40)
        .map(|n| format!("The service restarts after config change number {n}."))
        .collect();
    let one_line = stdout(&tics_with_stdin(&["score"], &(sentences.join(" ") + "\n"))?)?;
    let wrapped = stdout(&tics_with_stdin(
        &["score"],
        &(sentences.join("\n") + "\n"),
    )?)?;
    assert_eq!(one_line, wrapped);
    Ok(())
}

#[test]
fn several_hits_in_one_sentence_occupy_it_once() -> Result<()> {
    let text = "It works — mostly — fast — always. The cache is warm. The tests pass.\n";
    let json: Value = serde_json::from_str(&stdout(&tics_with_stdin(
        &["score", "--format", "json"],
        text,
    )?)?)?;
    assert_eq!(json[0]["rules"]["em-dash"], 3);
    assert_eq!(json[0]["occupied"]["em-dash"], 1);
    assert_eq!(json[0]["units"]["sentences"], 3);
    Ok(())
}

#[test]
fn readme_confidence_example_matches() -> Result<()> {
    let output = tics(&[
        "score",
        "tests/fixtures/notes.md",
        "tests/fixtures/answer.md",
    ])?;
    let text = stdout(&output)?;
    let readme = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))?;
    for line in text.lines() {
        assert!(readme.contains(line), "README lacks {line:?}");
    }
    Ok(())
}
