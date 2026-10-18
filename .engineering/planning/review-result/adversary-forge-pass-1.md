---
format: aep.planning-md/3
id: review-result:adversary-forge-pass-1
kind: review-result
status: active
title: Adversary, forge-pull-request-evidence, pass 1
relations:
- reviews: story:forge-pull-request-evidence
revision: 1
---
```
unit: story:forge-pull-request-evidence, commit 471662d3aa814a87497bc0c2ec7f36532afdb10daef on impl/forge-pull-request-evidence (base 97eae59), worktree takt-w4-forge
verdict: CONFIRMED
cases: executed 95→104, red 4
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory (the unit's scratch adversary-1/, 36 files, 1.6M; the mutant copy's target/ (662M) removed)
needs-coordinator: the `ess/domains/takt.yaml` declarations the unit made stale (coordinator-owned file); the design call on where a second case's branch forks from (judgement row 6)
```

## 1. Diff — proof of what was touched

```
 crates/takt/tests/forge.rs | 444 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 444 insertions(+)
 M crates/takt/tests/forge.rs
?? crates/takt-cli/tests/forge_cli.rs   (new, 118 lines)
```

Both paths are test files. `cargo fmt -p b10x-takt -p b10x-takt-cli --check` exit 0 after formatting my file only; `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` exit 0 (`Finished dev profile`). `df -h /`: 150G free before the first build.

## 2. Cases added (written before anything ran; each run alone first)

All in `crates/takt/tests/forge.rs` (appended after line 644) unless noted.

| # | case | asserts | now |
|---|---|---|---|
| 1 | `adv_pr_open_on_a_detached_head_pushes_nothing_before_refusing` | a case opened from a detached HEAD (base `None`) is refused by `pr.open` with no forge call before the refusal | **red** |
| 2 | `adv_a_case_discarded_over_another_cases_branch_returns_the_tree_to_that_branch` | a case opened while the tree is on `takt/adv-over-a` is discarded back to `takt/adv-over-a`, as `CaseDir::discard` documents | **red** |
| 3 | `adv_a_pull_request_number_planted_by_the_harness_does_not_earn_pr_opened_or_ci_green` | loop-level: a harness writes `pull_request: 7` into `branch.yaml` in its `implementation.edit` step; `pr.opened` and `ci.green` must not become true with no PR opened for the branch | **red** |
| 4 | `a_non_github_origin_does_not_make_the_bot_route_the_default` — `crates/takt-cli/tests/forge_cli.rs:103` | the binary with an `origin` on `gitlab.example.com` (ssh, https) or `file://` and no `--forge` prints no `forge: bot` line | **red** |
| 5 | `adv_a_second_revision_pushes_again_and_opens_no_second_pull_request` | second revision: calls are `push, view #1, checks #1`; `opened.len()==1`; `branch.yaml` pushed=r2, pull_request=1 | green (mutant-catcher) |
| 6 | `adv_a_closed_pull_request_at_a_second_revision_is_recorded_closed` | forge reports Closed at r2 → record result `closed`, `pr.opened` not true | green (mutant-catcher) |
| 7 | `adv_a_request_scan_text_refuses_never_reaches_the_api` | `scan-text` exit 1 → error names the scan, the `api` line never runs | green (mutant-catcher) |
| 8 | `adv_an_api_reply_without_a_number_is_an_error_not_a_pull_request` | API reply without `number` → error | green (mutant-catcher) |
| 9 | `adv_a_cancelled_check_fails_and_an_unknown_bucket_is_an_error` | bucket `cancel` → `Fail`; bucket `neutral` → error naming it | green (mutant-catcher) |

Red output, verbatim, each case alone:

Case 1 (`cargo test -p b10x-takt --test forge adv_pr_open_on_a_detached_head_pushes_nothing_before_refusing -- --exact`):
```
thread 'adv_pr_open_on_a_detached_head_pushes_nothing_before_refusing' (647203) panicked at crates/takt/tests/forge.rs:708:5:
the branch was pushed before the refusal: ["push takt/adv-detached"]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.02s
```

