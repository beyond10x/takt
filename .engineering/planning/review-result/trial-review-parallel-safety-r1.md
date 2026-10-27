---
format: aep.planning-md/3
id: review-result:trial-review-parallel-safety-r1
kind: review-result
status: active
title: Parallel-safety critic, downstream trial-review set, round 1
relations:
- reviews: epic:downstream-story-delivery
- reviews: epic:protocol-settings-in-the-run-file
- reviews: story:an-unqualified-harness-stops-before-the-first-session
- reviews: story:a-takt-prefixed-base-is-named-as-reserved
- reviews: story:a-cases-root-survives-worktree-cleanup
- reviews: story:every-harness-reads-the-repository-instructions-from-the-base
- reviews: story:step-commits-follow-a-configurable-style
- reviews: story:the-run-shows-each-repository-obligation
- reviews: story:harness-program-is-chosen-per-run
- reviews: story:declared-setup-runs-before-the-first-check
- reviews: story:a-command-forge-runs-the-configured-commands
- reviews: story:the-gitlab-forge-can-call-the-api-with-a-named-credential
- reviews: story:a-connectors-forge-adapter
- reviews: story:the-workspace-seam-enters-and-leaves-managed-worktrees
- reviews: story:the-run-file-sets-protocol-settings
- reviews: story:takt-ships-default-protocol-settings
- reviews: story:takt-lists-a-protocols-settings
- reviews: story:story-delivery-tests-first
- reviews: dependency-blocker:metaharness-qualify-without-a-model-call
- reviews: dependency-blocker:worktree-nested-keep-file
- reviews: dependency-blocker:engineering-gates-see-the-change
- reviews: dependency-blocker:story-delivery-2-with-settings
revision: 1
---
needs-revision
story:an-unqualified-harness-stops-before-the-first-session — its scope claims `crates/takt/src/main.rs`, which does not exist (the CLI is `crates/takt-cli/src/main.rs`). Waves therefore misses its overlap with the active stories scoped on the real file: story:a-run-keeps-every-decision-only-on-a-trial, story:run-flags-spell-metaharness-and-llm and story:harness-section-is-metaharness-session-request. It also claims all of `crates/takt/tests` instead of its own `harness_preflight` module (cited, body Scope) — .engineering/planning/story/an-unqualified-harness-stops-before-the-first-session.md:17
story:harness-program-is-chosen-per-run — its scope claims the nonexistent `crates/takt/src/main.rs` instead of `crates/takt-cli/src/main.rs`, which hides its overlap with the active stories scoped there. It also claims all of `crates/takt/tests` (cited) — .engineering/planning/story/harness-program-is-chosen-per-run.md:18
story:declared-setup-runs-before-the-first-check — its scope claims the nonexistent `crates/takt/src/main.rs` instead of `crates/takt-cli/src/main.rs`, which hides its overlap with the active stories scoped there. It also claims all of `crates/takt/tests` (cited) — .engineering/planning/story/declared-setup-runs-before-the-first-check.md:15
story:a-command-forge-runs-the-configured-commands — its scope claims the nonexistent `crates/takt/src/main.rs` instead of `crates/takt-cli/src/main.rs`, which hides its overlap with the active stories scoped there. It also claims all of `crates/takt/tests` (cited) — .engineering/planning/story/a-command-forge-runs-the-configured-commands.md:19
story:the-run-file-sets-protocol-settings — its scope claims the nonexistent `crates/takt/src/main.rs` instead of `crates/takt-cli/src/main.rs`, which hides its overlap with the active stories scoped there. It also claims all of `crates/takt/tests` (cited) — .engineering/planning/story/the-run-file-sets-protocol-settings.md:18
story:takt-ships-default-protocol-settings — its scope claims the nonexistent `crates/takt/src/main.rs` instead of `crates/takt-cli/src/main.rs`, which hides its overlap with the active stories scoped there. It also claims all of `crates/takt/tests` (cited) — .engineering/planning/story/takt-ships-default-protocol-settings.md:16
story:takt-lists-a-protocols-settings — its scope claims the nonexistent `crates/takt/src/main.rs` instead of `crates/takt-cli/src/main.rs`, which hides its overlap with the active stories scoped there. It also claims all of `crates/takt/tests` (cited) — .engineering/planning/story/takt-lists-a-protocols-settings.md:16
story:a-takt-prefixed-base-is-named-as-reserved — its scope claims all of `crates/takt/tests` instead of its `reserved_prefix` module. In this set, 29 pairs share only that directory, and waves splits all of them on that path alone (cited) — .engineering/planning/story/a-takt-prefixed-base-is-named-as-reserved.md:19
story:a-cases-root-survives-worktree-cleanup — its scope claims all of `crates/takt/tests` instead of its `cases_kept` module, which keeps it out of a wave with six in-set stories it shares nothing else with (cited) — .engineering/planning/story/a-cases-root-survives-worktree-cleanup.md:18
story:every-harness-reads-the-repository-instructions-from-the-base — its scope claims all of `crates/takt/tests` instead of its `step_context` module (cited) — .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md:24
story:step-commits-follow-a-configurable-style — its scope claims all of `crates/takt/tests` instead of its `commit_style` module (cited) — .engineering/planning/story/step-commits-follow-a-configurable-style.md:24
story:the-run-shows-each-repository-obligation — its scope claims all of `crates/takt/tests` instead of its `obligations` module (cited) — .engineering/planning/story/the-run-shows-each-repository-obligation.md:23
story:the-gitlab-forge-can-call-the-api-with-a-named-credential — its scope claims all of `crates/takt/tests` instead of its `gitlab_api` module (cited) — .engineering/planning/story/the-gitlab-forge-can-call-the-api-with-a-named-credential.md:22
story:a-connectors-forge-adapter — its scope claims all of `crates/takt/tests` instead of its `connectors_forge` module (cited) — .engineering/planning/story/a-connectors-forge-adapter.md:22
story:the-workspace-seam-enters-and-leaves-managed-worktrees — its scope claims all of `crates/takt/tests` instead of its `workspace_seam` module (cited) — .engineering/planning/story/the-workspace-seam-enters-and-leaves-managed-worktrees.md:24
story:story-delivery-tests-first — its acceptance reads the `gate_on: after_handover` default that story:takt-ships-default-protocol-settings creates (that story's line 33), and no depends_on edge runs to it. The two are ordered (waves 27 and 29) only by an inferred file collision — .engineering/planning/story/story-delivery-tests-first.md:49
story:takt-lists-a-protocols-settings — its acceptance reads the defaults `required`/`after_handover` that story:takt-ships-default-protocol-settings creates (that story's line 33), and no depends_on edge runs to it. Waves puts it in wave 26, before its producer in wave 27, and dependency-blocker:story-delivery-2-with-settings does not block it either — .engineering/planning/story/takt-lists-a-protocols-settings.md:46
story:a-command-forge-runs-the-configured-commands — it builds on the `Forge`/`Runner` seam that the active branch `story/scan-forge-adapters` rewrites (`crates/takt/src/forge.rs`, 811 lines changed). Nothing in the store holds it back, so waves places it in wave 1 as ready. A dependency-blocker would record the hold without the missing edge — .engineering/planning/story/a-command-forge-runs-the-configured-commands.md:32
story:the-gitlab-forge-can-call-the-api-with-a-named-credential — it builds on the same seam, and the branch also rewrites `crates/takt/src/forge/gitlab.rs`. Nothing in the store holds it back — .engineering/planning/story/the-gitlab-forge-can-call-the-api-with-a-named-credential.md:50
story:a-connectors-forge-adapter — it builds on the same unmerged seam, and nothing in the store holds it back — .engineering/planning/story/a-connectors-forge-adapter.md:41
story:harness-program-is-chosen-per-run — it scopes `run_config.rs` "after that story lands" (story:harness-section-is-metaharness-session-request) with no depends_on edge, so waves places it in wave 18, before that story's wave 24 — .engineering/planning/story/harness-program-is-chosen-per-run.md:56
epic:downstream-story-delivery — its Delivery rule does not say that the active branch `story/scan-forge-adapters` also changes `run.rs`, `run_config.rs` and `harness.rs`. Most stories in the set claim those files, so waves cannot see the collision — .engineering/planning/epic/downstream-story-delivery.md:59

Read: 22 artifacts (2 epics, 16 stories, 4 dependency-blockers) via `aep plan artifact show`, `waves` (text and yaml), `relations`, `list`, `validate` (valid); `git diff --stat HEAD...story/scan-forge-adapters`; source paths by `ls`/`git grep`. Surfaces: 16 cited (7 of them with a nonexistent path), 0 inferred, 0 unplaced. Acceptance reads: 12 traced to a producer in the set, 9 recorded by depends_on or a dependency-blocker.
Not established: whether switching the default store-story protocol to `story.delivery/2` (takt-ships) changes the fixture runs that `obligations` 1 and `commit_style` 1 rely on. No body says which protocol its fixture opens on (inferred, so not a finding).
Not established: `HarnessBinaries::candidates()` turns up nothing in `git grep`, and no item in the set produces it.
Not established: none of the four stores holds a loom item for the metaharness -> loom -> takt chain (outside my lane, for scope).

```findings
- {file: .engineering/planning/story/an-unqualified-harness-stops-before-the-first-session.md, line: 17, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests instead of its harness_preflight module"}
- {file: .engineering/planning/story/harness-program-is-chosen-per-run.md, line: 18, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests"}
- {file: .engineering/planning/story/declared-setup-runs-before-the-first-check.md, line: 15, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests"}
- {file: .engineering/planning/story/a-command-forge-runs-the-configured-commands.md, line: 19, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests"}
- {file: .engineering/planning/story/the-run-file-sets-protocol-settings.md, line: 18, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests"}
- {file: .engineering/planning/story/takt-ships-default-protocol-settings.md, line: 16, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests"}
- {file: .engineering/planning/story/takt-lists-a-protocols-settings.md, line: 16, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "scope claims the nonexistent crates/takt/src/main.rs instead of crates/takt-cli/src/main.rs, hiding overlap with the active stories scoped there, and claims all of crates/takt/tests"}
- {file: .engineering/planning/story/a-takt-prefixed-base-is-named-as-reserved.md, line: 19, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its reserved_prefix module; 29 in-set pairs share only that directory and waves splits each on it alone"}
- {file: .engineering/planning/story/a-cases-root-survives-worktree-cleanup.md, line: 18, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its cases_kept module, separating it from six in-set stories it shares nothing else with"}
- {file: .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md, line: 24, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its step_context module"}
- {file: .engineering/planning/story/step-commits-follow-a-configurable-style.md, line: 24, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its commit_style module"}
- {file: .engineering/planning/story/the-run-shows-each-repository-obligation.md, line: 23, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its obligations module"}
- {file: .engineering/planning/story/the-gitlab-forge-can-call-the-api-with-a-named-credential.md, line: 22, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its gitlab_api module"}
- {file: .engineering/planning/story/a-connectors-forge-adapter.md, line: 22, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its connectors_forge module"}
- {file: .engineering/planning/story/the-workspace-seam-enters-and-leaves-managed-worktrees.md, line: 24, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "scope claims all of crates/takt/tests instead of its workspace_seam module"}
- {file: .engineering/planning/story/story-delivery-tests-first.md, line: 49, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "its acceptance reads the gate_on after_handover default story:takt-ships-default-protocol-settings creates, and no depends_on edge runs to it; only an inferred file collision orders them"}
- {file: .engineering/planning/story/takt-lists-a-protocols-settings.md, line: 46, category: parallel-safety, severity: blocker, verdict: needs-revision, origin: introduced, message: "its acceptance reads the defaults story:takt-ships-default-protocol-settings creates, with no depends_on edge; waves puts it in wave 26 before its producer in 27, and the story.delivery/2 blocker does not block it"}
- {file: .engineering/planning/story/a-command-forge-runs-the-configured-commands.md, line: 32, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "it builds on the Forge/Runner seam the active branch story/scan-forge-adapters rewrites, and nothing in the store holds it back, so waves places it in wave 1 as ready"}
- {file: .engineering/planning/story/the-gitlab-forge-can-call-the-api-with-a-named-credential.md, line: 50, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "it builds on the unmerged seam of story/scan-forge-adapters, which also rewrites crates/takt/src/forge/gitlab.rs, and nothing in the store holds it back"}
- {file: .engineering/planning/story/a-connectors-forge-adapter.md, line: 41, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "it builds on the unmerged seam of story/scan-forge-adapters, and nothing in the store holds it back"}
- {file: .engineering/planning/story/harness-program-is-chosen-per-run.md, line: 56, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "it scopes run_config.rs after story:harness-section-is-metaharness-session-request lands, with no depends_on edge, so waves places it in wave 18 before that story's wave 24"}
- {file: .engineering/planning/epic/downstream-story-delivery.md, line: 59, category: parallel-safety, severity: warning, verdict: needs-revision, origin: introduced, message: "the Delivery rule does not say the active branch story/scan-forge-adapters also changes run.rs, run_config.rs and harness.rs, which most stories in the set claim"}
```
