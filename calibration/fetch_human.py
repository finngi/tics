"""Fetch the pre-ChatGPT human baseline: Stack Exchange Q&A and Wikipedia lead sections.

Everything is dated before ChatGPT's release (2022-11-30) so the human side is not AI-assisted.
Writes prompts.jsonl and human/<id>.md. Stack Exchange and Wikipedia text is CC BY-SA; the
source URL of each item is kept in prompts.jsonl for attribution.
"""
import json
import urllib.error
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

from bs4 import BeautifulSoup

ROOT = Path(__file__).parent
UA = {"User-Agent": "tics-corpus/0.1 (+https://github.com/finngi/tics)"}
CUTOFF = 1669766400  # 2022-11-30T00:00:00Z
SE_SITES = ["stackoverflow", "superuser", "unix", "english", "cooking", "diy",
            "physics", "money", "travel", "workplace", "softwareengineering", "gardening"]
SE_TARGET = 75
WIKI_TOPICS = [
    "Photosynthesis", "Plate tectonics", "Inflation", "Vaccine", "Black hole", "Blockchain",
    "Compound interest", "Supply and demand", "Climate change", "Natural selection",
    "Public-key cryptography", "Neural network (machine learning)", "Placebo", "Jet stream",
    "Fermentation", "Opportunity cost", "General relativity", "Antibiotic resistance",
    "Hash table", "Version control", "Garbage collection (computer science)", "Mitochondrion",
    "Tide", "Greenhouse effect", "Sourdough", "Compost", "Index fund", "Mortgage loan",
    "Circadian rhythm", "Placebo-controlled study", "Confirmation bias", "Cognitive dissonance",
    "Stoicism", "Utilitarianism", "Magna Carta", "Industrial Revolution", "Printing press",
    "Silk Road", "Renaissance", "Cold War", "French Revolution", "Roman Republic",
    "Domain Name System", "HTTP cookie", "Transmission Control Protocol", "Virtual machine",
    "Docker (software)", "Kubernetes", "Recursion (computer science)", "Big O notation",
    "Regular expression", "Unicode", "Relational database", "Cache (computing)",
    "Load balancing (computing)", "Two-factor authentication", "Phishing", "Ransomware",
    "Open-source software", "Agile software development", "Technical debt",
    "Minimum wage", "Universal basic income", "Gross domestic product", "Central bank",
    "Electric vehicle", "Lithium-ion battery", "Solar cell", "Wind turbine", "Nuclear fission",
    "Desalination", "Coral reef", "Honey bee", "Sleep deprivation", "Intermittent fasting",
]


CACHE = ROOT / ".cache"


def get(url, params, compressed=False):
    """GET JSON with an on-disk cache; honours Stack Exchange's `backoff` and throttle errors."""
    import gzip
    import hashlib
    query = urllib.parse.urlencode(params)
    key = CACHE / (hashlib.sha256(f"{url}?{query}".encode()).hexdigest() + ".json")
    if key.exists():
        return json.loads(key.read_text())
    for attempt in range(6):
        request = urllib.request.Request(f"{url}?{query}", headers=UA)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read()
        except urllib.error.HTTPError as error:
            raw = error.read()
        if raw[:2] == b"\x1f\x8b":
            raw = gzip.decompress(raw)
        data = json.loads(raw)
        if data.get("error_name") == "throttle_violation":
            wait = 30 * (attempt + 1)
            print(f"throttled, waiting {wait}s", file=sys.stderr)
            time.sleep(wait)
            continue
        if "error_id" in data:
            raise RuntimeError(f"{url}: {data}")
        CACHE.mkdir(exist_ok=True)
        key.write_text(json.dumps(data))
        time.sleep(data.get("backoff", 0) + 0.5)
        return data
    raise RuntimeError(f"{url}: still throttled")


def html_to_md(html):
    result = subprocess.run(["pandoc", "-f", "html", "-t", "gfm-raw_html", "--wrap=none"],
                            input=html, capture_output=True, text=True, check=True)
    return result.stdout.strip()


def words(text):
    return len(text.split())


