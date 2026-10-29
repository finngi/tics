# Calibration

Scripts that build the corpus behind tics' confidence values and fit the weights. They need
Python 3.12, `uv`, `pandoc`, network access, and the `claude` and `codex` CLIs signed in.

```sh
cd calibration
uv run --with beautifulsoup4 python fetch_human.py   # human/ and prompts.jsonl
python3 generate.py --jobs 4                          # ai/<model>/; resumable
python3 analyze.py --tics ../target/release/tics > report.md
uv run --with numpy --with scipy python fit.py --tics ../target/release/tics > fit-report.md
```

`fit.py` writes `weights.json`. Copy its values into `src/score.rs`; the
`constants_match_the_calibration_output` test checks them.

`report.md` (hit rates per rule, group and model, plus phrases models overuse) and
`fit-report.md` (cross-validation and final weights) are the outputs from the 2026-10-29 run.
The corpus itself is left out of the repository: the human text is CC BY-SA from Stack
Exchange and Wikipedia, and `prompts.jsonl` keeps each source URL for attribution.
