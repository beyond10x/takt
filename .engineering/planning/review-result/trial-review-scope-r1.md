---
format: aep.planning-md/3
id: review-result:trial-review-scope-r1
kind: review-result
status: active
title: Scope critic, downstream trial-review set, round 1
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
story:step-commits-follow-a-configurable-style — the body lets the repository's commit rule win only "when `style` is unset", so an explicit `style: semantic` in the run file overrides a commit rule in `AGENTS.md`/`CLAUDE.md`, which narrows the operator's "The repository's base instructions (AGENTS.md or CLAUDE.md) win when they define one" without recording that choice — .engineering/planning/story/step-commits-follow-a-configurable-style.md:45
story:every-harness-reads-the-repository-instructions-from-the-base — its top-level run-file key `context` (listed files, recorded by path and sha256) claims the same outcome as the `context` key that the active `story:harness-section-is-metaharness-session-request` admits in the `harness:` mapping (.engineering/planning/story/harness-section-is-metaharness-session-request.md:63), so the body should reuse that key or say why it needs a second surface — .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md:32
story:every-harness-reads-the-repository-instructions-from-the-base — it says the draft `story:codex-step-context` is stale and "This story closes it", but it records only `informed_by`, so both stories claim "a Codex step is given AGENTS.md" and both can be marked done — .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md:49
story:harness-program-is-chosen-per-run — it adds `program` under the run file's `harness:` mapping, which the active `story:harness-section-is-metaharness-session-request` excludes ("A binary path per harness", :103) and refuses through its admitted-six list (:63), and the body does not amend that story — .engineering/planning/story/harness-program-is-chosen-per-run.md:32
Read: 20 takt artifacts in full: 2 new epics, `epic:continuous-orchestration`, `design:continuous-orchestration`, 15 stories, 4 dependency-blockers. Also read `epic:run-config-proxies-llm-and-metaharness`, `story:one-run-file-for-host-and-loom-agent`, `story:harness-section-is-metaharness-session-request`, `story:codex-step-context`, `story:cases-live-under-dot-takt` and `story:gate-commands-find-programs-on-a-system-path`, using `aep plan artifact show`, `cat -n`, `grep` and `aep plan artifact validate` (valid). Promises: `epic:downstream-story-delivery` 9, all 9 traced. `epic:protocol-settings-in-the-run-file` 5, 4 traced. Of `epic:continuous-orchestration`, only the step-2 Workspace seam is reached, 1 of 1 traced. Operator decisions 4, all traced, 1 of them narrowed (commit style).
Not established: "Every protocol takt runs declares its settings, variants and options as Canon `case.inputs`" (epic/protocol-settings-in-the-run-file.md outcome). Only `story.delivery/2` gets a story. Six of eight engineering-protocols protocols declare no inputs, and I could not tell whether they have variants that are not declared.
Not established, outside my lane:
- The Workspace seam's acceptance 2 needs a pause. That is build step 7, which no story claims and no `depends_on` names.
- The amended `story:story-delivery-tests-first` uses the `protocol` key and the shipped defaults, but has no `depends_on` on either story that provides them.
- `every-harness-reads` has no Codex scenario.
```findings
- file: .engineering/planning/story/step-commits-follow-a-configurable-style.md
  line: 45
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body lets the repository's commit rule win only when style is unset, so an explicit style: semantic in the run file overrides a commit rule in AGENTS.md/CLAUDE.md, which narrows the operator's 'The repository's base instructions (AGENTS.md or CLAUDE.md) win when they define one' without recording that choice"
- file: .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md
  line: 32
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its top-level run-file key context (listed files, recorded by path and sha256) claims the same outcome as the context key that the active story:harness-section-is-metaharness-session-request admits in the harness: mapping (.engineering/planning/story/harness-section-is-metaharness-session-request.md:63), so the body should reuse that key or say why it needs a second surface"
- file: .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md
  line: 49
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it says the draft story:codex-step-context is stale and 'This story closes it', but it records only informed_by, so both stories claim 'a Codex step is given AGENTS.md' and both can be marked done"
- file: .engineering/planning/story/harness-program-is-chosen-per-run.md
  line: 32
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it adds program under the run file's harness: mapping, which the active story:harness-section-is-metaharness-session-request excludes ('A binary path per harness', :103) and refuses through its admitted-six list (:63), and the body does not amend that story"
```
