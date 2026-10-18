---
format: aep.planning-md/3
id: review-result:adversary-authority-pass-1
kind: review-result
status: active
title: Adversary, authority-hardening, pass 1
relations:
- reviews: story:authority-hardening
revision: 1
---
unit: story:authority-hardening, commit 9b803fb (base ae94ccb), worktree w6-authority-hardening, plus my untracked test file
verdict: NEEDS-CHANGE
cases: executed 136→139, red 2
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 5 paths (part 6)
needs-coordinator: yes. The unit added `check_id(id)?` inside `CaseDir::open` at case.rs:745, which the brief assigns to another unit. Expect a merge conflict there.

**1. Diff stat**

`git --no-pager diff --stat` prints nothing, because tracked files are unchanged. `git status --short` shows one path, a test file: `?? crates/takt/tests/authority_adv1.rs`. I did not stage it, since `git add` is forbidden.

**2. Cases added** (in `crates/takt/tests/authority_adv1.rs`)

| case | asserts | now | red output when run alone |
|---|---|---|---|
| `adv1_a_run_resumed_before_another_moved_the_case_is_not_admitted_the_merge_at_the_new_revision` | Setup: run A resumes at R1, where the grant was given. A commit R2 lands, and run B resumes, reaches the gate at R2 and lets go. Then A takes hold and evaluates. Asserts `repository.merge` is not admissible | red | `panicked at crates/takt/tests/authority_adv1.rs:164:5: assertion left != right failed: the grant given at a7a1073f… admitted the merge with the case at 9e0cff24…`. The decision's `computed_from` holds `authority: [{capability: repository.merge, decision: granted}]` with `case…implementation.revision: 9e0cff24…` |
| `adv1_an_empty_evidence_record_is_an_error_from_has_evidence` | `has_evidence` errors on an empty record and names the file, as its own doc comment promises | red | `panicked at crates/takt/tests/authority_adv1.rs:182:5: an empty record was an answer: Ok(false)` |
| `adv1_probe_evaluate_refuses_an_empty_record_naming_it` | Reachability probe: `evaluate` refuses the empty record and names it | green | n/a |

Both red cases came from `cargo test -p b10x-takt --locked --test authority_adv1`, with exit 101 (log `red-alone.log`). I added the probe after the first suite run and ran it alone: `1 passed; … 2 filtered out`.

**3. Suite, run after the cases existed**
- `cargo fmt -p b10x-takt --check`: exit 0.
- `cargo clippy -p b10x-takt --all-targets --locked -- -D warnings`: exit 0.
- `cargo test -p b10x-takt --locked --no-fail-fast`: exit 101.
  - lib 14, authority 11, forge 35, frontier 15, gate_validator 16, loop 2, merge_under_authority 30, story_delivery 13, doctests 0: all ok.
  - authority_adv1: `FAILED. 1 passed; 2 failed`.
  - Before = 136: the same run without the authority_adv1 binary.

**4. Findings**

| file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|
| case.rs:1115-1117 | `evaluate` filters the grant by the in-memory revision (`self.revision`) but gives Canon `case.yaml` from disk. A `CaseDir` that went stale during the gap between `resume` and `hold` therefore admits the merge at R2 under the R1 grant. Fix: take `at` from the `case_yaml` that is handed to Canon, and/or keep the hold from `resume` through `Run::run` (run.rs:414). | INFEASIBLE / introduced | The window is between `Run::resume` returning (main.rs:318) and `hold()` (run.rs:414). B's entire run must fit inside it, so only a suspended process gets there. Downstream, `PrMerged` still refuses: it observes at stale R1, while `pushed` is R2 (not measured end to end). |
| case.rs:1065 | `has_evidence` reads records as YAML. An empty file or a bare scalar parses, so it answers `Ok(false)` where its doc promises an error. At base, the JSON parse returned `Err`. | INFEASIBLE / introduced | The only caller is `is_due` (run.rs:91). It runs after `evaluate`, which refuses the empty record first (the probe is green). So this is a doc-versus-code mismatch only. |
| case.rs:745 | `check_id` inside `CaseDir::open`, a symbol the brief gives to another unit | CONFIRMED / introduced | The parallel unit editing `open` at merge time |

**5. Attacked, could not break**
- **Lock:** I found no way past `create_new` for hold, resume or grant.
- **Grants:**
  - a grant without a revision admits nothing;
  - a new grant replaces the old one, leaving one decision;
  - "already decides" applies only at the same revision;
  - `PrMerged` re-checks the grant against the pushed head and the PR head.
- **"At the gate":** the `computed_from` path matches real Canon output.
- **approve:** it records first; a failed record leaves no grant and no lock.
- **`check_id`:** it refuses `/`, `\`, `..`, a leading `/`, control characters, and nested ids from branch names in `current`.
- **`latest_decision`:** it is numeric past 9999.
- **Existing tests:** the mutants I traced (lexical sort, no filter, no `retain`, no `decided_at` check, no `grants_at`) are each caught by an existing test.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/authority-hardening/adv1/` (directory)
- `…/adv1/red-alone.log`
- `…/adv1/clippy.log`, `…/adv1/clippy2.log`
- `…/adv1/suite.log`, `…/adv1/suite2.log`

Test scratch trees went under the worktree's `target/tmp`, and each test removes its own on drop. No processes are left running.

```findings
- file: crates/takt/src/case.rs
  line: 1115
  category: concurrency
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: evaluate filters the grant by the in-memory implementation revision while Canon reads case.yaml from disk, so a run resumed before another moved the case admits repository.merge at R2 under the grant given at R1
- file: crates/takt/src/case.rs
  line: 1065
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: has_evidence answers Ok(false) for an empty or scalar record although its doc promises an error naming the file; evaluate refuses it first in the run
- file: crates/takt/src/case.rs
  line: 745
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: check_id was added inside CaseDir::open, a symbol the brief assigns to another unit, so the two units will conflict at merge
```
