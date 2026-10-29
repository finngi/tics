//! Scanning 1 MiB of Markdown must take well under a second (SPEC, "Performance").
//!
//! The one-second bound is asserted for release builds only
//! (`cargo test --release --test performance`); debug builds get a generous bound so CI on a
//! slow runner still catches a pathological regression without flaking.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

const MIB: usize = 1_048_576;

const MARKDOWN: &str = "## Request handling\n\n\
The client sends a header and a body. The server checks the length, not the type, before \
reading the buffer. It is not only fast but also robust \u{2014} it streams rather than \
buffering.\n\n\
- **Retries:** after the timer expires, see [the protocol](https://example.com/robust).\n\
- Use `delve()` in tests, not in production.\n\n\
| Field | Meaning |\n|---|---|\n| `len` | Body length, not header length |\n\n\
```rust\nlet value = 42; // robust \u{2014} not checked\n```\n\n";

fn bound() -> Duration {
    if cfg!(debug_assertions) {
        Duration::from_secs(20)
    } else {
        Duration::from_secs(1)
    }
}

fn exactly_one_mib(unit: &str) -> String {
    let mut text = unit.repeat(MIB / unit.len() + 1);
    let mut end = MIB;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

#[test]
fn one_mebibyte_scans_within_the_bound() -> Result<()> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("performance");
    std::fs::create_dir_all(&dir)?;
    for (name, unit) in [
        ("structured.md", MARKDOWN),
        ("one-long-line.md", "bytes robust, not slow "),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, exactly_one_mib(unit))?;
        let start = Instant::now();
        let status = Command::new(env!("CARGO_BIN_EXE_tics"))
            .arg(&path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("running tics")?;
        let elapsed = start.elapsed();
        assert_eq!(status.code(), Some(1), "{name}");
        assert!(
            elapsed < bound(),
            "{name}: 1 MiB took {elapsed:?} (bound {:?})",
            bound()
        );
    }
    Ok(())
}
