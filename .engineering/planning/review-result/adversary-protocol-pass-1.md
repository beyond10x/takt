---
format: aep.planning-md/3
id: review-result:adversary-protocol-pass-1
kind: review-result
status: active
title: Adversary, story-delivery-protocol, pass 1
relations:
- reviews: story:story-delivery-protocol
revision: 1
---
unit: story:story-delivery-protocol pass 1 — worktree takt-w1-protocol, branch impl/story-delivery-protocol at e50efdb (base 1638755a), plus my uncommitted additions to crates/takt/tests/story_delivery.rs
verdict: NEEDS-CHANGE
cases: executed 2→8, red 2
origin: introduced 7 / pre-existing 0 / undecided 0
wrote-outside-worktree: 24 paths under the unit's scratch adversary-1/ (part 6), one of them a 608M build directory
needs-coordinator: the merge record's kind is written as `pull_request` merged in story:story-delivery-protocol § Context/§ Acceptance and in story:merge-under-authority § Context/§ Acceptance, which Canon cannot honour; both texts are store artifacts, so the correction is yours to route. Also whether `risk` stays declared (finding 2: README, protocol or CLI).

## 1. `git --no-pager diff --stat` (working tree vs HEAD e50efdb)

```
 crates/takt/tests/story_delivery.rs | 278 ++++++++++++++++++++++++++++++++++++
 1 file changed, 278 insertions(+)
```

Test file only. `protocols/story-delivery/1.yaml` and `README.md` untouched. The only non-additive change is `cargo fmt -p b10x-takt` reformatting my own block; the unit's lines are byte-identical.

## 2. Cases added (all in `mod adv` at the end of crates/takt/tests/story_delivery.rs; loader honours `STORY_DELIVERY_PROTOCOL` so a mutated copy outside the tree can be driven)

