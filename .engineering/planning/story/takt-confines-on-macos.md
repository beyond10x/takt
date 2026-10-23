---
format: aep.planning-md/3
id: story:takt-confines-on-macos
kind: story
status: implemented
title: On macOS every step takt confines on Linux runs in Seatbelt or refuses by name
relations:
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T11:38:28Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T11:38:28Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-23T10:31:55Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: takt-confines-on-macos

## Outcome

On macOS, the four places that are confined or cleaned up on Linux each run in the macOS
platform sandbox (Seatbelt, `/usr/bin/sandbox-exec` with a profile takt generates) or refuse
by name. No step runs unconfined on macOS where Linux confines it. Linux keeps bubblewrap and
its namespaces, unchanged.

| path | Linux today | macOS after this story |
|---|---|---|
| promotion build and the built binary's `--version` (`crates/takt/src/promotion.rs`, `Confinement`) | bubblewrap: own PID and network namespaces, only the clone, toolchain, scratch `CARGO_HOME` and install root seen | Seatbelt: no network; writes only to the clone, its target directory, the scratch `CARGO_HOME`, the install root and a temporary directory; reads under the user's home only of the clone, the toolchain and the cached `CARGO_HOME` directories. `sandbox-exec` missing is a refusal naming it |
| the gate's test command (`crates/takt/src/forge.rs`, `without_login`) | user and mount namespace hiding `/run/user`, the runtime directory, D-Bus socket files and cargo registry logins | Seatbelt: writes only to the tree, the fresh home, the temporary directory and the cargo caches; reads of the cargo registry logins and the user's keychains denied |
| sweep of what a step left (`forge::sweep_left`) | subreaper adoption, then kill | every process of this user that carries the run's `TAKT_CHILD` mark and is not a running child is killed |
| end of a `run_child` call | the process group and adopted orphans | the process group and every process carrying the call's mark |

## Acceptance

1. A `macos-latest` CI job builds `takt` and runs the macOS confinement tests on the wave branch,
   green, its run id on the wave page.
2. In that job a test command that writes inside its tree succeeds, and one that writes a file
   outside the tree, the fresh home and the temporary directory is denied by the sandbox; the
   file does not exist afterwards.
3. In that job the confined promotion command cannot write outside its writable directories and
   cannot reach the network.
4. In that job a process a child started in a session of its own, still carrying the mark, is
   killed by `sweep_left` and by the end of `run_child`.
5. Where Seatbelt cannot give a bubblewrap guarantee (the build sees the clone at its real path,
   not at a fixed inside path; system directories stay readable; a process that clears its
   environment and leaves the process group escapes the sweep), the CHANGELOG's Unreleased
   Breaking list says so, one line each.
6. Linux `task check` stays green.

## Out of scope

The release (0.7.1) and the merge into `main`. The substrate-host move to 0.7.18 is a temporary
`[patch]` on the wave branch until loom 0.31.0, which pins it, is tagged; it is replaced by the
loom-sdk bump before any merge.

## Baseline on loom-sdk 0.35.0 (2026-10-23)

`darwin.yml` run 39877488594 at 1215f27, the first on loom-sdk 0.35.0, compiles on `macos-15` and
fails 105 tests:

| cause, from each failure's panic line | tests |
|---|---|
| bubblewrap (`bwrap`) is not on PATH | 37 |
| an exit code, `/proc`, a case-insensitive path (`.Engineering/planning`) and other Linux assumptions | 68 |

| test file | failures |
|---|---|
| `crates/takt-cli/tests/promote_cli.rs` | 33 |
| `crates/takt-cli/tests/self_improvement_cli.rs` | 32 |
| `crates/takt-cli/tests/finding_scripts.rs` | 27 |
| 10 other files | 13 |

The macOS wave that carries this story also takes the 68 and ends with `darwin.yml` green and the
`aarch64-apple-darwin` target back in `release.yml`.
