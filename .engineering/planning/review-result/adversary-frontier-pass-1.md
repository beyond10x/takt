---
format: aep.planning-md/3
id: review-result:adversary-frontier-pass-1
kind: review-result
status: active
title: Adversary, frontier-next-ready-story, pass 1
relations:
- reviews: story:frontier-next-ready-story
revision: 1
---
```
unit: story:frontier-next-ready-story, pass 1 — takt-w2-frontier at 3a411bf (base ccca575d), working tree with my two test files
verdict: NEEDS-CHANGE
cases: executed 7→14, red 2  (frontier 4→8, b10x-takt-cli 3→6; "before" = the suite run with my cases deselected by name, see part 3)
origin: introduced 4 / pre-existing 0 / undecided 0  (the base has no frontier.rs and `--intent` is required there)
wrote-outside-worktree: 4 paths (part 6)
needs-coordinator: the severity call on finding 1 (returned as blocker: the store's story ladder has no `active → proposed`); whether the story text's "order `waves` prints" or the brief's "id order" is the contract (finding 4); the new untracked test file crates/takt-cli/tests/frontier_cli.rs needs an owner
```

## 1. Diff stat (worktree, after lease release)

```
 crates/takt/tests/frontier.rs | 211 ++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 211 insertions(+)
 M crates/takt/tests/frontier.rs
?? crates/takt-cli/tests/            (frontier_cli.rs, new)
```
Both paths are test files. No implementation file touched; nothing under `.engineering/`.

## 2. Cases added (each run alone, before the suite)

