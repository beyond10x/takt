---
format: aep.planning-md/3
id: review-result:tui-parallel-safety-round-2
kind: review-result
status: active
title: Parallel-safety critic, terminal frontend, round 2
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
story:takt-serve-read-only — its acceptance reads `ess/ui/fixtures/` entries that story:tui-ui-document creates, and no `depends_on` edge runs from serve to the UI document, so `aep plan artifact waves` puts both in wave 36. Add the edge, or merge the two (reader beside producer; the serve story also claims `crates/takt-serve/tests/fixtures` as "the same files" the UI document commits) — .engineering/planning/story/takt-serve-read-only.md:61, .engineering/planning/story/tui-ui-document.md:48
story:takt-tui-crate — it shares `Taskfile.yml` and `AGENTS.md` with active story:docs-site-with-sidebar (cited, its `## Scope` section) and `Cargo.lock` with it (inferred: its new `crates/takt-docs` crate). Neither body names the other, no edge exists, and the docs-site store `scope` field is empty so the wave derivation cannot see it. Remedy: an ordering edge that records the shared files, or split the surface so the two stop sharing them — .engineering/planning/story/takt-tui-crate.md:61, .engineering/planning/story/docs-site-with-sidebar.md:30-32
story:takt-serve-read-only — it shares `Cargo.lock` with active story:docs-site-with-sidebar (inferred: that story adds the `crates/takt-docs` workspace member) and the body does not say so. Remedy: an ordering edge that records the shared file, or split the surface — .engineering/planning/story/takt-serve-read-only.md:54, .engineering/planning/story/docs-site-with-sidebar.md:29

What I read: 7 artifacts with `aep plan artifact show`, plus `aep plan artifact waves` and `list`. I also read the 11 active stories (full scope and relations; bodies of docs-site-with-sidebar, judge-guard, self-improvement-protocol, story-delivery-tests-first) and review-result:tui-parallel-safety-round-1. I ran `git grep` for scopes under `.engineering/planning/story/`, and read root `Cargo.toml` and `crates/takt/src/lib.rs`.

Round 1 status:
- The `ess/domains/takt.yaml` and `takt.citations.tsv` collisions with story-delivery-tests-first, scorecard-save-checks-binary, promotion-chain-head-anchored and check-finding-run-step-confined are fixed by edges from tui-read-model.
- The `main.rs` collisions are fixed: serve and tui-crate now depend on judge-guard and self-improvement-protocol.
- `Cargo.lock` is in scope and serve is a crate, not a module.
- tui-approve-action now depends on judge-guard.
- The three findings above are new. None repeats a round-1 finding.

Surfaces and reads:
- **Surfaces:** 5 stories placed by paths their bodies cite, which the store marks `inferred`. 0 unplaceable. `crates/takt-serve`, `crates/takt-tui`, `ess/http/` and `ess/ui/` do not exist yet.
- **Acceptance reads traced to a producer in the set:** 7. A `depends_on` edge records 6. The unrecorded one is the first finding.

What I could not establish:
- Whether `crates/takt-serve` needs new public functions in `crates/takt/src/case.rs` (which active stories cases-root-link-is-undone and judge-guard touch) to list cases. Serve's scope omits `crates/takt/src`, and `case.rs` has no case-listing function. I could not tell whether serve must add one, so I raise no finding.
- Out of my lane: the serve acceptance names "the fixture repository" and nothing says which item creates it.

```findings
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 61
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reads the ess/ui/fixtures entries that story:tui-ui-document creates (tui-ui-document.md:48) and no depends_on edge runs from takt-serve-read-only to tui-ui-document, so both land in wave 36; add the edge or merge the two stories"
- file: .engineering/planning/story/takt-tui-crate.md
  line: 61
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares Taskfile.yml and AGENTS.md (cited, docs-site-with-sidebar.md:30-32) and Cargo.lock (inferred from its new crates/takt-docs crate) with active story:docs-site-with-sidebar, whose store scope is empty so the wave derivation misses it; neither body names the other; remedy is an ordering edge recording the shared files, or splitting the surface"
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 54
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "shares Cargo.lock with active story:docs-site-with-sidebar (inferred: that story adds the crates/takt-docs workspace member) and the body does not say so; remedy is an ordering edge recording the shared file, or splitting the surface"
```
