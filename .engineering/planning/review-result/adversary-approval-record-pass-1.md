---
format: aep.planning-md/3
id: review-result:adversary-approval-record-pass-1
kind: review-result
status: active
title: Adversary, approval-record-reaches-base, pass 1
relations:
- reviews: story:approval-record-reaches-base
revision: 1
---
```
unit: story:approval-record-reaches-base, worktree w7-approval-record at 7da45ef plus my untracked test file
verdict: INFEASIBLE (3 red cases; none is reached by takt's own flow)
cases: executed 285→289, red 3
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, <wave-scratch>/scratch/approval-record/adv1/
needs-coordinator: whether to hold the unit for J1/J2, and whether the target repository requires status checks (I don't know, it needs the network)
```

Three of my four new tests fail against this branch, so the unit's tests are red. None of the three failures can be produced by takt itself: each needs someone outside a takt run, or a forge behaviour I modelled with a test double.

**1. Diff stat**
`git --no-pager diff --stat` prints nothing, because my file is untracked. `git status --short` shows one path, `?? crates/takt/tests/approval_record_adv1.rs` (475 lines). It is a test file, and no implementation file was touched.

**2. Cases added** (`crates/takt/tests/approval_record_adv1.rs`)

| case | asserts | now |
|---|---|---|
| `adv1_an_approve_subject_commit_rewriting_the_story_does_not_reach_the_base` | a hand-made commit titled like an approval (`takt: approve story:ready-one repository.merge`) that rewrites the story's acceptance does not end up on the base | red here, green on base c1445fa |
| `adv1_a_push_the_forge_applied_but_reported_failed_leaves_the_case_mergeable` | after a push that reached the forge but reported failure, a later run still merges | red |
| `adv1_required_checks_on_the_pushed_approval_commit_hold_the_merge_without_an_error` | when the forge requires checks on the approval commit, the run waits without an error, then merges once they pass | red |
| `adv1_a_stored_and_reported_head_with_a_foreign_commit_is_refused_by_carries` | only `carries` stops a foreign head that both `branch.yaml` and the forge name | green; red when `carries` is changed to always answer true |

Red output, from running each case alone:
- `:346` — `merged head 47c314d3 (forged 47c314d3, approval b88dcab6, granted 2e7347aa) put a rewritten acceptance on the base: Ok(Completed("accepted"))`
- `:394` — `left: [] right: [1]`; both retries said `pull request #1 has the head … at e8802b00, not … at 100eb571; nothing to merge`
- `:425` — `Err("repository.merge (takt.pr-merged): merge pull request #1 through the forge real: … 405 Required status check \"takt / task check\" is expected")`

Correction: the first version of the first case compared the whole story file against `main`. That was wrong, because the take commit also changes the file, and running it on base exposed the mistake. I changed it to assert only on the acceptance line and reran it alone: red here, green on base. Logs: `adv1/red-*.log`, `adv1/*-v2.log`, `adv1/base-run.log`.

Mutation check: I changed `carries` to always answer true in a scratch copy. The unit's refusal test `a_head_carrying_a_commit_that_is_no_approvals_is_refused_with_nothing_merged` stayed green, and my fourth case went red (`9088cdfc merged`). Log: `adv1/mutant-carries.log`.

**3. Suite** (run after the cases existed)

| step | exit | result |
|---|---|---|
| `cargo fmt -p b10x-takt -p b10x-takt-cli --check` | 0 | |
| `cargo clippy … --all-targets --locked -- -D warnings` | 0 | |
| `cargo test -p b10x-takt -p b10x-takt-cli --locked --no-fail-fast` | 101 | 286 passed, 3 failed, 289 executed (`adv1/suite-2.log`) |

The before number, 285, comes from the implementor's commit message and its log `after-2.log`, not from a run of mine.

**4. Findings**

