---
format: aep.planning-md/3
id: story:codex-step-gets-each-file-once
kind: story
status: implemented
title: A Codex step is given a file twice
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: codex-step-gets-each-file-once
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:codex-step-context
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T05:12:56Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T05:12:57Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-29T05:12:58Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# A Codex step is given a file twice

## What is wrong

Every Codex step's prompt carries `AGENTS.md` twice: Codex loads it from the session's copy
("# AGENTS.md instructions"), and takt gives the base's copy again as a context file; it gives
`CLAUDE.md` to Codex too. From `implementation.edit` on, a test file the case changed is given in
full as a context file and its changed lines are shown again in the change so far. A Codex step
sends the prompt on every request (five a run on scenario 1), so the copies cost tokens five times:
about 4,400 bytes a run for the instruction files and about 2,900 for the test file, a low estimate
of 1,780 tokens a run at 4 bytes a token. Takt 0.18.1's Codex runs of scenario 1
(`--model gpt-6.1-terra`, Codex 0.160.0, cap 85088 tokens) stopped `Tokens` at 87095 and 87065,
before `pr.open`.

## Check

`scripts/check-finding` on scenario 1 at 26ed4ac1 (0.18.1): `reproduced`; at the candidate
cb40b2d6: `not_reproduced`.

```expected_before
scenario: 1
command: printf '%s\n' '#!/bin/sh' '[ "$1" = "--version" ] && { echo "codex-cli 0.160.0"; exit 0; }' 'D=$(dirname "$0"); for a; do p=$a; done; n=$(ls "$D"/p.* 2>/dev/null | wc -l); printf "%s" "$p" > "$D/p.$n"; [ "$n" = 0 ] || exit 1' 'sed -i "s/^if __name__/    def test_remove(self):\n        self.assertEqual(self.run_cli(\"remove\", \"1\")[0], 1)\n\n\nif __name__/" tests/test_todo.py' 'd="$CODEX_HOME/sessions/2026/10/20"; mkdir -p "$d"; exec > "$d/rollout-2026-10-20T18-00-00-01a15a19-57e0-7d83-97b1-da333ebb91e5.jsonl"' 'r="Done.\\n{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"wrote a test\\\", \\\"commit\\\": {\\\"subject\\\": \\\"test: add a test\\\", \\\"body\\\": \\\"Adds a test for removal.\\\\nIt covers the numbering.\\\\nIt covers refusals.\\\"}}"' 't="\"timestamp\":\"2026-10-20T18:00:00.000Z\""' 'printf "{%s,\"type\":\"session_meta\",\"payload\":{\"session_id\":\"01a15a19-57e0-7d83-97b1-da333ebb91e5\",\"id\":\"01a15a19-57e0-7d83-97b1-da333ebb91e5\",\"timestamp\":\"2026-10-20T18:00:00.000Z\",\"cwd\":\"%s\",\"originator\":\"codex_exec\",\"cli_version\":\"0.160.0\",\"source\":\"exec\",\"model_provider\":\"openai\"}}\n" "$t" "$PWD"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"01a15a19-5963-7d42-b749-21695314f7a4\"}}\n" "$t"' 'printf "{%s,\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"final_answer\",\"content\":[{\"type\":\"output_text\",\"text\":\"%s\"}]}}\n" "$t" "$r"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":100,\"cached_input_tokens\":40,\"output_tokens\":20,\"reasoning_output_tokens\":0,\"total_tokens\":120}}}}\n" "$t"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"01a15a19-5963-7d42-b749-21695314f7a4\",\"last_agent_message\":\"%s\",\"duration_ms\":5}}\n" "$t" "$r"' > "$HOME/codex" && chmod +x "$HOME/codex" && printf 'scan: none\n' > "$HOME/run.yaml" && "$TAKT" run --repo . --intent "$(printf 'Remove a todo by its number\n\n## Scope\n\n- `todo.py`\n- `tests/test_todo.py`\n')" --harness codex --harness-program "$HOME/codex" --config "$HOME/run.yaml" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 4 > "$HOME/run.log" 2>&1; test -s "$HOME/p.1" || { tail -3 "$HOME/run.log"; exit 2; }; if grep -q "^<context-file-0 path=\"\(AGENTS\|CLAUDE\)\.md\"" "$HOME/p.0" "$HOME/p.1" || { grep -q "^<context-file-0 path=\"tests/test_todo.py\"" "$HOME/p.1" && grep -q "^diff --git a/tests/test_todo.py" "$HOME/p.1"; }; then echo "a Codex step was given a file twice"; exit 9; fi; exit 0
exit: 9
healthy_exit: 0
output: a Codex step was given a file twice
source: docs/runs/2026-10-28-trial-0.18.1.md
```

The `source` is the trial's run record, to be written under `docs/runs/` from the trial's notes.

## A candidate was rejected

Candidate branch `trial-cand/2026-10-28-26ed4ac1-1` at cb40b2d6 was rejected:

- Re-trial on scenario 1, three runs: two stopped `Tokens` at 86861 and 86930 with no pull
  request, one stopped `ApprovalRequired` at 84922 with the held-out check passing; worse than
  the trigger in 2 of 3 runs, and the set stopped after two runs with the same worse outcome. Its
  prompts were smaller (tests.write 7570 characters of takt's prompt against 8456,
  change.handover 8747 against 10967), but in the two runs over the cap the
  `implementation.edit` prompt carried the failed gate's printed output, 4241 bytes in run 1, on
  both of its requests (story:failed-gate-block-of-a-red-test-run-is-large).
- Reviewer verdict needs-revision: Codex never gets `CLAUDE.md`, even when it is the repository's
  only instruction file; the path `InstructionsNotLoaded` to `StepRefused` has no test.

A next candidate starts from these points.

## Outcome

Under Codex, `AGENTS.md` is not given as a context file when the session's copy has the base's
bytes and Codex loads it (under its 32 KiB project-doc limit, no `AGENTS.override.md`); otherwise
the base's bytes are given as now. A repository whose only instruction file is `CLAUDE.md` still
gets it. A test file shown in the change so far is shown whole within `CHANGE_SHOWN_BYTES`, so
`kept_files` does not give it again.

## Acceptance

1. The predicate above gives `not_reproduced` at the candidate and `reproduced` at the promoted
   revision.
2. A test: a repository with `CLAUDE.md` and no `AGENTS.md` gives the Codex step `CLAUDE.md`.
3. A test: a step whose instructions Codex did not load stops `StepRefused` through
   `InstructionsNotLoaded`.
4. A re-trial on scenario 1 is not worse than the promoted baseline under the promotion rule.

## Scope

- `crates/takt/src/harness.rs`: `context_files`, `context_files_after`, `instruction_files`,
  `DEFAULT_INSTRUCTION_FILES`, `kept_files`, `CHANGE_SHOWN_BYTES` (cited)
- `crates/takt/src/run.rs`: the change so far handed to the step (inferred)
- `crates/takt/tests/`: the cases of acceptance 2 and 3 (inferred)
