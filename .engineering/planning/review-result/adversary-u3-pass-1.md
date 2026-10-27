---
format: aep.planning-md/3
id: review-result:adversary-u3-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on U3: fresh HOME and the shared temporary directory'
relations:
- reviews: story:fresh-home-first-test-run-is-not-refused
- reviews: story:staging-tests-pass-under-a-shared-temp-directory
revision: 1
---
# Adversary pass 1 on U3 (fresh HOME, shared temporary directory)

Tree: `wave/38` on b95cdaaa plus the uncommitted unit edits. Test file added:
`crates/takt/tests/fresh_home_outside_adv1.rs`: 1 passed, 4 failed. `step_guard_sees_test_script_imports`
36 passed; `harness::endpoint_tests` 32 passed with `TMPDIR` a 2777 directory.

| id | file:line | verdict | origin | measured |
|---|---|---|---|---|
| F1 | crates/takt/src/run.rs:7091 | warning | introduced | in a fresh HOME a `tests.run` that plants a crate source under `~/.cargo/registry/src` is accepted; the base refused it |
| F2 | crates/takt/src/run.rs:6845 | warning | pre-existing | the same plant in an existing `~/.cargo` is accepted at the base and in the tree |
| F3 | crates/takt/src/run.rs:7134 | warning | pre-existing | refusing a step that made `~/.cargo` and planted a program removes all of `~/.cargo` |
| F4 | crates/takt/src/run.rs:6816 | warning | pre-existing | a rustup toolchain linked into the tree plus `settings.toml` is accepted |

Routing (coordinator): F1 is parity with F2, accepted for this unit; F1 and F2 filed as
story:cargo-home-sources-are-step-changes, F3 as story:a-refused-step-leaves-the-cargo-home-it-made,
F4 as story:a-toolchain-linked-into-the-tree-is-a-step-change; the four cases re-pinned to the current
behaviour, each naming its story.

```findings
- file: crates/takt/src/run.rs
  line: 7091
  category: property
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: in a fresh HOME a tests.run that plants a crate source marked unpacked under ~/.cargo/registry/src is accepted with a pass, where the base refused it
- file: crates/takt/src/run.rs
  line: 6845
  category: property
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: Outside never watches the sources cargo compiles from CARGO_HOME (registry/src, git/checkouts), so a plant there is accepted in an existing ~/.cargo
- file: crates/takt/src/run.rs
  line: 7134
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: refusing a step that made ~/.cargo and planted ~/.cargo/bin/cargo-forge runs remove_dir_all on all of ~/.cargo, deleting the program and the cache that takt.yaml:3724 says are left
- file: crates/takt/src/run.rs
  line: 6816
  category: property
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a rustup toolchain linked into the tree plus settings.toml making it the default is accepted, because the in-tree link target is skipped and settings.toml is not watched
```
