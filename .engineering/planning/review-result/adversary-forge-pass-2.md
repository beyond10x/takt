---
format: aep.planning-md/3
id: review-result:adversary-forge-pass-2
kind: review-result
status: active
title: Adversary, forge-pull-request-evidence, pass 2
relations:
- reviews: story:forge-pull-request-evidence
revision: 1
---
```
unit: story:forge-pull-request-evidence, commits 471662d + ab60835 on impl/forge-pull-request-evidence (base 97eae59), worktree takt-w4-forge plus my two test files
verdict: CONFIRMED
cases: executed 108→120, red 8
origin: introduced 7 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory (the unit's scratch adversary-2/, 1.8M; the mutant copy's target/, 865M, removed)
needs-coordinator: whether the push target read from harness-writable branch.yaml (finding 1) is this unit's or the confinement story's; finding 5 needs one real `gh pr checks --json bucket` on a head with no check run, which I could not make offline
```

## 1. Diff

```
 crates/takt-cli/tests/forge_cli.rs |  36 ++++
 crates/takt/tests/forge.rs         | 431 +++++++++++++++++++++++++++++++++++++
 2 files changed, 467 insertions(+)
```
Both test files; `src/` untouched. `cargo fmt -p b10x-takt -p b10x-takt-cli --check` exit 0; `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets -- -D warnings` exit 0. `df -h /`: 150G free before the first build.

## 2. Cases added (written before anything ran; each run alone first)

`crates/takt/tests/forge.rs` (appended after line 1276):

| # | case (line) | asserts | now |
|---|---|---|---|
| 1 | `adv2_pr_open_pushes_the_case_branch_not_the_name_the_harness_wrote_into_branch_yaml` (:1333) | loop-level: a harness rewrites `branch.yaml`'s `name` to `HEAD:main` in its `implementation.edit` step; every forge push must name `takt/adv2-repoint` | **red** |
| 2 | `adv2_a_case_opened_over_a_case_branch_without_a_base_does_not_fork_from_its_commits` (:1433) | tree on `takt/a` opened from a detached HEAD (base None) with a commit; opening `b` refuses or does not put HEAD at `a`'s tip | **red** |
| 3 | `adv2_a_case_opened_over_a_leftover_case_branch_does_not_fork_from_its_commits` (:1455) | same with `a`'s case directory gone (`git clean -fdx` state) | **red** |
| 4 | `adv2_a_credential_in_the_origin_url_is_not_printed_by_the_bot_routes_refusal` (:1472) | `BotRoute::for_tree` refusal text on `https://ci-user:t0k3n-not-real@gitlab.example.com/owner/repo.git` and `…@github.com/repo` does not contain the credential | **red** |
| 5 | `adv2_an_origin_host_written_in_upper_case_is_github` (:1496) | `for_tree` on origin `https://GitHub.com/owner/repo.git` succeeds with `owner/repo` | **red** |
| 6 | `adv2_a_chain_of_cases_each_forks_from_the_shared_base_and_discards_to_the_branch_it_left` (:1518) | a→b→c: c has base `main`, left `takt/adv2-chain-b`, HEAD at main's tip; discard returns to `takt/adv2-chain-b` at b's tip; a, b untouched | green (mutant-catcher) |
| 7 | `adv2_a_case_opened_over_another_cases_branch_forks_from_the_bases_tip_as_it_is_now` (:1576) | main moved while the tree stayed on `takt/a`; b forks from main's new tip, implementation = new tip, tree is main's | green |
| 8 | `adv2_a_case_opened_from_a_detached_head_discards_back_to_that_detached_head` (:1621) | left None; discard → detached at the same commit, branch and cases dir gone | green (mutant-catcher) |
| 9 | `adv2_a_dirty_tree_over_another_cases_branch_is_refused_before_the_case_is_written` (:1642) | dirt git would overwrite → open errors naming the file; no dir, no branch, tree still on `takt/a` with its dirt | green |
| 10 | `adv2_no_checks_reported_yet_is_pending_not_an_error` (:1700) | runner answering exit 1, empty stdout, stderr `no checks reported on the 'takt/c1' branch` → `checks()` is `Pending` | **red** (INFEASIBLE, see finding 5) |

