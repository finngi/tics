# tics corpus report

{'human': 146, 'codex': 292, 'claude': 146} documents.

## Rule hit rates

Docs with a hit (%), then hits per 1,000 words. LOR = log-odds AI vs human on docs with a hit (z in brackets); positive means more common in AI.

| rule | human | claude | codex | LOR (z) |
|---|---|---|---|---|
| stock-section-label | 0% / 0.0 | 33% / 1.6 | 0% / 0.0 | +3.62 (+2.5) |
| bold-lead-in | 1% / 0.1 | 48% / 6.9 | 7% / 1.7 | +3.21 (+3.9) |
| bold-label-line | 5% / 0.2 | 71% / 10.4 | 3% / 0.3 | +1.86 (+4.9) |
| x-not-y | 4% / 0.2 | 25% / 1.6 | 11% / 1.0 | +1.40 (+3.4) |
| em-dash | 10% / 0.7 | 0% / 0.0 | 36% / 4.6 | +0.99 (+3.6) |
| rather-than | 16% / 0.7 | 27% / 1.2 | 18% / 1.4 | +0.34 (+1.5) |
| ai-vocabulary | 1% / 0.1 | 3% / 0.1 | 1% / 0.1 | +0.25 (+0.3) |
| summary-opener | 1% / 0.0 | 3% / 0.1 | 0% / 0.0 | +0.21 (+0.2) |
| does-x-does-not-y | 1% / 0.0 | 1% / 0.1 | 1% / 0.1 | +0.00 (+0.0) |
| promotional | 0% / 0.0 | 1% / 0.0 | 0% / 0.0 | +0.00 (+0.0) |
| rule-of-three | 2% / 0.1 | 1% / 0.1 | 2% / 0.2 | -0.21 (-0.3) |
| vague-attribution | 1% / 0.0 | 1% / 0.0 | 0% / 0.0 | -0.59 (-0.6) |
| title-case-heading | 1% / 0.0 | 1% / 0.1 | 0% / 0.0 | -1.11 (-1.2) |
| not-x-but-y | 7% / 0.2 | 1% / 0.1 | 1% / 0.1 | -2.01 (-3.6) |
| not-x-its-y-split | 1% / 0.0 | 0% / 0.0 | 0% / 0.0 | -2.20 (-1.3) |
| canned-assurance | 1% / 0.0 | 0% / 0.0 | 0% / 0.0 | -2.20 (-1.3) |
| negative-parallelism | 1% / 0.0 | 0% / 0.0 | 0% / 0.0 | -2.72 (-1.8) |
| importance-hedge | 1% / 0.0 | 0% / 0.0 | 0% / 0.0 | -2.72 (-1.8) |
| copula-avoidance | 1% / 0.0 | 0% / 0.0 | 0% / 0.0 | -2.72 (-1.8) |
| transition-opener | 3% / 0.1 | 0% / 0.0 | 0% / 0.0 | -3.53 (-2.4) |

## Per model: docs with a hit (%)

