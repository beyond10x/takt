---
format: aep.planning-md/3
id: review-result:goal-a-plan-parallel-round-1
kind: review-result
status: active
title: Plan critic parallel, goal A cluster-delivery set, round 1
relations:
- reviews: story:a-run-stops-at-a-default-token-budget
- reviews: story:a-killed-step-records-its-tokens
- reviews: story:bot-forge-reads-through-the-app
- reviews: story:forge-preflight
- reviews: story:pr-open-not-idempotent-on-resume
- reviews: story:fenced-forge-dispatch
- reviews: story:an-unattended-run-merges-under-a-delegated-grant
- reviews: story:report-sums-cases
revision: 1
---
needs-revision

Seven stories in the set; `story:report-sums-cases` was read for context only. Four findings:

- story:an-unattended-run-merges-under-a-delegated-grant — it and story:fenced-forge-dispatch both change `PrMerged` in `crates/takt/src/validator.rs` (cited, both bodies), and the plan does not say so. The delegation read sits at `validator.rs:4846-4852` and `:5056-5100`, inside the `PrMerged` that fenced's "merge step" records an outcome in (the `merge_with` call is at `:5357`). U4's acceptance runs the in-memory forge, whose write calls fenced changes. No edge or note joins U3 and U4. Remedies, not chosen: an ordering edge naming `validator.rs` `PrMerged` as the reason, or splitting the surface so the delegation read and the outcome record live apart. Citation: `.engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md:27,47` and `.engineering/planning/story/fenced-forge-dispatch.md:32`.
- story:an-unattended-run-merges-under-a-delegated-grant — it and story:a-killed-step-records-its-tokens both edit `crates/takt/src/run.rs`, 14,300 lines, and neither says so (inferred, both scope lines). U1 edits the hour-budget and harness-error paths; U4 edits the grant step. Their regions are probably different, but neither body names its region. Remedies: an edge recording `run.rs` as the reason, or naming the two regions. Citation: `.engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md:45` and `.engineering/planning/story/a-killed-step-records-its-tokens.md:40`.
- story:a-run-stops-at-a-default-token-budget — it shares `crates/takt-cli/src/main.rs` with story:forge-preflight (`LoopArgs::tokens` at `:1077` against `forge_of` at `:3195`), and `website/docs/reference/configuration.md` with story:bot-forge-reads-through-the-app (the `tokens` row against the bot row at `:336`), and neither body says so (cited, both bodies). The hunks are far apart, so a clean textual merge is likely, but the shared files are unrecorded. Remedies: an edge, or the shared files named as accepted. Citation: `.engineering/planning/story/a-run-stops-at-a-default-token-budget.md:42,46` and `.engineering/planning/story/forge-preflight.md:56`.
- story:fenced-forge-dispatch — its scope names only `forge.rs` and cites lines that no longer hold the code. `forge.rs:293` and `:302` are the end of a struct and `is_object_id`. The write calls are the `Forge` trait at `forge.rs:369` and `:378`, implemented in `forge/bot.rs:544` and `:632`, `forge/connectors.rs:508` and `:570`, `forge/gitlab.rs:728` and `:777`, `forge/local.rs:599` and `:709`, and `forge/command.rs:724` and `:847`. Returning an outcome class changes all five adapters (cited, by grep). That puts `bot.rs` on the surface of U2 and of story:pr-open-not-idempotent-on-resume, whose edge points only at story:bot-forge-reads-through-the-app. Story:forge-preflight also edits `bot.rs` (`for_tree`, `:130`) and no edge reaches it. So "U3 after U2" lives in your message, not in the store. Remedies: a `depends_on` edge from story:fenced-forge-dispatch and story:pr-open-not-idempotent-on-resume to story:forge-preflight with `bot.rs` as the reason, or splitting the adapter change from the validator change. The scope should also be re-cited and widened to the adapters. Citation: `.engineering/planning/story/fenced-forge-dispatch.md:27,31` and `.engineering/planning/story/forge-preflight.md:57`.

Collisions I found no problem with:
- story:bot-forge-reads-through-the-app and story:forge-preflight (`bot.rs`): `forge-preflight` has a `depends_on` edge.
- story:pr-open-not-idempotent-on-resume and story:fenced-forge-dispatch (`PrOpen` and the shared read): pr-open's body says "one unit delivers both".
- The two stories in U1.

**What I read:** 8 artifacts (7 in the set plus `story:report-sums-cases`) and `decision-blocker:unattended-merge-grant`, with `aep plan artifact show` on each, `relations`, `waves`, and grep and sed on `forge.rs`, `forge/*.rs`, `validator.rs`, `run.rs`, `main.rs`, `case.rs` and `configuration.md` at 421f4303.

Surfaces established:

| Basis | Items |
|---|---|
| Cited | 6 |
| Inferred | 1 (`story:a-killed-step-records-its-tokens`) |
| Unplaceable | 0 |

Acceptance reads traced to a producer in the set: 4. A `depends_on` edge records 3 of them; the fourth, fenced reading pr-open's open-PR read, is recorded only in pr-open's body.

**What I could not establish:**
- The `run.rs` regions of U1 and U4. Both are inferred, so finding 2 rests on inferred surfaces.
- Whether the `ess/domains/takt.yaml` entry in fenced (inferred) collides with anything. No other body names `ess/`.
- Out of my lane: U4's `## Ambiguities` still cites `decision-blocker:unattended-merge-grant` as open, but it is cleared (answer A); that is the acceptance and design critics' to judge.
- Out of my lane: `story:forge-preflight` has a front-matter `forge.rs` scope that its body scope omits.
- Outside this set: `story:report-sums-cases` reads the stop-line `tokens` that `story:a-killed-step-records-its-tokens` changes from 0 to unknown, and has no edge to it. The wave order covers that.

```findings
- file: .engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md
  line: 47
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "this and story:fenced-forge-dispatch both change PrMerged in crates/takt/src/validator.rs (cited: delegation read at 4846-5100, the merge step's outcome record) and the in-memory forge its acceptance runs, and neither body or edge says so; record an ordering edge naming validator.rs PrMerged, or split the surface"
- file: .engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md
  line: 45
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "this and story:a-killed-step-records-its-tokens both edit crates/takt/src/run.rs and neither says so (inferred surface on both); record an edge naming run.rs or name the two regions"
- file: .engineering/planning/story/a-run-stops-at-a-default-token-budget.md
  line: 42
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "this shares crates/takt-cli/src/main.rs with story:forge-preflight and website/docs/reference/configuration.md with story:bot-forge-reads-through-the-app (distinct hunks, cited in both bodies) and neither body says so; record an edge or name the shared files as accepted"
- file: .engineering/planning/story/fenced-forge-dispatch.md
  line: 27
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the scope cites forge.rs:293/302/1051/1149 for the write calls but they live in the Forge trait (forge.rs:369,378) and five adapters (bot.rs, connectors.rs, gitlab.rs, local.rs, command.rs), so the real surface includes bot.rs, shared with story:forge-preflight (for_tree, bot.rs:130) and story:pr-open-not-idempotent-on-resume, whose edges point only at story:bot-forge-reads-through-the-app; re-cite and widen the scope and add an edge to story:forge-preflight naming bot.rs, or split the adapter change from the validator change"
```