| # | file | asserts | now | red output when written (verbatim) |
|---|---|---|---|---|
| 1 | crates/takt-cli/tests/frontier_cli.rs:111 `a_run_that_fails_after_the_take_leaves_the_story_proposed_and_opens_no_case` | binary `takt run --repo R --harness none --iterations 1 --test-cmd true --protocol no-such/1` on a one-ready copy exits 1, opens no case, and `story:ready-one` is still `proposed` | RED | `panicked at crates/takt-cli/tests/frontier_cli.rs:132:5: assertion `left == right` failed: a run that opened no case must not leave the story taken; stdout frontier: took story:ready-one: Make the check pass / stderr takt: protocol no-such/1: UnknownName { name: "no-such", major: 1 } / left: "active" / right: "proposed"` |
| 2 | crates/takt/tests/frontier.rs:425 `show_and_take_refuse_a_repository_without_a_planning_store` | `Aep::show` and `Aep::take` on a directory without `.engineering/project.yaml` return `Err` containing "no planning store" | RED | `panicked at crates/takt/tests/frontier.rs:436:5: aep plan artifact show story:adversary-absent: error: <worktree>/.engineering/planning holds no `story:adversary-absent`; it would be at story/adversary-absent.md` — `show` reached this worktree's real store; the id exists nowhere, so the probe only read; `take` was never reached in the red run |
| 3 | crates/takt/tests/frontier.rs:294 `a_refused_take_is_printed_verbatim_and_the_next_ready_story_is_taken` | with a stub `aep` (shell script written into the test's scratch; refuses `move story:a`, accepts `story:b`): `next` returns `story:b`, prints exactly `frontier: story:a not taken: <refusal>\n`, and the move sequence logged is `story:a` then `story:b` (no `story:c`) | GREEN (red on both mutants, part 4) | `test result: ok. 1 passed` |
| 4 | crates/takt/tests/frontier.rs:331 `two_ready_stories_are_taken_in_id_order_one_per_ask` | real `aep`: `new story aaa-first` + `move --to proposed`; three asks give `story:aaa-first`, `story:ready-one`, `None`; statuses move one per ask; nothing refused | GREEN | `test result: ok. 1 passed` |
| 5 | crates/takt/tests/frontier.rs:371 `an_archived_dependency_is_done_and_a_rejected_one_is_not` | real `aep`: `draft-dep → archived` makes `waits-one` ready and taken; `draft-dep → proposed → rejected` leaves `waits-one` `proposed` and `next` = `None` | GREEN | `test result: ok. 1 passed` |
| 6 | crates/takt-cli/tests/frontier_cli.rs:140 `a_repository_without_a_store_exits_one_and_names_the_missing_store` | binary exits 1, stderr contains "no planning store", stdout has no `frontier: took`, no `.engineering` created | GREEN | `test result: ok. 1 passed` |
| 7 | crates/takt-cli/tests/frontier_cli.rs:164 `nothing_ready_exits_zero_from_the_binary` | binary exits 0, stdout is exactly `frontier: nothing ready\n`, stderr empty, no cases dir | GREEN | `test result: ok. 1 passed` |

Case 2 first failed to compile (my `r#"…"#` literal contained `"#`); fixed to `r##`, then run alone: red as above. That compile error is a typo, not a finding.

## 3. Suite run (after part 2 existed)

`cargo test -p b10x-takt --test frontier`
```
running 8 tests
test show_and_take_refuse_a_repository_without_a_planning_store ... FAILED
test a_repository_without_a_planning_store_is_refused_not_searched_upward ... ok
test a_refused_take_is_printed_verbatim_and_the_next_ready_story_is_taken ... ok
test an_intent_is_the_unit_when_given ... ok
test a_store_with_nothing_ready_gives_no_unit_and_moves_nothing ... ok
test the_one_ready_story_is_taken_once_and_becomes_the_case ... ok
test two_ready_stories_are_taken_in_id_order_one_per_ask ... ok
test an_archived_dependency_is_done_and_a_rejected_one_is_not ... ok
test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
```
`cargo test -p b10x-takt-cli`
```
Running unittests src/main.rs: running 3 tests … test result: ok. 3 passed; 0 failed
Running tests/frontier_cli.rs: running 3 tests
test a_repository_without_a_store_exits_one_and_names_the_missing_store ... ok
test nothing_ready_exits_zero_from_the_binary ... ok
test a_run_that_fails_after_the_take_leaves_the_story_proposed_and_opens_no_case ... FAILED
  (same panic as part 2, case 1: left: "active" / right: "proposed")
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```
Both exit non-zero. "Before" counts: of the 8 and 6 cases that ran, 4 and 3 are mine (named above); the unit's own are 4 + 3 = 7.

## 4. Findings (tree: worktree at 3a411bf + my test files)

| # | file:line | what was measured | what reaches it | verdict | origin | severity |
|---|---|---|---|---|---|---|
| 1 | crates/takt-cli/src/main.rs:178 (take) vs :189 (`Run::open_unit`, which loads the protocol and opens the case) | case 1: exit 1, no case directory, `story:ready-one` = `active`. Probe in my scratch copy: `aep plan artifact move story:ready-one --to proposed` → `story:ready-one is active; a story may move to: implemented, archived`, exit 1 — the ladder (`artifacts/lifecycles/story.yaml` at the pinned snapshot: `active: [implemented, archived]`) has no way back, and resuming an `active` story is out of this story's scope | an operator's `--protocol` typo; a relative `--protocol protocols/story-delivery/1.yaml` from another cwd (README.md:24-25 warns the path is read from the cwd); any protocol with declared inputs not given (`CaseDir::open` refuses after the take); `--case-id` that already exists. The comment at main.rs:174-175 ("asked for only once everything else about the run is in order") promises the opposite order | CONFIRMED | introduced | blocker |
| 2 | crates/takt/src/frontier.rs:202 (`show`), :223 (`take`) | only `list` (:193) calls `Self::store`; case 2 shows `show` on a storeless directory under `target/` reading this worktree's real store (`aep` walks up); `take` has the same shape and would run `move` against it | nothing found: the only caller of either is `next` (:241), which calls `list` first on the same `repo`; both are `pub` | INFEASIBLE | introduced | warning |
| 3 | crates/takt/src/frontier.rs:241-253 (`next`, refusal branch) | mutant A (`return Ok(None)` after printing) and mutant B (`Take::Refused(_) => {}`) in the scratch copy: the unit's 9 lib + 4 integration + 3 CLI cases all stay green under both; case 3 fails under A (`panicked at frontier.rs:302:10: story:b is ready and taken`) and under B (`left: "" / right: "frontier: story:a not taken: …"`) | the story's own text: "the refusal is printed verbatim, and the run goes on to the next ready story"; the fixture stores cannot produce a refusal (every `proposed` story may go `active`), so the unit's suite never exercises the one store-write path's failure branch | CONFIRMED | introduced | warning |
| 4 | crates/takt/src/frontier.rs:130-134 (`ready`, id order) vs the story artifact (revision 6, Context) | the story says the adapter reads `aep plan artifact waves --format json --status proposed` for the order and a ready story "comes first in the order `waves` prints"; the code never calls `waves` and sorts by id. Probe on the fixture: `waves` places only `story:ready-one` and prints `unassessed: [story:blocked-one, story:waits-one]` ("a story with no scope … never placed"), so a scope-less ready story is a unit under the code and never under the story's text | the brief's "What the unit is" chose id order, so this is the story text and the code disagreeing, with the brief siding with the code | CONFIRMED | introduced | note |

Fix for 1 (named, not applied): resolve `LoadedProtocol` (and check the declared inputs) before `Aep::next` is asked, as the comment at main.rs:174-175 already claims; or take the story after `CaseDir::open` succeeds. Fix for 2: `Self::store(repo)?` in `show` and `take` (or in `plan`). Fix for 4: correct the story/brief text, or order by `waves`.

## 5. Attacked and not broken

- `blocked_by` as `{blocker, type}` objects: deserialised, blocked story skipped (fixture + real store listing).
- `depends_on` → archived: listed by `aep list` (real store lists 5 archived stories) and counted done (case 5); → rejected: not done (case 5); → draft/proposed/active: not done (unit's test).
- Two ready stories: id order, one per ask, third ask `None` (case 4).
- `draft` story never taken; `active` story (no case) never taken twice: the store refuses a second `move --to active` (`is active; may move to: implemented, archived`), so two concurrent runs cannot both take one story.
- Refusal printed verbatim and the next candidate tried (case 3, green against the tree).
- Missing `.engineering/project.yaml`: `next` refused, exit 1 from the binary, parent store not consulted via `list` (case 6).
- `intent.md` = title + blank line + body; `aep show --format json` `body` starts at `# Story:` with no frontmatter; case id `story-<name>-<instant>` with `now()` fixed to `YYYY-MM-DDTHH:MM:SSZ` (no fractional seconds or offset to leak into the id).
- `--intent` with a frontier store: no `frontier:` line, story stays `proposed` (unit's CLI test).
- Exit codes: nothing ready → 0 with exactly `frontier: nothing ready\n` (case 7); frontier error → 1.
- Real-store write from a test: none; the unit's tests run `aep` only inside fixture copies under `target/tmp`, both scratch helpers `Drop`-remove their trees (`target/tmp` is empty now), and `git status` of `.engineering/` is clean.
- Fixture `project.yaml` pins the same protocols snapshot as the real store, so the tests add no new fetch.

## 6. Paths written outside the worktree

- the unit's scratch `adversary-1/probe/` — fixture copy (git-initialised; stories moved/created in it by my probes)
- the unit's scratch `adversary-1/mutant/` — source copy without `target/` and `.git` (1016K); its build directory `adversary-1/mutant/target/` is 702M (mutant B left applied in `crates/takt/src/frontier.rs` there)
- the unit's scratch `adversary-1/mutant-A.log`
- the harness's own background-task log for my mutant build

Inside the worktree, besides the two test files: `target/tmp/adv-storeless/` was created for one probe and removed.

```findings
- file: crates/takt-cli/src/main.rs
  line: 178
  category: concurrency
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: "the story is moved proposed→active at :178 before the protocol is loaded and the case opened at :189, so a failing --protocol or input leaves the story active with no case and the store's ladder (active: [implemented, archived]) offers no way back, while the comment at :174-175 promises the opposite order"
- file: crates/takt/src/frontier.rs
  line: 202
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "Aep::show (:202) and Aep::take (:223) are public and skip the Self::store guard that list (:193) applies, so on a repository without a store aep walks up and reads, and for take would write, the store above it; only next reaches them today and it lists first"
- file: crates/takt/src/frontier.rs
  line: 241
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the refusal branch of Aep::next has no case in the unit's suite: ending the frontier on a refusal or dropping the printed refusal leaves all 16 unit cases green, and only the added stub-aep case catches both"
- file: crates/takt/src/frontier.rs
  line: 130
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the story artifact (revision 6) says the unit is first in the order aep plan artifact waves prints, which never places a story without scope, while ready() sorts by id and waves is never called, so the story text and the code disagree on a scope-less ready story"
```
