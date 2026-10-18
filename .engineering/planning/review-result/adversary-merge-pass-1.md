---
format: aep.planning-md/3
id: review-result:adversary-merge-pass-1
kind: review-result
status: active
title: Adversary, merge-under-authority, pass 1
relations:
- reviews: story:merge-under-authority
revision: 1
---
```
unit: story:merge-under-authority, commit 8953dffd025cbc67f6ecbe3ccc9bca5c6b6150c4 on impl/merge-under-authority (base feed6b712fb0e56dd4959679f1d66d3107814adf), worktree takt-w5-merge
verdict: NEEDS-CHANGE
cases: executed 137→145, red 6
origin: introduced 8 / pre-existing 2 / undecided 0
wrote-outside-worktree: 1 directory tree, the unit's scratch adversary-1/ (listed in part 6); the shared sccache cache
needs-coordinator: (a) the fix shape for the harness self-grant (loop refusal vs store cross-check vs grant outside the tree) is a design call; (b) whether "a finished case is not an open case" (story moved off `active`, or `resumable` skipping a case with a legitimate outcome) is this unit's or story:story-done-end-to-end's
```

## 1. Diff stat

```
 crates/takt/tests/merge_under_authority.rs | 319 +++++++++++++++++++++++++++++
 1 file changed, 319 insertions(+)
 M crates/takt/tests/merge_under_authority.rs
?? crates/takt-cli/tests/merge_cli.rs
```

Both paths are test files. No `src/` path, no store write, no git state change.

## 2. Cases added (each run alone before the suite; red output verbatim)

### `crates/takt/tests/merge_under_authority.rs` (appended, existing 10 cases untouched)

**`adv_a_grant_the_harness_writes_into_the_case_does_not_merge` — RED.** A harness double writes into `<case>/authority.yaml` the bytes `takt approve` writes, changes nothing else, reports `repository.inspect`. Asserts no merge call, no `Completed`, no `merge_observation`.
```
thread 'adv_a_grant_the_harness_writes_into_the_case_does_not_merge' (1843141) panicked at crates/takt/tests/merge_under_authority.rs:1102:5:
the harness granted itself repository.merge and the run merged pull request #1; calls ["push takt/adv-self-grant", "open takt/adv-self-grant #1", "view #1", "checks #1", "view #1", "merge #1", "view #1"]:
...
frontier: ci.read (admissible), ..., repository.merge (approval-required), ...
step 3: repository.inspect (self-granting)
  said: read the repository
  effect: no file changed
frontier: ci.read (admissible), ..., repository.merge (admissible), ...
step 4: repository.merge {}
  effect: merged pull request #1 at 96556c0e through memory: merged
  evidence: merge_observation merged
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.03s
EXIT=101
```

**`adv_a_completed_case_is_not_resumed_by_the_next_run` — RED.** `open_next` on the `one-ready` fixture with the gate-only protocol and a `GateRun` validator completes the story's case; the next `open_next` must not resume it.
```
thread 'adv_a_completed_case_is_not_resumed_by_the_next_run' (1843913) panicked at crates/takt/tests/merge_under_authority.rs:1178:5:
the finished case story-ready-one-20261018T075934Z is resumed again, and nothing else is taken while story:ready-one stays active: frontier: resumed story:ready-one: Make the check pass (case story-ready-one-20261018T075934Z)
test result: FAILED. 0 passed; 1 failed; ... 17 filtered out; finished in 2.08s
EXIT=101
```

**`adv_a_commit_on_the_case_branch_between_runs_returns_the_claims_to_unknown` — RED.** After the first run stops at the gate, a commit is made on the case branch; after the grant, `Run::resume` must be about the branch's HEAD (story Ambiguities: "a new commit returns pr.opened and ci.green to UNKNOWN").
```
thread 'adv_a_commit_on_the_case_branch_between_runs_returns_the_claims_to_unknown' (1844205) panicked at crates/takt/tests/merge_under_authority.rs:1219:5:
assertion `left == right` failed: the resumed case is about the revision it stopped at, not the branch's HEAD
  left: Some("f1695a14adafb78261fb68ee4ff623eb9332f309")
 right: Some("33ce2d795dde7c152bced2e03b3516a6eb4fa6a0")
test result: FAILED. 0 passed; 1 failed; ... finished in 0.03s
EXIT=101
```

