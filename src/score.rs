//! Confidence: how strongly a file's findings point to AI-styled text, and why.
//!
//! A language model settles on a pattern early (a list with bold labels, an `X, not Y`
//! cadence) and then keeps copying it from its own context, so hits within one file are not
//! independent. Each file is modelled as drawing its own rate for a rule, then hitting `k` of
//! its `n` opportunities (sentences, list items, headings or blocks, by rule), where `k` counts
//! opportunities holding at least one hit, so three em dashes in one sentence count once:
//!
//! ```text
//! rate ~ Beta(a_c, b_c)      k ~ Binomial(n, rate)      for class c in {human, ai}
//! ```
//!
//! The evidence for a rule is the log-likelihood ratio of `k` in `n` under the AI and human
//! fits. Each class is assumed to write like the other a fraction `EPS` of the time, which
//! caps any single rule's evidence near `ln(1 / EPS)`. A file's evidence is the sum over the
//! selected rules, absences included; `SCALE` and `BIAS` turn evidence into a probability with
//! even prior odds. The constants come from `calibration/fit.py` (DESIGN.md, "Calibration");
//! a test keeps them equal to `calibration/weights.json`.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::rules::Rule;

/// One opportunity for a rule to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Unit {
    Sentence,
    ListItem,
    Heading,
    Block,
}

/// How many opportunities of each kind a file or section has.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct Units {
    pub(crate) sentences: usize,
    pub(crate) list_items: usize,
    pub(crate) headings: usize,
    pub(crate) blocks: usize,
}

impl Units {
    /// Whether there is any prose to judge: a sentence, list item or block.
    pub(crate) fn has_prose(&self) -> bool {
        self.sentences + self.list_items + self.blocks > 0
    }

    pub(crate) fn of(&self, unit: Unit) -> usize {
        match unit {
            Unit::Sentence => self.sentences,
            Unit::ListItem => self.list_items,
            Unit::Heading => self.headings,
            Unit::Block => self.blocks,
        }
    }
}

/// Chance that a human text looks AI-written for one rule, and the reverse.
const EPS: f64 = 0.02;
const SCALE: f64 = 1.051;
const BIAS: f64 = 0.0487;

/// Per rule, in rule-table order: Beta `(a, b)` for human text, then for AI text.
#[expect(
    clippy::unreadable_literal,
    reason = "copied from calibration/weights.json, which a test compares digit for digit"
)]
static PARAMS: &[(&str, [f64; 4])] = &[
    (
        "negative-parallelism",
        [0.012671, 15.038655, 0.002492, 14.400408],
    ),
    ("not-x-but-y", [0.293637, 61.270475, 0.062932, 67.025079]),
    (
        "not-x-its-y-split",
        [0.004095, 12.352963, 0.001604, 11.9281],
    ),
    ("x-not-y", [0.143334, 39.974295, 0.918466, 55.528711]),
    (
        "does-x-does-not-y",
        [0.015476, 29.209327, 0.031937, 35.221665],
    ),
    ("rather-than", [0.948009, 72.551658, 2.187257, 118.260038]),
    (
        "not-sentence-start",
        [0.000182, 9.570529, 0.000316, 9.485807],
    ),
    (
        "trailing-ing-analysis",
        [0.000182, 9.570529, 0.000316, 9.485807],
    ),
    (
        "significance-puffing",
        [0.000182, 9.570529, 0.000316, 9.485807],
    ),
    (
        "importance-hedge",
        [0.014425, 17.86238, 0.002839, 17.162157],
    ),
    (
        "vague-attribution",
        [0.006497, 11.79227, 0.007458, 11.873047],
    ),
    ("summary-opener", [0.017271, 31.238467, 0.042432, 38.481902]),
    ("signposting", [0.000182, 9.570529, 0.000316, 9.485807]),
    (
        "rhetorical-reveal",
        [0.000182, 9.570529, 0.000316, 9.485807],
    ),
    ("whether-youre", [0.000182, 9.570529, 0.000316, 9.485807]),
    ("todays-world", [0.000182, 9.570529, 0.000316, 9.485807]),
    (
        "transition-opener",
        [0.064418, 27.445178, 0.00529, 26.336112],
    ),
    ("rule-of-three", [0.053635, 34.398743, 0.073761, 41.844711]),
    ("ai-vocabulary", [0.037433, 33.793214, 0.088355, 45.816301]),
    ("promotional", [0.000987, 11.162072, 0.003192, 11.580584]),
    ("copula-avoidance", [0.012204, 14.295111, 0.0024, 13.675021]),
    (
        "canned-assurance",
        [0.004795, 15.216582, 0.001872, 14.736687],
    ),
    ("chatbot-residue", [0.000182, 9.570529, 0.000316, 9.485807]),
    ("placeholder", [0.000182, 9.570529, 0.000316, 9.485807]),
    ("leaked-markup", [0.000324, 9.664048, 0.000539, 9.594178]),
    ("em-dash", [0.178111, 17.303693, 0.554349, 18.881532]),
    ("bold-lead-in", [0.021658, 0.765361, 0.123009, 0.291645]),
    ("bold-label-line", [0.082425, 7.133671, 0.376744, 3.516018]),
    (
        "stock-section-label",
        [0.030403, 32.223307, 0.860177, 43.356767],
    ),
    (
        "title-case-heading",
        [0.659205, 14.114717, 0.836257, 13.355319],
    ),
];

