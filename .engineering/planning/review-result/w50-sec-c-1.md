---
format: aep.planning-md/3
id: review-result:w50-sec-c-1
kind: review-result
status: active
title: Wave 50 security review on story:an-unrelated-program-install-refuses-no-step
relations:
- reviews: story:an-unrelated-program-install-refuses-no-step
revision: 1
---
```
unit: c (story:an-unrelated-program-install-refuses-no-step), diff 8f28ab1d1..309189273; findings cover the tree at 9385bf5b1 plus my uncommitted test file
verdict: red
cases: executed 6→10, red 2
origin: introduced 0, pre-existing 2, undecided 0
wrote-outside-worktree: none
needs-coordinator: yes
```
**Diff:** `git diff --stat` is empty because both new files are untracked. `git status` lists `?? crates/takt/tests/w50_sec_unattributed.rs` (mine) and `?? crates/takt/tests/w50_adv_unattributed.rs` (the adversary's, not touched). No implementation path was changed.

**My cases**, run alone with `cargo test --locked -p b10x-takt --test w50_sec_unattributed`. Result: `test result: FAILED. 2 passed; 2 failed`.
- `an_unattributed_change_is_a_refusal_to_every_reader_and_repeats_no_content`, green. During a step, another writer adds a cargo config above the tree (with a marker in it) and installs a program. The run stops `StepRefused` and names both paths after "origin unknown". The config is put back and the program left. The marker is not in the reason, the console or `events.jsonl`. The `refusal` line holds the stop's reason, the last `stop` is `StepRefused`, and the scorecard row is `StepRefused` with refusals ≥1, ranked below accepted.
- `an_observation_whose_only_write_is_a_program_is_refused_as_of_unknown_origin`, green. `tests.run` installs a program and writes nothing in the tree. It is refused as `tests.run: what cargo…`, with no "wrote", and the program is named as not put back. This is the only case that checks the new `|| !outside_moved.is_empty()` condition: the unit's own third case also writes `stray.txt`.
- `a_program_rewritten_through_its_link_on_path_during_a_step_refuses_it`, red: `a step that rewrote the program …/w50-sec-…-through-link/tool/probe finds through …/own-home-…/bin-0/w50-sec-linked-… on PATH is refused naming it; the run stopped Ok(NoProgress)`
- `a_toolchain_install_into_the_rustup_home_during_a_step_does_not_reach_a_test`, red. The test starts itself again as a child process, with `RUSTUP_HOME` pointing at a copy under `target/tmp`. The child stopped: `StepRefused("implementation.edit: what cargo or rustup reads outside the tree, … origin unknown: the step or another process: …/rustup/toolchains/probe-toolchain/bin/cargo-clippy; not put back …")`

**Suite:** `cargo test --locked -p b10x-takt --test unattributed_program_write --test tests_have_a_home_of_their_own --test w50_sec_unattributed` (EXIT=101):
- unattributed_program_write: `test result: ok. 3 passed; 0 failed`
- tests_have_a_home_of_their_own: `test result: ok. 3 passed; 0 failed`
- w50_sec_unattributed: `test result: FAILED. 2 passed; 2 failed`

`cargo clippy --locked -p b10x-takt --test w50_sec_unattributed -- -D warnings` finished in 5.32s, EXIT=0. `rustfmt --check` on my file is clean. `du -sh target` is 2.6G.

**Findings** (both pre-existing; I read the base with `git show` but did not run there; the diff touches neither spot):
1. `crates/takt/src/run.rs:7610`, warning. The guard reads a link in a program directory as the link only (`Entry::Link`). So a step that rewrites the program behind the link changes a program on `PATH` without being refused. Here the run went on to `NoProgress`. Had the step also edited a file, `tests.run` would have run the rewritten program.
   - **What reaches it:** any linked program in a program directory (pipx, `~/.local/bin`, dotfile managers) together with a step or test command that can write outside the tree. That holds under `--confinement none`, as in the test. I did not check confined runs.
   - **Fix to name:** fingerprint a program link's target, or refuse a change behind it.
   - **Related hazard:** `support/own_home.rs` builds the private bin as links to the real programs. A test step that writes through an existing name therefore writes into the real home, and the guard does not see it.
2. `crates/takt/tests/support/own_state.rs:16` (and `run.rs:7334`), warning. Acceptance 1 is not met for `$RUSTUP_HOME/toolchains/*/bin`. own_state.rs excludes it on purpose (the confined toolchain bind), but the commit says "Acceptance 1 holds", which claims more than that.
   - **What reaches it:** toolchains in this machine's rustup home were installed on 2026-10-28, 10-30 and 11-01, which is the same kind of event as the wave 39 incident.
   - **Your call:** narrow acceptance 1, or give the tests a guard root of their own.

**Checked and cleared:**
- **Nothing refused before is now admitted.** The validator condition is equivalent to the old one, and the judgement-step condition is unchanged.
- **No reader treats the new wording as cleared.** The scorecard (`outcome_of`), the trace (`refusal` by kind), `Declined::last` and the prompt (`StepContext` holds no refusal) do not read the reason text, and no src code reads it. Case 1 confirms this by running the real code.
- **No secret enters the reason.** It names the same outside paths as before plus fixed wording. Config file contents never reach the text (case 1).
- **In-tree writes are still said to be the step's.** The outside paths exclude the tree.
- **Unchanged by this unit:** a resumed run re-reads its baseline (`run.rs:10204`), so a program left behind by a refused step becomes the next run's baseline.

**Scope:**

| path | status |
|---|---|
| `crates/takt/src/run.rs` | confirmed |
| `crates/takt/tests/approval_riders_adv2.rs` | confirmed (unchanged, includes own_home) |
| `crates/takt/tests/unattributed_program_write.rs` | added (by the unit) |
| `crates/takt/tests/w50_sec_unattributed.rs` | added (mine) |

**Written outside the worktree:** none. The logs are in `<tree>/target/scratch/sec-c-{first,suite,clippy}.log`, and the fixtures were under `<tree>/target/tmp` and removed by the cases. I took no worktree lease, because the invariants forbid worktree commands.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 7610, "category": "guard-bypass", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "a program in a program directory rewritten through its link is not seen by the step guard, so the step is not refused (test: crates/takt/tests/w50_sec_unattributed.rs::a_program_rewritten_through_its_link_on_path_during_a_step_refuses_it)"},
  {"file": "crates/takt/tests/support/own_state.rs", "line": 16, "category": "isolation", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "acceptance 1 unmet for $RUSTUP_HOME/toolchains/*/bin: another process's toolchain install still refuses a test's step, though the unit claims acceptance 1 holds (test: crates/takt/tests/w50_sec_unattributed.rs::a_toolchain_install_into_the_rustup_home_during_a_step_does_not_reach_a_test)"}
]
```
