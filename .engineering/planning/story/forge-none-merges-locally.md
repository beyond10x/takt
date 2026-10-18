---
format: aep.planning-md/3
id: story:forge-none-merges-locally
kind: story
status: draft
title: On --forge none an approved case merges into its base
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:an-intent-run-can-finish
revision: 1
---
## Outcome

A case run with `--forge none` reaches its merged outcome: once `repository.merge` is granted, takt itself merges the case branch into the base (a local `git merge`, checked by the same validator that reads the merge on a forge), and a story case is offered the merge approval once its change is verified. No step asks the harness to do what the prompt forbids.

## Context

From the planner's operator-path variety round on takt at a29d8c7 (2026-10-19), scenario 1 at seed 865eaa8 with no origin and a fake harness. The help says `pr.open`, `ci.read` and `repository.merge` "are left to the harness" on `--forge none` (`crates/takt-cli/src/main.rs:201`, `:750`, `:757`), but the harness prompt says "Do not run commands" (`crates/takt/src/harness.rs:149`). Observed: a story case is never offered the merge approval and stops NoProgress on every resume (exit 3, `step 1: pr.open (claude-code)`); an intent case, approved (`takt approve … repository.merge` exit 0), is offered `repository.merge` as a judgement step, the step "merges", `effect: no file changed`, the run stops NoProgress and `main` stays at 865eaa8. `--forge none` is the default for every origin not on github.com, so every local or non-GitHub repository is affected. Related: story:an-intent-run-can-finish (the accepted outcome's evidence kinds) and story:no-merge-approval-without-a-change (the approval offered before any commit, confirmed again in this round).

## Scope

- `crates/takt-cli/src/main.rs`: the `none` forge and its help (cited)
- `crates/takt/src/forge.rs`: a local merge for the `none` forge (inferred)
- `crates/takt/src/validator.rs`: the merge observation without a forge (inferred)
- `crates/takt/src/harness.rs`: the prompt (cited)

## Acceptance

```expected_before
scenario: 1
command: PATH="<scratch>/fake:$PATH" "$TAKT" run --repo . --intent probe --case-id probe --forge none --iterations 20 --test-cmd "sh check.sh" >/dev/null 2>&1; "$TAKT" approve --repo . --by human:reviewer probe repository.merge >/dev/null 2>&1 && FAKE_MODE=merge PATH="<scratch>/fake:$PATH" "$TAKT" run --repo . --case probe --forge none --iterations 20 --test-cmd "sh check.sh"
exit: 3
output: stopped: NoProgress; steps 1
source: REVIEW_RECORD
```

After the fix, the same sequence ends with the case's merged outcome, `git log main` contains the case branch's commits, and a story case on `--forge none` is offered the merge approval after its tests pass. The fake harness must be inlined in the predicate before `scripts/check-finding` can run it.
