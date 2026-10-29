"""Fit tics' confidence weights from the corpus and check them with grouped cross-validation.

Model: logistic regression over BM25-saturated rule counts,
    x_r = tf_r * (k1 + 1) / (tf_r + k1 * (1 - b + b * words / avg_words))
    p(AI | doc) = sigmoid(bias + sum_r w_r * x_r),   w_r >= 0
with an L2 penalty pulling every w_r towards PRIOR, so a rule the corpus never saw keeps a
sensible weight. Classes are balanced (equal prior on human and AI). Folds are grouped by
prompt so the human and AI answers to one question never sit on both sides of a split.

usage: python fit.py --tics PATH  (writes weights.json and prints a Markdown report)
"""
import argparse
import collections
import json
import math
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).parent
PRIOR, LAMBDA = 1.0, 2.0
GRID = [(k1, b) for k1 in (0.5, 1.2, 2.0) for b in (0.0, 0.5, 0.75)]
FOLDS = 5


def scan(tics, path):
    result = subprocess.run([tics, "--summary", "--format", "json", str(path)],
                            capture_output=True, text=True)
    summary = json.loads(result.stdout)["summary"]
    return summary["rules"], summary["words"]


def load(tics):
    rules = [line.split()[0] for line in subprocess.run(
        [tics, "rules"], capture_output=True, text=True).stdout.splitlines() if line.strip()]
    docs = []
    for path in sorted((ROOT / "human").glob("*.md")):
        docs.append({"path": path, "y": 0, "model": "human"})
    for path in sorted((ROOT / "ai").glob("*/*.md")):
        docs.append({"path": path, "y": 1, "model": path.parent.name})
    for doc in docs:
        doc["counts"], doc["words"] = scan(tics, doc["path"])
        doc["prompt"] = doc["path"].stem
        doc["kind"] = "qa" if doc["prompt"].startswith("se-") else "explain"
    return rules, docs


def features(doc, rules, k1, b, avg_words):
    norm = k1 * (1 - b + b * max(doc["words"], 1) / avg_words)
    return [doc["counts"].get(r, 0) * (k1 + 1) / (doc["counts"].get(r, 0) + norm) for r in rules]


def sigmoid(z):
    return 1 / (1 + math.exp(-max(-30, min(30, z))))


def train(xs, ys, n_rules, steps=None, rate=None):
    """Balanced log-loss + LAMBDA/2 * |w - PRIOR|^2 with w >= 0, solved with L-BFGS-B."""
    import numpy as np
    from scipy.optimize import minimize
    x = np.asarray(xs, dtype=float)
    y = np.asarray(ys, dtype=float)
    pos, neg = y.sum(), len(y) - y.sum()
    sw = np.where(y == 1, len(y) / (2 * pos), len(y) / (2 * neg))

    def objective(params):
        w, bias = params[:-1], params[-1]
        z = x @ w + bias
        p = 1 / (1 + np.exp(-np.clip(z, -30, 30)))
        loss = np.sum(sw * (np.logaddexp(0, z) - y * z)) + LAMBDA / 2 * np.sum((w - PRIOR) ** 2)
        err = sw * (p - y)
        grad = np.concatenate([x.T @ err + LAMBDA * (w - PRIOR), [err.sum()]])
        return loss, grad

    start = np.concatenate([np.full(n_rules, PRIOR), [0.0]])
    bounds = [(0, None)] * n_rules + [(None, None)]
    result = minimize(objective, start, jac=True, method="L-BFGS-B", bounds=bounds)
    return list(result.x[:-1]), float(result.x[-1])


def predict(x, w, bias):
    return sigmoid(bias + sum(wi * xi for wi, xi in zip(w, x)))


def auc(scores, ys):
    pos = [s for s, y in zip(scores, ys) if y]
    neg = [s for s, y in zip(scores, ys) if not y]
    wins = sum((p > n) + 0.5 * (p == n) for p in pos for n in neg)
    return wins / (len(pos) * len(neg))


