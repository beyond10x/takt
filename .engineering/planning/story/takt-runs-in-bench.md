---
format: aep.planning-md/3
id: story:takt-runs-in-bench
kind: story
status: draft
title: An exported takt run is a record the organization's benchmark instrument accepts into its store
relations:
- informed_by: review-result:benchmark-acceptance-round-1
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-run-export
revision: 1
---
## Outcome

The output of `takt export --bench`, appended to the seed file of a checkout of the organization's benchmark instrument (design:benchmark-integration § The organization's instrument) and imported there, passes the instrument's store validator. takt then sits beside the other harnesses in the instrument's fact table, which is the second half of epic:benchmark-integration's acceptance.

## Context

story:bench-run-export writes the entry. The instrument's validator checks a whole store, including that every reference resolves, so one entry cannot be validated on its own. This story is that end-to-end check.

Today the instrument's harness kinds cover only Claude Code, Codex and the b10x loop. It has no harness of takt's kind, no takt arm, and no corpora for takt's scenarios, Terminal-Bench 2.0 or SWE-bench Verified Mini. Those records are the instrument's own work, filed in its own store, as the design says. This story cannot be implemented until they land. Record that wait as a blocker in this store when the story starts; an edge into another store is not possible.

The instrument commit the check runs against is pinned in `docs/bench/instrument.pin`. The validator runs offline.

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/bench/check-instrument-import.sh` exits non-zero.

## Scope

- `docs/bench/check-instrument-import.sh` (inferred)
- `docs/bench/instrument.pin` (inferred)

## Acceptance

`docs/bench/check-instrument-import.sh <instrument checkout> <fixture case>` (it copies the checkout at the commit `docs/bench/instrument.pin` names, appends `takt export --bench` of the fixture case to the copy's seed file, imports it and runs the instrument's validator) exits 0 with the validator's last line `valid`, where at d17bc32 the script does not exist.
