# Wave 13: the check-finding confinement, promotion bound to its evidence, properties and fuzzing, the loop scorecard, scan before push

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Approval: the planner role named these units (the wave 13 lanes E1, E2 and property-and-fuzz-tests, and the carry-over of loop-scorecard, scan-before-push and improvement turn 4 from wave 12 under its close rule); each judge-surface merge needs the planner's grant of its exact commit, on a reviewer verdict. Mode: interactive, up to five implementors in parallel.

## Units

| unit | serves | acceptance (condensed) | judge surface |
|---|---|---|---|
| E1: `story:check-finding-builds-confined`, `story:check-finding-positive-control` | `vision:safe-agentic-coding` | `check-finding` builds the revision under check and runs the predicate in bubblewrap; it says `not_reproduced` only on the predicate's healthy exit and `error` for any status it does not name | yes |
| E2: `story:promote-binds-scorecard-and-merge`, `story:scorecard-against-needs-the-trials`, `story:scorecard-outcome-from-held-out-check`, `story:promotion-refusal-is-not-final` | `vision:safe-agentic-coding` | promotion refuses a revision not on origin's main and runs of another binary; no verdict on fewer cases than the trials; a scenario case's outcome from its held-out result; a refused revision judged again on new cases | yes |
| `story:property-and-fuzz-tests` | `vision:safe-agentic-coding` | `task check` runs every property with a fixed seed; `task fuzz` runs each target 60 s and exits 0; the mutated `Events::read` crashes the events target | yes |
| `story:loop-scorecard` (from wave 12) | `vision:safe-agentic-coding` | over three recorded turns `loop.measure` writes a `loop_measurement` whose figures each name their case ids | yes |
| `story:scan-before-push` (from wave 12) | `vision:safe-agentic-coding` | `pr.open` scans commit messages and the diff before anything is pushed; a refused scan pushes nothing | yes |
| improvement turn 4: `story:step-guard-sees-test-script-imports` (from wave 12) | `vision:safe-agentic-coding` | a step cannot shadow the test runner the test script loads | yes |

## Selection path

The planner's wave 13 lanes after the wave 12 close: E1 and E2 from the review of main's areas, property-and-fuzz-tests on the owner's request, and the three units wave 12's close rule moved here. Every unit was built on its own branch while main was frozen for release 0.1.0; this branch opens from main at c9abebd, after the release's CI fixes, and takes each unit on its grant. Its merge into main waits for the release tag.

## Left out

- `story:story-delivery-tests-first`: its correction is committed and not yet gated against this base or granted.
- L1 to L4 (cases-root-link-is-undone, ignored-directories-are-guarded, untracked-cargo-lock-remedy, test-command-reaches-no-forge-login, forge-preflight, scan-policy-outside-the-tree, merge-checks-the-pull-request-base, git-filter-drivers-do-not-run, git-reads-no-user-excludes) and review-of-main-lows: the next dispatch, from this branch.
- `story:abandon-a-case`: wave 14.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at c9abebd; `task check` exit 0 at 44f96a4 (814 passed, citations 578 of 578); a CI-like run at c9e99b4 (cut PATH, fresh HOME, colour on) 814 passed, 0 failed |
| model budget | not stated; N = 5 |

## The commits this wave makes

One opening commit (this page), each unit's commits on its own branch, the merges of the units into this branch on their grants, a merge of main when it moves, the closing planning-store commit, and the merge into local main after the release tag. No push, tag or release.

## Log

- 2026-10-19: wave opened from main at c9abebd.
