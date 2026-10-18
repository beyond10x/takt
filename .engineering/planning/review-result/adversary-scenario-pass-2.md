---
format: aep.planning-md/3
id: review-result:adversary-scenario-pass-2
kind: review-result
status: active
title: Adversary, scenario-corpus, pass 2
relations:
- reviews: story:scenario-corpus
revision: 1
---
The correction does not hold against two runs at once. If another checkout pushes `trial/<n>` between this run's fetch and its push, this run's unforced push moves that branch forward. takt then refuses the run, and its error says nothing was created or pushed.

```
unit: story:scenario-corpus, branch impl/scenario-corpus at 82e8d74 (ba0a148 + correction 1) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 295→299, red 3
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 6 paths
needs-coordinator: whether the live bot route passes `--force-with-lease=refs/heads/trial/<n>:` through to git push (the fix for finding 1); only the network can answer that
```

**1. Diff stat.** `git --no-pager diff --stat` is empty because the only change is untracked. `git status --short` shows `?? crates/takt-cli/tests/scenario_run_adv2.rs`, which is a test file. I touched no implementation file.

**2. Cases added** (`<trees>/w7-scenario-corpus/crates/takt-cli/tests/scenario_run_adv2.rs`). Each was run alone with `cargo test -p b10x-takt-cli --test scenario_run_adv2 --locked -- --exact <name>`. The bot-route double runs a real `git fetch` and a real, unforced `git push` into a bare repo, and passes every option takt gives straight through.

| Case | Asserts | Now | Red output when run alone |
|---|---|---|---|
| `a_trial_branch_another_run_creates_after_the_fetch_is_not_moved_by_this_runs_push` (:268) | Another run pushes `trial/0` at its seed between this run's fetch and push. The remote's `trial/0` must stay at that seed. | red | `the other run's trial/0 was moved from its seed to this run's seed 9081eca…: exit 1, stderr … did not report trial/0 created, so it was not created: the remote holds it already ("up to date") …; it reported: To …/remote.git \| refs/heads/trial/0:refs/heads/trial/0	a8fcce9..9081eca \| Done`, `left: "9081eca…" right: "a8fcce9…"` |
| `a_collision_the_fetch_would_number_past_does_not_refuse_the_run` (:292) | The remote holds `trial/0`. The checkout has the case branch `takt/scenario-1-trial-0` but no `trial/0`. The run must take `trial/1`. | red | `exit 1 … takt: scenario 1: the case's branch takt/scenario-1-trial-0 exists already, though no trial/0 branch is known …; calls []` |
| `a_proposed_story_the_frontier_calls_blocked_is_refused_before_the_push` (:326) | A scenario on `story:blocked-one` (status `proposed`, but an open decision blocks it) is refused before the push. | red | `a blocked story was pushed for and taken: exit 3, the remote holds [refs/heads/trial/0], the tree is on takt/scenario-3-trial-0; stdout … frontier: took story:blocked-one` |
| `a_new_tag_in_the_push_report_does_not_count_as_the_trial_branch_created` (:356) | With `push.followTags` set, the push reports `*` for a new tag and `=` for `trial/0`. The run must be refused. | green | It kills a mutant the implementor's suite misses: changing `refs.ends_with(&target)` to `true` at main.rs:545 leaves all 9 of `scenario_run.rs` green, and this case goes red (`left: 3 right: 1`). I built the mutant in a scratch copy, not in the worktree. |

Two of my cases changed before they counted as findings:
- Case 3 was first written with a `pre-commit` hook to make the take's commit fail after the push. It failed for the wrong reason: takt runs git with `core.hooksPath=/dev/null` (crates/takt/src/git.rs:30), so the hook never ran. Its rewrite also first assumed `aep` refuses a blocked take. It does not, so the doc and assertion now state what was measured.
- Case 4 was first green for the wrong reason: the setup push had already sent the tag to the remote. I reordered the setup and re-ran it against the mutant.

**3. Suite run, after the cases above.** Command: `cargo test -p b10x-takt -p b10x-takt-cli --locked --no-fail-fast`, exit 101. The 33 `test result` lines add up to 296 passed and 3 failed; the 3 are cases 1–3 above. The run ends with `error: test failed, to rerun pass -p b10x-takt-cli --test scenario_run_adv2`. The "before" count of 295 is the same run with my file excluded (299 − 4). All four of pass 1's cases (`scenario_run_adv1.rs`) now pass, so correction 1 held for them. `cargo clippy -p b10x-takt-cli --all-targets --locked -- -D warnings` is clean and `rustfmt --check` on the new file is clean. Log: `<wave-scratch>/scratch/scenario-corpus/adv2/suite.log`.

**4. Findings** (against 82e8d74)

