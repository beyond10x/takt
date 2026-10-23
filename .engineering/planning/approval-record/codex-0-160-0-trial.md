---
format: aep.planning-md/3
id: approval-record:codex-0-160-0-trial
kind: approval-record
status: approved
title: 'Codex 0.160.0 on the stock-install trial: worse at the token cap, the trial goes back to 0.153.4'
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T17:41:07Z", actor: "human:timo", revision: 3}
---
# Approval: Codex 0.160.0 on the stock-install trial

The trial agent files nothing to this store (`AGENTS.md` § Roles), so the dispatcher records the
decision it ran under.

## Decision

The orchestrator, under the owner's delegation (2026-10-23), chose A: point the user-local `codex`,
which metaharness puts first on the child's PATH, at Codex 0.160.0 and rerun the Codex arm of the
stock-install trial of takt 0.10.1 on it.

The stock-install trial before it ran its Codex arm on 0.153.4: metaharness builds the child's
PATH with the user-local bin directory first, and the `codex` there was 0.153.4, so every
transcript recorded `cli_version 0.153.4` although 0.160.0 was first on the session's PATH.

## Result

Trial cycle 2026-10-23-54b8dd83, scenario 1, cap 85088 tokens (the compared baseline's saved
maximum). Every transcript of both sets records `cli_version 0.160.0`.

| set | model | runs | pass | tokens | verdict |
|---|---|---|---|---|---|
| x101 (before, Codex 0.153.4) | gpt-6-astra | 5 | 4/5 | 84292 / 84989 / 85120 (min / median / max) | not worse than the promoted Codex baseline |
| x102 | gpt-6.1-sol (the 0.160.0 default) | 2 (early stop) | 0/2 | 85944, 85794 | worse: both over the cap |
| x103 | gpt-6-astra | 2 (early stop) | 0/2 | 85462, 85468 | worse: both over the cap |

On the same model, x103 spends 473 and 479 tokens a run more than x101's median, about +200 in
`tests.write`, +170 in `implementation.edit` and +90 in `change.handover`.

## What follows from it

- After the rerun the user-local `codex` points at Codex 0.153.4 again, so the trial's Codex arm
  runs on 0.153.4.
- takt and its pins do not change: metaharness 0.15.0 keeps both 0.153.4 and 0.160.0 pinned.
- On Codex 0.160.0 every run of x102 and x103 ended over the current Codex cap.
