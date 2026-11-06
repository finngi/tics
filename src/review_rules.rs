//! Additional review candidates, deliberately excluded from legacy authorship scoring.
//!
//! Corpus rationale: span-lab/setup/lexical-0.3-research/README.md in tics-corpus.
//! Guided tours include legitimate teaching and invitations: matches require adjudication.

use crate::rules::{Category, Detector, Rule, Severity};

pub(crate) static REVIEW_RULES: &[Rule] = &[Rule {
    id: "guided-tour-variants",
    category: Category::Rhetoric,
    severity: Severity::Info,
    since: "0.3.0",
    source: "https://github.com/finngi/tics/blob/main/DESIGN.md",
    description: "guided-tour framing; check whether it adds useful orientation",
    hint: "Retain useful instructions, invitations, teaching scaffolding and quoted speech. Shorten only redundant narration about the writing itself; preserve substantive information.",
    detector: Detector::Text {
        find: r"\blet(?:['’]s|\s+us)\s+(?:
            take\s+(?:a\s+)?(?:closer\s+look|moment)
          | explore | examine | embark | break\s+down
          | dive\s+deep\b | look\s+at
        )\b",
        reject: None,
        check: None,
    },
}];

#[cfg(test)]
mod tests {
    use super::REVIEW_RULES;
    use crate::{document::Mode, engine};

    fn scan(text: &str, mode: Mode) -> engine::FileReport {
        let selected = REVIEW_RULES.iter().collect::<Vec<_>>();
        let compiled = engine::compile(&selected).expect("review rule compiles");
        engine::scan("fixture.md", text, mode, &compiled)
    }

    #[test]
    fn guided_tour_variants_preserve_exact_source_spans() {
        for cue in [
            "Let's take a closer look",
            "Let’s take a moment",
            "Let us explore",
            "LET US EXAMINE",
            "Let's embark",
            "Let’s break down",
            "Let's dive deep",
            "Let us look at",
        ] {
            let source = format!("é — {cue} the example.");
            let report = scan(&source, Mode::Plain);
            assert_eq!(report.findings.len(), 1, "{cue}");
            let finding = &report.findings[0];
            assert_eq!(finding.rule, "guided-tour-variants");
            assert_eq!(&source[finding.span.clone()], cue);
        }
    }

    #[test]
    fn code_and_unrelated_word_forms_do_not_trigger() {
        let source = "`Let's explore`\n\n```text\nLet us examine this.\n```\n\nShe explores the map. Let us explorer. Let's breakdown. Let's look atlas.";
        assert_eq!(scan(source, Mode::Markdown).findings, []);
    }

    #[test]
    fn useful_teaching_and_invitations_remain_conditional_candidates() {
        for source in [
            "Let's examine the specimen under the microscope.",
            "Let us explore the park together.",
        ] {
            assert_eq!(scan(source, Mode::Plain).findings.len(), 1);
        }
        assert!(REVIEW_RULES[0].hint.contains("Retain useful instructions"));
        assert!(
            REVIEW_RULES[0]
                .hint
                .contains("Shorten only redundant narration")
        );
    }
}
