# Wave 55: the gate holds against a step's edits

Stage: **proposed**. `wave/55` branches from 9e7a3904e, the release of 0.32.0, and merges `store/close-wave-54` (the
close-out of wave 54) first, as bfe7871f5. Skill version 0.26.0 (aep:implementing, wave mode). Approved: pending.
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
| a | story:the-gate-reads-paths-inside-arguments | a glued short option and the words of `sh -c "<line>"` are compared at the base, or the command is refused | `gate_reads_paths_in_arguments` | proposed |
| b | story:the-gate-compares-taskfile-includes | a Taskfile pulled in by `includes:` is compared at the base | `gate_compares_taskfile_includes` | proposed |
| c | story:the-gate-holds-against-lint-allows-in-source | a crate-level `allow` of a denied lint group added by a step records no `gate.green` | `gate_holds_against_lint_allows` | proposed |
| d | story:the-gate-does-not-trust-a-step-accepted-citation | a changed citation digest or a dropped `ess-inputs.yaml`/`system.yaml` entry records no `gate.green` | `gate_does_not_trust_step_citations` | proposed |

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
