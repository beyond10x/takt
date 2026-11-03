# Wave 49: tests first, the CI wait watches the check, the token rule, the docs site

Stage: **dispatched**. `wave/49` branches from 97c56193, 0.27.0's merge. Skill version 0.26.0
(aep:implementing, wave mode). Approved: approval-record:wave-49-four-stories.

## Selection

`aep plan artifact waves --status active` (aep 0.79.0) at 97c56193, verbatim in the approval
record. story:the-cli-suite-fits-its-ci-budget is already `implemented` (2026-10-29) and leaves
the wave. The tests-first and CI-wait stories collide on `protocols/story-delivery/3.yaml` and run
one after the other.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:story-delivery-tests-first | the acceptance still open after `story.delivery/3`: the gate deferred at a red revision, tests optional | inferred | dispatched |
| b | story:ci-wait-watches-the-check | `ci.wait` polls `gh pr checks` at 2 s growing to 15 s; `Run::POLL` goes | cited | after a merges |
| c | story:the-token-rule-holds-for-an-unchanged-binary | the token rule tolerates run-to-run variance and refuses a 10 % per-request regression | inferred | dispatched |
| d | story:docs-site-with-sidebar | the site at 5d8adae0 checked against the acceptance; no code | cited | coordinator |

| unit | agent | branch | tree | build dir | scratch root |
|---|---|---|---|---|---|
| a | `aep:implementor` | `unit/49-a` | `takt-w49-a` | the tree's `target/` | the tree's `target/scratch` |
| b | `aep:implementor` | `unit/49-b` | `takt-w49-b` | the tree's `target/` | the tree's `target/scratch` |
| c | `aep:implementor` | `unit/49-c` | `takt-w49-c` | the tree's `target/` | the tree's `target/scratch` |

Trees are managed worktrees under the worktree manager's `takt` directory. Each unit runs its
targeted tests only; the full gate runs once on `wave/49`, then one pull request through the bot
route. Reviews: `aep:adversary` on a, b and c after each merges into `wave/49`.

Commits this wave makes: the unit commits, the merges into `wave/49`, the store commits, the
release prepare, one pull request and its merge to `main`.