| id | file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|---|
| J1 | `crates/takt/src/case.rs:746` (code at `validator.rs:1087`) | The comment says "nothing else rides along", but `approvals_only` accepts any commit titled like an approval that touches only the planning store. `repository.merge` then pushes and merges it under the human's grant. Before this unit such a commit stayed on the branch. Suggested fix: accept only commits that add new files under `.engineering/evidence/<story>/`, at most one per grant in `authority.yaml`. | INFEASIBLE / introduced, warning | Only something writing the case branch outside a takt run. Steps and validators that move `HEAD` or any branch are refused (`run.rs:973-976`, `:781`). |
| J2 | `crates/takt/src/validator.rs:1092-1094` | `branch.pushed` is written only after the push returns. A push that reached the forge but reported failure leaves `branch.yaml` at the granted revision and the forge at the approval commit. Every later run then refuses, and the push branch never runs again. Suggested fix: record `pushed` before pushing, or also accept a forge head that differs from `pushed` when it is the local head and `carries` it. | INFEASIBLE / introduced, warning | The bot-route child timing out after the remote ref moved. Not observed. |
| J3 | `crates/takt/src/validator.rs:1108` | The stated residue. Pushing the approval commit right before merging means a forge with required checks refuses with 405, and the run ends in an error. CI was read only on the granted revision. It recovers on rerun. If CI fails on the approval commit, the case stays unmergeable while it still carries `ci_result pass`. | INFEASIBLE / introduced, warning | Only a base branch with required checks. I don't know whether any target repository has that. |
| J4 | `protocols/story-delivery/1.yaml:31,38,118` | The protocol text still says the pull request's head is an implementation revision, and that `pr.open` pushes "the implementation revision". Both are now false. | CONFIRMED / introduced, note | Every merge after an approval. |
| J5 | `crates/takt/tests/approval_record.rs` (refusal case) | The refusal test is refused by the head equality check, never by `carries`. It stays green when `carries` always answers true. Case 4 covers that gap. | CONFIRMED / introduced, note | The suite only. |

**5. Attacked and not broken**
- Merge commits, root commits, approval commits for another story, and paths outside the store are all refused by `approvals_only`.
- Race between the check, the push and the merge: the merge names the head it read again after pushing. The bot route sends `{"sha": head}` and the double checks the same thing.
- A forge head that lags behind the push (seen in a live run, commit 1c036c3) gives "nothing merged" with no error, and the next run merges.
- A steps-or-validator forgery on HEAD's branch is refused by existing guards. I did not write a separate test for it.
- `ci.read` on an approval head is never reached in story-delivery: `prepare_grant` requires the gate at the revision, and that comes after `ci.green`.
- The merge observation still names the granted revision, and `merged-head-<seq>.json` names the merged head.

**6. Paths written outside the worktree**
All under `<wave-scratch>/scratch/approval-record/adv1/`:
- `mut/`: the mutated copy, 713M including its `target/`
- `base/`: a `git archive` of c1445fa plus its `target/`
- the logs `red-*.log`, `*-v2.log`, `base-run.log`, `mutant-carries.log`, `suite.log`, `suite-2.log`, `clippy.log`

Under your rule against deleting `target/` by hand, I left the two build directories in place for you to clean up.

```findings
[
  {"file": "crates/takt/src/case.rs", "line": 746, "category": "judgement", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "approvals_only admits any approve-subject commit touching only store paths, so repository.merge now merges a hand-made store rewrite (here the story's acceptance) under the human's grant; red at approval_record_adv1.rs:346, green on base"},
  {"file": "crates/takt/src/validator.rs", "line": 1092, "category": "concurrency", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a push the forge applied but reported failed leaves branch.yaml at the revision and the forge at the approval commit, and every later run merges nothing; red at approval_record_adv1.rs:394"},
  {"file": "crates/takt/src/validator.rs", "line": 1108, "category": "acceptance", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a forge requiring checks refuses the merge of the approval commit pushed just before it and the run ends in an error, while CI was only read on the granted revision; red at approval_record_adv1.rs:425"},
  {"file": "protocols/story-delivery/1.yaml", "line": 38, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "pull_request, merge_observation and pr.open still say the head is the implementation revision, which no longer holds after an approval"},
  {"file": "crates/takt/tests/approval_record.rs", "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the refusal case stays green with carries mutated to always answer true; approval_record_adv1.rs case 4 kills that mutant"}
]
```
