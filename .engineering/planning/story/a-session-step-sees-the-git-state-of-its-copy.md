---
format: aep.planning-md/3
id: story:a-session-step-sees-the-git-state-of-its-copy
kind: story
status: implemented
title: A judgement step in the session's scratch copy sees the repository state it saw in the case tree, so tests.write stops exploring
summary: 'Trial at main 6ad32d3c: scenario 1 median 82,034 tokens vs 67,331 at 0.6.0, all in tests.write (Glob **/* first, CLAUDE.md and AGENTS.md read); the session copy is not a git repository'
tags:
- finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T19:22:57Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T19:22:57Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T19:22:58Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a-session-step-sees-the-git-state-of-its-copy

## Outcome

A judgement step that runs in the metaharness session's scratch copy starts with the same
orientation the print-mode step had in the case tree: the copy is a git repository at the case's
`HEAD` (or the session is told the repository's state some other way the vendor reads), so
Claude Code puts its git snapshot in the step's context, and `tests.write` on scenario 1 goes back
to reading the two files it changes instead of exploring the tree first. The run's median tokens
on scenario 1 return to the 0.6.0 level, or the difference is explained by a measured cause.

## Context

Found by the trial agent at main 6ad32d3c (wave 23, which made `--harness claude` run Claude Code
through a metaharness session in a scratch copy, story:harness-proposes-takt-performs):

- Scenario 1 base set at 6ad32d3c (trial/129-133): 5 of 5 exit 4, held-out PASS, 13 steps; tokens
  82,034 / 82,024 / 72,808 / 82,155 / 82,186, median 82,034. At 0.6.0 (fd395d22, trial/124-128)
  the median was 67,331: +14,703. Against the promoted 7ae5b05b rebaseline, +15,613, still not
  worse by the rule (the saved maximum is 83,589).
- All of it is in `tests.write`: 36,663-46,047 tokens a run at 6ad32d3c against 24,511-25,175 at
  fd395d22. `implementation.edit` (27,142-27,199) and `change.handover` (8,966-9,018) did not
  move; the change-so-far gain of story:step-prompt-omits-the-change-so-far holds.
- `tests.write` tool calls at 6ad32d3c: `Glob **/*` first in 5 of 5 runs, `Read CLAUDE.md` and
  `Read AGENTS.md` in 4 of 5, 4-5 requests. At fd395d22: `Read todo.py`, `Read
  tests/test_todo.py`, `Edit`, 3 requests in 5 of 5.
- The step's working directory is the session copy (`~/.cache/claude-tmp/session-<pid>-…/copy`,
  the stream's `system`/`init` `cwd`); its `permissionMode` moved from `acceptEdits` to
  `default`.
- Inferred, not run: the copy is not a git repository (`harness.rs` `copied` writes the files
  `git ls-files --exclude-standard` lists), so Claude Code puts no git snapshot in the step's
  context, and the step orients by exploring, as the 0.4.0 cycle saw when the snapshot was turned
  off (Glob plus reads in 2 of 3 runs; candidate 2d28be88 rejected).

Records on the trial agent's side: `~/.cache/takt-trials/2026-10-20-fd395d22/logs/14-collect-base.log`,
the scorecard of 6ad32d3c, `rollouts/base-<i>/transcripts`. Comes from self-improvement work, so
it carries `rsi`.

The trial agent's predicate (red at 6ad32d3c: exit 3, `StepRefused (copy-is-not-a-git-repository was not offered)`; healthy at fd395d22: exit 4, `ApprovalRequired (repository.merge)`; both run outside bubblewrap in a seed clone with `env -i`). Its fake `claude` checks `git rev-parse --is-inside-work-tree` in its working directory: it tests that the copy is a git work tree, not that the vendor reads it; acceptance 1 decides the cause.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/fake" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a marker line\"}"}' > "$HOME/fake/tests-write.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"repository.merge\", \"summary\": \"the copy is a git repository\"}"}' > "$HOME/fake/approval.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"copy-is-not-a-git-repository\", \"summary\": \"no git state in the copy\"}"}' > "$HOME/fake/no-git.json" && printf '%s\n' '#!/bin/sh' 'f="$HOME/fake"' 'in=$(cat)' 'if [ -e "$f/called" ]; then cat "$f/approval.json"; exit 0; fi' 'if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then cat "$f/no-git.json"; exit 0; fi' 'touch "$f/called"' 'echo "# a marker line" >> tests/test_todo.py' 'cat "$f/tests-write.json"' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 6
exit: 3
healthy_exit: 4
output: StepRefused (copy-is-not-a-git-repository was not offered)
source: crates/takt/src/harness.rs
```

## Scope

- `crates/takt/src/harness.rs` (`ClaudeSession`: the copy and what the session is started with),
  possibly `crates/session-executor/src/lib.rs` (the permission mode), and tests; a probe first.

## Acceptance

1. A probe settles the cause before any fix: one `tests.write` step on scenario 1 with the copy
   as today and one with a `.git` at the case's `HEAD` in the copy (the same model, three runs
   each); the record names the tool calls and tokens of each.
2. If the probe confirms it, the copy carries the repository state the vendor reads (a git
   repository at the case's `HEAD`, with no remote, no hooks and no config beyond what takt's own
   git commands use) and the step guard still refuses a proposal touching `.git`; a test.
3. If `permissionMode` matters (the probe shows edits asked for instead of made), the session is
   started with the mode the print-mode harness used, `acceptEdits`, for the file tools only.
4. Re-trial on scenario 1, five runs: median tokens not worse than at fd395d22 by the scorecard
   rule; `tests.write` back to about 3 requests.