Case 2:
```
thread 'adv_a_case_discarded_over_another_cases_branch_returns_the_tree_to_that_branch' (647282) panicked at crates/takt/tests/forge.rs:754:5:
assertion `left == right` failed: the tree is not back on the branch the case left
  left: "main"
 right: "takt/adv-over-a"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.03s
```

Case 3 (run trace abridged to the steps that matter; full log at `red-adv_a_pull_request_number_planted_by_the_harness_does_not_earn_pr_opened_or_ci_green.log`):
```
thread '...' (647343) panicked at crates/takt/tests/forge.rs:845:5:
assertion `left != right` failed: pr.opened holds with no pull request opened for the case's branch; the forge was asked ["push takt/adv-planted", "view #7", "checks #7"]; stopped ApprovalRequired (repository.merge)
step 4: implementation.edit (planter)
  effect: committed 1 path(s); implementation is now d71f15b5
step 7: pr.open {}
  effect: pushed takt/adv-planted at d71f15b5 through any; pull request #7 is open
  evidence: pull_request open
step 8: ci.read {}
  effect: checks on pull request #7 at d71f15b5 through any: pass
  evidence: ci_result pass
step 9: repository.merge (planter)
  effect: none; repository.merge needs an approval
  left: "true"
 right: "true"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 5.23s
```

