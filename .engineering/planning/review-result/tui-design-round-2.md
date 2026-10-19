---
format: aep.planning-md/3
id: review-result:tui-design-round-2
kind: review-result
status: active
title: Design critic, terminal frontend, round 2
relations:
- reviews: epic:terminal-frontend
- reviews: story:tui-read-model
- reviews: story:tui-ui-document
- reviews: story:takt-serve-read-only
- reviews: story:takt-tui-crate
- reviews: story:tui-approve-action
revision: 1
---
needs-revision
story:takt-serve-read-only — its acceptance compares every served body with the matching `ess/ui/fixtures/` entry, which story:tui-ui-document creates, and no `depends_on` edge runs from it to story:tui-ui-document; add that edge, or drop the cross-copy comparison from this story and leave it to story:takt-tui-crate, which already depends on both — .engineering/planning/story/takt-serve-read-only.md:55,61 and .engineering/planning/story/tui-ui-document.md:33,48

The two options trade off as follows.
- **Add the edge.** The set then runs as a queue: read-model, ui-document, serve, crate. `aep plan artifact waves` places serve beside ui-document today, so serve would be checked against a fixture that does not exist yet.
- **Drop the comparison.** Serve checks only its own `crates/takt-serve/tests/fixtures` copies, and the "the pages tested are the pages served" check moves to story:takt-tui-crate. This keeps serve and ui-document parallel. The epic's acceptance line at `.engineering/planning/epic/terminal-frontend.md:51` would then need rewording.

**What I read.** I read 7 artifacts and review-result:tui-design-round-1 with `aep plan artifact show`, then ran `relations`, `graph`, `validate` and `waves`. I walked the whole graph (146 `depends_on` and `blocks` edges, outside the set included). `tsort` found no cycle. `validate` reported `valid`, and its only complaints are outside this set. The round-1 gaps are closed: the edges to story:judge-guard and story:self-improvement-protocol are now recorded on read-model, serve, crate and approve-action. I traced 9 acceptance reads to a producer in the set. Eight have an edge, and the missing one is the finding above.

The 9 reads:
- ui-document reads the views and component from read-model.
- serve reads the model and binding from read-model.
- crate reads the ui document from ui-document.
- crate reads the server from serve.
- approve-action reads the crate from takt-tui-crate.
- approve-action reads `crates/takt-serve`, through the crate.
- approve-action reads `ess/ui/takt.ui.yaml` and the `ess/http/takt.http.yaml` binding, through the crate and serve.
- approve-action's blocker is recorded as `blocks`.
- serve reads `ess/ui/fixtures`, which is the missing edge.

**What I could not establish**
- Out of my lane (acceptance): story:tui-approve-action's acceptance ends with "`ess verify conform` covers the command". The epic itself says ESS 0.60.0 has no HTTP target (epic:terminal-frontend, Acceptance, last paragraph).
- Out of my lane (acceptance): the serve acceptance calls an `ess/ui/fixtures` entry "a JSON body byte-equal". In `examples/partner-portal` at ess 0.60.0 (`git show 0.60.0:examples/partner-portal/fixtures/index.yaml`), fixtures are YAML indexed per view, and some files are shared by several views. That makes "the same file" per route unclear.
- Out of my lane (parallel safety): the epic's claim that the read-model edges cover "every active story" holding `ess/domains/takt.yaml` is not true on the store. `aep plan artifact waves` still lists collisions of tui-read-model with story:an-intent-run-can-finish, story:bench-nouns-in-ess and others, for example at line 317.
- I re-read ess 0.60.0 `generate.rs:1-15`, `http.rs:165-175` and `app.rs:188-198`. They match the epic's claims about `ESS_UI_AUTHORIZATION`, the `http://`-only base URL and `live:` polling. I did not recheck `generate.rs:226-249`.

```findings
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 61
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance compares every served body with the matching ess/ui/fixtures/ entry, which story:tui-ui-document creates, and no depends_on edge runs from it to story:tui-ui-document; add that edge, or drop the cross-copy comparison from this story and leave it to story:takt-tui-crate, which already depends on both"
```
