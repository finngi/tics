"""Fit tics' confidence model from the corpus and evaluate it with nested, grouped CV.

Model (see src/score.rs): per rule r and class c, a file's rate is Beta(a, b) and `k` of its `n`
opportunities hold a hit (sentences of three or more words, list items, headings or Markdown
blocks, as counted by tics). Evidence is the log-likelihood ratio AI vs human, with each class
writing like the other a fraction EPS of the time, kept monotone in k in the direction the
rule leans. A file's evidence is the sum over rules, mapped to a probability by
sigmoid(SCALE * evidence + BIAS). Files with no prose (no opportunities) are not scored.

Fitting: joint MAP per rule over both classes, parameterised by the human rate and the log rate
ratio delta (AI over human), delta ~ N(ln PRIOR_RATIO, delta_sd); log-concentrations ~ N(ln 10,
1) with the AI one tied to the human one by N(0, CONCENTRATION_SD).

Evaluation: 5 outer folds grouped by prompt. Inside each outer training set, an inner 4-fold
grouped CV picks (EPS, delta_sd) by balanced log-loss and gives the out-of-fold evidence that
SCALE and BIAS are fitted on, exactly as the final model is built. Metrics are balanced (human
and AI weighted equally), thresholds use scores rounded to two decimals as the CLI prints them,
and intervals come from a prompt-cluster bootstrap. A length-and-structure baseline (log words
and opportunity counts, no rules) runs through the same folds for comparison.

usage: uv run --with numpy --with scipy python fit.py --tics PATH > fit-report.md
"""
import argparse
import json
import math
import random
import subprocess
from pathlib import Path

import numpy as np
from scipy.optimize import minimize
from scipy.special import betaln, gammaln

ROOT = Path(__file__).parent
PRIOR_RATIO = math.e
CONCENTRATION_SD = 0.5
GRID = [(eps, delta_sd) for eps in (0.02, 0.05, 0.1) for delta_sd in (0.5, 1.0, 2.0)]
FOLDS, INNER_FOLDS, SEED, BOOTSTRAP = 5, 4, 7, 1000
UNIT_KEY = {"sentence": "sentences", "list-item": "list_items", "heading": "headings",
            "block": "blocks"}


# ---------------------------------------------------------------- data

def load(tics):
    rules = json.loads(subprocess.run([tics, "rules", "--format", "json"], capture_output=True,
                                      text=True, check=True).stdout)
    unit_of = {rule["id"]: UNIT_KEY[rule["unit"]] for rule in rules}
    paths = sorted((ROOT / "human").glob("*.md")) + sorted((ROOT / "ai").glob("*/*.md"))
    result = subprocess.run([tics, "score", "--format", "json", *map(str, paths)],
                            capture_output=True, text=True, check=True)
    docs, skipped = [], 0
    for path, row in zip(paths, json.loads(result.stdout)):
        if row["score"] is None:
            skipped += 1
            continue
        human = path.parent.name == "human"
        docs.append({"rel": str(path.relative_to(ROOT)), "y": 0 if human else 1,
                     "model": "human" if human else path.parent.name, "prompt": path.stem,
                     "words": row["words"], "units": row["units"], "k": row["occupied"]})
    return [r["id"] for r in rules], unit_of, docs, skipped


def kn(doc, rule, unit_of):
    n = doc["units"][unit_of[rule]]
    return min(doc["k"].get(rule, 0), n), n


def folds_by_prompt(docs, folds, seed):
    prompts = sorted({d["prompt"] for d in docs})
    random.Random(seed).shuffle(prompts)
    fold_of = {p: i % folds for i, p in enumerate(prompts)}
    return [fold_of[d["prompt"]] for d in docs]


# ---------------------------------------------------------------- model

def ln_lik(k, n, a, b):
    return betaln(k + a, n - k + b) - betaln(a, b)


