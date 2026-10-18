---
format: aep.planning-md/3
id: review-result:scope-round-1
kind: review-result
status: active
title: Scope critic, round 1
relations:
- reviews: story:confinement-seam
- reviews: story:declared-loops
- reviews: story:forge-pull-request-evidence
- reviews: story:frontier-next-ready-story
- reviews: story:gate-validator
- reviews: story:merge-under-authority
- reviews: story:run-record-and-story-evidence
- reviews: story:second-harness-adapter
- reviews: story:story-delivery-protocol
- reviews: story:takt-domain-model
- reviews: story:week-of-daily-runs
- reviews: story:workflow-schema-and-loader
- reviews: decision-blocker:approval-grant-and-case-resume
- reviews: epic:composable-agentic-work
revision: 1
---
needs-revision

second-harness-adapter — the M4 proof "the M2 story runs under both adapters with the same decision documents" is quoted in Context but the acceptance narrows it to the scratch-crate failing-test run, and no body records that the M2-story proof is deferred to the undrafted end-to-end story — .engineering/planning/story/second-harness-adapter.md:47 (promise at docs/design/takt.md:75)
takt-domain-model — no sentence in the epic or in design §§ 3-5 asks for an ESS domain document, `ess/` files or an edit to `AGENTS.md`; the Context cites the planning skill, not the parent, so the outcome reaches beyond what the epic promises — .engineering/planning/epic/composable-agentic-work.md:10-22 (nothing about a specification; AGENTS.md:37 only says none exists)
workflow-schema-and-loader — the design's `gate` block ("a `canon-expr/1` assertion and its provider", docs/design/takt.md:59) has no claimant: this body maps `gate` to story:gate-validator "when present", while that story evaluates only the repository's `.engineering/gates.yaml` and lists "`gate` blocks inside `workflow/1`" as Out of Scope, so each defers to the other; this body must say how a gate block's assertion is evaluated or record that it is not — .engineering/planning/story/workflow-schema-and-loader.md:35 (and .engineering/planning/story/gate-validator.md:53)

**What I read.** 14 artifacts: the epic, 12 draft stories and the decision-blocker, plus `docs/design/takt.md` in full and `AGENTS.md:20-37`. Commands: `aep plan artifact show` on each id, `aep plan artifact list --kind story --status draft`, `aep plan artifact graph`, and `cat -n` on the epic file and the design.

**Promise count.** I extracted 17 promises from the epic and the design sections it cites: 6 from the epic (workflow YAML, harness drives, validated with AEP as record, intent or frontier unit, iteration/hours budgets, the acceptance) and 11 from the design (M2 to M5 deliverables, the five seams, § 3 blocks). I traced 15 to an item.
- **Not traced, drafter named:** the end-to-end acceptance and the M2 and M4 end-to-end proofs. decision-blocker:approval-grant-and-case-resume ("What this stops") names the end-to-end story as not drafted, so I did not count it as a gap.
- **Traced only partly:** the `gate` block, which is the third finding.

**What I could not establish.**
- I did not read the archived stories; the graph shows them as `decomposes` the epic but they are outside the set. That is why the end-to-end omission is counted as drafter-named and not as covered by `story:story-end-to-end`.
- Out of my lane, so not setting the verdict: the third finding could also be read as a design question for `plan-critic-design` (circular hand-off between two stories).
- The acceptance of story:second-harness-adapter and several others use "at least" and "equal in every claim value" wording; whether they are checkable is `plan-critic-acceptance`'s lane.

```findings
- file: .engineering/planning/story/second-harness-adapter.md
  line: 47
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the M4 proof 'the M2 story runs under both adapters with the same decision documents' (docs/design/takt.md:75) is quoted in Context but the acceptance narrows it to the scratch-crate failing-test run, and no body records that the M2-story proof is deferred to the undrafted end-to-end story"
- file: .engineering/planning/epic/composable-agentic-work.md
  line: 10
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:takt-domain-model declares an ESS domain document and edits AGENTS.md, but no sentence in the epic or in docs/design/takt.md sections 3 to 5 asks for it; the outcome is not traceable to the parent"
- file: .engineering/planning/story/workflow-schema-and-loader.md
  line: 35
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the design's gate block (a canon-expr/1 assertion and its provider, docs/design/takt.md:59) has no claimant: this body maps gate to story:gate-validator 'when present' while story:gate-validator evaluates only .engineering/gates.yaml and lists gate blocks inside workflow/1 as Out of Scope (gate-validator.md:53), so each defers to the other"
```
