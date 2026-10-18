---
format: aep.planning-md/3
id: review-result:adversary-frontier-pass-2
kind: review-result
status: active
title: Adversary, frontier-next-ready-story, pass 2
relations:
- reviews: story:frontier-next-ready-story
revision: 1
---
```
unit: story:frontier-next-ready-story, pass 2 — takt-w2-frontier at 366735b (base ccca575d), working tree with my additions to the two test files
verdict: CONFIRMED
cases: executed 16→24, red 2  (gate: frontier 10→15, b10x-takt-cli 6→9; "before" from the implementor's c1-green-run-2.log; both packages whole 45→53)
origin: introduced 5 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 directory tree (837M) plus the harness's 3 background-task logs (part 6)
needs-coordinator: finding 1 is pre-existing in case.rs but the unit's open_next (run.rs:168) could absorb the fix — route it; the stale doc comment at frontier_cli.rs:104-109 is in a test file I left alone (hard rule 2)
```

## 1. Diff stat (worktree, lease released)

```
 crates/takt-cli/tests/frontier_cli.rs | 177 +++++++++++++++++++++++++
 crates/takt/tests/frontier.rs         | 242 ++++++++++++++++++++++++++++++++++
 2 files changed, 419 insertions(+)
```
Both paths are test files. No implementation file touched; nothing under `.engineering/`. Clippy (`-p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings`) and `cargo fmt --check` pass with my files (I ran `cargo fmt` on the two packages; it changed only my lines).

## 2. Cases added (each run alone before the suite; line numbers after fmt)

| # | file:line | asserts | now | red output when written (verbatim) |
|---|---|---|---|---|
| 1 | `crates/takt/tests/frontier.rs:739` `an_open_refused_on_its_inputs_asks_for_no_move_and_leaves_no_case` | stub aep; `options.inputs[0] = risk=bogus`; `open_next` → Err naming `risk`; no `move` in the stub log; no `cases/` dir | RED | `panicked at crates/takt/tests/frontier.rs:755:5: assertion left == right failed: a story that was not taken has no case directory / left: Some(["story-a-20261018T040955Z"]) / right: None` (post-fmt: `:760:5`) |
| 2 | `crates/takt-cli/tests/frontier_cli.rs:266` `a_run_whose_input_is_refused_leaves_the_story_proposed_and_no_case_directory` | binary `--input risk=bogus` on one-ready: exit 1, stderr names `risk`, story `proposed`, no `cases/` dir | RED | `panicked at crates/takt-cli/tests/frontier_cli.rs:284:5: … stderr takt: the input risk takes one of ["trivial", "standard", "elevated", "critical"], not `bogus` / left: Some(["story-ready-one-20261018T040957Z"]) / right: None` (post-fmt: `:289:5`) |
| 3 | `frontier.rs:594` `a_store_refusing_every_candidate_leaves_no_cases_directory_behind` | stub refuses a, b, c: `None`; stdout = three verbatim refusals in id order; `case_names == None`; 3 moves logged | GREEN (red under M2) | `test result: ok. 1 passed` |
| 4 | `frontier.rs:627` `a_given_case_id_names_the_taken_story_case_after_a_refusal` | `case_id = Some("given")`, stub refuses a: `run.case.id == "given"`, `cases == ["given"]`, `intent.md == "Title of story:b\n\n# Story\n"`, stdout exact | GREEN (red under M13) | `ok. 1 passed` |
| 5 | `frontier.rs:664` `a_refusal_removes_only_the_case_it_opened_not_its_neighbours` | pre-existing `cases/earlier/case.yaml`; stub refuses a: `cases == ["earlier", story-b-…]`, `earlier/case.yaml` still a file | GREEN (red under M14) | `ok. 1 passed` |
| 6 | `frontier.rs:698` `an_open_that_fails_asks_for_no_move_and_leaves_no_case` | stub; `protocol = no-such/1`: Err names it, stdout empty, log has `show story:a` and no `move`, no `cases/` | GREEN (red under M3) | `ok. 1 passed` |
| 7 | `frontier_cli.rs:299` `a_given_case_id_names_the_story_case_from_the_binary` | binary `--case-id given`, no intent: exit 3, stdout starts `frontier: took story:ready-one: Make the check pass`, `cases == ["given"]`, intent.md is the story, status `active` | GREEN (red under M13) | `ok. 1 passed` |
| 8 | `frontier_cli.rs:333` `a_store_refusing_every_story_exits_zero_with_nothing_ready_and_no_cases_directory` | binary with a refuse-all `aep` first on PATH: exit 0, stderr empty, stdout = 3 refusals + `frontier: nothing ready\n`, no `cases/` | GREEN (red under M2) | `ok. 1 passed` |

