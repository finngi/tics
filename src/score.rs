//! Confidence scores: how strongly findings suggest AI-style writing, from counts in context.
//!
//! Each rule's hit count in a file is saturated and length-normalised as in BM25, so the tenth
//! bold lead-in adds less than the first and a long file needs more hits than a short one:
//!
//! ```text
//! x = count * (K1 + 1) / (count + K1 * (1 - B + B * words / AVG_WORDS))
//! ```
//!
//! A logistic model turns that evidence into a probability with equal prior odds of human and
//! AI text: `sigmoid(BIAS + weight * x)` for one rule's findings, and the sum over all rules for
//! a whole file. The constants are fitted by `calibration/fit.py` on the corpus described in
//! DESIGN.md ("Calibration"); a test keeps them equal to `calibration/weights.json`.

use std::collections::BTreeMap;

const K1: f64 = 2.0;
const B: f64 = 0.75;
const AVG_WORDS: f64 = 202.2;
const BIAS: f64 = -0.8958;

/// Weight of a rule the corpus could not estimate; also the fit's prior mean.
const PRIOR: f64 = 1.0;

/// Per-rule weights, in rule-table order. Zero means humans used the pattern as often.
static WEIGHTS: &[(&str, f64)] = &[
    ("negative-parallelism", 0.0969),
    ("not-x-but-y", 0.0000),
    ("not-x-its-y-split", 0.3227),
    ("x-not-y", 1.0354),
    ("does-x-does-not-y", 0.8865),
    ("rather-than", 0.5988),
    ("not-sentence-start", 1.0000),
    ("trailing-ing-analysis", 1.0000),
    ("significance-puffing", 1.0000),
    ("importance-hedge", 0.5232),
    ("vague-attribution", 0.5103),
    ("summary-opener", 0.9780),
    ("signposting", 1.0000),
    ("rhetorical-reveal", 1.0000),
    ("whether-youre", 1.0000),
    ("todays-world", 1.0000),
    ("transition-opener", 0.0000),
    ("rule-of-three", 0.5382),
    ("ai-vocabulary", 1.0538),
    ("promotional", 1.0153),
    ("copula-avoidance", 0.2347),
    ("canned-assurance", 0.7096),
    ("chatbot-residue", 1.0000),
    ("placeholder", 1.0000),
    ("leaked-markup", 1.0000),
    ("em-dash", 1.1197),
    ("bold-lead-in", 1.5356),
    ("bold-label-line", 1.2771),
    ("stock-section-label", 1.3222),
    ("title-case-heading", 0.2325),
];

fn weight(rule: &str) -> f64 {
    WEIGHTS
        .iter()
        .find(|(id, _)| *id == rule)
        .map_or(PRIOR, |(_, weight)| *weight)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "hit and word counts stay far below 2^52, where f64 is exact"
)]
fn saturate(count: usize, words: usize) -> f64 {
    let count = count as f64;
    let length = words.max(1) as f64 / AVG_WORDS;
    count * (K1 + 1.0) / (count + K1 * (1.0 - B + B * length))
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// Two decimals, so output is stable and easy to compare.
fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Confidence that `count` hits of `rule` in a file of `words` words come from AI-style text.
pub(crate) fn confidence(rule: &str, count: usize, words: usize) -> f64 {
    round(sigmoid(BIAS + weight(rule) * saturate(count, words)))
}

/// Confidence that a whole file is AI-style text, from its hit count per rule.
pub(crate) fn file_score(counts: &BTreeMap<&str, usize>, words: usize) -> f64 {
    let evidence: f64 = counts
        .iter()
        .map(|(rule, count)| weight(rule) * saturate(*count, words))
        .sum();
    round(sigmoid(BIAS + evidence))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::{Context, Result};

    use super::{AVG_WORDS, B, BIAS, K1, PRIOR, WEIGHTS, confidence, file_score};
    use crate::rules::RULES;

    #[test]
    fn every_rule_has_exactly_one_weight_in_table_order() {
        let ids: Vec<&str> = WEIGHTS.iter().map(|(id, _)| *id).collect();
        let rules: Vec<&str> = RULES.iter().map(|rule| rule.id).collect();
        assert_eq!(ids, rules);
    }

    #[test]
    fn constants_match_the_calibration_output() -> Result<()> {
        let fitted: serde_json::Value =
            serde_json::from_str(include_str!("../calibration/weights.json"))?;
        let number = |key: &str| {
            fitted[key]
                .as_f64()
                .with_context(|| format!("{key} missing"))
        };
        assert!((number("k1")? - K1).abs() < 1e-9);
        assert!((number("b")? - B).abs() < 1e-9);
        assert!((number("avg_words")? - AVG_WORDS).abs() < 1e-9);
        assert!((number("bias")? - BIAS).abs() < 1e-9);
        assert!((number("prior")? - PRIOR).abs() < 1e-9);
        for (id, weight) in WEIGHTS {
            let expected = fitted["weights"][*id]
                .as_f64()
                .with_context(|| format!("no fitted weight for {id}"))?;
            assert!((expected - weight).abs() < 1e-9, "{id}");
        }
        Ok(())
    }

    #[test]
    fn more_hits_raise_confidence_but_with_diminishing_returns() {
        let one = confidence("bold-lead-in", 1, 200);
        let three = confidence("bold-lead-in", 3, 200);
        let thirty = confidence("bold-lead-in", 30, 200);
        assert!(one < three && three < thirty);
        assert!(three - one > thirty - three, "{one} {three} {thirty}");
        assert!(thirty < 1.0);
    }

    #[test]
    fn the_same_hits_count_for_less_in_a_longer_file() {
        assert!(confidence("bold-lead-in", 2, 2000) < confidence("bold-lead-in", 2, 100));
    }

    #[test]
    fn rules_humans_use_as_often_add_nothing() {
        let baseline = file_score(&BTreeMap::new(), 200);
        assert!((confidence("not-x-but-y", 5, 200) - baseline).abs() < 1e-9);
        assert!(baseline < 0.5);
    }

    #[test]
    fn a_file_score_combines_rules() {
        let mut counts = BTreeMap::new();
        counts.insert("bold-lead-in", 4);
        let one_rule = file_score(&counts, 200);
        counts.insert("x-not-y", 2);
        assert!(file_score(&counts, 200) > one_rule);
    }
}