def stack_exchange():
    items = []
    per_site = -(-SE_TARGET // len(SE_SITES))
    for site in SE_SITES:
        questions = get("https://api.stackexchange.com/2.3/questions", {
            "site": site, "sort": "votes", "order": "desc", "todate": CUTOFF - 86400 * 30,
            "pagesize": 60, "filter": "withbody"}, compressed=True)["items"]
        ids = ";".join(str(q["question_id"]) for q in questions)
        answers = get(f"https://api.stackexchange.com/2.3/questions/{ids}/answers", {
            "site": site, "sort": "votes", "order": "desc", "pagesize": 100,
            "filter": "withbody"}, compressed=True)["items"]
        best = {}
        for answer in answers:
            edited = answer.get("last_edit_date", answer["creation_date"])
            if answer["creation_date"] >= CUTOFF or edited >= CUTOFF:
                continue
            body = html_to_md(answer["body"])
            if not 80 <= words(body) <= 700:
                continue
            qid = answer["question_id"]
            if qid not in best or answer["score"] > best[qid][0]["score"]:
                best[qid] = (answer, body)
        taken = 0
        for question in questions:
            if taken == per_site or len(items) == SE_TARGET:
                break
            if question["question_id"] not in best:
                continue
            answer, body = best[question["question_id"]]
            prompt_body = html_to_md(question["body"])
            if words(prompt_body) > 400:
                continue
            title = BeautifulSoup(question["title"], "html.parser").get_text()
            items.append({
                "id": f"se-{site}-{question['question_id']}", "kind": "qa", "site": site,
                "prompt": f"{title}\n\n{prompt_body}",
                "source": f"https://{site}.stackexchange.com/a/{answer['answer_id']}"
                if site not in ("stackoverflow", "superuser") else
                f"https://{site}.com/a/{answer['answer_id']}",
                "human": body})
            taken += 1
        print(f"{site}: {taken}", file=sys.stderr)
        time.sleep(1)
    return items


def wikipedia():
    items = []
    api = "https://en.wikipedia.org/w/api.php"
    for topic in WIKI_TOPICS:
        pages = get(api, {"action": "query", "prop": "revisions", "titles": topic,
                          "rvprop": "ids|timestamp", "rvlimit": 1, "redirects": 1,
                          "rvstart": "2022-11-01T00:00:00Z", "rvdir": "older",
                          "format": "json"})["query"]["pages"]
        page = next(iter(pages.values()))
        if "revisions" not in page:
            print(f"skip {topic}: no revision", file=sys.stderr)
            continue
        revid = page["revisions"][0]["revid"]
        html = get(api, {"action": "parse", "oldid": revid, "section": 0, "prop": "text",
                         "format": "json"})["parse"]["text"]["*"]
        soup = BeautifulSoup(html, "html.parser")
        for junk in soup.select("table, sup.reference, .hatnote, .mw-empty-elt, style, "
                                ".shortdescription, .noprint, .mw-editsection, figure, "
                                ".thumb, .navbox, .ambox, .metadata, span.IPA, .rt-commentedText"):
            junk.decompose()
        paragraphs = [p.get_text(" ", strip=True) for p in soup.find_all("p")]
        text = "\n\n".join(p for p in paragraphs if words(p) > 8)
        text = " ".join(text.split(" ")).replace(" ,", ",").replace(" .", ".").replace("( ", "(").replace(" )", ")")
        if words(text) < 80:
            print(f"skip {topic}: lead too short ({words(text)})", file=sys.stderr)
            continue
        slug = page["title"].lower().replace(" ", "-").replace("(", "").replace(")", "")
        items.append({
            "id": f"wiki-{slug}", "kind": "explain", "site": "wikipedia",
            "prompt": f"Write a short explanation of {page['title']} for a general reader.",
            "source": f"https://en.wikipedia.org/w/index.php?oldid={revid}",
            "human": text})
        time.sleep(0.3)
    return items


def main():
    items = stack_exchange() + wikipedia()
    (ROOT / "human").mkdir(exist_ok=True)
    with open(ROOT / "prompts.jsonl", "w") as out:
        for item in items:
            (ROOT / "human" / f"{item['id']}.md").write_text(item.pop("human") + "\n")
            out.write(json.dumps(item) + "\n")
    print(f"{len(items)} prompts", file=sys.stderr)


if __name__ == "__main__":
    main()
