---
format: aep.planning-md/3
id: review-result:adversary-step-forge-pass-1
kind: review-result
status: active
title: Adversary, step-cannot-forge-its-validators, pass 1
relations:
- reviews: story:step-cannot-forge-its-validators
revision: 1
---
unit: story:step-cannot-forge-its-validators, worktree w10-step-forge at 38472f0 (base 9bd9d18) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 476→485, red 7
origin: introduced 1 / pre-existing 7 / undecided 0
wrote-outside-worktree: the unit's scratch `adv1/`
needs-coordinator: whether the manifest's build and test-target keys belong in this story (decided: the build-affecting keys yes; test selection no)

**Cases** (`crates/takt/tests/step_cannot_forge_adv1.rs`, run alone first)

| case | command | expected | head | base |
|---|---|---|---|---|
| `[lib] test = false` in Cargo.toml | `cargo test` | no pass | pass, accepted | red |
| `build = "gen.rs"` in Cargo.toml | `cargo test` | refused | not refused | red |
| ignored `build.rs` in a path dependency | `cargo test` | refused | refused | green |
| `check.sh` sources `lib.sh`, step rewrites `lib.sh` | `./check.sh` | no pass | pass | red |
| `bash -c ./check.sh` | | no pass | refused | red |
| `env -u X ./check.sh` | | no pass | pass | red |
| `sh -o errexit tests/run.sh` | | no pass | pass | red |
| `timeout 600 ./check.sh` | | no pass | pass | red |
| `make test`, Makefile rewritten | | no pass | pass | red |

**Gate.** `task check` exit 201 at tests (367 of 367 citations); without fail-fast 478 passed, 7 failed, all in `step_cannot_forge_adv1`.

**Attacked, not broken.** `bash -c`; an ignored nested `build.rs`; nested `.cargo/` and `build.rs` at any depth in git's view; renames; deletions; the unit's link and case tests; the defensive check excluding `.engineering/**`. Not tested: `ci.read` at the pull-request head; `python -m pytest` with `conftest.py`.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 545, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "a build script named by Cargo.toml's build key runs on every cargo test and the step adding it is accepted, bypassing the by-name build.rs rule under the default test command"},
  {"file": "crates/takt/src/validator.rs", "line": 104, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "Cargo.toml [lib] test = false drops the failing test and earns tests.pass under the default command; deleting the test in src/ has the same effect and is likewise unguarded"},
  {"file": "crates/takt/src/run.rs", "line": 419, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "only the test command's top script is guarded; a file it sources or execs is rewritten to exit 0 and earns tests.pass"},
  {"file": "crates/takt/src/run.rs", "line": 424, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "after env, the argument of -u is taken as the program, leaving the script unguarded"},
  {"file": "crates/takt/src/run.rs", "line": 435, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "after an interpreter, an option's separate argument is taken as the script"},
  {"file": "crates/takt/src/run.rs", "line": 419, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "a wrapper program that is neither env nor an interpreter (timeout 600 ./check.sh) hides the script from the guard"},
  {"file": "crates/takt/src/run.rs", "line": 327, "category": "acceptance", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "make test with the Makefile rewritten earns tests.pass; build-tool recipe files are not on the surface"},
  {"file": "crates/takt/tests/validator_confinement.rs", "line": 243, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the rewritten confinement tests rely on the unguarded exec'd tests/case.sh, so closing the script-indirection gap would turn them into step refusals"}
]
```
