# Wave 55: the gate holds against a step's edits

Stage: **delivered**. Released as 0.33.0 from fedfbc28c, the merge of pull request 62. `wave/55` branches from 9e7a3904e, the release of 0.32.0, and merges `store/close-wave-54` (the
close-out of wave 54) first, as bfe7871f5. Skill version 0.26.0 (aep:implementing, wave mode). Approved: approval-record:wave-55-gate-holds.
Release: 0.33.0.

## Selection

No planned wave is left. The one active story, story:harness-child-confined, is blocked:
dependency-blocker:loom-child-confinement is open, and Loom's `main` still asks Substrate for no writable root
(`crates/loom-intake-slice/src/confinement.rs:1572`, `writable_roots: Vec::new()`; Loom's story:writable-host-root is a
draft). Wave 55 is proposed from four `rsi` drafts (the `rsi` stories go first, `AGENTS.md` § Rules), filed by the
review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base. Each was read as still present on `main`:

| unit | story | still present at 9e7a3904e |
|---|---|---|
| a | story:the-gate-reads-paths-inside-arguments | `argument_paths` (`crates/takt/src/validator.rs:2557`) takes whole arguments and `-x=value` only |
| b | story:the-gate-compares-taskfile-includes | `GATE_TOOL_CONFIG` (`validator.rs:2945`) names Taskfiles by name; nothing reads `includes:` |
| c | story:the-gate-holds-against-lint-allows-in-source | `lint_changes` (`validator.rs:3110`) compares manifest lint tables only; no source attribute is read |
| d | story:the-gate-does-not-trust-a-step-accepted-citation | nothing in `validator.rs` compares `ess/**/*.citations.tsv`, `ess/ess-inputs.yaml` or `ess/system.yaml` |

`aep plan artifact waves --kind story --status draft` after their scope entries, the lines for these four:

```
collision: story:the-gate-compares-taskfile-includes story:the-gate-does-not-trust-a-step-accepted-citation crates/takt/src/validator.rs
collision: story:the-gate-compares-taskfile-includes story:the-gate-holds-against-lint-allows-in-source crates/takt/src/validator.rs
collision: story:the-gate-compares-taskfile-includes story:the-gate-reads-paths-inside-arguments crates/takt/src/validator.rs
collision: story:the-gate-does-not-trust-a-step-accepted-citation story:the-gate-holds-against-lint-allows-in-source crates/takt/src/validator.rs
collision: story:the-gate-does-not-trust-a-step-accepted-citation story:the-gate-reads-paths-inside-arguments crates/takt/src/validator.rs
collision: story:the-gate-holds-against-lint-allows-in-source story:the-gate-reads-paths-inside-arguments crates/takt/src/validator.rs
```

The collision is handled by order: the units run one after the other in one tree, each after the one before is
committed. Scope: `validator.rs` cited (read), each new test target inferred. Each unit also re-points the citations
in `ess/domains/` its change moves (`validator.rs` is cited about 120 times there).

## Units

| unit | story | objective | new test target | stage |
|---|---|---|---|---|
| a | story:the-gate-reads-paths-inside-arguments | a glued short option and the words of `sh -c "<line>"` are compared at the base, or the command is refused | `gate_reads_paths_in_arguments` | delivered |
| b | story:the-gate-compares-taskfile-includes | a Taskfile pulled in by `includes:` is compared at the base | `gate_compares_taskfile_includes` | delivered |
| c | story:the-gate-holds-against-lint-allows-in-source | a crate-level `allow` of a denied lint group added by a step records no `gate.green` | `gate_holds_against_lint_allows` | delivered |
| d | story:the-gate-does-not-trust-a-step-accepted-citation | a changed citation digest or a dropped `ess-inputs.yaml`/`system.yaml` entry records no `gate.green` | `gate_does_not_trust_step_citations` | delivered |

Tree: the managed tree `takt-w55-int` on `wave/55`, one `target/` inside it. Implementors edit that tree; the wave
commits each unit.

Targeted tests, each unit's new test target plus `cargo test -p b10x-takt --test gate_compares_tool_config` and
`--lib`, `cargo fmt --all --check`, `cargo clippy -p b10x-takt --all-targets --locked -- -D warnings`, the citations
check and `ess specify validate --path ess`. New tests read no `/proc` outside `cfg(target_os = "linux")`, and library
code starts every process through the confinement seam.

