# tics confidence calibration

146 human and 438 AI files (0 without prose skipped), 30 rules. Outer 5-fold and inner 4-fold CV, both grouped by prompt. Balanced metrics; 95% prompt-cluster bootstrap intervals; thresholds on scores rounded as the CLI prints them.

| model | AUC | balanced log-loss | human ≥ 0.8 | AI ≥ 0.8 |
|---|---|---|---|---|
| tics | 0.808 (0.770-0.846) | 0.544 (0.508-0.578) | 3% | 30% |
| length and structure only (no rules) | 0.871 (0.830-0.910) | 0.463 (0.400-0.538) | 7% | 39% |
| tics, Stack Exchange prompts only | 0.795 (0.742-0.846) | 0.538 (0.483-0.592) | 3% | 32% |
| length and structure, Stack Exchange only | 0.744 (0.676-0.815) | 0.651 (0.539-0.771) | 13% | 35% |

Inner CV chose (EPS, delta sd) per outer fold: [(0.02, 2.0), (0.02, 2.0), (0.02, 2.0), (0.02, 2.0), (0.02, 2.0)].

### tics held-out scores by group

| group | n | score ≥ 0.5 | score ≥ 0.8 |
|---|---|---|---|
| human | 146 | 16% | 3% |
| all Claude | 146 | 87% | 70% |
| all Codex | 292 | 46% | 10% |
| Stack Exchange human | 75 | 13% | 3% |
| Stack Exchange AI | 225 | 65% | 32% |
| Wikipedia human | 71 | 20% | 3% |
| Wikipedia AI | 213 | 54% | 28% |
| gpt-5.6-luna | 36 | 56% | 6% |
| gpt-5.6-sol | 36 | 72% | 8% |
| gpt-5.6-terra | 36 | 53% | 8% |
| gpt-6-astra | 37 | 49% | 14% |
| gpt-6-luna | 36 | 39% | 8% |
| gpt-6-sol | 37 | 27% | 11% |
| gpt-6.1-sol | 37 | 41% | 19% |
| gpt-6.1-terra | 37 | 30% | 5% |
| haiku | 48 | 69% | 58% |
| opus | 49 | 96% | 71% |
| sonnet | 49 | 96% | 80% |

### Final model

EPS 0.02, delta sd 2, SCALE 1.0510, BIAS 0.0487 (calibrated on out-of-fold evidence).

Strength is the signed Jeffreys divergence between the AI and human distributions of occupied opportunities out of ten (negative: people use it more). The evidence columns are the log-likelihood ratio for one and five occupied opportunities out of ten.

| rule | unit | strength | 1 of 10 | 5 of 10 | human rate | AI rate | files with a hit (human / AI) |
|---|---|---|---|---|---|---|---|
| bold-lead-in | list_items | +1.03 | +1.15 | +1.59 | 0.0275 | 0.2967 | 1 / 89 |
| bold-label-line | blocks | +0.74 | +1.25 | +2.72 | 0.0114 | 0.0968 | 7 / 112 |
| stock-section-label | blocks | +0.42 | +2.61 | +2.91 | 0.0009 | 0.0195 | 0 / 49 |
| x-not-y | sentences | +0.16 | +1.38 | +1.62 | 0.0036 | 0.0163 | 6 / 69 |
| em-dash | sentences | +0.14 | +0.88 | +1.23 | 0.0102 | 0.0285 | 15 / 105 |
| title-case-heading | headings | +0.03 | +0.15 | +0.54 | 0.0446 | 0.0589 | 2 / 2 |
| rather-than | sentences | +0.02 | +0.32 | +0.36 | 0.0129 | 0.0182 | 23 / 92 |
| ai-vocabulary | sentences | +0.00 | +0.58 | +0.58 | 0.0011 | 0.0019 | 2 / 9 |
| summary-opener | sentences | +0.00 | +0.70 | +0.70 | 0.0006 | 0.0011 | 1 / 5 |
| does-x-does-not-y | sentences | +0.00 | +0.55 | +0.55 | 0.0005 | 0.0009 | 1 / 4 |
| promotional | sentences | +0.00 | +1.10 | +1.10 | 0.0001 | 0.0003 | 0 / 1 |
| rule-of-three | sentences | +0.00 | +0.15 | +0.15 | 0.0016 | 0.0018 | 3 / 8 |
| leaked-markup | blocks | +0.00 | +0.49 | +0.51 | 0.0000 | 0.0001 | 0 / 0 |
| vague-attribution | sentences | +0.00 | +0.13 | +0.13 | 0.0006 | 0.0006 | 1 / 2 |
| whether-youre | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| trailing-ing-analysis | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| todays-world | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| signposting | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| significance-puffing | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| rhetorical-reveal | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| placeholder | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| not-sentence-start | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| chatbot-residue | sentences | +0.00 | +0.53 | +0.55 | 0.0000 | 0.0000 | 0 / 0 |
| not-x-its-y-split | sentences | -0.00 | -0.87 | -0.87 | 0.0003 | 0.0001 | 1 / 0 |
| canned-assurance | sentences | -0.00 | -0.88 | -0.88 | 0.0003 | 0.0001 | 1 / 0 |
| importance-hedge | sentences | -0.01 | -1.50 | -1.50 | 0.0008 | 0.0002 | 2 / 0 |
| negative-parallelism | sentences | -0.01 | -1.50 | -1.50 | 0.0008 | 0.0002 | 2 / 0 |
| copula-avoidance | sentences | -0.01 | -1.50 | -1.50 | 0.0009 | 0.0002 | 2 / 0 |
| transition-opener | sentences | -0.04 | -2.24 | -2.26 | 0.0023 | 0.0002 | 5 / 0 |
| not-x-but-y | sentences | -0.05 | -1.49 | -2.15 | 0.0048 | 0.0009 | 10 / 4 |