| model | n | words | negative-parallelism | not-x-but-y | not-x-its-y-split | x-not-y | does-x-does-not-y | rather-than | importance-hedge | vague-attribution | summary-opener | transition-opener | rule-of-three | ai-vocabulary | promotional | copula-avoidance | canned-assurance | em-dash | bold-lead-in | bold-label-line | stock-section-label | title-case-heading |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| human | 146 | 301 | 1 | 7 | 1 | 4 | 1 | 16 | 1 | 1 | 1 | 3 | 2 | 1 | 0 | 1 | 1 | 10 | 1 | 5 | 0 | 1 |
| gpt-5.6-luna | 36 | 124 | 0 | 0 | 0 | 17 | 0 | 22 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 47 | 6 | 0 | 3 | 0 |
| gpt-5.6-sol | 36 | 158 | 0 | 3 | 0 | 17 | 0 | 14 | 0 | 3 | 0 | 0 | 3 | 3 | 0 | 0 | 0 | 67 | 11 | 0 | 0 | 0 |
| gpt-5.6-terra | 36 | 121 | 0 | 0 | 0 | 14 | 3 | 22 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 50 | 3 | 0 | 0 | 0 |
| gpt-6-astra | 37 | 176 | 0 | 3 | 0 | 8 | 3 | 22 | 0 | 0 | 0 | 0 | 5 | 3 | 0 | 0 | 0 | 49 | 5 | 8 | 0 | 0 |
| gpt-6-luna | 36 | 131 | 0 | 0 | 0 | 19 | 0 | 19 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 14 | 6 | 3 | 0 | 0 |
| gpt-6-sol | 37 | 100 | 0 | 0 | 0 | 5 | 0 | 11 | 0 | 0 | 0 | 0 | 3 | 3 | 0 | 0 | 0 | 14 | 5 | 0 | 0 | 0 |
| gpt-6.1-sol | 37 | 134 | 0 | 0 | 0 | 0 | 0 | 16 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 41 | 16 | 5 | 0 | 0 |
| gpt-6.1-terra | 37 | 128 | 0 | 0 | 0 | 11 | 0 | 19 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 8 | 0 | 5 | 0 | 0 |
| haiku | 48 | 203 | 0 | 2 | 0 | 25 | 2 | 27 | 0 | 0 | 2 | 0 | 2 | 0 | 2 | 0 | 0 | 0 | 38 | 38 | 12 | 0 |
| opus | 49 | 289 | 0 | 2 | 0 | 24 | 2 | 29 | 0 | 2 | 6 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 51 | 88 | 41 | 2 |
| sonnet | 49 | 257 | 0 | 0 | 0 | 24 | 0 | 24 | 0 | 0 | 2 | 0 | 0 | 10 | 0 | 0 | 0 | 0 | 55 | 88 | 45 | 2 |

## Structural features: docs with feature (%)

| feature | human | claude | codex | LOR (z) |
|---|---|---|---|---|
| bold_lead_items>=2 | 2 | 69 | 16 | +3.03 (+5.6) |
| bold_lead_share>=0.5 | 1 | 56 | 14 | +3.65 (+4.4) |
| bold_paragraph_label | 3 | 68 | 2 | +2.10 (+4.8) |
| bold_span_any | 23 | 92 | 46 | +1.67 (+9.1) |
| colon_intro_line | 25 | 30 | 37 | +0.43 (+2.4) |
| em_dash | 10 | 0 | 36 | +0.99 (+3.6) |
| emoji | 0 | 1 | 0 | +0.52 (+0.3) |
| en_dash_spaced | 5 | 0 | 0 | -3.99 (-2.7) |
| ends_with_offer | 1 | 2 | 1 | -0.31 (-0.4) |
| ends_with_question | 0 | 0 | 0 | +0.00 (+0.0) |
| has_heading | 7 | 10 | 1 | -0.50 (-1.3) |
| has_list | 18 | 86 | 29 | +1.43 (+6.9) |
| short_answer_first_line | 19 | 25 | 16 | -0.01 (-0.0) |
| tldr | 1 | 17 | 1 | +1.87 (+2.2) |

## Phrases most over-used by AI (doc frequency, ≥8 AI docs)

### 1-grams

| phrase | AI docs | human docs | z |
|---|---|---|---|
| keep | 104 | 7 | 5.0 |
| it | 375 | 112 | 4.3 |
| why | 82 | 8 | 4.3 |
| your | 183 | 45 | 3.9 |
| you | 225 | 61 | 3.9 |
| works | 59 | 5 | 3.7 |
| helps | 56 | 4 | 3.7 |
| so | 192 | 52 | 3.6 |
| usually | 95 | 18 | 3.6 |
| matters | 51 | 1 | 3.4 |
| check | 46 | 3 | 3.4 |
| the | 419 | 145 | 3.3 |
| a | 406 | 140 | 3.3 |
| can | 269 | 85 | 3.3 |
| useful | 55 | 7 | 3.3 |
| someone | 54 | 7 | 3.2 |
| how | 110 | 26 | 3.2 |
| don’t | 39 | 2 | 3.1 |
| ordinary | 38 | 2 | 3.1 |
| makes | 47 | 6 | 3.0 |
| roughly | 50 | 7 | 3.0 |
| every | 64 | 12 | 2.9 |
| and | 394 | 141 | 2.9 |
| ask | 44 | 6 | 2.9 |
| add | 53 | 9 | 2.8 |
| short | 59 | 11 | 2.8 |
| faster | 35 | 4 | 2.7 |
| happens | 31 | 3 | 2.6 |
| move | 37 | 5 | 2.6 |
| turns | 28 | 2 | 2.6 |
| keeps | 28 | 2 | 2.6 |
| needs | 39 | 6 | 2.6 |
| isn't | 27 | 2 | 2.5 |
| run | 57 | 12 | 2.5 |
| rule | 29 | 3 | 2.5 |
| still | 79 | 20 | 2.5 |
| travel | 26 | 2 | 2.5 |
| make | 103 | 29 | 2.5 |
| getting | 24 | 1 | 2.4 |
| shared | 25 | 2 | 2.4 |