def fit_rule(human, ai, delta_sd):
    hk, hn = (np.asarray(v, float) for v in human)
    ak, an = (np.asarray(v, float) for v in ai)
    pooled = (hk.sum() + ak.sum() + 0.5) / (hn.sum() + an.sum() + 1)

    def unpack(theta):
        human_mean = 1 / (1 + np.exp(-theta[0]))
        ai_mean = min(0.999, human_mean * np.exp(theta[1]))
        hc, ac = np.exp(theta[2]), np.exp(theta[3])
        return human_mean * hc, (1 - human_mean) * hc, ai_mean * ac, (1 - ai_mean) * ac

    def negative(theta):
        a0, b0, a1, b1 = unpack(theta)
        ll = ln_lik(hk, hn, a0, b0).sum() + ln_lik(ak, an, a1, b1).sum()
        prior = (-0.5 * ((theta[1] - math.log(PRIOR_RATIO)) / delta_sd) ** 2
                 - 0.5 * (theta[2] - math.log(10)) ** 2
                 - 0.5 * ((theta[3] - theta[2]) / CONCENTRATION_SD) ** 2
                 - 0.5 * ((theta[0] - math.log(pooled / (1 - pooled))) / 3) ** 2)
        return -(ll + prior)

    start = np.array([math.log(pooled / (1 - pooled)), math.log(PRIOR_RATIO), math.log(10),
                      math.log(10)])
    bounds = [(-18, 4), (-8, 8), (math.log(0.05), math.log(1e4)), (math.log(0.05), math.log(1e4))]
    result = minimize(negative, start, method="L-BFGS-B", bounds=bounds)
    return [float(v) for v in unpack(result.x)]


def fit_params(docs, rules, unit_of, delta_sd):
    params = {}
    for rule in rules:
        data = {0: ([], []), 1: ([], [])}
        for doc in docs:
            k, n = kn(doc, rule, unit_of)
            if n:
                data[doc["y"]][0].append(k)
                data[doc["y"]][1].append(n)
        params[rule] = fit_rule(data[0], data[1], delta_sd)
    return params


def mixture_pmf(n, p, eps):
    ks = np.arange(n + 1, dtype=float)
    choose = gammaln(n + 1) - gammaln(ks + 1) - gammaln(n - ks + 1)
    human = np.exp(choose + ln_lik(ks, n, p[0], p[1]))
    ai = np.exp(choose + ln_lik(ks, n, p[2], p[3]))
    return (1 - eps) * ai + eps * human, (1 - eps) * human + eps * ai


def strength(p, eps, n=10):
    ai, human = mixture_pmf(n, p, eps)
    divergence = float(np.sum((ai - human) * (np.log(ai) - np.log(human))))
    lean = float(np.sum(np.arange(n + 1) * (ai - human)))
    return math.copysign(divergence, lean)


def raw_evidence(k, n, p, eps):
    if n == 0:
        return 0.0
    k = min(k, n)
    human, ai = ln_lik(k, n, p[0], p[1]), ln_lik(k, n, p[2], p[3])
    keep, swap = math.log(1 - eps), math.log(eps)
    return float(np.logaddexp(keep + ai, swap + human) - np.logaddexp(keep + human, swap + ai))


def rule_evidence(k, n, p, eps, lean):
    """Mirror of score::evidence: monotone in k in the rule's direction."""
    k = min(k, n)
    first = raw_evidence(k, n, p, eps)
    if k <= 1:
        return first
    values = [raw_evidence(j, n, p, eps) for j in range(1, k)] + [first]
    return max(values) if lean >= 0 else min(values)


def leans(params, eps):
    """Unrounded strength per rule; its sign is the direction evidence is kept monotone in."""
    return {r: strength(p, eps) for r, p in params.items()}


def evidence(doc, rules, unit_of, params, eps, lean):
    return sum(rule_evidence(*kn(doc, r, unit_of), params[r], eps, lean[r]) for r in rules)


