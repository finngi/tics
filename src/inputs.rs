//! Resolves command-line paths into the files to scan, in a deterministic order.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::document::Mode;

const EXTENSIONS: &[&str] = &["md", "markdown", "mdx", "txt"];
const SKIPPED_DIRS: &[&str] = &["target", "node_modules"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Input {
    Stdin,
    File(PathBuf),
}

/// Expands `paths`: `-` or no paths is stdin, directories are walked recursively (sorted,
/// skipping hidden directories, `target/` and `node_modules/`), files are taken as given.
pub(crate) fn collect(paths: &[PathBuf]) -> Result<Vec<Input>> {
    if paths.is_empty() {
        return Ok(vec![Input::Stdin]);
    }
    let mut inputs = Vec::new();
    for path in paths {
        if path.as_os_str() == "-" {
            inputs.push(Input::Stdin);
            continue;
        }
        let metadata =
            std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
        if metadata.is_dir() {
            let mut files = Vec::new();
            walk(path, &mut files)?;
            inputs.extend(files.into_iter().map(Input::File));
        } else {
            inputs.push(Input::File(path.clone()));
        }
    }
    Ok(inputs)
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading directory {}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("reading directory {}", dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // `file_type` does not follow symlinks, so symlinked directories are not walked
        // (no cycles); a symlink to a file is followed by `metadata`.
        let file_type = entry
            .file_type()
            .with_context(|| format!("reading {}", path.display()))?;
        if file_type.is_dir() {
            if !name.starts_with('.') && !SKIPPED_DIRS.contains(&name.as_ref()) {
                walk(&path, files)?;
            }
        } else if has_prose_extension(&path) && path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn has_prose_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
}

/// `.txt` is plain text; everything else (including stdin) is Markdown.
pub(crate) fn mode_of(input: &Input) -> Mode {
    match input {
        Input::File(path)
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("txt")) =>
        {
            Mode::Plain
        }
        _ => Mode::Markdown,
    }
}

/// The display name and UTF-8 contents of an input.
pub(crate) fn read(input: &Input) -> Result<(String, String)> {
    match input {
        Input::Stdin => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .context("reading stdin as UTF-8")?;
            Ok(("<stdin>".to_owned(), without_bom(text)))
        }
        Input::File(path) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading {} as UTF-8", path.display()))?;
            Ok((path.display().to_string(), without_bom(text)))
        }
    }
}

/// Drops a leading UTF-8 byte order mark, so front matter at the start is recognised and
/// first-line columns match what an editor shows.
fn without_bom(text: String) -> String {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::Result;

    use super::{Input, collect, has_prose_extension, mode_of, without_bom};
    use crate::document::Mode;

    #[test]
    fn no_paths_or_dash_means_stdin() -> Result<()> {
        assert_eq!(collect(&[])?, [Input::Stdin]);
        assert_eq!(collect(&[PathBuf::from("-")])?, [Input::Stdin]);
        Ok(())
    }

    #[test]
    fn missing_path_is_an_error() {
        assert!(collect(&[PathBuf::from("/definitely/not/here.md")]).is_err());
    }

    #[test]
    fn only_prose_extensions_are_walked() {
        for (name, expected) in [
            ("a.md", true),
            ("a.MARKDOWN", true),
            ("a.mdx", true),
            ("a.txt", true),
            ("a.rs", false),
            ("README", false),
        ] {
            assert_eq!(has_prose_extension(name.as_ref()), expected, "{name}");
        }
    }

    #[test]
    fn a_leading_byte_order_mark_is_dropped() {
        assert_eq!(without_bom("\u{feff}---\n".to_owned()), "---\n");
        assert_eq!(without_bom("a\u{feff}".to_owned()), "a\u{feff}");
    }

    #[test]
    fn txt_is_plain_and_everything_else_is_markdown() {
        assert_eq!(mode_of(&Input::File("a.txt".into())), Mode::Plain);
        assert_eq!(mode_of(&Input::File("a.md".into())), Mode::Markdown);
        assert_eq!(mode_of(&Input::Stdin), Mode::Markdown);
    }
}
