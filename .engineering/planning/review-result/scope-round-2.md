---
format: aep.planning-md/3
id: review-result:scope-round-2
kind: review-result
status: active
title: Scope critic, round 2
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
- reviews: story:story-done-end-to-end
- reviews: story:takt-domain-model
- reviews: story:week-of-daily-runs
- reviews: story:workflow-schema-and-loader
- reviews: epic:composable-agentic-work
- reviews: vision:safe-agentic-coding
revision: 1
---
needs-revision
story:workflow-schema-and-loader — the M3 proof "the same story from a YAML file" is claimed by no item: this acceptance runs only the intent case on the scratch crate, and story:story-done-end-to-end puts "the run from a `workflow/1` file" out of scope and sends it back here, so each defers to the other and no body records that the proof is dropped — .engineering/planning/story/workflow-schema-and-loader.md:63 (promise at docs/design/takt.md:74)
story:workflow-schema-and-loader — the `approval` block (capability, approver; docs/design/takt.md:60) is mapped to "`approval` → recorded for story:merge-under-authority", but that story reads no workflow `approval` block (its grant is `takt approve` against the protocol's capability, and it lists checking the block's `approver` as Out of Scope), so the block's capability has no claimant and "recorded" says nothing about where — .engineering/planning/story/workflow-schema-and-loader.md:40 (and .engineering/planning/story/merge-under-authority.md:79)
story:takt-domain-model — the epic promises the nouns "are declared in an ESS domain document before stories build on them", yet no story `depends_on` this one (the graph shows only `decomposes` and `serves` edges on it) and the body records neither that ordering nor that it is waived — .engineering/planning/epic/composable-agentic-work.md:17 (command: `aep plan artifact graph | grep takt-domain-model`)

**What I read.** 17 artifacts: the epic, 13 draft stories, vision:safe-agentic-coding, review-result:scope-round-1 and `docs/design/takt.md` in full. Commands: `aep plan artifact show` on each id, `aep plan artifact list --kind story --status draft`, `aep plan artifact graph`, `cat -n docs/design/takt.md`, and `grep -n` on the story files for line numbers.

**Round-1 findings.**
- Finding 1 (second-harness-adapter) is fixed. Context line 47 and Out of Scope now name the deferral to story:story-done-end-to-end, and that story's last Check carries the parity proof.
- Finding 2 (takt-domain-model reach) is fixed. The epic's Why now carries the ESS sentence at line 17, and the story traces to it. The `AGENTS.md:37` edit is a consequence of that sentence, not reach.
- Finding 3 (`gate` block) is fixed. story:gate-validator now evaluates the block, story:workflow-schema-and-loader hands it over, and each Out of Scope points the right way.

**Promise count.** I extracted 26 promises: 7 from the epic and 19 from design §§ 3 and 4. I traced 24 to an item.
- The two not traced are the M3 proof and the `approval` block, which are findings 1 and 2.
- The epic acceptance is claimed by story:story-done-end-to-end, whose Checks cover "none by the harness's text".
- Nothing reaches into § 5. Confinement, the report command and the approve command all trace to § 3 Seams, M5 and the cleared blocker.

**What I could not establish.**
- Finding 3 sits on the line between my lane and the design critic's. I judged it against a literal epic sentence, so I kept it as a scope finding.
- Out of my lane, not setting the verdict. story:takt-domain-model marks "who writes it" and "whether a later run resumes that case" as `UNMAPPED`, citing decision-blocker:approval-grant-and-case-resume. That blocker is cleared and decided in story:merge-under-authority, so the marker is stale.
- Out of my lane, not setting the verdict. story:story-done-end-to-end has no `depends_on` on story:second-harness-adapter, so its "Once ... is implemented" Check can sit unexercised. Sequencing is the design critic's and the parallel-safety critic's.

```findings
- file: .engineering/planning/story/workflow-schema-and-loader.md
  line: 63
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the M3 proof 'the same story from a YAML file' (docs/design/takt.md:74) is claimed by no item: this acceptance runs only the intent case on the scratch crate, and story:story-done-end-to-end puts 'the run from a workflow/1 file' out of scope and sends it back here, so each defers to the other and no body records that the proof is dropped"
- file: .engineering/planning/story/workflow-schema-and-loader.md
  line: 40
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the approval block (capability, approver; docs/design/takt.md:60) is mapped to 'recorded for story:merge-under-authority', but that story reads no workflow approval block (its grant is takt approve against the protocol's capability, and checking the approver is Out of Scope at merge-under-authority.md:79), so the block's capability has no claimant and 'recorded' names no place"
- file: .engineering/planning/epic/composable-agentic-work.md
  line: 17
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic promises the nouns 'are declared in an ESS domain document before stories build on them', yet no story depends_on story:takt-domain-model (the graph shows only decomposes and serves edges on it) and its body records neither that ordering nor that it is waived"
```
