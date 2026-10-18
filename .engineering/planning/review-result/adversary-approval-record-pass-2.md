---
format: aep.planning-md/3
id: review-result:adversary-approval-record-pass-2
kind: review-result
status: active
title: Adversary, approval-record-reaches-base, pass 2
relations:
- reviews: story:approval-record-reaches-base
revision: 1
---
```
unit: story:approval-record-reaches-base, worktree w7-approval-record at 506efa1 plus the untracked crates/takt/tests/approval_record_adv2.rs
verdict: INFEASIBLE (1 red case, from a forge behaviour I built in a test double); 3 CONFIRMED test gaps or drift
cases: executed 292→297, red 1
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, <wave-scratch>/scratch/approval-record/adv2/
needs-coordinator: whether a merge the forge refuses for required checks must become "nothing merged" (what correction 2 J3 literally asked for), which also needs a forge.rs change outside this unit
```

One of my five new cases fails. A run still ends in an error when the forge refuses the merge for a required check, because the validator only reads the checks before merging and trusts the answer. The other four cases pass. Three of them kill mutants that the unit's own tests let survive.

**1. Diff stat**
`git --no-pager diff --stat` prints nothing, because my file is untracked. `git status --short` shows one path, `?? crates/takt/tests/approval_record_adv2.rs`. It is a test file, and no implementation file was touched.

**2. Cases added** (`crates/takt/tests/approval_record_adv2.rs`)

| case | asserts | now |
|---|---|---|
| `adv2_an_approve_commit_that_is_not_a_pure_addition_of_a_regular_file_is_no_approval` | a single approve-subject commit on the granted revision is not accepted as an approval when it adds an executable file, adds a symlink, adds a record and rewrites one, only rewrites one, changes a file mode, renames or deletes | green |
| `adv2_names_are_read_byte_for_byte_and_a_story_name_prefix_is_not_the_story` | a file name with a newline and a tab in it is accepted; `story/ready-one-x/…` and `story/ready-onerecord.json` are refused | green |
| `adv2_a_push_that_never_reached_the_forge_is_pushed_again_and_merged_by_the_next_run` | when the push fails before reaching the forge, the pull request stays at the granted revision while `branch.yaml` names the approval head; the next run pushes again and merges | green |
| `adv2_checks_failing_then_passing_on_the_approval_head_merge_on_the_next_run` | failing checks on the approval head: nothing is merged and the run ends without an error; after the checks pass, the next run merges, with one observation about the granted revision | green |
| `adv2_checks_answering_for_the_head_before_the_push_do_not_end_the_run_in_an_error` | the forge reports the checks of the head before the push (green) and then refuses the merge with a 405 for a required check; the run must not end in an error | **red** |

Red output from running the file alone (`adv2/red-alone.log`, `:643` before formatting, `:651` after):
```
the checks read answered for 828f244e (the granted revision, before the push of 9285bd23), and the merge of 9285bd23 refused for its required check ended the run in an error: Err("repository.merge (takt.pr-merged): merge pull request #1 through the forge per-head: b10x-gates api --method PUT --path /repos/o/r/pulls/1/merge exited 1: 405 Required status check \"takt / task check\" is expected")
```

Mutation probes, run on a `git archive` copy in `adv2/mut/` (the worktree was never mutated):

| mutant | unit `approval_record.rs` | adv1 | adv2 |
|---|---|---|---|
| M1 `case.rs:817` `*to_mode != "100644"` → `false` | 6/6 green | not run | red at `:320` ("an executable file") |
| M2 `case.rs:815-816` status and from_mode checks → `false` | 6/6 green | not run | red at `:340` ("a record added beside a rewritten one") |
| M3 `validator.rs:1046` `&& view.head.sha != revision` removed | 6/6 green | 4/4 green | red (never-reached case) |

Logs: `adv2/mutant-M1.log`, `adv2/mutant-M2.log`, `adv2/mutant-M3.log`.

**3. Suite** (run after the cases existed)

| step | exit | result |
|---|---|---|
| `cargo fmt -p b10x-takt -p b10x-takt-cli --check` | 0 | after `rustfmt --edition 2024` on my file only |
| `cargo clippy … --all-targets --locked -- -D warnings` | 0 | |
| `cargo test -p b10x-takt -p b10x-takt-cli --locked --no-fail-fast` | 101 | 296 passed, 1 failed, 297 executed (`adv2/suite.log`) |

The before number, 292, comes from the implementor's `c2-test.log`, not from a run of mine.

**4. Findings** (they cover 506efa1 plus my test file)