### 2-grams

| phrase | AI docs | human docs | z |
|---|---|---|---|
| so the | 54 | 4 | 3.9 |
| it can | 59 | 9 | 3.5 |
| rather than | 64 | 11 | 3.5 |
| a few | 34 | 3 | 3.0 |
| if the | 59 | 13 | 2.9 |
| can make | 28 | 2 | 2.8 |
| keep the | 27 | 1 | 2.7 |
| a small | 45 | 9 | 2.7 |
| when you | 36 | 6 | 2.7 |
| over time | 37 | 7 | 2.5 |
| but it | 37 | 7 | 2.5 |
| for a | 60 | 16 | 2.5 |
| can help | 22 | 1 | 2.5 |
| a real | 22 | 1 | 2.5 |
| it works | 22 | 2 | 2.4 |
| not a | 24 | 3 | 2.4 |
| it and | 24 | 3 | 2.4 |
| use a | 29 | 5 | 2.4 |
| it for | 21 | 2 | 2.4 |
| a year | 19 | 1 | 2.3 |
| it also | 22 | 3 | 2.2 |
| why it | 31 | 0 | 2.2 |
| for example | 65 | 20 | 2.2 |
| with no | 17 | 1 | 2.2 |
| or a | 40 | 10 | 2.2 |
| into a | 29 | 6 | 2.1 |
| and your | 16 | 1 | 2.1 |
| a useful | 16 | 1 | 2.1 |
| you a | 15 | 1 | 2.1 |
| the usual | 15 | 1 | 2.1 |
| add a | 15 | 1 | 2.1 |
| how much | 17 | 2 | 2.0 |
| it matters | 26 | 0 | 2.0 |
| to keep | 25 | 5 | 2.0 |
| you can | 78 | 27 | 2.0 |
| and a | 57 | 18 | 2.0 |
| such as | 115 | 44 | 2.0 |
| a short | 19 | 3 | 2.0 |
| gives you | 14 | 1 | 2.0 |
| easier to | 14 | 1 | 2.0 |

### 3-grams

| phrase | AI docs | human docs | z |
|---|---|---|---|
| why it matters | 24 | 0 | 2.0 |
| such as a | 15 | 2 | 1.9 |
| much of the | 11 | 1 | 1.8 |
| how it works | 15 | 0 | 1.6 |
| it does not | 11 | 2 | 1.5 |
| is a way | 11 | 0 | 1.3 |
| this is called | 9 | 0 | 1.2 |
| the same way | 9 | 0 | 1.2 |
| so you can | 8 | 2 | 1.0 |
| in the same | 8 | 2 | 1.0 |
| out of the | 9 | 3 | 0.8 |
| a way to | 8 | 3 | 0.6 |
| this is the | 10 | 4 | 0.6 |
| you want to | 15 | 7 | 0.4 |
| a lot of | 10 | 5 | 0.2 |
| most of the | 9 | 5 | 0.0 |
| part of the | 10 | 6 | -0.1 |
| if you want | 12 | 8 | -0.3 |
| it is not | 8 | 6 | -0.5 |
| it is the | 9 | 7 | -0.6 |
| the united states | 8 | 8 | -1.1 |
| this is a | 9 | 9 | -1.2 |
| such as the | 12 | 13 | -1.6 |
| as well as | 8 | 15 | -2.7 |

