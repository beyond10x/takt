---
format: aep.planning-md/3
id: approval-record:codex-0-160-0-token-cap
kind: approval-record
status: approved
title: 'Codex 0.160.0 token cap of the stock-install trial: 85554, the compared saved maximum moved by +466'
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T18:24:20Z", actor: "human:timo", revision: 3}
---
# Approval: the Codex 0.160.0 token cap of the stock-install trial

The trial agent files nothing to this store (`AGENTS.md` § Roles), so the dispatcher records the
decision and the cap it sets.

## Decision

The orchestrator, under the owner's delegation (2026-10-23), kept the Codex 0.160.0 pin (option
A) and asked the dispatcher to "rebase the stock-install trial's Codex 0.160.0 token cap … Set
the cap by the rule your trial uses for other versions". This follows
approval-record:codex-0-160-0-trial, whose x102 and x103 sets on 0.160.0 ended over the 0.153.4
cap.

## Basis

A request-body probe of Codex 0.153.4 against 0.160.0 on one model, gpt-6-astra, gives these
per-run token medians from the trial rollouts of cycle 2026-10-23-54b8dd83 (set x101, 5 runs on
0.153.4, against set x103, 2 runs on 0.160.0):

| step | model calls | input delta | output delta |
|---|---|---|---|
| tests.write | 2 | +197 | −2 |
| implementation.edit | 2 | +183 | −11 |
| change.handover | 1 | +86 | +4 |
| run | 5 | +466 | about −9 |

+466 on a run of 84989 tokens (x101's median) is +0.55%. About +93 tokens per call are not
attributed; the probe infers they are content Codex fetches over the network.

## The rule and the cap

The trial's rule (`.agents/trial-agent.md` § Budgets per run) caps each run at the saved maximum,
for that scenario, of the scorecard the set is compared with. For the Codex arm that scorecard is
mgiv, the Codex set at f4f8d8d2. Its five scenario 1 rows are 84976, 85057, 85088, 84266 and 84984
tokens, so its saved maximum is 85088.

That scorecard was recorded on Codex 0.153.4. Its rows are moved to 0.160.0 by the measured
per-run cost of the version, +466, and the rule is applied to the moved rows:

| arm | compared scorecard | saved maximum | version shift | cap |
|---|---|---|---|---|
| Codex 0.153.4 | mgiv (f4f8d8d2 Codex) | 85088 | 0 | 85088 (unchanged) |
| Codex 0.160.0 | mgiv moved to 0.160.0 | 85554 | +466 | **85554** |

The Claude arm keeps 49030, the saved maximum of the f4f8d8d2 promotion.

## What the dispatcher decided under it

- The shift is the run-level median delta (+466), not the per-step deltas added to each step of
  the baseline. Both give +466 for the run, and only the run total is capped.
- The shift goes to the saved maximum, not to the median, so a 0.160.0 run gets the same 0.12%
  margin over the moved median that a 0.153.4 run gets over mgiv's.
- On 0.160.0 a takt verdict is compared against mgiv moved by +466 until a promotion is recorded
  on 0.160.0. From then on that promotion's saved maximum is the cap, with no shift.
- x103's two runs, 85462 and 85468, are under the new cap. They stopped `Tokens` under 85088 and
  are not rescored. The next Codex 0.160.0 set runs under `--tokens 85554`.
