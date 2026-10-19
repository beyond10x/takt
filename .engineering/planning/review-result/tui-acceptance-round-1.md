---
format: aep.planning-md/3
id: review-result:tui-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, terminal frontend, round 1
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
story:takt-serve-read-only — the acceptance's `ess verify conform` against a running `takt serve` cannot be run at ESS 0.60.0, where `--target` takes only `billing`, `oracle-fixture` and `interpreted` and the HTTP target is an unshipped stage 3; the clause names no other mechanism, and "answers each declared view" names no expected body, so nothing in it checks what a view returns — .engineering/planning/story/takt-serve-read-only.md:49 (`ess verify conform run --help`; `git -C ~/beyond10x/ess show 0.60.0:docs/design/http-presentation-binding.md` lines 590-602)
epic:terminal-frontend — the clause "every view the app reads passes `ess verify conform` against `takt serve`" names a check that does not exist at 0.60.0, the same gap as the serve story — .engineering/planning/epic/terminal-frontend.md:47 (`ess verify conform run --help`)
story:tui-ui-document — the acceptance's `ess ui check --model ess ess/ui/takt.ui.yaml` and bare `ess ui test` do not parse, because both require `--path` and `ess ui test` also requires test files, so the check as written exits 2 before and after the work — .engineering/planning/story/tui-ui-document.md:52 (`ess ui check --model ess ess/ui/takt.ui.yaml` prints "unexpected argument"; `ess ui test` prints "required arguments were not provided")
epic:terminal-frontend — the clauses "a case shows its claims, actions and outcomes…" and "the approvals page shows a case at `ApprovalRequired`…" name no check, because `--screen-once` renders only the home page after one read and the one headless approvals test runs on fixtures, not the served surface — .engineering/planning/epic/terminal-frontend.md:47 (`git -C ~/beyond10x/ess show 0.60.0:crates/ui/ess-ui-tui/src/generate.rs`, the `--screen-once` doc comment)

What I read: 7 of 7 ids (`epic:terminal-frontend`, `story:tui-read-model`, `story:tui-ui-document`, `story:takt-serve-read-only`, `story:takt-tui-crate`, `story:tui-approve-action`, `dependency-blocker:approve-route-credential`) with `aep plan artifact show`, plus `aep plan artifact kinds` and `lifecycle` for epic, story and dependency-blocker. I also read `--help` of the local `ess` 0.60.0 for `ui check|run|test`, `generate ui`, `specify validate`, `specify http validate` and `verify conform run|synthesize`. In the ess clone at 0.60.0 I read `generate.rs`, `http.rs`, `app.rs` and the conformance design pages. In takt I checked `Taskfile.yml`, `ess/ess-inputs.yaml`, `scripts/`, `docs/runs` and the `reached_by` count.

What I could not establish:
- Whether `ess ui run --tui` with fixtures can be observed repeatably (tui-ui-document's last clause). It has no `--screen-once`, so only a person can check it. I did not count that as a finding.
- Whether `ess specify validate` prints a count of views, which is how "six declared views" would be read.
- Out of my lane: the acceptances of `story:takt-serve-read-only`, `story:takt-tui-crate` and the epic each join three to five independent outcomes with semicolons. I read them as facets of one outcome each and did not let that set the verdict. `story:tui-approve-action` and the blocker I found checkable (the `/proc` probe and the grant at the bound revision), so I raised nothing on them.

```findings
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 49
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance's `ess verify conform` against a running `takt serve` cannot be run at ESS 0.60.0, where `--target` takes only billing, oracle-fixture and interpreted and the HTTP target is an unshipped stage 3; the clause names no other mechanism, and 'answers each declared view' names no expected body, so nothing in it checks what a view returns"
- file: .engineering/planning/epic/terminal-frontend.md
  line: 47
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the clause 'every view the app reads passes `ess verify conform` against `takt serve`' names a check that does not exist at 0.60.0, the same gap as the serve story"
- file: .engineering/planning/story/tui-ui-document.md
  line: 52
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance's `ess ui check --model ess ess/ui/takt.ui.yaml` and bare `ess ui test` do not parse, because both require `--path` and `ess ui test` also requires test files, so the check as written exits 2 before and after the work"
- file: .engineering/planning/epic/terminal-frontend.md
  line: 47
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the clauses 'a case shows its claims, actions and outcomes' and 'the approvals page shows a case at ApprovalRequired' name no check, because `--screen-once` renders only the home page after one read and the one headless approvals test runs on fixtures, not the served surface"
```
