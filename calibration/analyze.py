"""Compare tics hit rates across human, Claude and Codex text, and mine uncovered AI patterns.

usage: python analyze.py [--tics PATH] > report.md
Per rule: share of documents with a hit and hits per 1,000 words, per group, plus a smoothed
log-odds ratio AI vs human. Pattern mining uses the log-odds with an informative Dirichlet prior
(Monroe, Colaresi & Quinn 2008) over document frequencies, so one verbose answer can't dominate.
"""
import argparse
import collections
import json
import math
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).parent
CLAUDE = {"opus", "sonnet", "haiku"}

FENCE = re.compile(r"^(```|~~~).*?^\1", re.M | re.S)
INLINE = re.compile(r"`[^`\n]+`")
URL = re.compile(r"https?://\S+")
WORD = re.compile(r"[A-Za-z][A-Za-z'’-]*")


def prose(text):
    return URL.sub(" ", INLINE.sub(" ", FENCE.sub(" ", text)))


def word_count(text):
    return len(WORD.findall(prose(text)))


def documents():
    docs = []
    for path in sorted((ROOT / "human").glob("*.md")):
        docs.append({"path": path, "group": "human", "model": "human", "id": path.stem})
    for path in sorted((ROOT / "ai").glob("*/*.md")):
        model = path.parent.name
        family = "claude" if model in CLAUDE else "codex"
        docs.append({"path": path, "group": family, "model": model, "id": path.stem})
    for doc in docs:
        doc["text"] = doc["path"].read_text()
        doc["words"] = word_count(doc["text"])
        doc["kind"] = "qa" if doc["id"].startswith("se-") else "explain"
    return docs


def run_tics(tics, docs):
    paths = [str(doc["path"]) for doc in docs]
    hits = collections.defaultdict(collections.Counter)
    for start in range(0, len(paths), 200):
        result = subprocess.run([tics, "--format", "json", *paths[start:start + 200]],
                                capture_output=True, text=True)
        for finding in json.loads(result.stdout or "[]"):
            hits[finding["path"]][finding["rule"]] += 1
    for doc in docs:
        doc["hits"] = hits[str(doc["path"])]


def log_odds(a_count, a_total, b_count, b_total, prior=0.5):
    """Smoothed log-odds ratio of a rate in group a vs group b, with its z-score."""
    a = (a_count + prior) / (a_total - a_count + prior)
    b = (b_count + prior) / (b_total - b_count + prior)
    delta = math.log(a) - math.log(b)
    variance = 1 / (a_count + prior) + 1 / (b_count + prior)
    return delta, delta / math.sqrt(variance)


def rule_table(docs, rules):
    groups = ["human", "claude", "codex"]
    models = sorted({d["model"] for d in docs if d["group"] != "human"})
    by = {g: [d for d in docs if d["group"] == g] for g in groups}
    by_model = {m: [d for d in docs if d["model"] == m] for m in models}
    print("## Rule hit rates\n")
    print("Docs with a hit (%), then hits per 1,000 words. LOR = log-odds AI vs human on docs "
          "with a hit (z in brackets); positive means more common in AI.\n")
    print("| rule | human | claude | codex | LOR (z) |")
    print("|---|---|---|---|---|")
    rows = []
    ai = by["claude"] + by["codex"]
    for rule in rules:
        cells = []
        for group in groups:
            members = by[group]
            with_hit = sum(1 for d in members if d["hits"][rule])
            per_k = 1000 * sum(d["hits"][rule] for d in members) / max(1, sum(d["words"] for d in members))
            cells.append(f"{100 * with_hit / max(1, len(members)):.0f}% / {per_k:.1f}")
        a = sum(1 for d in ai if d["hits"][rule])
        h = sum(1 for d in by["human"] if d["hits"][rule])
        if a + h == 0:
            continue
        delta, z = log_odds(a, len(ai), h, len(by["human"]))
        rows.append((delta, f"| {rule} | {' | '.join(cells)} | {delta:+.2f} ({z:+.1f}) |"))
    for _, row in sorted(rows, reverse=True):
        print(row)
    print("\n## Per model: docs with a hit (%)\n")
    top = [r for r in rules if sum(d["hits"][r] for d in docs) > 0]
    print("| model | n | words | " + " | ".join(top) + " |")
    print("|---|---|---|" + "---|" * len(top))
    for model in ["human", *models]:
        members = by["human"] if model == "human" else by_model[model]
        mean_words = sum(d["words"] for d in members) / max(1, len(members))
        cells = [f"{100 * sum(1 for d in members if d['hits'][r]) / max(1, len(members)):.0f}" for r in top]
        print(f"| {model} | {len(members)} | {mean_words:.0f} | " + " | ".join(cells) + " |")


def ngrams(text, n):
    tokens = [t.lower() for t in WORD.findall(prose(text))]
    return {" ".join(tokens[i:i + n]) for i in range(len(tokens) - n + 1)}