Unit terms: a unit reports at a `target/` of 8G or 20 cargo minutes, and stops at 10G or 30.

## Gate plan

On `wave/55` once the four units, the review fixes and the release preparation are in; one stage at a time, the build
cache discarded after the last stage.

| stage | named peak | cargo minutes | last measured (wave 54) |
|---|---|---|---|
| `task check-static` | 3G | 5 | 1G, 144 s cargo |
| `task test-cli` | 10G | 25 | 8G, 1147 s cargo |
| library, targeted: `-p b10x-takt --lib` and the gate, confinement and adversary test targets | 8G | 20 | not measured |

The whole library suite (44G peak) runs on the pull request's CI, as in wave 54.

## Units delivered

| unit | commit | targeted tests (the implementor's report) | peak `target/` |
|---|---|---|---|
| a | 24658b5a1 | `gate_reads_paths_in_arguments` 13, `gate_compares_tool_config`, lib 234 | 3.9G |
| b | 1f828f8e6 | `gate_compares_taskfile_includes` 9, `gate_compares_tool_config` 21, lib 234 | 4.0G |
| c | 5bcde0279 | `gate_holds_against_lint_allows` 11, lib 235, the other gate targets | 4.2G |
| d | d0e8fba54 | `gate_does_not_trust_step_citations` 14, lib 235, the other gate targets | 4.4G |
| correction | dbd7bba5f | `w55_adversary` 8, `w55_security` 8 (13 of 16 red before), lib 237, five gate targets | 4.7G |

The adversary pass found 8 red cases of 11, the security review 2 blockers and 4 warnings;
approval-record:wave-55-review-correction decided them: the ones inside the stories' acceptance were fixed in
dbd7bba5f, and nine `rsi` drafts take the rest. Unit b keeps the strict anchor rule
(approval-record:wave-55-taskfile-anchors).

## Gate

| stage | head | exit | wall | in slot | peak `target/` | result |
|---|---|---|---|---|---|---|
| `task check-static` | 017b756f6 | 0 | 381 s | 29 s | 4.7G | every step 0; citations 1480 resolved, 43 releases in form |
| `task test-cli` | 017b756f6 | 201 | 389 s | 189 s | 11G | stopped after 24 of 131 binaries at the one failure below |
| `cargo test --locked -p b10x-takt-cli --no-fail-fast` | 017b756f6 | 101 | 732 s | 460 s | 11G | 938 passed, 1 failed, 5 ignored, 131 binaries |
| library, targeted: `-p b10x-takt --lib` and 13 test targets | 017b756f6 | 0 | 182 s | 182 s | 4.2G | 436 passed, 0 failed, 3 ignored, 14 binaries |

- The one `test-cli` failure, `finding_scripts::the_predicate_reaches_no_network_ipc_or_socket_of_the_host`, is the
  host's, as in wave 54: the `ipip` module is loaded, so every new network namespace holds `tunl0` beside `lo` ("interfaces
  lo tunl0" in the log). No commit of the wave touches it; it counts green, and CI runs it on hosts without `tunl0`.
- The build cache was discarded between `test-cli` and the library stage (10634 MiB).
- The library stage's 13 targets: the five gate targets, `w55_adversary`, `w55_security`, `gate_validator`,
  `confinement`, `step_cannot_forge_its_validators`, `step_forge_reviewer_routes`, `step_cannot_forge_adv2` and
  `w54_adversary`. The whole library suite runs on the pull request's CI.

## Delivery

Pull request 62 first went red on CI on two jobs, neither from a change of the wave: `takt / darwin (test-cli)` was never
picked up by a runner ("The job was not acquired by Runner of type hosted even after multiple attempts"), and `takt /
task check (test-library)` failed `macos_confinement::a_marked_process_in_a_session_of_its_own_is_killed_by_the_sweep_and_by_the_end_of_run_child`
("stage 2 wrote no pid" within 10 s), a test no commit of the wave touches; the same target passed 5/5 locally at
7ad1ad146. Both failed jobs were run again and passed, and the pull request passed its 7 checks. It was merged as
fedfbc28c, main's check run 42215134770 passed, and the tag 0.33.0 was pushed; Release run 42217830116 published the
release with its assets, and verify-release passed. review-result:w55-adv-1 records the review.

The stories' close-out is a store-only commit on `store/close-wave-55`, cut from fedfbc28c. It rides along with the
next takt wave, whose integration branch merges it.
