---
format: aep.planning-md/3
id: review-result:tui-scope-round-2
kind: review-result
status: active
title: Scope critic, terminal frontend, round 2
relations:
- reviews: epic:terminal-frontend
- reviews: story:tui-read-model
- reviews: story:tui-ui-document
- reviews: story:takt-serve-read-only
- reviews: story:takt-tui-crate
- reviews: story:tui-approve-action
revision: 1
---
approve

I read the epic, the five stories, the blocker and the round-1 scope result, and nothing I found would change what was drafted.

**What I read:** 8 artifacts. I read `epic:terminal-frontend` first, then each story and `dependency-blocker:approve-route-credential`, with `aep plan artifact show`. I also ran `aep plan artifact graph` and read `review-result:tui-scope-round-1`. Where `show` truncated, I read the story files under `.engineering/planning/story/` directly.

**Coverage check:** I extracted 14 promises from the epic (9 from Outcome and Context, 5 from Acceptance) and traced all 14 to an item. The numbers match round 1.

| Promise | Claimed by |
|---|---|
| Cases of a repository, each case's frontier, run events | story:tui-read-model declares the views. story:tui-ui-document draws them. story:takt-serve-read-only serves them. |
| Pending approvals with the `takt approve` line | story:tui-read-model (capability, bound revision). story:tui-ui-document (the text, `tui-ui-document.md:38`). |
| Scorecard and journey of each run | story:tui-read-model and story:tui-ui-document |
| `ess-ui/1` document checked against the model | story:tui-ui-document (`ess ui check --model ess`) |
| Read-only local `takt serve` | story:takt-serve-read-only |
| `ess verify conform` holds the served data to the model | Deferred by name, not a gap. The epic says "when an ESS release takt pins ships one, a story adds it", and story:takt-serve-read-only line 120 excludes it. |
| App generated into its own crate | story:takt-tui-crate |
| The frontend writes nothing | story:tui-read-model and story:tui-ui-document (no command declared). story:takt-serve-read-only (405, 404, tree digest). story:takt-tui-crate (no `ESS_UI_AUTHORIZATION`). |
| Acceptance 1: `takt tui --screen-once 120x40` | story:takt-tui-crate |
| Acceptance 2: `ess ui test`, case detail and approvals page | story:tui-ui-document |
| Acceptance 3: `cargo test -p b10x-takt-serve --test views` | story:takt-serve-read-only |
| Acceptance 4: 405 and 404, tree digest unchanged | story:takt-serve-read-only |
| Acceptance 5: no `ess verify conform` HTTP target | Stated as a limit, so there is nothing to claim. |

- **Gap:** none. Round 1 found none, and neither does this round.
- **Reach beyond the parent:** none.
  - story:tui-approve-action is the one write route. The epic's decision section and story 5 defer it, and `dependency-blocker:approve-route-credential` blocks it.
  - The regeneration check and the AGENTS.md install line in story:takt-tui-crate trace to "generated from the document".
  - The symlink refusal in story:takt-serve-read-only traces to "read-only".
- **Two items claiming one outcome:** none. Each of the four active stories (read-model, ui-document, serve, tui-crate) owns a different step. The fixture bodies appear in both story:tui-ui-document and story:takt-serve-read-only, but the second only checks that the two copies are equal.
- **Narrowing:** none. The one thing the set does not do is conform, and the epic records that.

**What I could not establish:** none within my lane. These are out of my lane and did not set the verdict.
- The epic says "The order is recorded as edges", but "after release 0.2.0" has no edge in the graph. That is design or sequencing.
- story:takt-serve-read-only Outcome still says "`ess verify conform` holds it to the model" while its acceptance excludes it. That is acceptance or design.
- story:takt-tui-crate says the doc-writer documents the app "in the site", but no scope path covers the site. That is design.
- story:tui-ui-document has no test for the scorecard or journey pages. That is acceptance.
- I did not re-check the ESS 0.60.0 line citations. Round 1 did.

```findings
[]
```
