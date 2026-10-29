"""Answer every prompt with two Codex models and one Claude model, rotating through each range.

Resumable: an existing ai/<model>/<id>.md is skipped. Failures go to failures.log.
usage: python generate.py [--limit N] [--jobs N]
"""
import argparse
import json
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

ROOT = Path(__file__).parent
CODEX = "/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex"
CODEX_MODELS = ["gpt-6.1-sol", "gpt-6.1-terra", "gpt-6-astra", "gpt-6-sol", "gpt-6-luna",
                "gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]
CLAUDE_MODELS = ["opus", "sonnet", "haiku"]
PREFIX = "Do not run commands or read files; just answer.\n\n"
CLAUDE_SYSTEM = "You are Claude, a helpful AI assistant made by Anthropic."
TIMEOUT = 300


def assignments(prompts):
    for index, prompt in enumerate(prompts):
        yield "codex", CODEX_MODELS[(2 * index) % 8], prompt
        yield "codex", CODEX_MODELS[(2 * index + 1) % 8], prompt
        yield "claude", CLAUDE_MODELS[index % 3], prompt


def run(family, model, prompt, workdir):
    out = ROOT / "ai" / model / f"{prompt['id']}.md"
    if out.exists() and out.stat().st_size > 0:
        return model, prompt["id"], "skip"
    out.parent.mkdir(parents=True, exist_ok=True)
    text = PREFIX + prompt["prompt"]
    if family == "codex":
        command = [CODEX, "exec", "-m", model, "--skip-git-repo-check", "--ephemeral",
                   "-s", "read-only", "-C", workdir, "-o", str(out), "-"]
        result = subprocess.run(command, input=text, capture_output=True, text=True,
                                timeout=TIMEOUT)
    else:
        command = ["claude", "-p", "--model", model, "--no-session-persistence",
                   "--system-prompt", CLAUDE_SYSTEM, "--disallowedTools",
                   "Bash,Read,Write,Edit,Glob,Grep,WebFetch,WebSearch,Task"]
        result = subprocess.run(command, input=text, capture_output=True, text=True,
                                timeout=TIMEOUT, cwd=workdir)
        if result.returncode == 0:
            out.write_text(result.stdout)
    if result.returncode != 0 or not out.exists() or out.stat().st_size == 0:
        out.unlink(missing_ok=True)
        raise RuntimeError(f"exit {result.returncode}: {result.stderr.strip()[-300:]}")
    return model, prompt["id"], "ok"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--limit", type=int)
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    prompts = [json.loads(line) for line in open(ROOT / "prompts.jsonl")][: args.limit]
    work = list(assignments(prompts))
    done = failed = 0
    with tempfile.TemporaryDirectory() as workdir, ThreadPoolExecutor(args.jobs) as pool:
        futures = {pool.submit(run, *job, workdir): job for job in work}
        for future in as_completed(futures):
            family, model, prompt = futures[future]
            try:
                future.result()
                done += 1
            except Exception as error:  # noqa: BLE001 - logged and retried on the next run
                failed += 1
                with open(ROOT / "failures.log", "a") as log:
                    log.write(f"{model}\t{prompt['id']}\t{error}\n")
            if (done + failed) % 25 == 0:
                print(f"{done + failed}/{len(work)} ({failed} failed)", file=sys.stderr)
    print(f"done: {done} ok, {failed} failed of {len(work)}", file=sys.stderr)


if __name__ == "__main__":
    main()