def calibrate(xs, ys):
    x, y = np.asarray(xs, float), np.asarray(ys, float)
    weight = np.where(y == 1, len(y) / (2 * y.sum()), len(y) / (2 * (len(y) - y.sum())))

    def loss(p):
        z = p[0] * x + p[1]
        return np.sum(weight * (np.logaddexp(0, z) - y * z))

    scale, bias = minimize(loss, [1.0, 0.0]).x
    return float(scale), float(bias)


def sigmoid(z):
    return 1 / (1 + math.exp(-max(-30, min(30, z))))


# ---------------------------------------------------------------- metrics

def auc(scores, ys):
    pos = [s for s, y in zip(scores, ys) if y]
    neg = [s for s, y in zip(scores, ys) if not y]
    return sum((p > n) + 0.5 * (p == n) for p in pos for n in neg) / (len(pos) * len(neg))


def balanced_log_loss(scores, ys):
    pos = [math.log(max(1e-12, s)) for s, y in zip(scores, ys) if y]
    neg = [math.log(max(1e-12, 1 - s)) for s, y in zip(scores, ys) if not y]
    return -0.5 * (sum(pos) / len(pos) + sum(neg) / len(neg))


def bootstrap(docs, scores, metric):
    prompts = sorted({d["prompt"] for d in docs})
    members = {}
    for i, d in enumerate(docs):
        members.setdefault(d["prompt"], []).append(i)
    rng = random.Random(SEED)
    values = []
    for _ in range(BOOTSTRAP):
        picked = [i for _ in prompts for i in members[rng.choice(prompts)]]
        if len({docs[i]["y"] for i in picked}) < 2:
            continue
        values.append(metric([scores[i] for i in picked], [docs[i]["y"] for i in picked]))
    values.sort()
    return values[int(0.025 * len(values))], values[int(0.975 * len(values)) - 1]


# ---------------------------------------------------------------- tics model, nested CV

def out_of_fold_evidence(docs, rules, unit_of, eps, delta_sd, fold_ids, folds):
    evidence_of = [0.0] * len(docs)
    for fold in range(folds):
        train = [d for d, f in zip(docs, fold_ids) if f != fold]
        params = fit_params(train, rules, unit_of, delta_sd)
        lean = leans(params, eps)
        for i, (d, f) in enumerate(zip(docs, fold_ids)):
            if f == fold:
                evidence_of[i] = evidence(d, rules, unit_of, params, eps, lean)
    return evidence_of


def choose(docs, rules, unit_of, fold_ids, folds):
    """Pick (EPS, delta_sd) by balanced log-loss of calibrated out-of-fold scores, and return
    the calibration fitted on that out-of-fold evidence."""
    ys = [d["y"] for d in docs]
    best = None
    for eps, delta_sd in GRID:
        oof = out_of_fold_evidence(docs, rules, unit_of, eps, delta_sd, fold_ids, folds)
        scale, bias = calibrate(oof, ys)
        loss = balanced_log_loss([sigmoid(scale * x + bias) for x in oof], ys)
        if best is None or loss < best[0]:
            best = (loss, eps, delta_sd, scale, bias)
    return best[1:]


def nested_cv(docs, rules, unit_of):
    outer = folds_by_prompt(docs, FOLDS, SEED)
    scores, chosen = [0.0] * len(docs), []
    for fold in range(FOLDS):
        train = [d for d, f in zip(docs, outer) if f != fold]
        inner = folds_by_prompt(train, INNER_FOLDS, SEED + 1)
        eps, delta_sd, scale, bias = choose(train, rules, unit_of, inner, INNER_FOLDS)
        chosen.append((eps, delta_sd))
        params = fit_params(train, rules, unit_of, delta_sd)
        lean = leans(params, eps)
        for i, (d, f) in enumerate(zip(docs, outer)):
            if f == fold:
                scores[i] = sigmoid(scale * evidence(d, rules, unit_of, params, eps, lean) + bias)
    return scores, chosen