def cross_validate(docs, rules, k1, b):
    prompts = sorted({d["prompt"] for d in docs})
    random.Random(7).shuffle(prompts)
    fold_of = {p: i % FOLDS for i, p in enumerate(prompts)}
    predictions = {}
    for fold in range(FOLDS):
        train_docs = [d for d in docs if fold_of[d["prompt"]] != fold]
        test_docs = [d for d in docs if fold_of[d["prompt"]] == fold]
        avg = sum(d["words"] for d in train_docs) / len(train_docs)
        w, bias = train([features(d, rules, k1, b, avg) for d in train_docs],
                        [d["y"] for d in train_docs], len(rules), steps=800)
        for d in test_docs:
            predictions[id(d)] = predict(features(d, rules, k1, b, avg), w, bias)
    scores = [predictions[id(d)] for d in docs]
    ys = [d["y"] for d in docs]
    loss = -sum(math.log(s if y else 1 - s) for s, y in zip(scores, ys)) / len(ys)
    return scores, loss


def report(docs, scores, title):
    ys = [d["y"] for d in docs]
    print(f"\n### {title}\n")
    print(f"AUC {auc(scores, ys):.3f}\n")
    print("| group | n | mean score | flagged at 0.5 | flagged at 0.8 |")
    print("|---|---|---|---|---|")
    groups = collections.defaultdict(list)
    for d, s in zip(docs, scores):
        groups[d["model"]].append(s)
        groups[f"all {'AI' if d['y'] else 'human'} ({d['kind']})"].append(s)
    for name in sorted(groups, key=lambda n: (not n.startswith("all"), n)):
        values = groups[name]
        print(f"| {name} | {len(values)} | {sum(values) / len(values):.2f} | "
              f"{100 * sum(v >= 0.5 for v in values) / len(values):.0f}% | "
              f"{100 * sum(v >= 0.8 for v in values) / len(values):.0f}% |")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tics", required=True)
    args = parser.parse_args()
    rules, docs = load(args.tics)
    print("# tics confidence calibration\n")
    print(f"{sum(1 - d['y'] for d in docs)} human and {sum(d['y'] for d in docs)} AI documents, "
          f"{len(rules)} rules, {FOLDS}-fold CV grouped by prompt.\n")
    print("| k1 | b | CV log-loss |\n|---|---|---|")
    results = []
    for k1, b in GRID:
        scores, loss = cross_validate(docs, rules, k1, b)
        results.append((loss, k1, b, scores))
        print(f"| {k1} | {b} | {loss:.4f} |")
    loss, k1, b, scores = min(results, key=lambda r: r[0])
    report(docs, scores, f"Held-out scores (k1={k1}, b={b})")

    avg = sum(d["words"] for d in docs) / len(docs)
    w, bias = train([features(d, rules, k1, b, avg) for d in docs], [d["y"] for d in docs],
                    len(rules))
    hits = collections.Counter()
    for d in docs:
        for r in d["counts"]:
            hits[(r, d["y"])] += 1
    print("\n### Final weights (trained on all documents)\n")
    print(f"bias {bias:.3f}; a document with no findings scores {sigmoid(bias):.2f}\n")
    print("| rule | weight | AI docs | human docs | one hit in an average doc |")
    print("|---|---|---|---|---|")
    one_hit = (k1 + 1) / (1 + k1)
    for r, wi in sorted(zip(rules, w), key=lambda t: -t[1]):
        print(f"| {r} | {wi:.2f} | {hits[(r, 1)]} | {hits[(r, 0)]} | "
              f"{sigmoid(bias + wi * one_hit):.2f} |")
    (ROOT / "weights.json").write_text(json.dumps({
        "k1": k1, "b": b, "avg_words": round(avg, 1), "bias": round(bias, 4), "prior": PRIOR,
        "lambda": LAMBDA, "documents": {"human": sum(1 - d["y"] for d in docs),
                                        "ai": sum(d["y"] for d in docs)},
        "weights": {r: round(wi, 4) for r, wi in zip(rules, w)}}, indent=2))


if __name__ == "__main__":
    main()