| Case (line of the assertion) | Asserts | Now | Red output when written (run alone, before the suite) |
|---|---|---|---|
| `adv_the_merge_under_authority_sequence_earns_pr_merged` (:247) | the story's Acceptance sequence, fifth record `pull_request` merged, gives `pr.opened` true, `pr.merged` true, `accepted` legitimate | RED | `panicked at crates/takt/tests/story_delivery.rs:247:9` … `left: [("claim pr.opened", "unknown"), ("claim pr.merged", "unknown"), ("outcome accepted", "blocked")]` / `right: [("claim pr.opened", "true"), ("claim pr.merged", "true"), ("outcome accepted", "legitimate")]` — the decision's explanation: `"pr.opened": {"because": [{"evidence":"e3","status":"applied"},{"evidence":"e5","status":"applied"}], "value":"unknown"}`, `"pr.merged": {"because": [{"kind":"merge_observation","status":"absent","subject":"implementation"}]}`; `test result: FAILED. 0 passed; 1 failed; … 7 filtered out` EXIT=101 (full 180-line decision in `…/adversary-1/red-1-pr-merged.log`) |
| `adv_the_readme_invocation_opens_a_case_without_inputs` (:279) | `Run::open` with the README's `--protocol protocols/story-delivery/1.yaml` and no `--input` opens a case | RED | `panicked at crates/takt/tests/story_delivery.rs:279:27: the README's invocation does not open a case: the case input risk is not given; it takes one of ["trivial", "standard", "elevated", "critical"]`; `test result: FAILED. 0 passed; 1 failed` EXIT=101 (`…/red-2-readme.log`) |
| `adv_pr_open_needs_tests_pass_not_only_gate_green` (:308) | `gate_result` pass alone leaves `pr.open` blocked | green | mutant A (1.yaml:129 `- claim: tests.pass` deleted): `left: "admissible"` / `right: "blocked"` EXIT=101 |
| `adv_repository_merge_needs_pr_opened_not_only_ci_green` (:356) | tests, gate, ci pass with no `pull_request` leaves `repository.merge` blocked | green | mutant B (1.yaml:147 `- claim: pr.opened` deleted): `left: "approval-required"` / `right: "blocked"` EXIT=101 |
| `adv_a_failing_result_decides_each_claim_false` (:399) | fail/fail/closed/fail/closed records make all five claims `false`, `pr.open`, `repository.merge`, `accepted` blocked | green | mutant C (1.yaml:53,60,67,74,81 `result:` deleted): `left: {"ci.green": "true", "gate.green": "true", "pr.merged": "true", "pr.opened": "true", "tests.pass": "true"}` / `right: {… all "false"}` EXIT=101 |
| `adv_a_record_about_another_artifact_does_not_count` (:438) | `test_result` pass about `plan` at r0 leaves `tests.pass` unknown | green | mutant D (1.yaml:54 `subject: implementation` deleted): `left: "true"` / `right: "unknown"` EXIT=101 |

Mutant survival against the unit's own two cases, measured in a scratch copy of the tree (`…/adversary-1/tree-copy`, mutant copied over its protocol, `cargo test --offline -p b10x-takt --test story_delivery -- --skip adv_`): A, B, C, D each `test result: ok. 2 passed; 0 failed` EXIT=0. The unit's suite cannot tell the protocol from any of the four.

## 3. Suite run (after the cases in part 2 existed)

```
$ cargo test -p b10x-takt --test story_delivery -- --skip adv_
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.02s   EXIT=0
$ cargo test -p b10x-takt --test story_delivery
test adv::adv_the_merge_under_authority_sequence_earns_pr_merged ... FAILED
test adv::adv_the_readme_invocation_opens_a_case_without_inputs ... FAILED
test result: FAILED. 6 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s   EXIT=101
$ cargo fmt -p b10x-takt --check      EXIT=0
$ cargo clippy -p b10x-takt --all-targets -- -D warnings   Finished `dev` profile   EXIT=0
```

## 4. Findings (worktree at e50efdb + my test additions)

| # | file:line | Verdict | Origin | What is wrong | What reaches it |
|---|---|---|---|---|---|
| 1 | crates/takt/tests/story_delivery.rs:247 (protocols/story-delivery/1.yaml:40-44, 78-84) | NEEDS-CHANGE | introduced | The story's Acceptance names the fifth record `pull_request` merged; story:merge-under-authority § Context and § Acceptance build on that. The protocol binds `pr.merged` to a new kind `merge_observation`, and under Canon 0.6.0 a second `pull_request` result at one revision also returns `pr.opened` to unknown (reference/evaluation.md:39-42), so the story-literal sequence yields `pr.merged` unknown, `pr.opened` unknown, `repository.merge` blocked, `accepted` blocked. The protocol is the right call under Canon; the two story texts are wrong and nothing corrected them. Fix: amend both stories to `merge_observation` merged, or have the merge validator write that kind | story:merge-under-authority § Context line 29 and § Acceptance line 58 (planning store); this story's § Acceptance |
| 2 | README.md:25 (protocols/story-delivery/1.yaml:13-17) | NEEDS-CHANGE | introduced | The README's advertised invocation refuses to open a case: `takt run` defaults `risk` only for `software.change/1` (crates/takt-cli/src/main.rs:136-146) and `CaseDir::open` refuses a declared input not given (case.rs:78-80). `risk` is read by nothing in the document; deleting the `case:` block keeps the suite green. Fix: README says `--input risk=<value>`, or the protocol drops `case.inputs` (Canon: "to take a default, leave the key out"), or the CLI defaults inputs for every protocol | README.md:25 itself; `takt run --repo R --intent I --protocol protocols/story-delivery/1.yaml` |
| 3 | crates/takt/tests/story_delivery.rs:308 | CONFIRMED | introduced | Suite blind spot: `pr.open`'s precondition reduced to `gate.green` alone survives both unit cases; my case kills it | any run where the gate passes before the tests at a revision |
| 4 | crates/takt/tests/story_delivery.rs:356 | CONFIRMED | introduced | Suite blind spot: `repository.merge`'s precondition reduced to `ci.green` alone survives — the merge would become approval-required with no pull request open | a `ci_result` written without a `pull_request` record |
| 5 | crates/takt/tests/story_delivery.rs:399 | CONFIRMED | introduced | Suite blind spot: every `result:` on the five claims deleted survives — a `fail`/`closed` record would then make the claim true | every failing test, gate or CI run |
| 6 | crates/takt/tests/story_delivery.rs:438 | CONFIRMED | introduced | Suite blind spot: `subject: implementation` deleted from `tests.pass` survives | nothing found today writes a record about `plan` or `intent` — smallest of the four |
| 7 | crates/takt/tests/story_delivery.rs:146, :173 | CONFIRMED | introduced | `let _ = fs::remove_dir_all(&tree)` does not remove the scratch git tree after a passing run: 14 full trees under the worktree's `target/tmp/story-delivery-*`. Cause not determined (the error is discarded) | every run of the suite |

## 5. Attacked and not broken

- Stale revision: the unit's second case covers the move; a record bound to the old revision added afterwards is excluded by Canon's binding stage. Not re-tested.
- Records out of order: Canon ignores order; a `merge_observation` merged first makes `accepted` legitimate at once with `tests.required`, `gate.required`, `ci.required` open. That is what the story asks (`accepted` requires `pr.merged` only); Canon outcomes never read obligations, and the kernel reads them only into the no-progress string and the prompt. A design choice for the coordinator, not a break.
- `pull_request` closed: `pr.opened` false, `pr.open` and `repository.merge` blocked (case 5, green). Beside an `open` record at the same revision it goes unknown — same mechanism as finding 1 — merge blocked either way.
- `repository.merge` before `ci.green`: blocked.
- Protocol strict keys: the document and all four mutants load, validate and compile under Canon 0.6.0 in every run above.
- Action ids and effects of `repository.inspect`, `tests.write`, `implementation.edit`, `tests.run` match software.change/1 (engineering-protocols 0.9.0); the CLI's default `--gate repository.merge` matches the action id.
- `abandoned` can never be legitimate through the kernel (`evaluate` passes `decisions: None`; `case.yaml` carries no `revision`). A pre-existing kernel limit, not this unit's.
- `pr.open` stays admissible after `pr.opened` is true (no `is: unknown` guard); harmless once story:forge-pull-request-evidence makes it validator-owned.

## 6. Paths written outside the worktree (all under the unit's scratch adversary-1/)

`mutants/A-pr-open-without-tests-pass.yaml`, `mutants/B-merge-without-pr-opened.yaml`, `mutants/C-claims-without-result.yaml`, `mutants/D-tests-pass-without-subject.yaml`; logs `red-1-pr-merged.log`, `red-2-readme.log`, `control-adv_*.log` (4), `mutant-*.log` (4), `original-suite-on-*.log` (4), `suite-before.log`, `suite-after.log`, `fmt.log`, `fmt-after.log`, `clippy.log`, `clippy-after.log`; `tree-copy/` (rsync of the worktree without `target` and `.git`, its protocol now holding mutant D) and `tree-copy/target/` (608M, the compiler's build directory for the mutant-survival runs; not deleted). Inside the worktree: scratch trees `target/tmp/story-delivery-*-adv-*` left by my runs (finding 7). Lease `takt-w1-protocol-adv1` released.

```findings
- file: crates/takt/tests/story_delivery.rs
  line: 247
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "the story's Acceptance and story:merge-under-authority record the merge as `pull_request` merged, which the protocol does not read for `pr.merged` and which beside the `open` record returns `pr.opened` to unknown under Canon's disagree rule; the protocol's `merge_observation` is right under Canon and the two story texts need correcting"
- file: README.md
  line: 25
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "the advertised `--protocol protocols/story-delivery/1.yaml` invocation refuses to open a case because the protocol declares `risk`, nothing in it reads `risk`, and the CLI defaults inputs only for software.change/1"
- file: crates/takt/tests/story_delivery.rs
  line: 308
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the unit's cases stay green when `pr.open`'s precondition loses `tests.pass`; the added case turns red on that mutant"
- file: crates/takt/tests/story_delivery.rs
  line: 356
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the unit's cases stay green when `repository.merge`'s precondition loses `pr.opened`; the added case turns red on that mutant"
- file: crates/takt/tests/story_delivery.rs
  line: 399
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the unit's cases stay green when every claim's `result:` is removed, so a failing record would establish the claim; the added case turns red on that mutant"
- file: crates/takt/tests/story_delivery.rs
  line: 438
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the unit's cases stay green when `tests.pass` loses `subject: implementation`; the added case turns red on that mutant, though nothing today writes a record about another artifact"
- file: crates/takt/tests/story_delivery.rs
  line: 146
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the scratch git tree survives a passing run (14 intact trees under target/tmp), the `remove_dir_all` error being discarded; cause not determined"
```
