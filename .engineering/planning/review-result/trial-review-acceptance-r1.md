---
format: aep.planning-md/3
id: review-result:trial-review-acceptance-r1
kind: review-result
status: active
title: Acceptance critic, downstream trial-review set, round 1
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
story:the-run-file-sets-protocol-settings — scenarios 1 and 3 set `risk: high` and `input:risk=low`, which `software.change/1` does not declare (it declares `[trivial, standard, elevated, critical]`, engineering-protocols `protocols/software-change/1.yaml:23`), so scenario 1 can only end in scenario 2's refusal — .engineering/planning/story/the-run-file-sets-protocol-settings.md:62
story:the-run-file-sets-protocol-settings — the acceptance names no scenario for the story tag `protocol:<name/rev>` that the Outcome adds, so that tag reads the same before the work as after — .engineering/planning/story/the-run-file-sets-protocol-settings.md:49
epic:protocol-settings-in-the-run-file — the body has no `## Acceptance` between `## Stories` and `## Out of scope`, so nothing says when the epic is done — .engineering/planning/epic/protocol-settings-in-the-run-file.md:40
story:step-commits-follow-a-configurable-style — no scenario sets `trailer`, `types` or the `bug` tag, so the configurable trailer and the `fix` type are never observed — .engineering/planning/story/step-commits-follow-a-configurable-style.md:36
story:every-harness-reads-the-repository-instructions-from-the-base — no scenario sets the new `context` key or its flag, or uses a base that lacks a default file, so the configurable list is never observed — .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md:32
story:declared-setup-runs-before-the-first-check — no scenario shows a setup argv holding shell syntax arriving as one argument, so "never a shell string" is unchecked — .engineering/planning/story/declared-setup-runs-before-the-first-check.md:32
story:the-run-shows-each-repository-obligation — no scenario reads the obligation table from `takt report`, which the Outcome promises — .engineering/planning/story/the-run-shows-each-repository-obligation.md:33
story:a-connectors-forge-adapter — no scenario covers merge or CI evidence bound to the change request's head, though the Outcome claims both — .engineering/planning/story/a-connectors-forge-adapter.md:35
story:the-workspace-seam-enters-and-leaves-managed-worktrees — no scenario shows declared setup running in the child's tree, or `case.yaml` recording `parent` and `worktree` — .engineering/planning/story/the-workspace-seam-enters-and-leaves-managed-worktrees.md:41
story:story-delivery-tests-first — `tests_optional_keeps_todays_order` joins two cases with "or", and it says "today's order" without naming what is observed; the `story.delivery/1` case already holds before the work — .engineering/planning/story/story-delivery-tests-first.md:50
dependency-blocker:metaharness-qualify-without-a-model-call — it clears only when the blocked takt story's own scenario passes, so it cannot clear before the story it blocks is implemented; it names no upstream state — .engineering/planning/dependency-blocker/metaharness-qualify-without-a-model-call.md:21
dependency-blocker:worktree-nested-keep-file — it clears on the blocked story's scenario `cases_kept::a_finished_tree_keeps_its_case`, not on the worktree story's own tests, so it cannot clear first — .engineering/planning/dependency-blocker/worktree-nested-keep-file.md:21
dependency-blocker:engineering-gates-see-the-change — it clears on the blocked story's scenario, not on the engineering-protocols fixtures, so it cannot clear first — .engineering/planning/dependency-blocker/engineering-gates-see-the-change.md:22
dependency-blocker:story-delivery-2-with-settings — it clears on an unnamed "takt story run" check from one of the two stories it blocks, rather than on the upstream fixtures — .engineering/planning/dependency-blocker/story-delivery-2-with-settings.md:23
Read: 20 of 20 ids (2 epics, 15 stories, 4 dependency-blockers; `design:continuous-orchestration` § 6 for context) with `aep plan artifact show`, `kinds`, `lifecycle story|dependency-blocker`, `validate` (valid), plus the files with `cat -n`, and `git grep` on the named symbols.
Not established: out of lane (design/scope). Many scopes name `crates/takt/src/main.rs`, but `main.rs` is at `crates/takt-cli/src/main.rs`, at HEAD and at 110251a.
Not established: out of lane. `HarnessBinaries::candidates()` greps to nothing; only `frontier.rs` and `takt-cli` define a `candidates`.
Not established: whether `epic:downstream-story-delivery`'s acceptance, four checks on one fixture run, is meant as one scenario. Not counted as a finding.
```findings
- {file: .engineering/planning/story/the-run-file-sets-protocol-settings.md, line: 62, category: acceptance, severity: blocker, verdict: needs-revision, origin: introduced, message: "scenarios 1 and 3 set risk: high and input:risk=low, which software.change/1 does not declare (it declares [trivial, standard, elevated, critical]), so scenario 1 can only end in scenario 2's refusal"}
- {file: .engineering/planning/story/the-run-file-sets-protocol-settings.md, line: 49, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "the acceptance names no scenario for the story tag protocol:<name/rev> that the Outcome adds, so that tag reads the same before the work as after"}
- {file: .engineering/planning/epic/protocol-settings-in-the-run-file.md, line: 40, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "the body has no Acceptance section between Stories and Out of scope, so nothing says when the epic is done"}
- {file: .engineering/planning/story/step-commits-follow-a-configurable-style.md, line: 36, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "no scenario sets trailer, types or the bug tag, so the configurable trailer and the fix type are never observed"}
- {file: .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md, line: 32, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "no scenario sets the new context key or its flag, or uses a base that lacks a default file, so the configurable list is never observed"}
- {file: .engineering/planning/story/declared-setup-runs-before-the-first-check.md, line: 32, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "no scenario shows a setup argv holding shell syntax arriving as one argument, so never a shell string is unchecked"}
- {file: .engineering/planning/story/the-run-shows-each-repository-obligation.md, line: 33, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "no scenario reads the obligation table from takt report, which the Outcome promises"}
- {file: .engineering/planning/story/a-connectors-forge-adapter.md, line: 35, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "no scenario covers merge or CI evidence bound to the change request's head, though the Outcome claims both"}
- {file: .engineering/planning/story/the-workspace-seam-enters-and-leaves-managed-worktrees.md, line: 41, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "no scenario shows declared setup running in the child's tree, or case.yaml recording parent and worktree"}
- {file: .engineering/planning/story/story-delivery-tests-first.md, line: 50, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "tests_optional_keeps_todays_order joins two cases with or, and it says today's order without naming what is observed; the story.delivery/1 case already holds before the work"}
- {file: .engineering/planning/dependency-blocker/metaharness-qualify-without-a-model-call.md, line: 21, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "it clears only when the blocked takt story's own scenario passes, so it cannot clear before the story it blocks is implemented; it names no upstream state"}
- {file: .engineering/planning/dependency-blocker/worktree-nested-keep-file.md, line: 21, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "it clears on the blocked story's scenario cases_kept::a_finished_tree_keeps_its_case, not on the worktree story's own tests, so it cannot clear first"}
- {file: .engineering/planning/dependency-blocker/engineering-gates-see-the-change.md, line: 22, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "it clears on the blocked story's scenario, not on the engineering-protocols fixtures, so it cannot clear first"}
- {file: .engineering/planning/dependency-blocker/story-delivery-2-with-settings.md, line: 23, category: acceptance, severity: warning, verdict: needs-revision, origin: introduced, message: "it clears on an unnamed takt story run check from one of the two stories it blocks, rather than on the upstream fixtures"}
```