**`adv_a_capability_the_case_protocol_does_not_require_is_refused` — RED.** `grant(tree, case, "repository.merg", "operator")` must be refused with nothing written.
```
thread 'adv_a_capability_the_case_protocol_does_not_require_is_refused' (1844342) panicked at crates/takt/tests/merge_under_authority.rs:1250:5:
`repository.merg` is required by no action of story.delivery/1, and the grant was written: Ok(Approval { case: "adv-typo", story: None, capability: "repository.merg", approver: "operator", granted_at: "2026-10-18T07:59:39Z" })
test result: FAILED. 0 passed; 1 failed; ... finished in 0.01s
EXIT=101
```

**`adv_a_case_recording_a_story_the_store_does_not_list_active_is_not_resumed` — GREEN on the tree (mutant catcher for M5).** A case left behind, sorted before the active story's open case, recording a `proposed` story: the open case is resumed, not the stale one. `test result: ok. 1 passed`.

**`adv_resume_puts_the_tree_on_the_case_branch` — GREEN on the tree (mutant catcher for M6).** Tree checked out to `main`, `CaseDir::resume` puts it on `takt/<id>`. `test result: ok. 1 passed`.

Two probes were written, ran green alone, and were removed from the file before the suite run (reported in part 5): an approver `a: b #c "d" 'e' [f] {g} - h` round-trips verbatim and Canon gets two keys; a run resumed after `Completed` asks the forge nothing and stops `Completed` again.

### `crates/takt-cli/tests/merge_cli.rs` (new, modelled on `gate_cli.rs`)

**`the_run_after_a_story_case_completed_does_not_resume_it` — RED.** Two bare `takt run --repo R --harness none --protocol <gate-only>`; the second must say `nothing ready`.
```
thread 'the_run_after_a_story_case_completed_does_not_resume_it' (1847569) panicked at crates/takt-cli/tests/merge_cli.rs:140:5:
the finished case is resumed by every later run on the repository, and no other story is ever taken: exit 0 stdout frontier: resumed story:ready-one: Make the check pass (case story-ready-one-20261018T080018Z)
case story-ready-one-20261018T080018Z on gate.only/1 in .../target/tmp/merge-cli-1847568-completed; inputs none
frontier: gate.run (admissible)
stopped: Completed (accepted); steps 0, tokens 0, case .../.engineering/cases/story-ready-one-20261018T080018Z
test result: FAILED. 0 passed; 1 failed; ... 1 filtered out; finished in 2.09s
EXIT=101
```

**`a_run_with_no_harness_on_a_case_that_needs_only_the_grant_stops_approval_required` — RED.** A story-delivery case with the four records and no grant, `takt run --case gated --harness none --forge none`: expected exit 0 and `stopped: ApprovalRequired (repository.merge)`.
```
thread 'a_run_with_no_harness_on_a_case_that_needs_only_the_grant_stops_approval_required' (1848121) panicked at crates/takt-cli/tests/merge_cli.rs:219:5:
assertion `left == right` failed: stdout case gated on story.delivery/1 in ...; inputs none
frontier: ci.read (admissible), gate.run (admissible), implementation.edit (admissible), pr.open (admissible), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
 stderr takt: no harness: a judgement step among ci.read, implementation.edit, pr.open, repository.inspect, tests.write is left (approvals: repository.merge)
  left: 1
 right: 0
test result: FAILED. 0 passed; 1 failed; ... finished in 0.03s
EXIT=101
```

(One compile error in my own CLI file on the first attempt, `&&str` into `impl Into<String>`, was a typo in my file, fixed with `*kind`; not a finding.)

## 3. Suite run (after part 2 existed; `--no-fail-fast` because the first run stopped before `story_delivery.rs`)

`cargo fmt -p b10x-takt -p b10x-takt-cli --check` → clean, `EXIT=0`.
`cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` → `Finished ... EXIT=0`.

