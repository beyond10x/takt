---
format: aep.planning-md/3
id: story:codex-edit-step-ends-in-one-request
kind: story
status: draft
title: A Codex edit step spends a second request on its final line
tags:
- finding
- needs-decision
- priority:low
- rsi
refs:
- provider: finding
  reference: codex-edit-step-ends-in-one-request
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A Codex edit step spends a second request on its final line

## What is wrong

Under Codex, `tests.write` and `implementation.edit` each make two model requests: one that calls
`apply_patch`, and a second that only writes the final JSON line. On takt 0.18.1's Codex runs of
scenario 1 the second requests took 17122 and 17622 input tokens for 75 and 68 output tokens,
about 34,700 tokens a run of 87,095. The prompt asks Codex to change files only with
`apply_patch`.

A proposed change, for `--harness codex` only: the edit steps ask for the patch in the final
answer (`*** Begin Patch` ... `*** End Patch`, then the JSON line), so the turn ends after one
request, and takt applies the patch to the case tree under the same checks a session's proposal
goes through today (the action's path rule, the plain-path, symlink and `.engineering/` refusals,
the scan, refuse and revert).

## Needs a decision first

This story needs an ADR-level decision before any work. The proposal was rejected at review on
2026-10-29:

- takt would apply model-written patch text itself: untrusted input on the security surface;
  undecided which wins when the session's copy holds a diff and the answer holds a patch;
  read-only Codex edit steps are a Loom or metaharness setting, not takt's;
- it overlaps story:codex-step-gets-each-file-once in `crates/takt/src/harness.rs` and
  `crates/takt/src/run.rs`;
- its predicate is weak: `exit` and `healthy_exit` are both 3, and it tells the two apart only by
  its output; it should assert that `tests/test_remove.py` was committed.

## Check

`scripts/check-finding` on scenario 1 at 26ed4ac1 (0.18.1): `reproduced`. To be strengthened
before scheduling (above).

```expected_before
scenario: 1
command: printf '%s\n' '#!/bin/sh' '[ "$1" = "--version" ] && { echo "codex-cli 0.160.0"; exit 0; }' ':' 'd="$CODEX_HOME/sessions/2026/10/20"; mkdir -p "$d"; exec > "$d/rollout-2026-10-20T18-00-00-01a15a19-57e0-7d83-97b1-da333ebb91e5.jsonl"' 'r="*** Begin Patch\\n*** Add File: tests/test_remove.py\\n+import unittest\\n+\\n+class RemoveTest(unittest.TestCase):\\n+    def test_nothing(self):\\n+        pass\\n*** End Patch\\n{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"wrote a test\\\", \\\"commit\\\": {\\\"subject\\\": \\\"test: add a test\\\", \\\"body\\\": \\\"Adds a test for removal.\\\\nIt covers the numbering.\\\\nIt covers refusals.\\\"}}"' 't="\"timestamp\":\"2026-10-20T18:00:00.000Z\""' 'printf "{%s,\"type\":\"session_meta\",\"payload\":{\"session_id\":\"01a15a19-57e0-7d83-97b1-da333ebb91e5\",\"id\":\"01a15a19-57e0-7d83-97b1-da333ebb91e5\",\"timestamp\":\"2026-10-20T18:00:00.000Z\",\"cwd\":\"%s\",\"originator\":\"codex_exec\",\"cli_version\":\"0.160.0\",\"source\":\"exec\",\"model_provider\":\"openai\"}}\n" "$t" "$PWD"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"01a15a19-5963-7d42-b749-21695314f7a4\"}}\n" "$t"' 'printf "{%s,\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"final_answer\",\"content\":[{\"type\":\"output_text\",\"text\":\"%s\"}]}}\n" "$t" "$r"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":100,\"cached_input_tokens\":40,\"output_tokens\":20,\"reasoning_output_tokens\":0,\"total_tokens\":120}}}}\n" "$t"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"01a15a19-5963-7d42-b749-21695314f7a4\",\"last_agent_message\":\"%s\",\"duration_ms\":5}}\n" "$t" "$r"' > "$HOME/codex" && chmod +x "$HOME/codex" && printf 'scan: none\n' > "$HOME/run.yaml" && "$TAKT" run --repo . --intent "$(printf 'Remove a todo by its number\n\n## Scope\n\n- `todo.py`\n- `tests/test_todo.py`\n')" --harness codex --harness-program "$HOME/codex" --config "$HOME/run.yaml" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 2 2>&1
exit: 3
healthy_exit: 3
output: effect: no file changed
source: docs/runs/2026-10-28-trial-0.18.1.md
```

The `source` is the trial's run record, to be written under `docs/runs/` from the trial's notes.

## Outcome

Once the decision is recorded: a Codex edit step ends after one model request, or the decision
records why it does not and this story is closed on it. Whatever applies a patch from model output
does so under every check a session's proposal goes through today.

## Acceptance

1. The decision is recorded (an ADR, or the owner's decision in the store) before the story moves
   past draft.
2. The strengthened predicate gives `reproduced` at the promoted revision and `not_reproduced` at
   the candidate.
3. A re-trial on scenario 1 is not worse than the promoted baseline under the promotion rule.

## Scope

- `crates/takt/src/harness.rs`: the Codex rule and prompt of the edit steps (cited)
- `crates/takt/src/run.rs`: where a step's proposal is checked and applied (cited)
- an ADR on applying model-written patches, in the repository that owns the decision (inferred)