| # | file:line | Verdict / origin | Measured | What reaches it | Fix (named, not applied) |
|---|---|---|---|---|---|
| 1 | crates/takt-cli/src/main.rs:502 (push_argv), :545 | NEEDS-CHANGE / introduced | The push is unforced and only *read* afterwards. If the remote's `trial/<n>` sits at an ancestor of this run's seed, git fast-forwards it with flag ` ` and exit 0 (probed on git 2.55). takt then refuses, but another run's base has already moved under its open PR. The refusal also says "so it was not created … up to date … nothing else was pushed", which is false. | Two checkouts running scenarios at once on the scratch repo, where one seed is a descendant of the other. The window runs from the fetch through `aep list`, `aep show` and `validators_of` to the push. That is a timing race; I simulated it in the double. | Push with `--force-with-lease=refs/heads/trial/<n>:` (empty expected value means the ref must not exist) and keep the `*` check. Probed: an ancestor gives `!	[rejected] (stale info)` with exit 1 and the remote unchanged. The same commit still gives `=` with exit 0, which the `*` check already refuses. The double in `scenario_run.rs:173` drops every option except `--porcelain`, so it must pass options through before a lease can be tested there. |
| 2 | crates/takt-cli/src/main.rs:829 | INFEASIBLE / introduced | The check before the fetch adds no safety, because the check after the fetch covers it. It can refuse a run the fetch would have numbered past. | I built the state: a leftover case branch, the local `trial/0` deleted by hand, no remote-tracking ref. I found nothing that reaches it without a hand action. | Drop the check before the fetch when there is a trial route. |
| 3 | crates/takt-cli/src/main.rs:862 | INFEASIBLE / introduced | The check before the push compares status to `proposed` only and skips `frontier::is_ready` (blocked_by, depends_on), and `aep` moves a blocked story to `active` without refusing. A scenario on a blocked story or one with an open dependency gets pushed, taken and run, which `takt run` never does. | A `scenarios.conf` naming such a story. None was found. The live acceptance uses `story:remove-a-todo`, whose readiness I did not check. | Refuse unless `takt::frontier::is_ready(listed, &all)`, before the push. |

**5. Attacked, not broken**
- Credentials and limits: `TrialRoute` uses `Process::default()`, which goes through the same `without_credentials(bot_route_reads)` and `run_child` limits as `BotRoute`.
- Scan-text: no bypass. `BotRoute::push` scans nothing either; the scan only covers pull-request text.
- Forced fetch: it writes only `refs/remotes/origin/trial/*`, which plain `git fetch` would also force. The remote is always `origin`, the same as the push and the PR repository.
- Porcelain parse: the `:refs/heads/trial/<n>` suffix with its colon cannot match `trial/10`. A followed tag's `*` line is refused (case 4). A token in the `To` line is unlikely to reach takt's output: git anonymizes push status URLs, by my recollection, not measured.
- Number change between the two checks: the fetch can only add refs, unless the operator's config prunes on fetch, and the second check then covers it.
- Push that creates the ref but exits non-zero: the local branch is removed, the tree goes back, and the next run's fetch numbers past the orphan. This recovers on its own.
- Take or its commit failing after the push: hooks are disabled, so I found no reachable failure there.
- Side effect, not counted: the operator's `push.followTags` sends tags to the scratch remote along with the trial push. The case-branch pushes already did this.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/scenario-corpus/adv2/probe.sh` and `<wave-scratch>/scratch/scenario-corpus/adv2/probe/` (the git push probe)
- `<wave-scratch>/scratch/scenario-corpus/adv2/replace_case.py`
- `<wave-scratch>/scratch/scenario-corpus/adv2/reorder.py`
- `<wave-scratch>/scratch/scenario-corpus/adv2/suite.log`
- `<wave-scratch>/scratch/scenario-corpus/adv2/mutant/` (705M: a copy of the tree with the main.rs:545 mutant and its own `target/`; I left it in place rather than delete a `target/` by hand)

The test scratch went under the worktree's `target/tmp` and was removed when the tests dropped it. I took the session lease `w7-scenario-corpus-adv2` and have released it.

**7. Findings block**
```findings
[
  {"file": "crates/takt-cli/src/main.rs", "line": 502, "category": "concurrency", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the trial push is unforced and only read afterwards, so a trial/<n> another run created after this run's fetch, at an ancestor of this seed, is fast-forwarded on the remote before takt refuses and reports that nothing was created"},
  {"file": "crates/takt-cli/src/main.rs", "line": 829, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the trial_free check before the fetch refuses a run on a collision the fetched number would have passed, and adds no safety the check after the fetch lacks"},
  {"file": "crates/takt-cli/src/main.rs", "line": 862, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the pre-push check reads status proposed only and skips frontier::is_ready, so a scenario on a blocked story is pushed, taken and run, which takt run never does"}
]
```