`cargo test -p b10x-takt --no-fail-fast` (scratch `adversary-1/suite-takt.log`):
```
test result: ok. 14 passed; 0 failed; ... (unittests src/lib.rs)
test result: ok. 33 passed; 0 failed; ... (tests/forge.rs)
test result: ok. 15 passed; 0 failed; ... (tests/frontier.rs)
test result: ok. 16 passed; 0 failed; ... (tests/gate_validator.rs)
test result: ok. 2 passed; 0 failed; ... (tests/loop.rs)
test result: FAILED. 12 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s (tests/merge_under_authority.rs)
test result: ok. 13 passed; 0 failed; ... (tests/story_delivery.rs)
test result: ok. 0 passed; ... (doc-tests)
EXIT=101
```
`cargo test -p b10x-takt-cli --no-fail-fast` (`.../suite-cli.log`):
```
test result: ok. 17 passed; ... (unittests src/main.rs)
test result: ok. 3 passed; ... (tests/forge_cli.rs)
test result: ok. 6 passed; ... (tests/frontier_cli.rs)
test result: ok. 8 passed; ... (tests/gate_cli.rs)
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.08s (tests/merge_cli.rs)
EXIT=101
```
Executed: 109 + 36 = 145 (before: 137 from the implementor's gate logs and commit message). Red: 6, all mine.

## Mutants (on the scratch copy `adversary-1/mutant/`, sources restored and verified equal to the worktree; its `target/` (719M) removed)

| Mutant | Edit | Caught by |
|---|---|---|
| M1 drop the head check in `PrMerged` (`validator.rs:735`) | `if … != … \|\| … != … {` → `if false {` | implementor's `pr_merged_merges_nothing_when_the_head_is_not_the_case_branch_at_the_revision` |
| M2 `Authority::canon()` emits every key (`case.rs:166`) | serialize `self.decisions` | implementor's `the_grant_is_read_into_every_evaluation_of_the_case`, `without_a_grant_…`, `the_open_case_…` (Canon refuses `approver`) |
| M3 `merged: true` not checked (`forge.rs`) | `if response["merged"] != true {` → `if false {` | implementor's `the_bot_route_adapter_merges_through_the_api_with_the_head_to_match` |
| M4 drop the `active` scan (`run.rs:249`) | `let active = Vec::new()` | implementor's `the_open_case_of_an_active_story_is_resumed_…` and my case 6 |
| M5 drop `active.contains(&story)` (`case.rs:291`) | `&& !story.is_empty()` | **suite as handed: green (15 frontier + all 10 implementor cases pass)**; my `adv_a_case_recording_a_story_the_store_does_not_list_active_is_not_resumed` red |
| M6 drop the checkout in `CaseDir::resume` (`case.rs:491`) | `if false {` | **suite as handed: green**; my `adv_resume_puts_the_tree_on_the_case_branch` red |

## 4. Findings (all against commit 8953dff in the worktree above)

| # | file:line | what was measured | what reaches it | verdict | origin | severity |
|---|---|---|---|---|---|---|
| F1 | `crates/takt/src/run.rs:465` (with `case.rs:654`) | the harness wrote `authority.yaml` during step 3, the loop printed `effect: no file changed`, step 4 merged, `Completed (accepted)`; `merge #1` in the double's calls | every run on the default harness: `ClaudeCode` runs in the tree with write access, the prompt names the pending capability (`harness.rs:110-115`), the file is `<tree>/.engineering/cases/<id>/authority.yaml`, the loop filters `.engineering/cases` out of the step's change set and `commit_all` excludes it, `evaluate` re-reads it fresh | NEEDS-CHANGE | introduced | blocker |
| F2 | `crates/takt/src/case.rs:277` (`resumable`), `run.rs:250` | after `Completed (accepted)` the next bare run prints `frontier: resumed story:ready-one …`, `stopped: Completed (accepted); steps 0`, exit 0; `nothing ready` never; library and binary | the documented daily `takt run --repo R` after the first accepted story; nothing in the unit moves the story off `active`; with two active stories, name order means the second is resumed only once the first leaves `active`, i.e. never by a bare run. A regression of story:frontier-next-ready-story (before the unit an `active` story was skipped) | NEEDS-CHANGE | introduced | blocker (downgrade to warning if closing the story is routed to story:story-done-end-to-end) |
| F3 | `crates/takt/src/case.rs:479` (`CaseDir::resume`) | resumed case's `implementation` = `f1695a14…` (case.yaml) while the branch HEAD = `33ce2d79…`; claims stay true at the stale revision; the story's Ambiguities say unknown | an operator commit on the case branch between the gate stop and `takt approve` (the story's own scenario); the loop's own commits are not affected (`set_revision`) | NEEDS-CHANGE | introduced | warning |
| F4 | `crates/takt/src/case.rs:224` (`grant`) | `grant(…, "repository.merg", …)` → `Ok(Approval {…})`, file written, store record would follow | a typo on `takt approve`; the `approval` evidence on the story then names a capability nothing requires; recoverable by a second `takt approve` | NEEDS-CHANGE | introduced | warning |
| F5 | `crates/takt-cli/src/main.rs:121` (`NoHarness::step`) | `takt run --case gated --harness none --forge none` on a case needing only the grant: exit 1, `no harness: a judgement step among … is left (approvals: repository.merge)`, not `ApprovalRequired` | `--harness none` only (tests, CI); with `claude` the stop depends on the model naming the gate; a model that keeps inspecting ends `NoProgress` exit 3; nothing merges either way. The loop and `NoHarness` are unchanged from the base | CONFIRMED | pre-existing | warning |
| F6 | `.engineering/planning/story/merge-under-authority.md:89` | with `decision: denied` the frontier shows `repository.merge (blocked)` and `approvals: ` empty: no `ApprovalRequired` stop, the harness is asked for a judgement step (`probe-forge-none/run3-denied.log`) | nothing writes `denied` (`takt approve` writes `granted` only; denial is out of scope) | INFEASIBLE | pre-existing | note |
| F7 | `crates/takt-cli/src/main.rs:298` (`--forge none` branch) | a granted case resumed with `--forge none`: `repository.merge (admissible)` and it is in the list offered to the harness as a judgement step (`probe-forge-none/run2.log`); the CLI help says `repository.merge` is "left to the harness" | `takt run --case <id> --forge none` on a case opened with the bot forge; a harness that performs it writes no record and the run ends `NoProgress` | CONFIRMED | introduced | note |
| F8 | `crates/takt/src/case.rs:291` | M5 (drop `active.contains`) survives the suite as handed; the case directory alone would decide which story is resumed | my case 6 catches it; the code is right, the suite was not testing the line | CONFIRMED | introduced | note |
| F9 | `crates/takt/src/case.rs:491` | M6 (drop the checkout in `resume`) survives the suite as handed; every existing resume test runs with the tree already on the branch | my case 8 catches it; reached when the operator checks out another branch between runs, or another case was opened since | CONFIRMED | introduced | note |
| F10 | `crates/takt/src/case.rs:291` (`story_of` → `None`) | a case directory with `case.yaml` and no `story.yaml` (opened for a story before this unit) is never resumed by the frontier, resumes as an intent unit under `--case`, and `takt approve` records nothing on its story | nothing found: no such case exists outside test scratch before this unit | INFEASIBLE | introduced | note |

Fixes named, not applied: F1 — smallest: snapshot `authority.yaml` (and `story.yaml`) before a judgement step and return `StepRefused` when they changed; robust: on every evaluation accept a grant only when the story's `approval` evidence in the store matches it (approver, `granted_at`, reference), which option 3 of the decision already records; or keep the grant outside the tree. F2 — `resumable` skips a case whose latest decision has a legitimate outcome (or the run writes a `closed` marker / moves the story on `Completed`). F3 — `CaseDir::resume` compares `git::head(tree)` with the implementation revision and `set_revision`s when they differ. F4 — `grant` reads the case's protocol and refuses a capability no action `requires`. F5 — the CLI's `NoHarness` returns the gate action when `context.approvals` names it (what the test `Requester` does), so the run stops `ApprovalRequired`.

## 5. Attacked and could not break

- Acceptance two-run flow with the double: first stop `ApprovalRequired (repository.merge)`, nothing merged; resumed run merges once, one `merge_observation merged` numbered `0005-`, `Completed (accepted)`, decision document shows the grant (implementor's case holds; re-run green).
- A run resumed after `Completed`: no forge call, no second merge, `Completed` again (probe green, removed).
- Approver with YAML-significant characters (`a: b #c "d" 'e' [f] {g} - h`): written and read back verbatim, Canon handed two keys, evaluation computes from it (probe green, removed). Empty / whitespace approver: refused (implementor's case).
- A capability that would brick the evaluation: `grant`'s identifier rule equals Canon's (`canon/src/eval/mod.rs:627`: empty, whitespace, control), so no accepted grant makes Canon refuse.
- Second `takt approve` on the same capability: refused, file unchanged, one record; on another capability: appended, two records, by design (implementor's case plus code read).
- `takt approve` on an intent case and on a missing case: grant only / refused with nothing written (implementor's cases, re-run green).
- `--case` with `--intent`: refused before anything is opened (implementor's CLI case).
- Head moved: `branch.pushed != revision` → nothing read; forge head on another sha or branch → no merge call (implementor's case; M1 caught). Between view and merge: the double refuses and the run errors with the case left resumable (read, not run). A merged state read back as `open`: no record, summary says so (read).
- Merge response: `merged: false` with a message → error naming it (implementor's case); response without `merged` → `response["merged"] != true` bails (read); 405/409 → `succeed` bails on a non-zero exit (read; no real API called).
- Merge request body: `{"sha": <head>}` only; no `merge_method`, `commit_title` or `commit_message`, so GitHub applies the repository default; nothing textual to scan.
- A harness-written `branch.yaml` number or name: still guarded by the forge view and `branch_of` (wave 04 guards hold).
- `--harness none` when a validator step is still due: taken before any harness call (implementor's CLI resume case).
- Mutants M1-M4: caught by the implementor's cases.

## 6. Paths written outside the worktree

All under the unit's scratch `adversary-1/` (2.2M after cleanup): `append-lib.rs`, `lib-without-probes.rs`, `replace.py`, `run-mutants.sh`, `run-m1.sh`, `probe-forge-none.sh`, `probe-forge-none/` (a git repository with the probe case and `run1.log`, `approve.log`, `run2.log`, `run3-denied.log`), `mutant/` (the source copy, `target/` removed by me), and the logs `red-1-self-grant.log`, `red-2-completed.log`, `red-3-moved.log`, `red-4-typo.log`, `red-cli-1-completed.log`, `red-cli-2-no-harness.log`, `case-6-stale-story.log`, `case-8-branch.log`, `probe-1-approver.log`, `probe-2-again.log`, `suite-takt.log`, `suite-cli.log`, `gate-clippy.log`, `gate-fmt.log`, `mutant-build.log`, `mutants.log`, `mutant-M1…M6-*.log`. Builds used `RUSTC_WRAPPER=sccache` (shared cache). Test scratch trees under the worktree's `target/tmp/` and `CARGO_TARGET_TMPDIR` are removed by each case's `Scratch` drop. Lease `takt-w5-merge-adv1` released.

## 7. Findings

```findings
- file: crates/takt/src/run.rs
  line: 465
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A harness that writes <case>/authority.yaml during a judgement step grants itself repository.merge: the loop filters the case directory out of the step's change set, re-reads the file at the next evaluation, and merges (adv_a_grant_the_harness_writes_into_the_case_does_not_merge)."
- file: crates/takt/src/case.rs
  line: 277
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A story's case that reached Completed is resumed by every later bare `takt run` and nothing else is ever taken while the story stays active (adv_a_completed_case_is_not_resumed_by_the_next_run, the_run_after_a_story_case_completed_does_not_resume_it)."
- file: crates/takt/src/case.rs
  line: 479
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "CaseDir::resume keeps case.yaml's implementation revision, so a commit on the case branch between runs leaves every claim true at the stale revision, against the story's Ambiguities (adv_a_commit_on_the_case_branch_between_runs_returns_the_claims_to_unknown)."
- file: crates/takt/src/case.rs
  line: 224
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "grant accepts a capability the case's protocol requires nowhere (`repository.merg`), writes it and would record it on the story (adv_a_capability_the_case_protocol_does_not_require_is_refused)."
- file: crates/takt-cli/src/main.rs
  line: 121
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: "With --harness none a case that needs only the grant ends in an error exit 1 (\"no harness: a judgement step among ... (approvals: repository.merge)\") instead of stopping ApprovalRequired exit 0 (a_run_with_no_harness_on_a_case_that_needs_only_the_grant_stops_approval_required)."
- file: .engineering/planning/story/merge-under-authority.md
  line: 89
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: "The Ambiguities line says a denied decision stops ApprovalRequired the same way; Canon marks the action blocked and the loop asks the harness for a judgement step, and nothing writes `denied`."
- file: crates/takt-cli/src/main.rs
  line: 298
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "A granted case resumed with --forge none has repository.merge admissible with no validator, so the merge is offered to the harness as a judgement step; a harness performing it writes no record and the run ends NoProgress."
- file: crates/takt/src/case.rs
  line: 291
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Dropping `active.contains(&story)` in resumable survives the suite as handed; adv_a_case_recording_a_story_the_store_does_not_list_active_is_not_resumed catches it."
- file: crates/takt/src/case.rs
  line: 491
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Dropping the branch checkout in CaseDir::resume survives the suite as handed; adv_resume_puts_the_tree_on_the_case_branch catches it."
- file: crates/takt/src/case.rs
  line: 291
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "A story's case directory without story.yaml (opened before this unit) is never resumed by the frontier and its approval is recorded on no story; no such case was found outside test scratch."
```