| id | file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|---|
| K1 | `crates/takt/src/validator.rs:1110` | The checks are read before the merge, but a merge the forge then refuses for required checks still ends the run in an error. Correction 2 J3 asked for that refusal to mean "nothing merged". The checks read (`Forge::checks(tree, number)`, `forge.rs:151`) names no head, so nothing ties the answer to the head being merged. Fix: treat a 405 refusal of the merge as nothing merged, or read the checks for a named sha (forge.rs, outside this unit). | INFEASIBLE / introduced, warning | Only a forge whose checks lag its head, which I built. On GitHub, `gh pr checks` right after a push most likely answers "no checks reported", which takt reads as pending. I don't know if the stale answer happens live. |
| K2 | `crates/takt/src/case.rs:815` | The mode and status checks in `adds_records_only` (M1, M2) have no test in the unit's suite. Case 1 of adv2 kills both mutants. | CONFIRMED / introduced, note | The suite only. |
| K3 | `crates/takt/src/validator.rs:1046` | The new "or the revision" acceptance, which lets the next run retry a push that never reached the forge, has no test in the unit's suite or in adv1 (M3 survives both). Case 3 of adv2 kills it. Without the clause, a failed network push would leave the case stuck. | CONFIRMED / introduced, warning | Any push that fails before reaching the remote. |
| K4 | `ess/domains/takt.yaml:261` and `:1031` | The ESS still says an approval's commit is "store paths only", which is now narrower: new regular files in the story's evidence only, capped at the grant count. It also says the merge validator merges a head that is "the case's branch at the revision". Its `read from:` citations at `case.rs:748-755` and `validator.rs:942-952` now point at shifted text. `ess specify validate` still exits 0. | CONFIRMED / introduced, note | Every reader of the spec. `ess/` is not in this unit's file assignment. |

**5. Attacked and not broken**
- The `diff-tree --raw -z` parsing: renames (`--no-renames` turns them into a delete plus an add), mode changes, 100755 files, symlinks, newline and tab in names, and a sibling or prefixed story directory are all handled correctly. `git::run`'s `trim` and lossy UTF-8 decoding cannot reach a field.
- Grant count: `prepare_grant` keeps one decision per capability (`case.rs:602`), and story-delivery requires only `repository.merge`, so the count is at most 1. Two approve commits on one grant need a failed `authority.yaml` write, and that case recovers through a fresh gate.
- The real `aep` 0.74 evidence write adds one new file under `evidence/<ns>/<name>/` and leaves the document alone when it is unchanged (`aep-backend-markdown/src/provider.rs:761`). So the narrowed check accepts real approvals. The unit's own test runs the real `aep`.
- Protocol text change (J4): the embedded protocol is `include_str!`, and no test pins its text or IR. A resumed case runs on its own `protocol.yaml`. Only an explicit `--protocol story.delivery/1` on a case opened before the change would be refused (`main.rs:552`), and the repository has no tags, so no binary shipped the old text.
- A pull request whose head moves after the read: the merge names the sha and the forge refuses it with an error. That is the same behaviour as at the base, so it is not new.

**6. Paths written outside the worktree**
All under `<wave-scratch>/scratch/approval-record/adv2/`:
- `mut/`: a `git archive` copy with its own `target/`, 796M. I left it for you to clean up, under the rule against deleting `target/` by hand.
- `case.rs.orig`, `validator.rs.orig`
- `red-alone.log`, `mutant-M1.log`, `mutant-M2.log`, `mutant-M3.log`, `fmt.log`, `clippy.log`, `suite.log`

```findings
[
  {"file": "crates/takt/src/validator.rs", "line": 1110, "category": "acceptance", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a merge the forge refuses for a required check after a checks read that answered pass (for the head before the push) still ends the run in an error, not nothing merged as correction 2 J3 asked; red at approval_record_adv2.rs:651"},
  {"file": "crates/takt/src/case.rs", "line": 815, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "dropping the to_mode check, or the status and from_mode checks, in adds_records_only leaves the unit's suite green; approval_record_adv2.rs case 1 kills both"},
  {"file": "crates/takt/src/validator.rs", "line": 1046, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "removing the head-equals-revision acceptance that lets a never-delivered push be retried leaves the unit's suite and adv1 green; approval_record_adv2.rs case 3 kills it"},
  {"file": "ess/domains/takt.yaml", "line": 261, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the spec still says an approval's commit is store paths only and (at :1031) that the merge takes the case's branch at the revision; both are now false and the cited code lines have shifted"}
]
```