`crates/takt-cli/tests/forge_cli.rs` (appended after line 118):

| # | case (line) | asserts | now |
|---|---|---|---|
| 11 | `adv2_the_forge_bot_refusal_does_not_print_the_origin_credential` (:126) | binary, `--forge bot`, credentialed non-GitHub origin: exit ≠ 0 and neither stdout nor stderr contains the credential | **red** |
| 12 | `adv2_an_origin_host_written_in_upper_case_gets_the_bot_route_by_default` (:147) | binary, no `--forge`, origin `https://GitHub.com/owner/repo.git`: stdout has `forge: bot (owner/repo)` | **red** |

Red output, verbatim, each alone (`cargo test -p b10x-takt --test forge <name> -- --exact`):

Case 1:
```
thread 'adv2_pr_open_pushes_the_case_branch_not_the_name_the_harness_wrote_into_branch_yaml' (1024284) panicked at crates/takt/tests/forge.rs:1367:5:
the forge was asked to push what the harness wrote into branch.yaml: ["push HEAD:main", "open HEAD:main #1", "view #1", "checks #1"]; stopped ApprovalRequired (repository.merge)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 3.98s
```
Case 2 (captured before a setup fix to the helper shifted it to :1418; the assertion is unchanged):
```
thread 'adv2_a_case_opened_over_a_case_branch_without_a_base_does_not_fork_from_its_commits' (1024655) panicked at crates/takt/tests/forge.rs:1416:13:
assertion `left != right` failed: no base recorded: takt/adv2-nobase-b forks from takt/adv2-nobase-a's tip, the other case's commits; branch.yaml: Ok(Branch { name: "takt/adv2-nobase-b", base: None, left: Some("takt/adv2-nobase-a"), pushed: None, pull_request: None })
  left: "44c9e42f823debc62b696e6b189a72b07e8f073f"
 right: "44c9e42f823debc62b696e6b189a72b07e8f073f"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.02s
```
Case 3 (first run failed on my own setup line — `remove_dir_all(case.dir)` left `.engineering/cases/` standing, a typo not a finding; fixed to remove the cases directory as `git clean -fdx` does, re-run alone):
```
thread 'adv2_a_case_opened_over_a_leftover_case_branch_does_not_fork_from_its_commits' (1028438) panicked at crates/takt/tests/forge.rs:1418:13:
assertion `left != right` failed: case directory gone: takt/adv2-leftover-b forks from takt/adv2-leftover-a's tip, the other case's commits; branch.yaml: Ok(Branch { name: "takt/adv2-leftover-b", base: None, left: Some("takt/adv2-leftover-a"), pushed: None, pull_request: None })
  left: "91bd78bea8fdd5c37207afb5195861b2c2073c93"
 right: "91bd78bea8fdd5c37207afb5195861b2c2073c93"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.02s
```
Case 4:
```
thread 'adv2_a_credential_in_the_origin_url_is_not_printed_by_the_bot_routes_refusal' (1024713) panicked at crates/takt/tests/forge.rs:1484:9:
elsewhere: the refusal prints the credential: the origin remote https://ci-user:t0k3n-not-real@gitlab.example.com/owner/repo.git is on gitlab.example.com, not github.com: the bot route pushes under a GitHub App and reads through gh; give --forge none
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.01s
```
Case 5:
```
thread 'adv2_an_origin_host_written_in_upper_case_is_github' (1024733) panicked at crates/takt/tests/forge.rs:1506:9:
GitHub.com is github.com, a host name being case-insensitive: the origin remote https://GitHub.com/owner/repo.git is on GitHub.com, not github.com: the bot route pushes under a GitHub App and reads through gh; give --forge none
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.01s
```
Case 10:
```
thread 'adv2_no_checks_reported_yet_is_pending_not_an_error' (1024753) panicked at crates/takt/tests/forge.rs:1702:9:
no checks reported yet is pending, not an error: gh pr checks 7 --json bucket exited 1 and printed no reading (EOF while parsing a value at line 1 column 0): no checks reported on the 'takt/c1' branch
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.01s
```
Case 11 (`cargo test -p b10x-takt-cli --test forge_cli … -- --exact`):
```
thread 'adv2_the_forge_bot_refusal_does_not_print_the_origin_credential' (1026030) panicked at crates/takt-cli/tests/forge_cli.rs:138:5:
the credential is printed: exit 1 stdout  stderr takt: the origin remote https://ci-user:t0k3n-not-real@gitlab.example.com/owner/repo.git is on gitlab.example.com, not github.com: the bot route pushes under a GitHub App and reads through gh; give --forge none
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.01s
```
Case 12:
```
thread 'adv2_an_origin_host_written_in_upper_case_gets_the_bot_route_by_default' (1026056) panicked at crates/takt-cli/tests/forge_cli.rs:150:5:
GitHub.com is github.com: exit 3 stdout forge: none (origin GitHub.com is not github.com)
case upper on story.delivery/1 in .../target/tmp/forge-cli-1026055-upper/repo; inputs none
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.02s
```

