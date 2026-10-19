---
format: aep.planning-md/3
id: review-result:tui-scope-round-1
kind: review-result
status: active
title: Scope critic, terminal frontend, round 1
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

I read the epic, the five stories and the blocker, and found nothing that would change what was drafted.

**What I read:** 7 artifacts, each with `aep plan artifact show` (`epic:terminal-frontend` first, before the stories). I also ran `aep plan artifact graph` and `list`, and read `release-plan:takt-roadmap`, `product-requirements:takt` and `review-result:tui-design-round-1`.

**Coverage check:** I extracted 14 promises from the epic and traced all 14 to an item. The epic's Outcome gave 9 and its Acceptance gave 5.

| Promise | Claimed by |
|---|---|
| Case list, each case's frontier, run events (3) | story:tui-read-model declares the views; story:tui-ui-document draws them |
| Pending approvals with the `takt approve` line | story:tui-read-model (capability, bound revision); story:tui-ui-document (the text) |
| Scorecard and journey | story:tui-read-model; story:tui-ui-document |
| `ess-ui/1` document checked against the model | story:tui-ui-document |
| Read-only local `takt serve` held by `ess verify conform` | story:takt-serve-read-only |
| App generated into its own crate | story:takt-tui-crate |
| The frontend writes nothing | story:takt-serve-read-only (405 and 404, tree digest); story:tui-read-model and story:tui-ui-document (no command declared) |
| Acceptance: `takt tui --repo … --screen-once 120x40` | story:takt-tui-crate |
| Acceptance: case shows claims, actions and outcomes | story:tui-ui-document |
| Acceptance: approvals page at `ApprovalRequired` | story:tui-ui-document |
| Acceptance: conform passes for every view | story:takt-serve-read-only |
| Acceptance: 405 or 404 to non-GET, no file changed | story:takt-serve-read-only |

- **Reach beyond the parent:** none. The regeneration check, the `AGENTS.md` install line and the symlink refusal each trace to "generated from the document" or "read-only". story:tui-approve-action is the one write, and the epic names it as deferred (the decision section and story 5, "blocked (above)"). It is not reach.
- **Two items claiming one outcome:** none. story:tui-read-model declares the views, story:tui-ui-document draws them, story:takt-serve-read-only serves them and story:takt-tui-crate runs them.
- **Narrowing:** none. Scorecard and journey, the run events and the `takt tui` launcher are each claimed.
- **Cited facts:** I checked the ESS 0.60.0 lines the epic and stories cite. `generate.rs` (the `--base-url` and `--screen-once` flags, `ESS_UI_AUTHORIZATION`, the Git-tag dependency), `http.rs:171-172` and `app.rs:193-195` match the epic and stories. `check.yml` pins `ESS_VERSION: "0.60.0"`, and `harness.rs:272-275` disallows `Bash,WebFetch` as stated. I read `generate.rs` only up to line 250 and did not open the cited `http.rs` and `app.rs` lines. I confirmed `grep -c reached_by ess/domains/takt.yaml` prints 0 and that no `events.jsonl` is committed under `docs/runs/`.

**What I could not establish:**
- None within my lane. Out of my lane, and they did not set the verdict:
  - The epic's rule "after release 0.2.0; no story joins a wave while the RSI stories are open" is prose only, and `release-plan:takt-roadmap` does not mention the epic. That is sequencing, so design or parallel-safety owns it.
  - story:tui-approve-action's acceptance (it writes a grant) contradicts the epic's "writes nothing" and its 405 acceptance if both were ever done together. The epic names the deferral, so I did not count it.
  - story:tui-read-model and story:tui-ui-document will both edit the model and binding files. I left that to parallel-safety.
  - Whether the acceptance clauses are checkable is acceptance's lane.

```findings
[]
```
