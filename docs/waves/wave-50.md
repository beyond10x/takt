# Wave 50: the forge write outcome, the dollar budget, the unrelated install, the image

Stage: **delivered to the pull request**. `wave/50` branches from 40843ed9, 0.28.0's merge. Skill version 0.26.0
(aep:implementing, wave mode). Approved: approval-record:wave-50-four-stories. Release: 0.29.0.

## Selection

`aep plan artifact waves --kind story --status active` (aep 0.79.0) after the wave's scope
entries, verbatim in the approval record. story:bench-subset-and-budget and
story:an-unrelated-program-install-refuses-no-step collide on `crates/takt/src/run.rs`; they run at
once, the file split by function (b: the `Stop` enum and the budget checks; c: the step guard
and its refusal text), with a `git merge-tree` dry run before the first of them merges; this
replaces the order the approval record names, to run the two at once. story:release-publishes-the-image was built and
released before the wave (1d1e4554c, 0.27.0 and 0.28.0); the wave checked its acceptance against
the tree and moved it with no code.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:fenced-forge-dispatch | `pr.open` and `repository.merge` record `applied`, `refused`, `not_attempted` or `unknown`; a merge `unknown` is read, never sent again | cited | merged |
| b | story:bench-subset-and-budget | `--usd` stops a run on the cost the harness reports, `stopped: Usd (<dollars>)` | inferred | merged |
| c | story:an-unrelated-program-install-refuses-no-step | the guard tests have a home of their own; a write takt cannot attribute is decided and tested | inferred | merged |
| d | story:release-publishes-the-image | built and released before the wave; acceptance checked, no code | cited | implemented |

| unit | agent | branch | tree | build dir | scratch root |
|---|---|---|---|---|---|
| a | `aep:implementor` | `unit/50-a` | `takt-w50-a` | the tree's `target/` | the tree's `target/scratch` |
| b | `aep:implementor` | `unit/50-b` | `takt-w50-b` | the tree's `target/` | the tree's `target/scratch` |
| c | `aep:implementor` | `unit/50-c` | `takt-w50-c` | the tree's `target/` | the tree's `target/scratch` |

Trees are managed worktrees under the worktree manager's `takt` directory. Each unit runs its
targeted tests only; the full gate runs once on `wave/50`, in stages, then one pull request
through the bot route. Reviews: `aep:adversary` on a, b and c, and `aep:security-reviewer` on a
(the merge write) and c (the guard), each in its unit's tree after the unit merges.

Commits this wave makes: the unit commits, the merges into `wave/50`, the store commits, the
release prepare, one pull request and its merge to `main`.

## Reviews

| review | unit | outcome |
|---|---|---|
| review-result:w50-adv-b-1 | b | fixed: the dollar sum in micro-dollars, the acceptance command read with the fixture task's protocol |
| review-result:w50-adv-c-1 | c | fixed: a rustup home of their own for the tests; confinement binds the toolchain rustc resolves |
| review-result:w50-sec-c-1 | c | fixed: the toolchain bins; a program rewritten through its link filed as a draft |
| review-result:w50-adv-a-1 | a | fixed: the hold lasts until the forge answers; the write is recorded before it leaves |
| review-result:w50-sec-a-1 | a | fixed: an unknown merge is never sent again; no rider push or base update moves the head under it |

Found along the way and fixed in the wave: `promote_cli` read outcome version 3 as current after
unit b; `scripts/check-finding` bound a toolchain of links after unit c.

Filed as drafts, outside the wave (tag `from-finding`):
story:program-rewritten-through-a-link-refuses-the-step,
story:merge-is-not-sent-after-a-merged-read, story:pr-open-cut-off-by-a-crash-is-read-back,
story:a-relative-path-entry-is-no-test-program-directory (from the full gate: two cases red only
with a relative `.` entry on the operator's `PATH`).