## 3. Suite (after part 2 existed)

`cargo test -p b10x-takt --no-fail-fast` → exit 101:
```
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s      (unittests src/lib.rs)
test result: FAILED. 24 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.04s   (tests/forge.rs)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s      (tests/frontier.rs)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.78s      (tests/gate_validator.rs)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s       (tests/loop.rs)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s      (tests/story_delivery.rs)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s       (Doc-tests)
```
`cargo test -p b10x-takt-cli --no-fail-fast` → exit 101:
```
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.07s      (unittests src/main.rs)
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s   (tests/forge_cli.rs)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s       (tests/frontier_cli.rs)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s       (tests/gate_cli.rs)
```
Executed: before 108 (implementor's `c1-gate-1-test-takt.log` 80 + `c1-gate-2-test-cli.log` 28), after 120 (90 + 30). Red 8, all mine; every pass-1 and unit case green.

## Pass-1 findings re-tested

| pass-1 # | status |
|---|---|
| 1 push before base check | closed: `adv_pr_open_on_a_detached_head_pushes_nothing_before_refusing` green |
| 2 discard to lent base | closed: `adv_a_case_discarded_over_another_cases_branch_…` green |
| 3 planted number trusted | closed on the read side (`adv_a_pull_request_number_planted_…` green); the write side (push target) is open, finding 1 below |
| 4 bot default on any host | closed (`a_non_github_origin_does_not_make_the_bot_route_the_default` green); residue: host case, credential in the refusal (findings 3, 4) |
| 5 mutant catchers | kept, green |
| 6 fork from another case | closed when the other case records a base (unit's `fork` case, my cases 6, 7); open when it does not (finding 2) |
| 7 domain document | answered in the integration tree's `ess/domains/takt.yaml:841-848, :885-890` (not on this branch); it now states "never from another case's commits" and "pushes the case's branch", which findings 1 and 2 contradict |

## Mutants (copy under scratch; forge binary = unit 10 + pass-1 9 + mine 10; my six always-red cases omitted)

| mutant | file:line (worktree) | existing cases red | caught only by mine |
|---|---|---|---|
| base-vs-left: lend `left` instead of `base` | case.rs:184 | none | case 6 |
| leave-detached-dropped: `None` arm of `leave_branch` does nothing | git.rs:54 | none | case 8 |
| ci-read-pushed-guard-dropped: `if false` at the `pushed != revision` guard | validator.rs:581 | none | none: equivalent, the forge head check at :593 decides; the guard only saves a forge call |
| pr-open-head-and-to-or | validator.rs:458 | `a_pull_request_head_the_forge_reports_elsewhere_earns_no_record` | — |
| pr-open-head-check-dropped (`if true`) | validator.rs:458 | elsewhere, `adv_a_pull_request_number_planted_…` | — |
| ci-read-head-check-dropped (`if false`) | validator.rs:593 | elsewhere, `adv_…planted_…` | — |
| head-reread-dropped: implementation = HEAD before the fork | case.rs:200 | `a_case_opened_over_another_cases_branch_forks_from_the_base_…` | (also 6, 7) |
| fork-from-head-base-lent: `(base, None::<String>)` | case.rs:185 | the same | (also 6, 7, 9) |
| for-tree-host-check-dropped | forge.rs:236 | cli `forge_bot_on_a_non_github_origin_is_refused_before_the_run` | — |
| main-host-check-dropped | main.rs:224 | cli `with_a_non_github_origin_the_default_forge_is_none_and_the_run_says_so` | — |

## 4. Findings (commits 471662d + ab60835, working tree with my test files)

| # | file:line | measured | reach | verdict | origin | severity |
|---|---|---|---|---|---|---|
| 1 | `crates/takt/src/validator.rs:450` (also :409) | `pr.open` pushes and opens for `branch.name` read from `branch.yaml`, never checked against `Branch::name_of(id)` or the branch `current()` resolved the case from; case 1: forge asked `push HEAD:main`, `open HEAD:main #1`, run stops `ApprovalRequired` | a harness step writes 9 characters into the untracked case directory (exempt from change detection `run.rs:386`, commit, discard; the reach the coordinator accepted for pass-1 finding 3); the bot credential then pushes `HEAD:main`, `:main` or `--all` as given. Same hole class as pass-1 finding 3, on the write side; the fix is local (push the computed name, or refuse `name ≠ current branch`) | CONFIRMED | introduced | warning |
| 2 | `crates/takt/src/case.rs:181-186` | with the tree on `takt/a` and `a`'s base unknown (base None, or directory gone), `from` is None and `b` forks from `a`'s tip (cases 2, 3: HEAD == a's tip, `base: None`); `open`'s doc at :116-118 and the domain document say "never from another case's commits" | run 1 from a detached HEAD (CI checkout) then run 2 in the same tree; or `git clean -fdx` on a tree a run left on `takt/a` (the case directory is untracked) then a run. The second case carries the first's commits and fails at `pr.open` ("no base") after tests, gate and harness steps | CONFIRMED | introduced | warning |
| 3 | `crates/takt/src/forge.rs:234, :238` | both `for_tree` refusals print `{url}`; cases 4, 11: `t0k3n-not-real` in the error and on the binary's stderr | `takt run --forge bot` in a checkout whose `origin` carries a token in the URL (what CI runners leave in `.git/config`) on a non-GitHub, upper-case or malformed host | CONFIRMED | introduced | warning |
| 4 | `crates/takt/src/forge.rs:236` (also `main.rs:224`) | `origin.host != "github.com"` byte-compared; `https://GitHub.com/owner/repo.git` → default `none` and `--forge bot` refused (cases 5, 12) | a hand-typed remote; host names are case-insensitive (RFC 3986 §3.2.2) | CONFIRMED | introduced | note |
| 5 | `crates/takt/src/forge.rs:411` (`read_json` :334) | a `gh pr checks` exit 1 with no JSON is an error, not `Pending` (case 10); `ci.read` runs in the step right after `pr.open`, before a check run exists | `gh` 2.100.0 carries the message `no checks reported on the '%s' branch` (`strings /usr/bin/gh`); whether it is emitted under `--json` instead of `[]` I could not verify offline, and the unit's double assumes `[]` | INFEASIBLE | introduced | warning |
| 6 | `crates/takt/src/case.rs:184` | mutant base-vs-left leaves the unit's and pass-1's suite green; only a chain of three cases tells `base` from `left` (case 6) | the third run in one tree | CONFIRMED | introduced | note |
| 7 | `crates/takt-cli/src/main.rs:242` | with a forge registered, a case whose `branch.yaml` has no base is known at open to be refused at `pr.open`; the run still spends tests, gate and harness steps first (pass-1 case 1 shows the refusal, nothing refuses earlier) | `takt run` on a detached HEAD with a github.com origin (bot default) | CONFIRMED | introduced | note |

## 5. Attacked and could not break

- Acceptance against `Memory`, pending → no record, one pull request per case, `scan-text` before the API, the five buckets, `--forge none`: all unit and pass-1 cases green.
- Chain a→b→c and a base that moved: fork from main's current tip, `base`/`left` recorded, discard returns to `left` (cases 6, 7 green). Detached-origin discard returns to the detached commit (case 8).
- Dirty tree over another case's branch: git refuses when the dirt would be overwritten, no directory or branch written (case 9). Dirt git can carry is carried and committed by the first harness commit: the base's `commit_all` did the same, pre-existing, not raised.
- `request_of` (base, `intent.md`, decisions) runs before the push (pass-1 case 1 green).
- Open path trusting the reply's number: GitHub's `POST /pulls` answers 422 for an existing head, never another number; a reply without `number` errors (pass-1 case 8). Nothing further to break offline.
- `merged`/`closed` with a stale head, `ci.read` after a force-push: the forge's head sha decides, no record (`elsewhere` case covers both states' path).
- Short or upper-case head sha: `gh --json headRefOid` and `git rev-parse` both give full lowercase; not a reachable mismatch, not tested.
- `repository_of`: `ssh://…:22/o/r.git`, `git@…:o/r`, trailing `/`, `github.example.com`, `file://`, local paths all as documented (unit's case).
- `left` deleted between open and discard: unreachable, `discard` follows `open` inside `open_next`; not tested.

## 6. Paths written outside the worktree

All under the unit's scratch `adversary-2/` (1.8M): `forge-append.rs`, `forge_cli-append.rs`, `run-mutants.sh`, `run-mutants-2.sh`, `build-forge.log`, `build-forge-cli.log`, `alone-adv2_*.log` (12), `suite-takt.log`, `suite-takt-cli.log`, `fmt-check.log`, `clippy.log`, `mutants-summary.log`, `mutants-summary-2.log`, `mutant-*.diff` and `mutant-*.log` (10 each), `pristine/` (5 source copies), `mutant/` (the source copy, 1.4M; its `target/`, 865M, removed). Nothing under `/tmp`; no planning-store write; no `aep plan artifact` verb beyond `show`; lease `takt-w4-forge-adv2` started and ended.

```findings
- file: crates/takt/src/validator.rs
  line: 450
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "pr.open pushes and opens the pull request for the name field of branch.yaml, a file the harness can write, instead of the case's branch it resolved the case from, so a harness that writes name: HEAD:main makes the bot credential push HEAD:main; the push target must be Branch::name_of(id) or the current branch"
- file: crates/takt/src/case.rs
  line: 181
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "a case opened over a case branch whose base is unknown (recorded None, or its directory gone as git clean -fdx leaves it) forks from that branch's tip with base None, carrying the other case's commits, while open's doc and the domain document say a case never forks from another case's commits; refuse at open naming the branch to check out"
- file: crates/takt/src/forge.rs
  line: 234
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "both for_tree refusals print the origin URL verbatim, so a credential in it (a CI checkout's origin) lands in the error and on the binary's stderr under --forge bot; print the host or a URL with the userinfo stripped"
- file: crates/takt/src/forge.rs
  line: 236
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the host is compared byte-wise with github.com here and at main.rs:224, so an origin written GitHub.com gets no forge by default and is refused under --forge bot; host names are case-insensitive"
- file: crates/takt/src/forge.rs
  line: 411
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "checks() turns a gh pr checks exit 1 with no JSON into an error that ends the run, and gh 2.100.0 carries the message \"no checks reported on the '%s' branch\" for a head with no check run yet, the state right after pr.open when ci.read first runs; whether gh prints that instead of [] under --json needs one real call"
- file: crates/takt/src/case.rs
  line: 184
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "lending the other case's left instead of its base leaves the unit's and pass-1's suite green; only a chain of three cases (adv2_a_chain_of_cases_…) distinguishes them"
- file: crates/takt-cli/src/main.rs
  line: 242
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "with a forge registered, a case opened with no base is known at open to be refused at pr.open, yet the run spends tests, gate and harness steps before the refusal; the binary could refuse at open when a forge is registered and branch.yaml has no base"
```
