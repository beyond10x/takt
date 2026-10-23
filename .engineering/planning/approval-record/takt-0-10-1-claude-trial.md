---
format: aep.planning-md/3
id: approval-record:takt-0-10-1-claude-trial
kind: approval-record
status: approved
title: 'takt 0.10.1 on the stock-install trial: the Claude arm (c101) is not worse than the f4f8d8d2 promotion'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T22:00:13Z", actor: "human:timo", revision: 2}
---
# Record: takt 0.10.1 on the stock-install trial, the Claude arm rerun

The trial agent files nothing to this store (`AGENTS.md` § Roles), so the dispatcher records the
result on the orchestrator's request of 2026-10-23, for the ADR 0101 audit to cite.

## Result

Trial cycle 2026-10-23-54b8dd83, scenario 1, the Claude arm rerun on the stock install of takt
0.10.1: cap 49030 tokens (the compared baseline's saved maximum), `--iterations 60`.

| set | harness | model | runs | pass | tokens min / median / max | against | verdict |
|---|---|---|---|---|---|---|---|
| c101 | claude 2.1.309 | claude-opus-5-5 | 5 | 4/5 | 48493 / 48594 / 49053 | the f4f8d8d2 promotion | not worse (exit 0), median −343 |
| c101 | | | | | | the 7ae5b05b series start | not worse (exit 0), median −17827 |

The one failed run, c101-2, stopped `Tokens` at 49053, 23 over the cap, after the third model call
and before `pr.open`.

## What follows from it

- takt 0.10.1 on Claude Code 2.1.309 is not worse than the f4f8d8d2 promotion on scenario 1.
- The Codex arm of the same cycle is approval-record:codex-0-160-0-trial (x101).