fn params(rule: &str) -> Option<[f64; 4]> {
    PARAMS
        .iter()
        .find(|(id, _)| *id == rule)
        .map(|(_, params)| *params)
}

/// `ln Γ(x)` for `x > 0`, by the Lanczos approximation (g = 7, 9 terms), accurate to about
/// 1e-15 relative error, which is far below what the fitted parameters can resolve.
fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const COEFFICIENTS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection keeps the series in its accurate range.
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut sum = COEFFICIENTS[0];
    for (index, coefficient) in COEFFICIENTS.iter().enumerate().skip(1) {
        #[expect(clippy::cast_precision_loss, reason = "index is at most 8")]
        let offset = index as f64;
        sum += coefficient / (x + offset);
    }
    let t = x + G + 0.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + sum.ln()
}

fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// `ln P(k of n | Beta(a, b))` without the binomial coefficient, which cancels in a ratio.
fn ln_likelihood(k: f64, n: f64, a: f64, b: f64) -> f64 {
    ln_beta(k + a, n - k + b) - ln_beta(a, b)
}

fn ln_add_exp(x: f64, y: f64) -> f64 {
    let high = x.max(y);
    high + ((x - high).exp() + (y - high).exp()).ln()
}

/// Log-likelihood ratio, AI over human, of `count` occupied opportunities of `rule` out of
/// `opportunities`, kept monotone in `count`: once a rule's hits point one way, more of them
/// never point less that way. The fitted beta-binomials alone can turn back at high counts
/// (the two classes differ in how bursty they are), which reads as "more tics, less AI" and
/// has too little data behind it to trust.
pub(crate) fn evidence(rule: &str, count: usize, opportunities: usize) -> f64 {
    let count = count.min(opportunities);
    let first = raw_evidence(rule, count, opportunities);
    if count <= 1 {
        return first;
    }
    let leans_ai = lean(rule) >= 0.0;
    (1..count).fold(first, |best, fewer| {
        let other = raw_evidence(rule, fewer, opportunities);
        if leans_ai {
            best.max(other)
        } else {
            best.min(other)
        }
    })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "hit and unit counts stay far below 2^52, where f64 is exact"
)]
fn raw_evidence(rule: &str, count: usize, opportunities: usize) -> f64 {
    let Some([human_a, human_b, ai_a, ai_b]) = params(rule) else {
        return 0.0;
    };
    if opportunities == 0 {
        return 0.0;
    }
    let n = opportunities as f64;
    let k = count.min(opportunities) as f64;
    let human = ln_likelihood(k, n, human_a, human_b);
    let ai = ln_likelihood(k, n, ai_a, ai_b);
    let (keep, swap) = ((1.0 - EPS).ln(), EPS.ln());
    ln_add_exp(keep + ai, swap + human) - ln_add_exp(keep + human, swap + ai)
}

/// Opportunities used to compare rules in `tics rules`.
const STRENGTH_OPPORTUNITIES: usize = 10;

/// How well a rule separates AI from human text: the signed Jeffreys divergence between the
/// two classes' hit-count distributions over ten opportunities, to two decimals. Zero means
/// the rule tells them apart no better than chance; negative means people use it more.
pub(crate) fn strength(rule: &str) -> f64 {
    round(lean(rule))
}

/// The unrounded signed divergence behind `strength`; its sign is the rule's direction.
fn lean(rule: &str) -> f64 {
    let Some([human_a, human_b, ai_a, ai_b]) = params(rule) else {
        return 0.0;
    };
    #[expect(clippy::cast_precision_loss, reason = "small constant")]
    let n = STRENGTH_OPPORTUNITIES as f64;
    let (mut divergence, mut direction) = (0.0, 0.0);
    for count in 0..=STRENGTH_OPPORTUNITIES {
        #[expect(clippy::cast_precision_loss, reason = "count is at most 10")]
        let k = count as f64;
        let choose = ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0);
        let human = (choose + ln_likelihood(k, n, human_a, human_b)).exp();
        let ai = (choose + ln_likelihood(k, n, ai_a, ai_b)).exp();
        let (ai, human) = (
            (1.0 - EPS) * ai + EPS * human,
            (1.0 - EPS) * human + EPS * ai,
        );
        divergence += (ai - human) * (ai.ln() - human.ln());
        direction += k * (ai - human);
    }
    divergence.copysign(direction)
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// Two decimals, so output is stable and easy to compare.
fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Confidence from this rule's hits alone that the file is AI-styled. Below 0.5 means the
/// pattern, at this rate, is more typical of people.
pub(crate) fn confidence(rule: &Rule, count: usize, units: &Units) -> f64 {
    let evidence = evidence(rule.id, count, units.of(rule.unit()));
    round(sigmoid(SCALE * evidence + BIAS))
}