Case 4 (`cargo test -p b10x-takt-cli --test forge_cli a_non_github_origin_does_not_make_the_bot_route_the_default -- --exact`):
```
thread 'a_non_github_origin_does_not_make_the_bot_route_the_default' (648880) panicked at crates/takt-cli/tests/forge_cli.rs:113:9:
origin git@gitlab.example.com:owner/repo.git: the bot route is the default forge: exit 3 stdout forge: bot (owner/repo); pr.open and ci.read go through the bot route
case gitlab-ssh on story.delivery/1 in .../target/tmp/forge-cli-648879-gitlab-ssh/repo; inputs none
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

## 3. Suite run (after part 2 existed)

`cargo test -p b10x-takt --no-fail-fast` → exit 101:
```
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s      (unittests src/lib.rs)
test result: FAILED. 15 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.03s   (tests/forge.rs)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s      (tests/frontier.rs)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.45s      (tests/gate_validator.rs)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s       (tests/loop.rs)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s      (tests/story_delivery.rs)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s       (Doc-tests)
```
`cargo test -p b10x-takt-cli --no-fail-fast` → exit 101:
```
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.79s      (unittests src/main.rs)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s   (tests/forge_cli.rs)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s       (tests/frontier_cli.rs)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.30s       (tests/gate_cli.rs)
```
Executed: before 95 (implementor's `gate-1-test-takt.log` 70 + `gate-2-test-cli.log` 25), after 104 (78 + 26). Red 4.

## Mutants (copy under scratch; forge test binary = unit's 10 + my 9; cases 1–3 are red in every run and are omitted below)

| mutant | file:line (worktree) | unit's cases red | caught only by |
|---|---|---|---|
| one-pr: `match branch.pull_request.filter(\|_\| false)` | validator.rs:395 | none | mine: 5, 6 |
| open-hardcoded: record result `"open"` | validator.rs:452 | none | mine: 6 |
| scan-ignored: `let _ = self.succeed(scan)` | forge.rs:329-330 | none | mine: 7 |
| number-defaulted: `.or(Some(1))` | forge.rs:338 | none | mine: 8 |
| cancel-unknown: `Some("fail")` only | forge.rs:369 | none | mine: 9 |
| head-guard-dropped: `if false && head != revision` | validator.rs:384 | `pr_open_refuses_a_head_that_is_not_the_implementation_revision` | — |
| pending-pass-adapter: `else if pending { Checks::Pass }` | forge.rs:378 | `the_bot_route_adapter_reads_states_and_checks_from_gh` | — |
| pending-pass-ciread: `Checks::Pending => Some("pass")` | validator.rs:557 | `with_checks_pending_no_ci_result_is_written_and_ci_green_stays_unknown` | — |
| pushed-dropped | validator.rs:394 | the three `with_checks_*` cases | (also mine: 5, 6) |

## 4. Findings (commit 471662d, working tree with my test files)

| # | file:line | measured | reach | verdict | origin | severity |
|---|---|---|---|---|---|---|
| 1 | `crates/takt/src/validator.rs:391-394` | `push` is called before the `base` check at :404; case 1 red with calls `["push takt/adv-detached"]` | `takt run` on a detached HEAD with an `origin` (CI checkouts, `git checkout <sha>`): the bot route is the default, the case opens with `base: None` (`case.rs:177`), tests and gate run, the branch is pushed to the remote, then `pr.open` errors; case and branch left behind | CONFIRMED | introduced | warning |
| 2 | `crates/takt/src/case.rs:235` (with `:167-178`, `git.rs:52-59`) | discard of a case opened over `takt/adv-over-a` leaves the tree on `main`; the doc at :225-227 says "the branch the case left" | every finished run leaves the tree on `takt/<id>`; the next `takt run` whose frontier take is refused hits `discard` in `run.rs:202` from that state | CONFIRMED | introduced | warning |
| 3 | `crates/takt/src/validator.rs:395-409, :536-540` | a harness that writes `pull_request: 7` into `branch.yaml` gets `pull_request open` and `ci_result pass` at its revision; stop `ApprovalRequired (repository.merge)`; the forge was never asked to open | the case directory is exempt from change detection (`run.rs:388`), commit (`git.rs:78`) and discard (`git.rs:99`) at the base; the unit adds a file there whose number the real forge reads through `gh pr view/checks <n>` without checking the PR's head is the case's branch. Same class as the pre-existing ability to write `evidence/*.json`; new in that it steers the forge | CONFIRMED | introduced | warning |
| 4 | `crates/takt-cli/src/main.rs:206-212`; `forge.rs:392-407` | `forge: bot (owner/repo)` printed for `git@gitlab.example.com:owner/repo.git` (case 4; by reading, also `https://gitlab…` and `file:///srv/git/owner/repo.git` → `owner/repo`) | any `takt run` without `--forge` in a tree whose `origin` is not GitHub: the adapter pushes under a GitHub App and reads through `gh` (b10x-gates/gh help), so the run proceeds through tests and gate and then errors (or, for a reachable `file://` origin, pushes locally and POSTs `/repos/owner/repo/pulls` to GitHub) | CONFIRMED | introduced | warning |
| 5 | `crates/takt/src/validator.rs:395-409, :329-330, :338, :369` | five mutants above leave the unit's own suite green: no case covers a second revision, a non-open state, a refused scan, a numberless reply or `cancel` | the second revision is the normal path after any harness commit; the others are adapter error paths | CONFIRMED | introduced | note |
| 6 | `crates/takt/src/case.rs:164-178`, `git.rs:44-49` | a case opened while the tree is on `takt/a` forks `takt/b` from `takt/a`'s tip (checkout -b from HEAD) with base `main` lent from `a` (`branch.base == Some("main")`, case 2 setup) | the second `takt run` in a tree: `b`'s pull request against `main` carries `a`'s unmerged commits; the code comment calls this intended, the story's "one pull request per case … two heads for one revision line" ambiguity argues the other way | CONFIRMED | introduced | warning |
| 7 | `ess/domains/takt.yaml:307-309, :564-565` | "Two such steps today: tests.run … gate.run"; "`takt.tests-run`, and `takt.gate-run`" — now four validators; `Forge`, `Branch`, `pull_request`/`ci_result` producers undeclared; `AGENTS.md` says spec first | the unit's diff made the declaration stale; the file is the coordinator's | NEEDS-CHANGE | introduced | note |

## 5. Attacked and could not break

- Acceptance statement against the double: holds (unit's case 1; pending → no record, `ci.green` unknown, merge blocked).
- `subject_revision` binding: `pr.open` refuses HEAD ≠ revision (mutant caught); `ci.read` refuses `pushed ≠ revision` and no-PR (unit's cases; my case 5 at r2).
- `scan-text` scans `pull-request.request.json`: title, head, base and body as the kernel built them (`forge.rs:316-326`); the intent and the held claims are inside the body.
- `--policy` absent when `B10X_GATES_POLICY` unset (unit's case); `b10x-gates api/scan-text/bot` argv shapes match `b10x-gates --help`; `gh pr checks --json bucket` documents exactly the five buckets the adapter maps.
- `repository_of` on `git@host:o/r(.git)`, `ssh://`, `https://` with and without `.git`, trailing `/`, local paths → correct; host is simply never looked at (finding 4).
- Case id with git-refused characters (`a b`, `..`, leading `.`, trailing `/`): `git checkout -b` refuses before any case file is written (`case.rs:185` precedes `:195`); story ids are `:`→`-` sanitised (`frontier.rs:51`). `--case-id ''` and `foo/bar` likewise fine.
- Same `--case-id` twice: refused at `case.rs:120` before branch creation; branch-without-directory refused by git.
- `ci.read` without `pr.open` registered: no record (unit's case).
- `--forge bot` without origin, unknown `--forge`, `--forge none` with origin: refused/none as documented (unit's CLI cases).
- Protocol under `--forge none`: `pr.open` is offered to the harness, but a harness step writes no record; `pr.opened` cannot become true except through the pre-existing case-directory hole.
- Output order (6): `forge: bot (…)` is printed before the `case … on …` line (seen in case 4's stdout); no existing test asserts the `case` line is first.
- All-`skipping` checks → `Pass` while `[]` → `Pending`: consistent with GitHub treating skipped required checks as success; not raised.
- Unverified runtime assumptions (no network): whether `gh pr checks --json bucket` prints `[]` or exits with a message on a PR with no checks; whether a merged/closed PR's head follows a later push (`ci.read` trusts `pushed == revision` as proof the PR head is the revision, `validator.rs:536`).

## 6. Paths written outside the worktree

All under the unit's scratch `adversary-1/`: `forge-append.rs`, `forge_cli.rs`, `run-mutants.sh`, `red-*.log` (4), `alone-*.log` (5), `suite-takt.log`, `suite-takt-cli.log`, `suite-takt-no-fail-fast.log`, `suite-takt-cli-no-fail-fast.log`, `fmt-check.log`, `clippy.log`, `mutant-warmup.log`, `mutant-*.diff` and `mutant-*.log` (9 each), and `mutant/` (the source copy, 1.4M; its `target/`, 662M, removed). Nothing under `/tmp`; no planning-store write; no `aep plan artifact` command beyond `show`.

## 7. findings

```findings
- file: crates/takt/src/validator.rs
  line: 391
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "pr.open pushes the branch before checking the case has a base, so a case opened from a detached HEAD (a CI checkout) pushes to the remote and then errors; the base check at :404 must precede the push"
- file: crates/takt/src/case.rs
  line: 235
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "discard returns the tree to the lent base (main), not to the branch the case left (takt/<other>), contradicting its own doc at :225-227; reached by a refused frontier take on any run after the first"
- file: crates/takt/src/validator.rs
  line: 395
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "a pull request number the harness writes into branch.yaml (the case directory is exempt from change detection at run.rs:388) is what pr.open reads the state of and ci.read reads the checks of, so pr.opened and ci.green become true and the merge is requested with no pull request opened for the case's branch; the forge never verifies the number names the branch"
- file: crates/takt-cli/src/main.rs
  line: 206
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the bot route (a GitHub App and gh) is the default forge for any origin, including gitlab.example.com and file:// URLs, because repository_of (forge.rs:392) never looks at the host"
- file: crates/takt/src/validator.rs
  line: 395
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "five mutants leave the unit's suite green: always opening a new pull request, hard-coding the record result \"open\", ignoring a scan-text failure, defaulting a missing number, and dropping the cancel bucket; cases 5-9 in tests/forge.rs catch them"
- file: crates/takt/src/case.rs
  line: 164
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "a case opened while the tree is on another case's branch forks from that branch's tip with the other's base, so the second run's pull request against main carries the first case's unmerged commits; the design call (fork from base, or return the tree to base at run end) is the coordinator's"
- file: ess/domains/takt.yaml
  line: 307
  category: contract-drift
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "the domain declares two validators (tests.run, gate.run) and no Forge or Branch noun while the unit adds takt.pr-open, takt.ci-read, Forge and branch.yaml; AGENTS.md requires the declaration before code builds on it, and the file is the coordinator's"
```
