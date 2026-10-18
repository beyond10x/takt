---
format: aep.planning-md/3
id: review-result:variety-ops-a29d8c7
kind: review-result
status: active
title: 'Reviewer: operator-path variety round, takt a29d8c7'
relations:
- reviews: story:forge-none-merges-locally
prose_only: a trial report with run figures and diagnosed defects, not a plan critic verdict
revision: 1
---
Operator-path variety round by the reviewer role, 2026-10-19, takt built at a29d8c7 (`--version` names it). Scenario 1 copies at seed 865eaa8 with no origin, `--forge none`, `--test-cmd "sh check.sh"`; "fake" runs use a fake `claude` reporting 1,100 tokens a step; R1 and R2 used the real model.

| run | what | exit | stop line | steps | tokens | wall s |
|---|---|---|---|---|---|---|
| A | story, `--harness none` | 5 | error: no harness for the judgement step | 3 | 0 | 1.05 |
| B | story, fake, then resumed | 3, 3 | NoProgress (the fake picks `pr.open`) | 9, 1 | 3,300, 1,100 | 1.71, 0.15 |
| C | intent, fake; approve; resume twice | 4, 3, 3 | ApprovalRequired, then NoProgress twice; `repository.merge` as a judgement step changed no file; `main` unmoved | 6, 1, 1 | 3,300, 1,100, 1,100 | 0.93, 0.09, 0.08 |
| E, F | tracked edit / untracked files at open | 1, 1 | refused: uncommitted work | 0 | 0 | 0.12, 0.24 |
| G | edits before `tests.run` on resume | 3 | StepRefused: would observe with uncommitted changes | 1 | 0 | 0.02 |
| H | base whose own tests fail, fake | 3 | NoProgress; every test and gate run failed | 9 | 3,300 | 1.69 |
| I | 3 intent cases and 1 story case on one tree, fake | 4, 4, 4, 3 | separate branches from `main`; `takt scorecard` rows match the logs | 6–9 | 3,300 each | 0.7–1.9 |
| J | story:one-run-per-tree predicate | 5, 3 | "HEAD names no commit: refs/heads/takt/b" (still holds) | 1 | 0 | ~4 |
| K | `--iterations 1`; `--hours` 0, -1, nan, inf; `--hours 1e-9`, `--iterations 0`, `--tokens 0` | 3; 1; 3 | Iterations (1); refused; Hours / Iterations (0) / Tokens (0) with the story taken | 1; 0; 0 | 0 | ≤1.45 |
| L, M | `--hours 0.0005` with a 6 s step / a 6 s test command | 3, 3 | Hours after 6.65 s / 6.32 s (filed: hours-bounds-a-harness-step, hours-bounds-a-validator-step) | 3, 1 | 1,100, 0 | 6.65, 6.32 |
| R1 | real model, intent, `--hours 0.5` | 4 | ApprovalRequired (repository.merge), 2 commits; approval offered before any commit (filed: no-merge-approval-without-a-change) | 6 | 148,573 | 47.8 |
| R2 | real model, story, `--hours 0.5` | 3 | NoProgress: `pr.open` on the unchanged base (filed: forge-none-merges-locally) | 3 | 13,504 | 7.8 |

Findings filed from this round: story:forge-none-merges-locally (medium), story:takt-lists-a-trees-cases (low), story:harness-sees-what-failed (medium), story:a-budget-admits-a-step (low). Not run: kill and resume (declined).