Cases 1 and 6 first failed to compile (`expect_err` needs `Run: Debug`); rewritten as a `match` before any run — a typo, not a finding. Pass 1's `a_run_that_fails_after_the_take_leaves_the_story_proposed_and_opens_no_case` run alone against the correction: `ok. 1 passed`.

## 3. Suite run (after part 2 existed; pre-fmt line numbers)

`cargo test -p b10x-takt --test frontier`
```
running 15 tests
… (13 ok)
test an_open_refused_on_its_inputs_asks_for_no_move_and_leaves_no_case ... FAILED
---- stdout ----
panicked at crates/takt/tests/frontier.rs:755:5: … left: Some(["story-a-20261018T041131Z"]) right: None
test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
EXIT=101
```
`cargo test -p b10x-takt-cli`
```
Running unittests src/main.rs: running 3 tests … test result: ok. 3 passed; 0 failed
Running tests/frontier_cli.rs: running 6 tests
… (5 ok)
test a_run_whose_input_is_refused_leaves_the_story_proposed_and_no_case_directory ... FAILED
---- stdout ---- panicked at crates/takt-cli/tests/frontier_cli.rs:284:5: … left: Some(["story-ready-one-20261018T041133Z"]) right: None
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
EXIT=101
```

Mutants in the scratch copy (`cargo test -p b10x-takt -p b10x-takt-cli --no-fail-fast`, the unit's 45 cases):

| mutant (run.rs) | unit's 45 | my cases red |
|---|---|---|
| M2: the `remove_dir(cases)` block (:185-188) dropped | 45 green | #3 `frontier.rs:610`; #8 `frontier_cli.rs:359` |
| M14: `:187` `remove_dir(cases)` → `remove_dir_all(cases)` | 45 green | #5 `frontier.rs:682` |
| M13: `:124` `if options.case_id.is_none()` dropped | 45 green | #4 `frontier.rs:643`; #7 `frontier_cli.rs:319` |
| M3: take before open (the pass-1 defect reinstated) | 44 green, 1 red (the unit's `frontier_cli.rs:135`) | #6 `frontier.rs:724`, #1, #2 |

## 4. Findings (tree: worktree at 366735b + my test files)

| # | file:line | what was measured | what reaches it | verdict | origin | severity |
|---|---|---|---|---|---|---|
| 1 | `crates/takt/src/case.rs:48` (`create_dir_all` and the writes at :48-53 come before the input check at :78-85 and `git::head` at :54) | cases 1 and 2 red; probe on a fixture copy: `takt run --repo R --harness none --iterations 1 --test-cmd true --input risk=bogus` → exit 1, `story:ready-one` stays `proposed`, `.engineering/cases/story-ready-one-…/{intent.md,protocol.yaml,protocol.ir.json,evidence/,decisions/}` left with no `case.yaml`; with `--case-id fixed` the next run under `fixed` exits 1 `the case fixed already exists at …/cases/fixed` while the story is still `proposed` | an operator's `--input risk=…` typo (`main.rs:45-47`), a protocol file whose declared input is not given, a tree whose `rev-parse HEAD` fails. `open_next` (`run.rs:168`) propagates the error without removing the directory, so its contract at `run.rs:156` ("one case directory per taken story, none for a refused one") has a third outcome it does not name | CONFIRMED | pre-existing (`case.rs` byte-identical at ccca575; base `main.rs:167` `Run::open` reaches it with `--intent … --input`) | warning |
| 2 | `crates/takt/src/run.rs:187` (`let _ = std::fs::remove_dir(cases)`) | M2 and M14 both leave the unit's 45 cases green; cases 3, 5, 8 catch them | any repository takt ran on before holds earlier cases under `.engineering/cases`, so a refusal there is M14's input; all-candidates-refused is M2's | CONFIRMED | introduced | warning |
| 3 | `crates/takt/src/run.rs:124` (`if options.case_id.is_none()`) | M13 leaves the 45 green; cases 4 and 7 catch it | `takt run --repo R --case-id X` with no `--intent`; under M13 the id is silently ignored for a story | CONFIRMED | introduced | warning |
| 4 | `crates/takt/src/run.rs:168-175` (open, then take) | probe: an `aep` on PATH whose `move` sends `SIGKILL` to takt → exit 137; on disk a complete case, store `proposed`; the next run opens a second case and the orphan stays | a crash, OOM or power loss between `CaseDir::open` and `aep move`; nothing found in normal operation. The store side is right (never `active` without a case); the disk side is the accepted inverse, and a sweep or resume is decision-blocker:approval-grant-and-case-resume's (story Out of Scope) | INFEASIBLE | introduced | note |
| 5 | `crates/takt/src/frontier.rs:83-94` (`Frontier::next`, public default) | it takes a story with no case — the state the correction exists to prevent; callers: 4 of the unit's tests, none in the binary | nothing found outside tests | INFEASIBLE | introduced | note |
| 6 | `crates/takt-cli/tests/frontier_cli.rs:104-109` | the doc comment says "The story is taken … before the protocol is loaded … the story is left `active`" above a case that now asserts the opposite and is green | the next reader of the test file; committed unchanged in 366735b | CONFIRMED | introduced | note |

Fix for 1 (named, not applied): in `CaseDir::open` check the inputs and `HEAD` before `create_dir_all`, or `remove_dir_all(dir)` on any error after it; alternatively `open_next` removes `<tree>/.engineering/cases/<id>` when `open_unit` fails. For 2 and 3: keep cases 3, 4, 5, 7, 8. For 6: rewrite the comment.

## 5. Attacked and not broken

- Dropped or relaxed assertions in the correction: none. The one removed line, `assert_eq!(run.unit, Unit::Story(story))` in `the_one_ready_story…`, became tautological (`story` is now taken from `run.unit`) and an exact stdout assertion was added; pass 1's seven cases are verbatim.
- Reorder (M3): caught by the unit's own `frontier_cli.rs:135` and now by case 6 at the library level.
- Refusal with a case: the refused story's directory removed, the second candidate's case holds the second story's `intent.md` (case 4), stub log order show→move→show→move (unit's case).
- All candidates refused: `None`, three verbatim refusals in id order, no `cases/`; from the binary `frontier: nothing ready`, exit 0, empty stderr (cases 3, 8).
- `--case-id` with two candidates: the refused first candidate's case under the given id is removed and the id reused for the second; the binary passes `--case-id` through for a story (cases 4, 7).
- Wrong `--protocol` end to end on a fixture copy: exit 1 `takt: protocol no-such/1: UnknownName { name: "no-such", major: 1 }`, no `cases/`, story `proposed`.
- `show`/`take` store guard at `frontier.rs:244`, `:258`: pass 1's case green.
- Real-store writes from my cases: none (stub cases run a shell script; real-aep cases run in fixture copies under `target/tmp`, empty now; `.engineering/` clean in `git status`).

## 6. Paths written outside the worktree

- the unit's scratch `adversary-2/` (837M): `mutant/` (source copy without `target/` or `.git`, 1020K; its build dir `mutant/target/` 835M; M3 left applied in `mutant/crates/takt/src/run.rs`), `mutant-M{2,14,13,3}.diff`, logs, `mutate.py`, `run-mutants.sh`, `run-mutants.out`, `frontier-append.rs`, `frontier-cli-append.rs`, `fix-expect-err.py`, `probe-crash.sh`, `probe-crash/` (fixture copy, two cases, story `active`), `probe-protocol.sh`, `probe-protocol/` (two fixture copies).
- The harness's own background-task logs for my mutant builds.

Inside the worktree besides the two test files: nothing (`target/tmp` empty; test binaries rebuilt in `target/`).

```findings
- file: crates/takt/src/case.rs
  line: 48
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: "CaseDir::open creates the case directory and writes intent.md and the protocol before it checks the inputs (:78-85) and HEAD (:54), so a run refused on --input risk=bogus leaves a half-written case with no case.yaml while the story stays proposed, and a reused --case-id is dead until it is removed by hand; open_next (run.rs:168) inherits it and its contract names only taken and refused"
- file: crates/takt/src/run.rs
  line: 187
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the remove_dir(cases) line is untested: dropping it (an empty cases/ left after every candidate is refused) and widening it to remove_dir_all (an earlier run's case wiped on a refusal) both leave the unit's 45 cases green; the added cases at frontier.rs:594, :664 and frontier_cli.rs:333 catch both"
- file: crates/takt/src/run.rs
  line: 124
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "no case gives a story a case id, so making open_unit always derive the id from the story (ignoring --case-id) leaves the unit's 45 cases green; the added cases at frontier.rs:627 and frontier_cli.rs:299 catch it"
- file: crates/takt/src/run.rs
  line: 168
  category: concurrency
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "a crash between CaseDir::open and aep move leaves a complete case directory with its story still proposed, and the next run opens a second case for the same story beside the orphan; measured with a SIGKILL from a stub aep, reached only by a crash in that window, and the sweep or resume belongs to decision-blocker:approval-grant-and-case-resume"
- file: crates/takt/src/frontier.rs
  line: 83
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "Frontier::next stays a public default method that takes a story with no case, the state open_next was added to prevent; only four of the unit's tests call it and nothing in the binary does"
- file: crates/takt-cli/tests/frontier_cli.rs
  line: 104
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the doc comment above a_run_that_fails_after_the_take_leaves_the_story_proposed_and_opens_no_case still describes the pre-correction order and outcome (the story left active) that the case beneath it now asserts against"
```