# ---------------------------------------------------------------- structure baseline

def structure_features(d):
    u = d["units"]
    return [math.log1p(d["words"]), math.log1p(u["sentences"]), math.log1p(u["list_items"]),
            math.log1p(u["headings"]), math.log1p(u["blocks"])]


def logistic(xs, ys):
    x = np.column_stack([np.asarray(xs, float), np.ones(len(xs))])
    y = np.asarray(ys, float)
    weight = np.where(y == 1, len(y) / (2 * y.sum()), len(y) / (2 * (len(y) - y.sum())))

    def loss(w):
        z = x @ w
        return np.sum(weight * (np.logaddexp(0, z) - y * z)) + 0.5 * np.sum(w[:-1] ** 2)

    return minimize(loss, np.zeros(x.shape[1])).x


def structure_baseline(docs):
    outer = folds_by_prompt(docs, FOLDS, SEED)
    scores = [0.0] * len(docs)
    for fold in range(FOLDS):
        train = [d for d, f in zip(docs, outer) if f != fold]
        w = logistic([structure_features(d) for d in train], [d["y"] for d in train])
        for i, (d, f) in enumerate(zip(docs, outer)):
            if f == fold:
                scores[i] = sigmoid(float(np.dot(w[:-1], structure_features(d)) + w[-1]))
    return scores


# ---------------------------------------------------------------- report

def summary_row(name, docs, scores):
    ys = [d["y"] for d in docs]
    rounded = [round(s, 2) for s in scores]
    a, (alo, ahi) = auc(scores, ys), bootstrap(docs, scores, auc)
    b, (blo, bhi) = balanced_log_loss(scores, ys), bootstrap(docs, scores, balanced_log_loss)
    human = [s for s, y in zip(rounded, ys) if not y]
    ai = [s for s, y in zip(rounded, ys) if y]
    print(f"| {name} | {a:.3f} ({alo:.3f}-{ahi:.3f}) | {b:.3f} ({blo:.3f}-{bhi:.3f}) | "
          f"{100 * sum(s >= .8 for s in human) / len(human):.0f}% | "
          f"{100 * sum(s >= .8 for s in ai) / len(ai):.0f}% |")


