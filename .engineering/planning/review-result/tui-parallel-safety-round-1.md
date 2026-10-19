---
format: aep.planning-md/3
id: review-result:tui-parallel-safety-round-1
kind: review-result
status: active
title: Parallel-safety critic, terminal frontend, round 1
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
tui-read-model — shares `ess/domains/takt.yaml` and `ess/domains/takt.citations.tsv` with active story:story-delivery-tests-first, story:scorecard-save-checks-binary, story:promotion-chain-head-anchored and story:check-finding-run-step-confined, and its body names none of them (cited: their `scope` fields). Remedy: an ordering edge that records the shared file, or split the surface so the two no longer share it — .engineering/planning/story/tui-read-model.md:44-47
takt-serve-read-only — shares `crates/takt-cli/src/main.rs` with active story:judge-guard and story:self-improvement-protocol (cited, both scopes). Neither body names the other, no `depends_on` edge exists, and the only guard is one prose sentence on the epic that `aep plan artifact waves` does not read. Remedy: an ordering edge that records the shared file, or put the `serve` arm in its own module so the two stop sharing `main.rs` — .engineering/planning/story/takt-serve-read-only.md:44, epic/terminal-frontend.md:43, story/judge-guard.md:22
takt-serve-read-only — its `scope` omits `Cargo.lock`, which a new workspace member changes (root `Cargo.toml` has `members = ["crates/*"]`). Also, `crates/takt-serve` does not exist yet, and the body says "a new module or crate", so the surface is undecided. A module in `crates/takt` would land on judge-guard's `crates/takt/src` files instead. State crate or module, and add `Cargo.lock` to the scope — .engineering/planning/story/takt-serve-read-only.md:16,45
takt-tui-crate — shares `crates/takt-cli/src/main.rs` with active story:judge-guard and story:self-improvement-protocol, and its body names neither. It also changes `Cargo.toml` and `Cargo.lock` (which the body calls the judge surface) and `Taskfile.yml`, so the surface is wide. Its `depends_on` edges run only to the other TUI stories. Remedy: an ordering edge that records the shared file, or split the `tui` arm into its own module — .engineering/planning/story/takt-tui-crate.md:40,58-59
tui-approve-action — its outcome and context rely on story:judge-guard's rules ("refused unless story:judge-guard's rules admit it") and on the typed approvers that story creates, but no `depends_on` edge runs to story:judge-guard. Add the edge — .engineering/planning/story/tui-approve-action.md:37

What I read: 7 artifacts (epic, 5 stories, blocker) with `aep plan artifact show`; `aep plan artifact waves`, `graph` and `relations`; the full text of active story:judge-guard and story:self-improvement-protocol; and the scope, status and relations of 10 other stories. I also ran `git grep` on `Cargo.toml`, `lib.rs`, `case.rs`, `ess/` and `docs/runs`, and read `review-result:tui-design-round-1`.

Surfaces and reads:
- **Surfaces:** 5 stories placed by paths their bodies name. The store marks all five `inferred` because they are not source-derived. 0 are unplaceable, except that `crates/takt-serve`, `crates/takt-tui`, `ess/http/` and `ess/ui/` do not exist yet.
- **Acceptance reads traced to a producer in the set:** 6, and a `depends_on` edge records all 6. One more read, of judge-guard's rules, has no edge (the last finding).
- **Wave placement:** `aep plan artifact waves` puts tui-read-model in wave 7 and takt-serve-read-only in wave 11, while judge-guard is active now, so the epic's "not while RSI stories are open" rule is not enforced.

What I could not establish:
- **Out of my lane, not set against the verdict:** whether the "journey" view's data (`docs/metrics/journeys.jsonl`, written by `journey.py`) is what a per-case journey needs. That is acceptance or scope.
- **Unplaceable:** I could not place story:docs-site-with-sidebar, which is active and has no scope. It is outside the set.
- **Blocker evidence:** the blocker's second clause depends on story:test-command-reaches-no-forge-login, whose body does not cover a network namespace. The blocker already says "stated not done", so I raise no finding.

```findings
- file: .engineering/planning/story/tui-read-model.md
  line: 44
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares ess/domains/takt.yaml and ess/domains/takt.citations.tsv with active story:story-delivery-tests-first, story:scorecard-save-checks-binary, story:promotion-chain-head-anchored and story:check-finding-run-step-confined (cited, their scope fields) and the body names none of them; remedy is an ordering edge recording the shared file, or splitting the surface"
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 44
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares crates/takt-cli/src/main.rs with active story:judge-guard and story:self-improvement-protocol (cited, both scopes), neither body names the other, no depends_on edge exists and the epic's sequencing sentence is prose that the wave derivation does not read; remedy is an ordering edge recording the shared file, or putting the serve arm in its own module"
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 45
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scope omits Cargo.lock, which a new workspace member changes, and crates/takt-serve does not exist yet while the body says 'a new module or crate'; a module in crates/takt would land on story:judge-guard's files instead, so the surface is undecided"
- file: .engineering/planning/story/takt-tui-crate.md
  line: 58
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares crates/takt-cli/src/main.rs with active story:judge-guard and story:self-improvement-protocol (cited, both scopes) and also changes Cargo.toml, Cargo.lock and Taskfile.yml; the body names no collision and no edge runs outside the TUI set; remedy is an ordering edge recording the shared file, or splitting the tui arm into its own module"
- file: .engineering/planning/story/tui-approve-action.md
  line: 37
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome relies on story:judge-guard's rules and typed approvers, which that story creates, and no depends_on edge runs from this story to story:judge-guard; add the edge"
```
