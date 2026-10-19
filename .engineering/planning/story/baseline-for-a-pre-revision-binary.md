---
format: aep.planning-md/3
id: story:baseline-for-a-pre-revision-binary
kind: story
status: active
title: A baseline record names a binary that predates the revision in --version by its sha256
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T00:48:31Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-19T00:48:31Z", actor: "human:timo", revision: 5}
---
## Outcome

`takt promote --baseline-only` records a baseline for a binary built before `--version` named its revision, when the operator names the revision and the binary's sha256: the record holds both, and the command refuses when the installed binary's sha256 differs. A binary whose `--version` names a revision is checked as before.

## Context

Finding of 2026-10-19 from the developer of story:promotion: the promoted revision a0b8576 predates the revision in `--version` (its binary prints `takt 0.1.0`), and `--baseline-only` requires `<root>/bin/takt --version` to name the revision (`crates/takt/src/promotion.rs:1202-1212@df9681a`), so no baseline record can name a0b8576. The installed `~/.cargo/bin/takt` is the a0b8576 binary by sha256 (b7af9ec9…), the binary of the baseline runs T-14..18 and every round-4 and variety run.

Planner decision: the baseline stays a0b8576, identified by sha256. Making 834205f or cfbf7e4 the baseline would install a turn's candidate without the promotion rule deciding it, and turns 2 and 3 would then have no promotion to earn their `promoted` claim. Waiting for a self-naming revision with 5 runs has the same effect for the wave 12 tip.

## Scope

- `crates/takt/src/promotion.rs`: the baseline identity check (cited)
- `crates/takt-cli/src/main.rs`: the flags of `promote --baseline-only` (inferred)
- `crates/takt-cli/tests/promote_cli.rs`: the rows (inferred)

## Acceptance

With a fake installed binary that prints `takt 0.1.0`, `takt promote --baseline-only --revision <sha> --binary-sha256 <hex> --scorecard <file>` records a baseline naming `<sha>` and `<hex>` when the digest matches and exits non-zero naming both digests when it does not; without `--binary-sha256` it refuses as today. Each run in the scorecard must name its binary's sha256 (in its run record, or a `binary.txt` beside it); a run that names none, or a digest other than `<hex>`, is refused naming the run, so the scorecard is the measured binary's. `--binary-sha256` is refused when the installed binary's `--version` names a revision.

## Review

From the reviewer's verdict on this story's text (2026-10-19, low): the digest proves which binary is installed, but neither `--revision` nor the scorecard's runs were bound to it. The run-digest check above binds the runs; `--revision` stays the operator's statement, recorded as such, for a binary that cannot state it.
