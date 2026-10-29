//! A small, predictable sentence splitter for prose segments.
//!
//! A sentence ends at `.`, `!`, `?` or `…` (plus closing quotes or brackets) followed by
//! whitespace or the end of the text. A `.` does not end a sentence after a known
//! abbreviation (`e.g.`, `vs.`) or a single letter (`J. Smith`). A lower-case next word still
//! starts a sentence: technical prose often opens with `git`, `npm` or a product name.

use std::ops::Range;

const ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "eg", "ie", "etc", "vs", "cf", "approx", "al", "mr", "mrs", "ms", "dr", "st",
    "no", "fig", "inc", "ltd", "jr", "sr",
];

const CLOSERS: &[char] = &['"', '\'', '”', '’', ')', ']'];

/// Byte ranges of the sentences in `text`, without surrounding whitespace.
pub(crate) fn split(text: &str) -> Vec<Range<usize>> {
    let mut sentences = Vec::new();
    let mut start = None;
    let mut chars = text.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if start.is_none() && !character.is_whitespace() {
            start = Some(index);
        }
        if !matches!(character, '.' | '!' | '?' | '…') {
            continue;
        }
        let mut end = index + character.len_utf8();
        while let Some(&(next, closer)) = chars.peek() {
            if !CLOSERS.contains(&closer) {
                break;
            }
            end = next + closer.len_utf8();
            chars.next();
        }
        let rest = &text[end..];
        let at_boundary = rest.is_empty() || rest.starts_with(char::is_whitespace);
        if at_boundary
            && (character != '.' || ends_sentence(&text[..index]))
            && let Some(begin) = start.take()
        {
            sentences.push(begin..end);
        }
    }
    if let Some(begin) = start {
        sentences.push(begin..text.trim_end().len().max(begin));
    }
    sentences
}

fn ends_sentence(before: &str) -> bool {
    let word_start = before
        .char_indices()
        .rev()
        .find(|&(_, c)| !(c.is_alphanumeric() || c == '.'))
        .map_or(0, |(index, c)| index + c.len_utf8());
    let word = before[word_start..].to_lowercase();
    let single_letter = word.chars().count() == 1 && word.chars().all(char::is_alphabetic);
    !(ABBREVIATIONS.contains(&word.as_str()) || single_letter)
}

#[cfg(test)]
mod tests {
    use super::split;

    fn sentences(text: &str) -> Vec<&str> {
        split(text).into_iter().map(|range| &text[range]).collect()
    }

    #[test]
    fn splits_on_terminal_punctuation_followed_by_space() {
        assert_eq!(
            sentences("It runs. It stops!  Why? \"Done.\" Next"),
            ["It runs.", "It stops!", "Why?", "\"Done.\"", "Next"]
        );
    }

    #[test]
    fn keeps_abbreviations_initials_and_decimals_inside_a_sentence() {
        assert_eq!(
            sentences("Use a tool, e.g. ripgrep. J. Smith wrote v1.2 vs. v1.3 here."),
            [
                "Use a tool, e.g. ripgrep.",
                "J. Smith wrote v1.2 vs. v1.3 here."
            ]
        );
    }

    #[test]
    fn lower_case_words_start_sentences_unless_after_an_abbreviation() {
        assert_eq!(
            sentences("See approx. ten files."),
            ["See approx. ten files."]
        );
        assert_eq!(
            sentences("It reads the token. microsandbox stores it."),
            ["It reads the token.", "microsandbox stores it."]
        );
    }

    #[test]
    fn whitespace_only_text_has_no_sentences() {
        assert_eq!(sentences("   \n "), Vec::<&str>::new());
    }
}
