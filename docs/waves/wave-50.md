# Wave 50: the forge write outcome, the dollar budget, the unrelated install, the image

Stage: **dispatched**. `wave/50` branches from 40843ed9, 0.28.0's merge. Skill version 0.26.0
(aep:implementing, wave mode). Approved: approval-record:wave-50-four-stories. Release: 0.29.0.

## Selection

`aep plan artifact waves --kind story --status active` (aep 0.79.0) after the wave's scope
entries, verbatim in the approval record. story:bench-subset-and-budget and
story:an-unrelated-program-install-refuses-no-step collide on `crates/takt/src/run.rs`: unit b
starts from `wave/50` after unit c has merged. story:release-publishes-the-image was built and
released before the wave (1d1e4554c, 0.27.0 and 0.28.0); the wave checked its acceptance against
the tree and moved it with no code.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:fenced-forge-dispatch | `pr.open` and `repository.merge` record `applied`, `refused`, `not_attempted` or `unknown`; a merge `unknown` is read, never sent again | cited | dispatched |
| b | story:bench-subset-and-budget | `--usd` stops a run on the cost the harness reports, `stopped: Usd (<dollars>)` | inferred | waits for c |
| c | story:an-unrelated-program-install-refuses-no-step | the guard tests have a home of their own; a write takt cannot attribute is decided and tested | inferred | dispatched |
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
