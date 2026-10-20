---
format: aep.planning-md/3
id: story:commit-identity-refusal-names-the-identity-file
kind: story
status: draft
title: The commit identity refusal names .engineering/commit-identity.conf as what it is, not the planning store
summary: the refusal calls the identity file part of the planning store
tags:
- finding
- rsi
refs:
- provider: finding
  reference: commit-identity-refusal-names-the-identity-file
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: commit-identity-refusal-names-the-identity-file

## Outcome

A step refused for writing `.engineering/commit-identity.conf` is told it changed the commit
identity file, what decides whose name a candidate leaves the machine under, not that it wrote
into the planning store. The planning store's sentence names only the store's own files.

## Context

Found by the wave 16 review at 05f70c4f (docs/waves/wave-16.md), left for later. Wave 16 added
`.engineering/commit-identity.conf` (`COMMIT_IDENTITY_FILE`, `crates/takt/src/git.rs:1044` at
00a900ac). A step that writes it is refused, as it should be: the identity is read at the base and
the step guard covers `.engineering/**`. The refusal text is wrong. `surface_refusal`
(`crates/takt/src/run.rs:5898`) splits what a step touched with `JudgeSurface::is_store`
(`run.rs:4647`), which takes every path under `.engineering/` but the cases. So the identity file
is reported as "wrote into the planning store, which only takt writes, each write its own commit
(the take when a case opens, a grant's record, the story's test result and its move to
implemented)" (`run.rs:5912`). None of that is true of the identity file. An operator reading the
stop looks for a store write that never happened.

The predicate runs one harness step with a `claude` double that writes the identity file and
answers `implementation.edit`. The run stops `StepRefused` (exit 3) before and after the fix. The
output string is the store sentence followed by the identity file's path.

## Acceptance

1. A step that writes `.engineering/commit-identity.conf` is still refused, and its refusal names
   the file as the commit identity (or the judge surface), never as the planning store: a test.
2. A step that writes a file of the planning store (`.engineering/planning/**`) is still refused
   with the planning store's sentence: a test.
3. At the candidate, `scripts/check-finding story:commit-identity-refusal-names-the-identity-file
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin"; printf '%s\n' '{"result":"{\"action\": \"implementation.edit\", \"summary\": \"an identity\"}","usage":{"input_tokens":1}}' > "$HOME/answer.json"; printf '%s\n' '#!/bin/sh' 'echo "[identity] name = step" > .engineering/commit-identity.conf' "cat $HOME/answer.json" > "$HOME/bin/claude"; chmod +x "$HOME/bin/claude"; PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 2
exit: 3
healthy_exit: 3
output: the story's test result and its move to implemented): .engineering/commit-identity.conf
source: docs/waves/wave-16.md
```