def group_table(docs, scores):
    rounded = [round(s, 2) for s in scores]
    print("| group | n | score ≥ 0.5 | score ≥ 0.8 |\n|---|---|---|---|")
    groups = {}
    for d, s in zip(docs, rounded):
        groups.setdefault(d["model"], []).append(s)
        if d["y"]:
            family = "all Claude" if d["model"] in ("opus", "sonnet", "haiku") else "all Codex"
            groups.setdefault(family, []).append(s)
        source = "Stack Exchange" if d["prompt"].startswith("se-") else "Wikipedia"
        groups.setdefault(source + (" AI" if d["y"] else " human"), []).append(s)
    head = ["human", "all Claude", "all Codex", "Stack Exchange human", "Stack Exchange AI",
            "Wikipedia human", "Wikipedia AI"]
    for name in head + sorted(k for k in groups if k not in head):
        values = groups[name]
        print(f"| {name} | {len(values)} | {100 * sum(v >= .5 for v in values) / len(values):.0f}% "
              f"| {100 * sum(v >= .8 for v in values) / len(values):.0f}% |")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tics", required=True)
    parser.add_argument("--predictions", help="write tics' out-of-fold scores here (JSON)")
    args = parser.parse_args()
    rules, unit_of, docs, skipped = load(args.tics)
    ys = [d["y"] for d in docs]
    print("# tics confidence calibration\n")
    print(f"{len(ys) - sum(ys)} human and {sum(ys)} AI files ({skipped} without prose skipped), "
          f"{len(rules)} rules. Outer {FOLDS}-fold and inner {INNER_FOLDS}-fold CV, both grouped "
          f"by prompt. Balanced metrics; 95% prompt-cluster bootstrap intervals; thresholds on "
          f"scores rounded as the CLI prints them.\n")
    scores, chosen = nested_cv(docs, rules, unit_of)
    baseline = structure_baseline(docs)
    se = [i for i, d in enumerate(docs) if d["prompt"].startswith("se-")]
    print("| model | AUC | balanced log-loss | human ≥ 0.8 | AI ≥ 0.8 |\n|---|---|---|---|---|")
    summary_row("tics", docs, scores)
    summary_row("length and structure only (no rules)", docs, baseline)
    summary_row("tics, Stack Exchange prompts only", [docs[i] for i in se], [scores[i] for i in se])
    summary_row("length and structure, Stack Exchange only", [docs[i] for i in se],
                [baseline[i] for i in se])
    print(f"\nInner CV chose (EPS, delta sd) per outer fold: {chosen}.\n")
    print("### tics held-out scores by group\n")
    group_table(docs, scores)

    fold_ids = folds_by_prompt(docs, FOLDS, SEED)
    eps, delta_sd, scale, bias = choose(docs, rules, unit_of, fold_ids, FOLDS)
    # Round first, so the report, the parity table and weights.json all use what ships.
    params = {r: [round(v, 6) for v in p] for r, p in
              fit_params(docs, rules, unit_of, delta_sd).items()}
    scale, bias = round(scale, 4), round(bias, 4)
    lean = leans(params, eps)
    print(f"\n### Final model\n\nEPS {eps}, delta sd {delta_sd:g}, SCALE {scale:.4f}, "
          f"BIAS {bias:.4f} (calibrated on out-of-fold evidence).\n")
    print("Strength is the signed Jeffreys divergence between the AI and human distributions of "
          "occupied opportunities out of ten (negative: people use it more). The evidence columns "
          "are the log-likelihood ratio for one and five occupied opportunities out of ten.\n")
    print("| rule | unit | strength | 1 of 10 | 5 of 10 | human rate | AI rate | "
          "files with a hit (human / AI) |\n|---|---|---|---|---|---|---|---|")
    rows = []
    for r in rules:
        p = params[r]
        hits = [sum(1 for d in docs if d["y"] == c and d["k"].get(r)) for c in (0, 1)]
        rows.append((lean[r], f"| {r} | {unit_of[r]} | {lean[r]:+.2f} | "
                     f"{rule_evidence(1, 10, p, eps, lean[r]):+.2f} | "
                     f"{rule_evidence(5, 10, p, eps, lean[r]):+.2f} | {p[0] / (p[0] + p[1]):.4f} | "
                     f"{p[2] / (p[2] + p[3]):.4f} | {hits[0]} / {hits[1]} |"))
    for _, row in sorted(rows, reverse=True):
        print(row)
    # Reference evidence for the Rust parity test (src/score.rs).
    parity = {r: {str(n): [round(rule_evidence(k, n, params[r], eps, lean[r]), 9)
                           for k in range(n + 1)] for n in (1, 5, 10, 40)} for r in rules}
    (ROOT / "parity.json").write_text(json.dumps(parity) + "\n")
    (ROOT / "weights.json").write_text(json.dumps({
        "model": "beta-binomial likelihood ratio", "eps": eps, "scale": round(scale, 4),
        "bias": round(bias, 4), "delta_sd": delta_sd, "concentration_sd": CONCENTRATION_SD,
        "prior_ratio": PRIOR_RATIO,
        "files": {"human": len(ys) - sum(ys), "ai": sum(ys)},
        "params": {r: [round(v, 6) for v in params[r]] for r in rules}}, indent=2) + "\n")
    if args.predictions:
        Path(args.predictions).write_text(json.dumps({d["rel"]: s for d, s in zip(docs, scores)}))


if __name__ == "__main__":
    main()
