---
format: aep.planning-md/3
id: review-result:trial-review-design-r1
kind: review-result
status: active
title: Design critic, downstream trial-review set, round 1
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
story:every-harness-reads-the-repository-instructions-from-the-base — it adds a top-level run-file `context` key and flag for the files given to every harness. That is the surface `story:harness-section-is-metaharness-session-request` already admits as `harness: {context}` and `story:harness-flags-spell-metaharness-names` adds as `--context` (metaharness's own `context`, which already hands every kind declared files with path and sha256). The body has to make itself the read-at-base rule of that one key instead of a second key and flag — .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md:32, .engineering/planning/story/harness-section-is-metaharness-session-request.md:63, .engineering/planning/story/harness-flags-spell-metaharness-names.md:47
story:an-unqualified-harness-stops-before-the-first-session — its always-on stop overlaps the `strict-version` key and flag that harness-section and harness-flags own for metaharness's version refusal. It also contradicts harness-flags' acceptance, where an out-of-pin version runs without the flag. The body has to say how the preflight and `strict-version` relate — .engineering/planning/story/an-unqualified-harness-stops-before-the-first-session.md:30, .engineering/planning/story/harness-flags-spell-metaharness-names.md:60
story:harness-program-is-chosen-per-run — it puts `program` into the `harness:` mapping, which harness-section owns as an admitted list of exactly six keys with "a binary path per harness" out of scope. Only an informed_by edge links them, and the waves place this story (wave 18) before harness-section (wave 24). The run-file half needs a depends_on edge to harness-section, or its own story that amends the admitted list — .engineering/planning/story/harness-program-is-chosen-per-run.md:31, .engineering/planning/story/harness-section-is-metaharness-session-request.md:103
story:takt-lists-a-protocols-settings — acceptance 1 reads the built-in defaults (`required`, `after_handover`) that story:takt-ships-default-protocol-settings creates. No depends_on edge runs to it, and the waves put the reader (wave 26) before the producer (wave 27) — .engineering/planning/story/takt-lists-a-protocols-settings.md:46, .engineering/planning/story/takt-ships-default-protocol-settings.md:31
story:story-delivery-tests-first — its amended acceptance reads the `gate_on: after_handover` default of story:takt-ships-default-protocol-settings and the run-file `protocol` key, and no depends_on edge runs to story:takt-ships-default-protocol-settings — .engineering/planning/story/story-delivery-tests-first.md:49, .engineering/planning/story/takt-ships-default-protocol-settings.md:31
story:the-workspace-seam-enters-and-leaves-managed-worktrees — the outcome keeps each child's records under the root's cases directory, which the design says is never in a child's worktree. Yet acceptance 3 and the depends_on edge to story:a-cases-root-survives-worktree-cleanup expect the child tree's finish archive to hold `.takt/cases/<id>/`. The body has to pick one, and the edge either stays or goes with that choice — .engineering/planning/story/the-workspace-seam-enters-and-leaves-managed-worktrees.md:36, .engineering/planning/design/continuous-orchestration.md:131
Read: 22 artifacts plus 8 neighbours (the run-config-proxies epic and four of its stories, the continuous-orchestration epic and design, story:cases-live-under-dot-takt, story:gate-commands-find-programs-on-a-system-path). Commands: `relations`, `graph --format json`, `validate` (valid), `waves`. I walked 202 depends_on/blocks edges across the whole store with tsort and found no cycle; the edges proposed above add none. I traced 13 acceptance reads to a producer, and 10 of them have an edge.
Not established: whether loom-sdk already re-exports `metaharness::qualify`. No loom item is in the set, and I could not cite a source inside these worktrees.
Not established: the forge stories' need for story:scan-and-forge-are-adapters. It is unrecordable until that story reaches the store, and it is outside the set, so it is not a finding.
Not established (outside my lane): scopes cite `crates/takt/src/main.rs`, a file that does not exist in this tree (the CLI is `crates/takt-cli/src/main.rs`). The epic's gate-program failure (epic line 37) has no story in this epic.
```findings
- {file: .engineering/planning/story/every-harness-reads-the-repository-instructions-from-the-base.md, line: 32, category: design, severity: blocker, verdict: needs-revision, origin: introduced, message: "it adds a top-level run-file context key and flag for the files given to every harness, the surface harness-section already admits as harness.context and harness-flags adds as --context; the body must make itself the read-at-base rule of that one key instead of a second key and flag"}
- {file: .engineering/planning/story/an-unqualified-harness-stops-before-the-first-session.md, line: 30, category: design, severity: blocker, verdict: needs-revision, origin: introduced, message: "its always-on stop overlaps the strict-version key and flag that harness-section and harness-flags own for metaharness's version refusal, and contradicts harness-flags' acceptance where an out-of-pin version runs without the flag; the body must say how the preflight and strict-version relate"}
- {file: .engineering/planning/story/harness-program-is-chosen-per-run.md, line: 31, category: design, severity: warning, verdict: needs-revision, origin: introduced, message: "it puts program into the harness mapping, which harness-section owns as an admitted list of six keys with a binary path out of scope, linked only by informed_by and placed in an earlier wave; the run-file half needs a depends_on edge to harness-section or its own story that amends the admitted list"}
- {file: .engineering/planning/story/takt-lists-a-protocols-settings.md, line: 46, category: design, severity: warning, verdict: needs-revision, origin: introduced, message: "acceptance 1 reads the built-in defaults story:takt-ships-default-protocol-settings creates, with no depends_on edge to it, and the waves put the reader before the producer"}
- {file: .engineering/planning/story/story-delivery-tests-first.md, line: 49, category: design, severity: warning, verdict: needs-revision, origin: introduced, message: "its amended acceptance reads the gate_on default of story:takt-ships-default-protocol-settings and the run-file protocol key, with no depends_on edge to story:takt-ships-default-protocol-settings"}
- {file: .engineering/planning/story/the-workspace-seam-enters-and-leaves-managed-worktrees.md, line: 36, category: design, severity: warning, verdict: needs-revision, origin: introduced, message: "the outcome keeps child records under the root cases directory, never in a child worktree, yet acceptance 3 and the depends_on edge to story:a-cases-root-survives-worktree-cleanup expect the child tree's archive to hold them; the body must pick one"}
```
