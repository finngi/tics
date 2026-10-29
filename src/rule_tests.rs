//! Example-based tests: every rule has at least two sentences it must flag and two realistic
//! pieces of technical prose it must leave alone.

use anyhow::Result;

use crate::document::Mode;
use crate::engine::{Compiled, compile, scan};
use crate::rules::RULES;

struct Case {
    rule: &'static str,
    flags: &'static [&'static str],
    passes: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        rule: "negative-parallelism",
        flags: &[
            "It is not only fast but also safe.",
            "The tool isn't just a linter, it is a habit.",
            "It's not merely a cache; it is the source of truth.",
        ],
        passes: &[
            "Do not just restart the daemon; check the logs first.",
            "The runtime is not just-in-time compiled.",
            "The cache is used not only for builds.",
        ],
    },
    Case {
        rule: "not-x-but-y",
        flags: &[
            "It's not a bug, but a feature.",
            "This is not a linter — it's a guide.",
            "The tool isn't a replacement; it's a companion.",
        ],
        passes: &[
            "This is not supported on Windows, but you can run it under WSL.",
            "The value is not cached, but it is recomputed on each call.",
            "The file is not found; it is created on first run.",
        ],
    },
    Case {
        rule: "not-x-its-y-split",
        flags: &[
            "It isn't a linter. It's a mirror.",
            "This is not about speed. It's about trust.",
        ],
        passes: &[
            "It is not installed by default. It is available from crates.io.",
            "This does not block. It returns a future.",
        ],
    },
    Case {
        rule: "x-not-y",
        flags: &[
            "Policies are enforced on your Mac, not inside the VM.",
            "Use tabs, not spaces.",
            "The limit applies per user — not per project.",
            "Host services are reachable at `host.microsandbox.internal`, not only Ollama.",
            "Any service listening on your host is reachable this way, not only model servers.",
            "It needs a short reviewed list, not every config key.",
            "Should a placeholder sent to a non-GitHub host stop the sandbox, not just block the request?",
            "Refs are writable, so an agent can move any branch, not only its own.",
        ],
        passes: &[
            "If the file exists, not much happens.",
            "Most flags are stable, though not the plugin API.",
            "Whether or not the flag is set, the build runs.",
            "The cache is shared, not only for builds but also for tests.",
            "It retries once, not just to be safe but because the API is flaky.",
        ],
    },
    Case {
        rule: "does-x-does-not-y",
        flags: &[
            "It runs locally. It does not phone home.",
            "The parser reads UTF-8. The parser doesn't read UTF-16.",
        ],
        passes: &[
            "The parser reads UTF-8. The lexer does not allocate.",
            "The cache is not shared. The cache is not persisted.",
        ],
    },
    Case {
        rule: "rather-than",
        flags: &[
            "Use a lockfile rather than pinning by hand.",
            "Prefer `Result` instead of panicking.",
        ],
        passes: &[
            "Use a lockfile instead.",
            "The rather long build step is cached.",
        ],
    },
    Case {
        rule: "not-sentence-start",
        flags: &[
            "Not a toy. A real tool.",
            "It ships today. Not because it is done, but because it is useful.",
        ],
        passes: &[
            "Note the trailing slash in the URL.",
            "Nothing is written to disk.",
        ],
    },
    Case {
        rule: "trailing-ing-analysis",
        flags: &[
            "The release added caching, highlighting the team's focus on speed.",
            "Downloads doubled, reflecting growing interest in local tools.",
        ],
        passes: &[
            "Syntax highlighting is enabled by default.",
            "After reflecting on the trade-offs, we chose SQLite.",
        ],
    },
    Case {
        rule: "significance-puffing",
        flags: &[
            "The scheduler plays a pivotal role in the system.",
            "The project is a testament to open collaboration.",
            "This release sets the stage for plugins.",
        ],
        passes: &[
            "The IAM role is crucial for access to the bucket.",
            "Set a reminder in the scheduler before the cut-off.",
        ],
    },
    Case {
        rule: "importance-hedge",
        flags: &[
            "It's worth noting that the API is unstable.",
            "It should be noted that tests run in parallel.",
        ],
        passes: &[
            "It is important to run migrations before deploying.",
            "The extra second of build time is worth it.",
        ],
    },
    Case {
        rule: "vague-attribution",
        flags: &[
            "Experts argue that memory safety matters.",
            "Some critics say the format is too loose.",
        ],
        passes: &[
            "Some users report that the CLI is slow on network drives.",
            "The critic model says nothing about latency.",
        ],
    },
    Case {
        rule: "summary-opener",
        flags: &[
            "In summary, the change is safe.",
            "The tests pass. Overall, the branch is ready.",
        ],
        passes: &[
            "Overall latency dropped by 30%.",
            "In short-lived processes, the cache is never warm.",
        ],
    },
    Case {
        rule: "signposting",
        flags: &[
            "Here's the thing: caching is hard.",
            "Let's dive in and look at the parser.",
            "In this section, we will configure the runner.",
        ],
        passes: &[
            "Let's run the tests again with logging enabled.",
            "This section covers installation on Linux.",
        ],
    },
    Case {
        rule: "rhetorical-reveal",
        flags: &[
            "The result? A build that takes seconds.",
            "We tried it. The catch? It only works on Linux.",
        ],
        passes: &[
            "What is the result? Check the exit code.",
            "The result is cached for an hour.",
        ],
    },
    Case {
        rule: "whether-youre",
        flags: &[
            "Whether you're a beginner or an expert, this guide helps.",
            "Whether you are on Linux or macOS, install it with cargo.",
        ],
        passes: &[
            "Check whether you're on the main branch or a fork.",
            "It works whether or not you are online.",
        ],
    },
    Case {
        rule: "todays-world",
        flags: &[
            "In today's fast-paced world, builds must be quick.",
            "In today's digital landscape, privacy matters.",
        ],
        passes: &[
            "In today's release we fixed two crashes.",
            "Today's nightly build is green.",
        ],
    },
    Case {
        rule: "transition-opener",
        flags: &[
            "Additionally, the CLI supports JSON.",
            "It is fast. Moreover, it is small.",
        ],
        passes: &[
            "Additional flags are listed below.",
            "It is notably faster on ARM.",
        ],
    },
    Case {
        rule: "rule-of-three",
        flags: &[
            "The new API is fast, reliable, and secure.",
            "We value accountability, inclusivity and openness.",
        ],
        passes: &[
            "Install git, curl, and make before building.",
            "The function takes a path, a mode, and a callback.",
            "It supports Linux, macOS, and Windows.",
            "You need macOS on Apple Silicon or Linux with KVM, Homebrew, and Rust (1.88 or newer).",
            "The dashboard attaches, starts, stops and removes sandboxes.",
        ],
    },
    Case {
        rule: "ai-vocabulary",
        flags: &[
            "We delve into the rich tapestry of options.",
            "This robust tool lets you leverage your data.",
            "The result underscores the need for tests.",
        ],
        passes: &[
            "Rotate the device to landscape mode.",
            "The guard unlocks the mutex when it is dropped.",
            "Use an underscore to separate words.",
            "The installer requires elevated privileges.",
        ],
    },
    Case {
        rule: "promotional",
        flags: &[
            "The editor boasts a plugin system.",
            "A cutting-edge, best-in-class runtime.",
        ],
        passes: &[
            "The World class owns all entities.",
            "Edge cases are covered by the test suite.",
        ],
    },
    Case {
        rule: "copula-avoidance",
        flags: &[
            "The file serves as the single source of truth.",
            "The daemon functions as a proxy.",
        ],
        passes: &[
            "The function as a whole is pure.",
            "This port serves HTTP traffic.",
        ],
    },
    Case {
        rule: "canned-assurance",
        flags: &[
            "This ensures that every build is reproducible.",
            "The tool streamlines your workflow.",
        ],
        passes: &[
            "Ensure the port is free before starting.",
            "The compliance report lists violations.",
        ],
    },
    Case {
        rule: "chatbot-residue",
        flags: &[
            "Certainly! Here is the config.",
            "I hope this helps, and let me know if you need more.",
            "As an AI language model, I cannot run code.",
        ],
        passes: &[
            "Of course, the cache must be warm first.",
            "It ships as an AIX package.",
        ],
    },
    Case {
        rule: "placeholder",
        flags: &[
            "Contact [Insert name here] for access.",
            "Released on YYYY-XX-XX by INSERT_AUTHOR.",
        ],
        passes: &[
            "See [Your first sandbox](docs/first.md) for a walkthrough.",
            "Dates use the YYYY-MM-DD format.",
            "SQL INSERT statements are batched.",
        ],
    },
    Case {
        rule: "leaked-markup",
        flags: &[
            "Read the [guide](https://example.com/?utm_source=chatgpt.com).",
            "Rust is fast. :contentReference[oaicite:0]{index=0}",
        ],
        passes: &[
            "Read the [guide](https://example.com/?utm_source=newsletter).",
            "Search logs for `utm_source=chatgpt.com` to find leaks.",
        ],
    },
    Case {
        rule: "em-dash",
        flags: &["Fast — and small.", "It works—mostly."],
        passes: &["Pages 1–5 cover setup.", "Run `a — b` to compare files."],
    },
    Case {
        rule: "bold-lead-in",
        flags: &[
            "- **Speed:** builds in seconds.",
            "1. **Setup**: install the toolchain.",
        ],
        passes: &[
            "- **Note** the trailing slash.",
            "- Run `cargo test`: it must pass.",
        ],
    },
    Case {
        rule: "title-case-heading",
        flags: &[
            "# Getting Started With Docker Compose",
            "## Why This Approach Matters",
            "## Impact Of Technology And Digitalization",
        ],
        passes: &[
            "# Getting started with the CLI",
            "## Install from source",
            "# Reach Ollama or LM Studio on your Mac",
            "## Decisions so far",
            "## Install",
            "## Licence",
        ],
    },
];

fn rules_hit(compiled: &[Compiled], text: &str) -> Vec<&'static str> {
    scan("case.md", text, Mode::Markdown, compiled)
        .findings
        .into_iter()
        .map(|finding| finding.rule)
        .collect()
}

#[test]
fn every_rule_has_two_positive_and_two_negative_examples() {
    for rule in RULES {
        let case = CASES.iter().find(|case| case.rule == rule.id);
        assert!(
            case.is_some_and(|case| case.flags.len() >= 2 && case.passes.len() >= 2),
            "{} needs at least two flagged and two passing examples",
            rule.id
        );
    }
    assert_eq!(CASES.len(), RULES.len(), "a case names an unknown rule");
}

#[test]
fn every_rule_flags_its_examples_and_passes_its_counter_examples() -> Result<()> {
    let all: Vec<_> = RULES.iter().collect();
    let compiled = compile(&all)?;
    let mut failures = Vec::new();
    for case in CASES {
        for text in case.flags {
            if !rules_hit(&compiled, text).contains(&case.rule) {
                failures.push(format!("{} missed: {text}", case.rule));
            }
        }
        for text in case.passes {
            if rules_hit(&compiled, text).contains(&case.rule) {
                failures.push(format!("{} false positive: {text}", case.rule));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}
