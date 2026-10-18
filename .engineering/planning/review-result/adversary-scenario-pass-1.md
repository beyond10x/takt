---
format: aep.planning-md/3
id: review-result:adversary-scenario-pass-1
kind: review-result
status: active
title: Adversary, scenario-corpus, pass 1
relations:
- reviews: story:scenario-corpus
revision: 1
---
unit: story:scenario-corpus, branch impl/scenario-corpus at ba0a148 plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 289→293, red 4
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 paths
needs-coordinator: whether a scenario run must refuse a remote `trial/<n>` it has not fetched. The real bot route answers that only with the network (finding 2).

**1. Diff stat.** `git --no-pager diff --stat` is empty because the only change is a new, untracked file. `git status --short` shows:
```
?? crates/takt-cli/tests/scenario_run_adv1.rs
```
That is a test file. No implementation file was touched.

**2. Cases added** (`crates/takt-cli/tests/scenario_run_adv1.rs`). Each was written first and then run alone with `cargo test -p b10x-takt-cli --test scenario_run_adv1 -- --exact <name>`. All four are red now.

| Case | Asserts | Red output (run alone) |
|---|---|---|
| `a_scenario_declared_only_through_an_include_outside_the_repository_is_refused` | A `scenarios.conf` holding only `[include] path=<file outside repo>` gives exit 1, "declares no scenario 1", no branch and no push | `left: 3 right: 1`, stdout `scenario 1: story:ready-one proposed on trial/0 at 61b2e547, pushed through bot` |
| `a_trial_branch_the_remote_holds_but_the_checkout_never_fetched_is_not_reused` | The remote holds `trial/0` at the seed and the checkout has not fetched it. The run must not take `trial/0` (refusing is also acceptable). The double runs a real unforced `git push` into a bare repo | `the run took trial/0, which the remote held already: exit 3, stdout scenario 1: story:ready-one proposed on trial/0 at 917f9bc7, pushed through bot` |
| `a_case_branch_that_collides_is_refused_before_the_trial_branch_is_pushed` | An existing `takt/scenario-1-trial-0` branch is refused before the push, and the tree stays on main | `trial/0 was pushed before the case's branch collided: ["bot -- push origin trial/0"]; stderr takt: git checkout -q -b takt/scenario-1-trial-0 ... fatal: a branch named 'takt/scenario-1-trial-0' already exists` |
| `a_trial_number_at_the_top_of_u64_is_refused_not_a_panic` | `refs/remotes/origin/trial/18446744073709551615` gives a refusal in one line, not a panic | `exit 101 ... panicked at crates/takt-cli/src/main.rs:652:30: attempt to add with overflow` |

**3. Suite run, after the cases above.** Command: `cargo test -p b10x-takt -p b10x-takt-cli --locked --no-fail-fast`, exit 101. The 32 `test result` lines add up to 289 passed and 4 failed; the 4 are exactly the cases above. The last lines read `error: 1 target failed: -p b10x-takt-cli --test scenario_run_adv1`. The 289 is the "before" count from the same run with my file excluded. The full log is in scratch (part 6).

**4. Findings** (all against ba0a148)

| # | file:line | Verdict / origin | Measured | What reaches it | Fix (named, not applied) |
|---|---|---|---|---|---|
| 1 | crates/takt/src/git.rs:201 | NEEDS-CHANGE / introduced | `git config --blob … --list` follows `include.path` and `includeIf` (probed on git 2.55). The doc comment at :188-192 says "no include is followed". A declaration can come from any file on the operator's disk, or from a FIFO that blocks | Whoever can commit `.engineering/scenarios.conf` to main of the scratch repo, which includes a merged case branch | Add `--no-includes` |
| 2 | crates/takt-cli/src/main.rs:754 (with :638) | NEEDS-CHANGE / introduced | An unforced push of a `trial/<n>` the remote already holds at the seed succeeds as up to date. The run prints "pushed" and opens its case and PR on another run's base, which breaks the Acceptance ("its own trial/<n>"). Scenario runs stop at approval, so the remote `trial/<n>` stays at the seed | A second checkout or clone of the scratch repo that has not fetched since another checkout ran a scenario. Two runs in one checkout do not reach it (the local branch exists) | Push create-only, e.g. `--force-with-lease=refs/heads/trial/<n>:` if the bot route admits it, or `ls-remote` first. The operator has to check this live |
| 3 | crates/takt-cli/src/main.rs:703 | INFEASIBLE / introduced | The collision check covers only the case directory. When the branch `takt/scenario-<id>-trial-<n>` already exists, the failure comes after the push, leaving `trial/n` pushed and the tree checked out on it | I built the state: the branch is there while no local or remote-tracking `trial/n` exists. Nothing found that reaches it | Also check `refs/heads/takt/<case_id>` before cutting |
| 4 | crates/takt-cli/src/main.rs:652 | INFEASIBLE / introduced | `n + 1` is unchecked: a debug build panics, and a release build wraps to `trial/0` and refuses on every run after that | I built the state. `refs/remotes/` is outside the step ref guard (git.rs:1020-1027 guards only heads and tags), but I did not show a step writing there | `checked_add` with a refusal |

**5. Attacked, not broken**
- Seed injection: `--since=`, `--all`, `-q` and `--git-dir` all fail `rev-parse --verify …^{commit}` (exit 1).
- Story id: it must equal an id from `aep list` exactly.
- Scenario id: limited to `[A-Za-z0-9_-]`. A leading dash is harmless because the id is always prefixed.
- Dirty tree: it is refused before any change, as the implementor's test also shows.
- A refused push or a story not `proposed`: the branch is removed and the tree restored.
- Drift from `take_next`: the case uses the same `take_subject`, CaseDir::open applies the same clean-tree rule, and the base and base commit are `trial/n` at the seed.
- Wave 06 guards: the branch is cut and pushed before the run, so `Run` keeps the same checkpoint guards.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/scenario-corpus/adv1/probe/` (the git repo for the include and rev-parse probes)
- `<wave-scratch>/scratch/scenario-corpus/adv1/suite.log`

The test scratch went under the worktree's `target/tmp` and was removed when the tests dropped it. The session lease `w7-scenario-corpus-adv1` was taken and has been released.

**7. Findings block**
```findings
[
  {"file": "crates/takt/src/git.rs", "line": 201, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "git config --blob follows include.path/includeIf, so a scenario can be declared by a file outside the repository, contrary to config_at's 'no include is followed'"},
  {"file": "crates/takt-cli/src/main.rs", "line": 754, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "an unforced push of a trial/<n> the remote already holds at the seed succeeds as up to date, so a checkout that has not fetched reuses another run's base"},
  {"file": "crates/takt-cli/src/main.rs", "line": 703, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "an existing takt/scenario-<id>-trial-<n> branch is detected only after trial/<n> was pushed, leaving the tree on the pushed trial branch"},
  {"file": "crates/takt-cli/src/main.rs", "line": 652, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "next_trial adds 1 unchecked, so a trial/18446744073709551615 ref panics a debug build and wraps to trial/0 in release"}
]
```