/// Confidence that a file (or section) is AI-styled, from every selected rule, including the
/// ones that did not fire where they had the chance.
pub(crate) fn score(counts: &BTreeMap<&str, usize>, units: &Units, selected: &[&Rule]) -> f64 {
    let evidence: f64 = selected
        .iter()
        .map(|rule| {
            let count = counts.get(rule.id).copied().unwrap_or(0);
            evidence(rule.id, count, units.of(rule.unit()))
        })
        .sum();
    round(sigmoid(SCALE * evidence + BIAS))
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};

    use super::{BIAS, EPS, PARAMS, SCALE, evidence, ln_gamma, strength};
    use crate::rules::RULES;

    #[test]
    fn ln_gamma_matches_known_values() {
        let cases = [
            (1.0, 0.0),
            (2.0, 0.0),
            (0.5, std::f64::consts::PI.sqrt().ln()),
            (10.0, 362_880_f64.ln()),
            (0.1, 2.252_712_651_734_206),
            (150.5, 602.513_954_870_585_3),
        ];
        for (x, expected) in cases {
            assert!((ln_gamma(x) - expected).abs() < 1e-9, "ln_gamma({x})");
        }
    }

    #[test]
    fn every_rule_has_parameters_in_table_order() {
        let ids: Vec<&str> = PARAMS.iter().map(|(id, _)| *id).collect();
        let rules: Vec<&str> = RULES.iter().map(|rule| rule.id).collect();
        assert_eq!(ids, rules);
        assert!(PARAMS.iter().all(|(_, p)| p.iter().all(|v| *v > 0.0)));
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
        assert!((number("eps")? - EPS).abs() < 1e-9);
        assert_eq!(fitted["model"], "beta-binomial likelihood ratio");
        assert!((number("scale")? - SCALE).abs() < 1e-9);
        assert!((number("bias")? - BIAS).abs() < 1e-9);
        for (id, params) in PARAMS {
            let row = &fitted["params"][*id];
            for (index, value) in params.iter().enumerate() {
                let expected = row[index]
                    .as_f64()
                    .with_context(|| format!("{id}[{index}] missing"))?;
                assert!((expected - value).abs() < 1e-9, "{id}[{index}]");
            }
        }
        Ok(())
    }

    #[test]
    fn evidence_matches_the_python_fit_for_every_rule() -> Result<()> {
        let parity: serde_json::Value =
            serde_json::from_str(include_str!("../calibration/parity.json"))?;
        for (id, _) in PARAMS {
            for n in [1_usize, 5, 10, 40] {
                let expected = parity[*id][n.to_string()]
                    .as_array()
                    .with_context(|| format!("{id} n={n} missing"))?;
                for (k, value) in expected.iter().enumerate() {
                    let value = value.as_f64().context("a number")?;
                    let got = evidence(id, k, n);
                    assert!((got - value).abs() < 1e-6, "{id} {k}/{n}: {got} vs {value}");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn evidence_never_turns_back_as_hits_grow() {
        for (id, _) in PARAMS {
            let values: Vec<f64> = (1..=30).map(|k| evidence(id, k, 30)).collect();
            let rising = values.windows(2).all(|pair| pair[1] >= pair[0] - 1e-12);
            let falling = values.windows(2).all(|pair| pair[1] <= pair[0] + 1e-12);
            assert!(rising || falling, "{id}: {values:?}");
        }
    }

    #[test]
    fn a_consistent_pattern_is_stronger_evidence_than_a_stray_one() {
        // Six of six list items with a bold label versus one of twelve.
        assert!(evidence("bold-lead-in", 6, 6) > evidence("bold-lead-in", 1, 12));
    }

    #[test]
    fn no_rule_alone_is_proof() {
        let cap = ((1.0 - EPS) / EPS).ln() + 1e-9;
        for (id, _) in PARAMS {
            for (count, opportunities) in [(1, 1), (1, 10), (10, 10), (50, 50)] {
                assert!(evidence(id, count, opportunities).abs() <= cap, "{id}");
            }
        }
    }

    #[test]
    fn unknown_rules_and_empty_files_carry_no_evidence() {
        assert!(evidence("no-such-rule", 3, 10).abs() < f64::EPSILON);
        assert!(evidence("em-dash", 0, 0).abs() < f64::EPSILON);
        assert!(strength("no-such-rule").abs() < f64::EPSILON);
    }
}
