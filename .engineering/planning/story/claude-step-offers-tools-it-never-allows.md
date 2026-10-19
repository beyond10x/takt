---
format: aep.planning-md/3
id: story:claude-step-offers-tools-it-never-allows
kind: story
status: active
title: Each Claude step is offered 20 built-in tools where it may use 5; the 15 others cost 7,004 tokens a request
summary: '--tools beside --allowedTools: first three Claude steps 122k -> 66k tokens (-45.7%)'
tags:
- finding
refs:
- provider: finding
  reference: claude-step-offers-tools-it-never-allows
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T16:18:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T16:18:10Z", actor: "human:timo", revision: 3}
---
# Story: claude-step-offers-tools-it-never-allows

## Outcome

The `claude` child of a judgement step is offered only the file tools the step may use: its argv
names `--tools Read,Edit,Write,Glob,Grep` beside `--allowedTools`. The first request of a step on
scenario 1 drops from 12,554 prompt tokens to 5,550, and every later request of the step by the
same 7,004.

## Context

Found by the RSI token investigation of 2026-10-19 into the promotion judge's refusal of takt 0.2.2
(f0728964) against takt 0.1.0 (44f96a48) on scenario 1 (median tokens 229,309 against a saved
maximum of 122,058).

That refusal is not a prompt regression. A replay of scenario 1 with both binaries, each calling a
recording `claude` that answers from a fixed file state, gives byte-identical step prompts (1,934,
1,969 and 1,934 bytes for steps 3, 6 and 9), argv (313 bytes), environment names and `git status`.
Ten interleaved live runs, five a binary, give a median of 122,355 tokens over the first three
Claude steps for 44f96a48 and 122,415 for f0728964. The spread is the model's: a step that makes
one more API request than usual (the docstring edit sent in a response of its own, a `Glob`
before a `Read`) re-reads the whole context and costs about 16.7k tokens more. That happened in
15 of 55 `implementation.edit` steps and 2 of 55 `tests.write` steps over nine takt revisions.
The saved baseline drew 0 of 5 and the candidate 3 of 5.

What takt controls is the size of each request. `ClaudeCode::step` (`crates/takt/src/harness.rs`,
the `--allowedTools` argument) lists the tools the step may use without asking, but never which
tools exist. So Claude Code 2.1.303 offers all 20 of its built-in tools: CronDelete, CronList,
DesignSync, EnterWorktree, ExitWorktree, ListAgents, PublishPlugin, PushNotification,
ReportFindings, ScheduleWakeup, SendMessage, ShareOnboardingGuide, Skill, TaskStop and
ToolSearch besides the five file tools, and the Skill tool lists 19 skills. With the same
argv, one fixed prompt in the scenario seed, the first request carries 12,554 tokens; adding
`--tools Read,Edit,Write,Glob,Grep` brings it to 5,550. Scenario 1's first three Claude steps
make 8 requests, so that is about 56k of their 122k tokens. Each extra request then costs
7k less, which also narrows the spread that the judge's token rule reads.

The tools a step may not use are a security question too: a judgement step has no use for
SendMessage, PushNotification, ScheduleWakeup, EnterWorktree, PublishPlugin or Skill. In print
mode, `--allowedTools` leaves them offered but asking for permission (inferred: not run).

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && printf '#!/bin/sh\ncase " $* " in *" --tools "*) echo claude-argv-names-its-tools >&2 ;; *) echo claude-argv-offers-every-built-in-tool >&2 ;; esac\nexit 1\n' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 3
exit: 5
healthy_exit: 5
output: claude-argv-offers-every-built-in-tool
source: crates/takt/src/harness.rs
```

## Scope

- `crates/takt/src/harness.rs`: `ClaudeCode::step`, one `--tools Read,Edit,Write,Glob,Grep`
  beside `--allowedTools`; `MultiEdit` is not a tool of Claude Code 2.1.303 and stays out of
  `--tools` (inferred: an unknown name in `--tools` was not tried)
- `crates/takt/tests/validator_confinement.rs`: `the_harness_child_loads_no_project_or_user_settings`
  also asserts `--tools` and its value

## Acceptance

At the candidate, `scripts/check-finding story:claude-step-offers-tools-it-never-allows
<candidate>` prints `not_reproduced` and at the promoted revision `reproduced`; five scenario-1
trials of the candidate have a median token count over the first three Claude steps below the
promoted baseline's minimum.
