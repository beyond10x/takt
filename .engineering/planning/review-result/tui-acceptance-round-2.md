---
format: aep.planning-md/3
id: review-result:tui-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, terminal frontend, round 2
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
story:tui-approve-action — the clause "and `ess verify conform` covers the command" names a check ESS 0.60.0 cannot run, and the blocker's clearing condition does not list the missing HTTP target — .engineering/planning/story/tui-approve-action.md:54 (`ess verify conform run --help` lists `--target` as billing, oracle-fixture, interpreted; the epic says as much at .engineering/planning/epic/terminal-frontend.md:54)
story:takt-serve-read-only — the acceptance has `cargo test -p b10x-takt-serve --test views` start `takt serve`, but the `takt` binary belongs to b10x-takt-cli, so that command builds no binary for the test to start and `CARGO_BIN_EXE_takt` exists only for b10x-takt-cli's own tests — .engineering/planning/story/takt-serve-read-only.md:59 (`crates/takt-cli/tests/forge_cli.rs:67`, `crates/takt-cli/Cargo.toml` `[[bin]] name = "takt"`)
story:takt-tui-crate — the regeneration check names `ess generate ui --target tui --model ess --path ess/ui/takt.ui.yaml` without the required `--out`, so that command exits 2 and cannot be the "fresh" output `task check` compares against — .engineering/planning/story/takt-tui-crate.md:65 (`ess generate ui --target tui --model ess --path ess/ui/takt.ui.yaml` prints "required arguments were not provided: --out <OUT>")
epic:terminal-frontend — the clause "a body byte-equal to the committed fixture" is stricter than the serve story's check, which is byte-equal "after key-sorted canonical encoding", so the epic names a check the story's test does not run — .engineering/planning/epic/terminal-frontend.md:51 (.engineering/planning/story/takt-serve-read-only.md:61)

What I read: 7 of 7 ids (`epic:terminal-frontend`, `story:tui-read-model`, `story:tui-ui-document`, `story:takt-serve-read-only`, `story:takt-tui-crate`, `story:tui-approve-action`, `dependency-blocker:approve-route-credential`), each with `aep plan artifact show`. I also ran `aep plan artifact kinds` and `lifecycle` for epic, story and dependency-blocker, and read round 1 with `aep plan artifact show review-result:tui-acceptance-round-1`. I ran `--help` for `ess ui check`, `ui test`, `generate ui`, `specify validate`, `specify http validate` and `verify conform run`. I ran `ess specify validate --path ess --format json`, read `generate.rs` at 0.60.0, and checked `scripts/check-citations`, `docs/runs` and the takt test sources.

Round 1 against round 2:
- Three of the four round-1 findings are fixed:
  - The `ess verify conform` clauses are gone from the epic and the serve story.
  - The `ess ui check` and `ess ui test` commands now parse.
  - The epic's detail-page and approvals clauses are now tied to `ess ui test` on fixtures and to byte-equality of the served bodies.
- The conform finding still holds for `story:tui-approve-action`. Round 1 did not raise it there, but the clause was already in the story then (`git show aa09d00:.engineering/planning/story/tui-approve-action.md`, line 53).

What I could not establish:
- Whether "six declared views" and "one GET route per view" in `story:tui-read-model` can be read from a command's output. `ess specify validate --format json` reports `commands` but no views count. I counted that as checkable by reading the YAML and raised nothing.
- `story:takt-serve-read-only` uses "the fixture repository", and no Scope or Context line names or creates it. Only the expected bodies are scoped. I did not count this as a finding.
- Out of my lane, for `plan-critic-design` and `plan-critic-parallel-safety`:
  - `story:takt-serve-read-only` compares against `ess/ui/fixtures/` but has no `depends_on story:tui-ui-document`.
  - `story:tui-ui-document` says fixtures are "copied from real recorded cases and never edited by hand", yet the serve story needs them byte-equal to served view bodies, which are not raw case files.
  - The generated `crates/takt-tui/Cargo.toml` declares its own `[workspace]` (`generate.rs` `manifest`), which conflicts with the root `members = ["crates/*"]` and with the story's "workspace member" scope.
  - The UI binding appears to come from the model's `reached_by: network` component, not from `ess/http/takt.http.yaml` (`crates/ui/ess-ui-check/src/model.rs:1577`).
- Judgement call: I read the multi-clause acceptances as facets of one outcome each, as round 1 did, and did not let that set the verdict.

```findings
- file: .engineering/planning/story/tui-approve-action.md
  line: 54
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the clause 'and `ess verify conform` covers the command' names a check ESS 0.60.0 cannot run, and the blocker's clearing condition does not list the missing HTTP target"
- file: .engineering/planning/story/takt-serve-read-only.md
  line: 59
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has `cargo test -p b10x-takt-serve --test views` start `takt serve`, but the `takt` binary belongs to b10x-takt-cli, so that command builds no binary for the test to start and CARGO_BIN_EXE_takt exists only for b10x-takt-cli's own tests"
- file: .engineering/planning/story/takt-tui-crate.md
  line: 65
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the regeneration check names `ess generate ui --target tui --model ess --path ess/ui/takt.ui.yaml` without the required `--out`, so that command exits 2 and cannot be the fresh output `task check` compares against"
- file: .engineering/planning/epic/terminal-frontend.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the clause 'a body byte-equal to the committed fixture' is stricter than the serve story's check, which is byte-equal after key-sorted canonical encoding, so the epic names a check the story's test does not run"
```