def structure(text):
    lines = text.splitlines()
    items = [l for l in lines if re.match(r"^\s*([-*+]|\d+[.)])\s+", l)]
    bold_lead = [l for l in items if re.match(r"^\s*([-*+]|\d+[.)])\s+\*\*[^*]+\*\*", l)]
    features = {
        "has_heading": any(re.match(r"^#{1,6}\s", l) for l in lines),
        "has_list": bool(items),
        "bold_lead_items>=2": len(bold_lead) >= 2,
        "bold_lead_share>=0.5": bool(items) and len(bold_lead) / len(items) >= 0.5,
        "bold_paragraph_label": any(re.match(r"^\*\*[^*]{2,60}\*\*:?\s*$|^\*\*[^*]{2,60}:\*\*\s+\S", l) for l in lines),
        "bold_span_any": "**" in text,
        "em_dash": "—" in text,
        "en_dash_spaced": " – " in text,
        "colon_intro_line": any(re.search(r"\w:\s*$", l) and not l.lstrip().startswith(("-", "*", "#")) for l in lines),
        "ends_with_offer": bool(re.search(r"(if you('d)? (want|like)|let me know|i can also|want me to)[^\n]*\s*$", text.strip()[-300:], re.I)),
        "ends_with_question": text.strip().endswith("?"),
        "emoji": bool(re.search(r"[\U0001F300-\U0001FAFF\u2705\u274C\u2728]", text)),
        "short_answer_first_line": len(lines[0].split()) <= 15 if lines else False,
        "tldr": bool(re.search(r"\b(tl;dr|in short|bottom line|short answer)\b", text, re.I)),
    }
    return features


def mine(docs, n_values=(1, 2, 3), top=40, min_docs=8):
    ai = [d for d in docs if d["group"] != "human"]
    human = [d for d in docs if d["group"] == "human"]
    print("\n## Structural features: docs with feature (%)\n")
    feats = collections.defaultdict(lambda: collections.Counter())
    for d in docs:
        for name, value in structure(d["text"]).items():
            if value:
                feats[d["group"]][name] += 1
    sizes = collections.Counter(d["group"] for d in docs)
    names = sorted({n for g in feats.values() for n in g})
    print("| feature | human | claude | codex | LOR (z) |")
    print("|---|---|---|---|---|")
    for name in names:
        a = feats["claude"][name] + feats["codex"][name]
        delta, z = log_odds(a, sizes["claude"] + sizes["codex"], feats["human"][name], sizes["human"])
        cells = [f"{100 * feats[g][name] / max(1, sizes[g]):.0f}" for g in ("human", "claude", "codex")]
        print(f"| {name} | {' | '.join(cells)} | {delta:+.2f} ({z:+.1f}) |")

    print(f"\n## Phrases most over-used by AI (doc frequency, ≥{min_docs} AI docs)\n")
    for n in n_values:
        df_ai, df_h = collections.Counter(), collections.Counter()
        for d in ai:
            df_ai.update(ngrams(d["text"], n))
        for d in human:
            df_h.update(ngrams(d["text"], n))
        prior_total = df_ai + df_h
        alpha0 = 0.01 * sum(prior_total.values())
        n_ai, n_h = sum(df_ai.values()), sum(df_h.values())
        scored = []
        for gram, count in df_ai.items():
            if count < min_docs:
                continue
            alpha = alpha0 * prior_total[gram] / sum(prior_total.values())
            a, h = count, df_h[gram]
            delta = (math.log((a + alpha) / (n_ai + alpha0 - a - alpha))
                     - math.log((h + alpha) / (n_h + alpha0 - h - alpha)))
            z = delta / math.sqrt(1 / (a + alpha) + 1 / (h + alpha))
            scored.append((z, gram, a, h))
        scored.sort(reverse=True)
        print(f"### {n}-grams\n")
        print("| phrase | AI docs | human docs | z |")
        print("|---|---|---|---|")
        for z, gram, a, h in scored[:top]:
            print(f"| {gram} | {a} | {h} | {z:.1f} |")
        print()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tics", default=str(Path.home() / ".cargo/bin/tics"))
    args = parser.parse_args()
    docs = documents()
    run_tics(args.tics, docs)
    rules = [line.split()[0] for line in subprocess.run([args.tics, "rules"], capture_output=True,
                                                          text=True).stdout.splitlines() if line.strip()]
    counts = collections.Counter(d["group"] for d in docs)
    print(f"# tics corpus report\n\n{dict(counts)} documents.\n")
    rule_table(docs, rules)
    mine(docs)
    with open(ROOT / "doc_hits.json", "w") as out:
        json.dump([{"id": d["id"], "model": d["model"], "group": d["group"], "kind": d["kind"],
                    "words": d["words"], "hits": d["hits"],
                    "structure": structure(d["text"])} for d in docs], out)


if __name__ == "__main__":
    main()
