# Takt

Composable agentic work: one binary that drives a repository's work through engineering protocols,
every step validated by Canon over evidence. `docs/design/takt.md` is the design; this file is for
agents working on this repository. `README.md` is for people.

## Layout

| path | what |
|---|---|
| `crates/takt` | the library `b10x-takt` (`use takt::…`): protocol loading, case files, evaluation, workflow types, the loop, the seams and their first adapters |
| `crates/takt-cli` | the binary `takt` |
| `docs/design/` | design documents |
| `.engineering/planning` | the AEP planning store; change it only through `aep plan artifact` verbs |

## Commands

```
task check          # format, lint, tests
cargo run -p b10x-takt-cli -- run --repo <path> --intent "<text>" --iterations 1
```

## Rules

- Evidence comes from validators only, never from a model's text. A new claim needs a validator
  that writes a `canon-evidence/1` record bound to a revision.
- Protocols are data: use `b10x-canon-engineering`'s built-ins, or a `protocol/1` file; never a
  model of a protocol written in Rust.
- A judgement step offers the model only the actions the frontier admits; a step that does more is
  refused and reverted, not accepted.
- Work in a managed worktree (`worktree create --purpose <what>`), build into its own `target/`.
- Every commit, push, pull request and tag goes through the bot route (`b10x-gates bot -- <git
  args>`); API writes through `b10x-gates api`. Pull-request and release bodies pass
  `b10x-gates scan-text` first.
- This repository is public. Committed text names no private repository, session or decision id,
  host detail or person.
- Spec first: `ess/` is the ESS specification of takt's nouns and their relations, checked with
  `ess specify validate --path ess`; a story that introduces a noun declares it there before code
  builds on it.
