# tics confidence calibration

146 human and 438 AI documents, 30 rules, 5-fold CV grouped by prompt.

| k1 | b | CV log-loss |
|---|---|---|
| 0.5 | 0.0 | 0.6062 |
| 0.5 | 0.5 | 0.5947 |
| 0.5 | 0.75 | 0.5895 |
| 1.2 | 0.0 | 0.6065 |
| 1.2 | 0.5 | 0.5883 |
| 1.2 | 0.75 | 0.5802 |
| 2.0 | 0.0 | 0.6072 |
| 2.0 | 0.5 | 0.5852 |
| 2.0 | 0.75 | 0.5755 |

### Held-out scores (k1=2.0, b=0.75)

AUC 0.726

| group | n | mean score | flagged at 0.5 | flagged at 0.8 |
|---|---|---|---|---|
| all AI (explain) | 213 | 0.58 | 49% | 32% |
| all AI (qa) | 225 | 0.67 | 72% | 39% |
| all human (explain) | 71 | 0.39 | 18% | 1% |
| all human (qa) | 75 | 0.38 | 17% | 4% |
| gpt-5.6-luna | 36 | 0.57 | 56% | 17% |
| gpt-5.6-sol | 36 | 0.66 | 78% | 22% |
| gpt-5.6-terra | 36 | 0.56 | 56% | 22% |
| gpt-6-astra | 37 | 0.55 | 57% | 19% |
| gpt-6-luna | 36 | 0.43 | 33% | 8% |
| gpt-6-sol | 37 | 0.41 | 22% | 11% |
| gpt-6.1-sol | 37 | 0.55 | 49% | 22% |
| gpt-6.1-terra | 37 | 0.40 | 24% | 0% |
| haiku | 48 | 0.71 | 73% | 52% |
| human | 146 | 0.38 | 18% | 3% |
| opus | 49 | 0.90 | 96% | 86% |
| sonnet | 49 | 0.92 | 96% | 90% |

### Final weights (trained on all documents)

bias -0.896; a document with no findings scores 0.29

| rule | weight | AI docs | human docs | one hit in an average doc |
|---|---|---|---|---|
| bold-lead-in | 1.54 | 89 | 1 | 0.65 |
| stock-section-label | 1.32 | 49 | 0 | 0.61 |
| bold-label-line | 1.28 | 112 | 7 | 0.59 |
| em-dash | 1.12 | 105 | 15 | 0.56 |
| ai-vocabulary | 1.05 | 9 | 2 | 0.54 |
| x-not-y | 1.04 | 69 | 6 | 0.53 |
| promotional | 1.02 | 1 | 0 | 0.53 |
| not-sentence-start | 1.00 | 0 | 0 | 0.53 |
| trailing-ing-analysis | 1.00 | 0 | 0 | 0.53 |
| significance-puffing | 1.00 | 0 | 0 | 0.53 |
| signposting | 1.00 | 0 | 0 | 0.53 |
| rhetorical-reveal | 1.00 | 0 | 0 | 0.53 |
| whether-youre | 1.00 | 0 | 0 | 0.53 |
| todays-world | 1.00 | 0 | 0 | 0.53 |
| chatbot-residue | 1.00 | 0 | 0 | 0.53 |
| placeholder | 1.00 | 0 | 0 | 0.53 |
| leaked-markup | 1.00 | 0 | 0 | 0.53 |
| summary-opener | 0.98 | 5 | 1 | 0.52 |
| does-x-does-not-y | 0.89 | 4 | 1 | 0.50 |
| canned-assurance | 0.71 | 0 | 1 | 0.45 |
| rather-than | 0.60 | 92 | 23 | 0.43 |
| rule-of-three | 0.54 | 8 | 3 | 0.41 |
| importance-hedge | 0.52 | 0 | 2 | 0.41 |
| vague-attribution | 0.51 | 2 | 1 | 0.40 |
| not-x-its-y-split | 0.32 | 0 | 1 | 0.36 |
| copula-avoidance | 0.23 | 0 | 2 | 0.34 |
| title-case-heading | 0.23 | 2 | 2 | 0.34 |
| negative-parallelism | 0.10 | 0 | 2 | 0.31 |
| not-x-but-y | 0.00 | 4 | 10 | 0.29 |
| transition-opener | 0.00 | 0 | 5 | 0.29 |
