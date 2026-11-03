---
format: aep.planning-md/3
id: review-result:w50-adv-c-1
kind: review-result
status: active
title: Wave 50 adversary pass 1 on story:an-unrelated-program-install-refuses-no-step
relations:
- reviews: story:an-unrelated-program-install-refuses-no-step
revision: 1
---
```
unit: c (story:an-unrelated-program-install-refuses-no-step), covering tree takt-w50-c at 9385bf5b1 plus my uncommitted test file
verdict: red
cases: executed 6→9, red 2
origin: introduced 1, pre-existing 2, undecided 2
wrote-outside-worktree: none
needs-coordinator: yes
```

**Diff.** `git status`: `?? crates/takt/tests/w50_adv_unattributed.rs` (mine, a test file). `w50_sec_unattributed.rs` belongs to the security reviewer and I did not touch it. No tracked file changed.

**Cases added** in `crates/takt/tests/w50_adv_unattributed.rs`:
- `no_rustup_toolchain_bin_the_guard_reads_is_shared_with_another_process` (:112) is red. In every test binary the guard reads `~/.rustup/toolchains/*/bin` (`program_dirs`, run.rs:7325-7336). `own_home.rs` swaps out PATH and CARGO_HOME (:173) but leaves RUSTUP_HOME and HOME alone. First run, verbatim: `...acceptance 1: the step guard of every test reads rustup toolchain program directories another process can write (rustup update, a toolchain install): ["<home>/.rustup/toolchains/1.91-x86_64-unknown-linux-gnu/bin", ... 13 dirs]`
- `a_toolchain_program_another_process_installs_reaches_no_step_of_a_test` (:130) is red. It re-runs the test binary under a private HOME and RUSTUP_HOME in `target/scratch`, with `own_home` active. Another thread installs a program into the toolchain `bin` while an idle step runs. First run, verbatim: `...acceptance 1: a program another process installed into the home's rustup toolchain bin refused a test's step: Ok(StepRefused("implementation.edit: what cargo or rustup reads outside the tree, ... origin unknown: the step or another process: <scratch>/home/.rustup/toolchains/probe/bin/takt-w50-probe; not put back (...)"))`
- **What reaches it:** cargo on this machine is the rustup proxy (`/usr/bin/cargo -> rustup`). A `rustup update` or toolchain install by another session during a test refuses that test's step. This is the same class as the wave-39 failure, by another route.
- **Fix, named only:** `own_home` also gives RUSTUP_HOME a private copy: links to each toolchain, with each `bin` copied, as it already does for the cargo home.

**Suite run after the cases existed:**
- `cargo test --locked -p b10x-takt --test unattributed_program_write --test tests_have_a_home_of_their_own --test w50_adv_unattributed`: exit 101. `ok. 3 passed` (home), `ok. 3 passed` (unattributed), `FAILED. 1 passed; 2 failed` (mine).
- `cargo clippy --locked -p b10x-takt --lib --test w50_adv_unattributed -- -D warnings`: exit 0.
- `rustfmt --edition 2024 --check` on my file: exit 0 after formatting it.
- `du -sh target`: 2.6G.

**Judgement findings** (on 9385bf5b1):
- `tests_have_a_home_of_their_own.rs:37`: the test name says no program directory the guard reads is shared, but it checks only PATH and CARGO_HOME. It passed in the suite run while the guard reads 13 shared rustup bins. The file is the same at base (empty diff).
- `ess/domains/takt.yaml:5139-5144` still says "the run stops naming the changed program". The decided "origin unknown" wording is recorded only in the commit message, not in `ess/`.
- `run.rs:2962`: the Python user site under the real HOME (`~/.local/lib/python3.x/site-packages`) is read as an import directory, and `own_home` does not cover HOME. I found no test that reaches it: the Python tests I checked give their child a HOME of its own.
- `website/docs/guides/run-in-a-container.md:138-139` shows the old wording for `/var/lib/takt/.cargo`. It is marked as a 0.14.0 recording. Out of scope.

**Attacked, not broken:**
- Tree paths named as of unknown origin: none. `writable_outside` and `under` leave the tree out.
- A step that writes in the tree and outside: each half is named under the right heading.
- The judgement and validator refusals use the same wording.
- Removed programs and symlinks: an `Entry` is `None` or `Link`, and both are named the same way.
- Text consumers: every caller that matches "wrote what its validator does not" still matches.
- The "or outside_moved" condition change: `step_guard_sees_test_script_imports` bin variant catches the mutant (`records.is_empty()`).

**Scope:**
- `crates/takt/src/run.rs`: confirmed.
- `crates/takt/tests/approval_riders_adv2.rs`: confirmed (includes `own_home`).
- `crates/takt/tests/support/own_home.rs`: added (it is where acceptance 1 lives).

**Left for the coordinator:**
- The acceptance 2 predicate writes into the real `~/.cargo/bin`, so I did not run it, and the commit reports no run of it.
- My cases' origin is undecided: I did not run them at base. The unit diff touches neither `own_home.rs` nor `program_dirs`.

```findings
[
{"file": "crates/takt/tests/support/own_home.rs", "line": 173, "category": "isolation", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "acceptance 1 fails: own_home leaves RUSTUP_HOME/HOME real, so a program installed into ~/.rustup/toolchains/*/bin by another process refuses a test's step (test: crates/takt/tests/w50_adv_unattributed.rs::a_toolchain_program_another_process_installs_reaches_no_step_of_a_test)"},
{"file": "crates/takt/tests/w50_adv_unattributed.rs", "line": 112, "category": "isolation", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "every test binary's step guard reads 13 writable rustup toolchain bins outside its private root (test: crates/takt/tests/w50_adv_unattributed.rs::no_rustup_toolchain_bin_the_guard_reads_is_shared_with_another_process)"},
{"file": "crates/takt/tests/tests_have_a_home_of_their_own.rs", "line": 37, "category": "test-quality", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "test claims no guarded program directory is shared but checks only PATH and CARGO_HOME; green while rustup bins are shared"},
{"file": "ess/domains/takt.yaml", "line": 5143, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "spec still says the run stops naming the changed program; the decided origin-unknown naming is not recorded in ess"},
{"file": "crates/takt/src/run.rs", "line": 2962, "category": "isolation", "severity": "note", "verdict": "INFEASIBLE", "origin": "undecided", "message": "Python user site under the real HOME is a guarded import dir own_home does not isolate; no test found that reaches it"},
{"file": "website/docs/guides/run-in-a-container.md", "line": 138, "category": "docs", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "0.14.0 recording shows the old 'wrote what its validator does not: /var/lib/takt/.cargo' wording"}
]
```
