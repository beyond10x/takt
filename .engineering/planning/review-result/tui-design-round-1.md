---
format: aep.planning-md/3
id: review-result:tui-design-round-1
kind: review-result
status: active
title: Design critic, terminal frontend, round 1
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

Read 7 artifacts (the epic, the five stories and the blocker) with `aep plan artifact show`, then `relations`, `graph` and `validate`. `validate` reported `valid`. Its only complaints are a story outside this set with no scope and five review-results with no findings block, and none of them is mine.

I walked the whole graph, not just these ids. The set's own edges are:
- `tui-ui-document` and `takt-serve-read-only` each `depends_on` `tui-read-model`.
- `takt-tui-crate` `depends_on` both of those.
- `tui-approve-action` `depends_on` `takt-tui-crate`, and the blocker `blocks` it.

There is no cycle, and the set is not a queue, because `tui-ui-document` and `takt-serve-read-only` can run in parallel. I traced six acceptance reads to a producer in the set, and an edge records all six:
- `tui-ui-document` reads the views and component from `tui-read-model`.
- `takt-serve-read-only` reads the model and the HTTP binding from `tui-read-model`.
- `takt-tui-crate` reads `ess/ui/takt.ui.yaml` from `tui-ui-document`.
- `takt-tui-crate` reads the server from `takt-serve-read-only`.
- `tui-approve-action` reads the crate from `takt-tui-crate`.
- `tui-approve-action` reads `crates/takt-serve` from `takt-serve-read-only`, reached transitively through `takt-tui-crate`.

The spec-first split (model, UI document, server, crate) is not a horizontal slice. Each item can be shown working on its own with its own ESS command: `specify validate`, `ui test`, `verify conform`, `--screen-once`. The `tui-read-model` to `takt-serve-read-only` seam has a one-way, recorded edge, and the epic names spec-first as its method.

What I could not establish:
- Out of my lane, and not set against the verdict: the epic's rule "no story joins a wave while the RSI stories are open" is prose only. `aep plan artifact waves` puts `story:tui-read-model` in wave 7 regardless. That is a dependency outside the set, and the scope or parallel-safety critic can judge it.
- Out of my lane: `story:takt-tui-crate` edits `Cargo.toml` and `Cargo.lock`, which its body calls takt's judge surface. No edge points to `story:judge-guard`, which is active. Parallel safety owns this.
- I checked the ESS 0.60.0 citations the set rests on and they match: `generate.rs:9` (`ESS_UI_AUTHORIZATION`), `http.rs:171-172` (`http://` only) and `app.rs:193-195` (polling). I did not check the other cited ESS lines.

```findings
[]
```
