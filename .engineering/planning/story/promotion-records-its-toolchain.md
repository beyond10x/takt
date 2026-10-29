---
format: aep.planning-md/3
id: story:promotion-records-its-toolchain
kind: story
status: draft
title: A promotion record names its toolchain, and a changed toolchain is refused
summary: the record carries the toolchain directory and rustc -vV; a different toolchain needs --toolchain-change; a channel in rust-toolchain.toml is refused
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: ess/domains/takt.yaml
revision: 10
---
## Outcome

A promotion record names the toolchain that built the binary: the toolchain directory and the `rustc -vV` output. `takt promote` refuses a build whose toolchain differs from the previous record's unless the operator passes `--toolchain-change`, and the record then names both. A `rust-toolchain.toml` that names a `channel` is refused, as one that names a `path` already is (`crates/takt/src/promotion.rs:1938-1960`).

## Context

From the reviewer's verdict "approve" on story:promotion at 2d10b7f (2026-10-19, logs outside the repository):

- Medium C3: a `RUSTUP_HOME` whose `settings.toml` names 1.88.0 produced an installed binary built by rustc 1.88.0, recorded as a clean promotion (`crates/takt/src/promotion.rs:753-826@2d10b7f`). `~/.rustup/toolchains` and `settings.toml` belong to the operator's user.
- Low C2: a committed `channel = "1.88.0"` was built with 1.99.0 without a word (`:1124-1146@2d10b7f`).

At 338082d the record (`Promotion`, `promotion.rs:173-233`) has no toolchain field, and the build takes rustup's default toolchain from its settings file (`Build::toolchain`, `:1473-1494`).

Split in plan-critic round 1: the crash windows are story:promotion-survives-a-kill, and the installed binary's digest check is story:installed-binary-checked-by-its-digest. The rewrite of the chain's newest record (C6, C8) needs write access to the store and is story:confinement-seam's.

## Ordering

- `crates/takt/src/promotion.rs`, `crates/takt-cli/tests/promote_cli.rs` and `ess/domains/takt.yaml` are also scoped by story:installed-binary-checked-by-its-digest (wave 40) and story:promotion-survives-a-kill (after it). This story's hunks are the record's fields (`promotion.rs:173-233`) and the build's toolchain (`:1473-1494`, `:1938-1960`), apart from `installed_names_latest` (`:2008-2069`) and the store's writes (`:1347-1399`, `:654-660`); no edge, and `aep plan artifact waves` keeps the three in different waves.
- `ess/domains/takt.yaml` and `ess/domains/takt.citations.tsv` are also rewritten by wave 39: its line moves in `run.rs`, `validator.rs`, `harness.rs` and `scorecard.rs` re-point `read from:` citations scattered over both files. Those re-pointings are far from this story's lines (`takt.work.Promotion`, `takt.yaml:5834`), so this story does not wait for wave 39; whichever merges second re-points the citations at its merge (`python3 -I scripts/check-citations --require-digests`).
- `crates/takt-cli/src/main.rs` is also scoped by story:normalized-harness-trace (`TraceArgs`, `:437-452`) and story:abandon-a-case (a new subcommand). This story's hunk is `PromoteArgs` (`:297-330`). No edge; story:normalized-harness-trace is `priority:low` and goes after this one.

## Expected before

On takt at 338082d, holds; after the change, fails:

- `takt promote --help | grep -c -- --toolchain-change` prints 0.
- `grep -nE "^    pub (toolchain|rustc)" crates/takt/src/promotion.rs` prints only `Build::toolchain` (`:1477`), no field of `Promotion`.

## Scope

- `crates/takt/src/promotion.rs`: the record's toolchain fields, the toolchain check, the `channel` refusal (cited)
- `crates/takt-cli/src/main.rs`: `PromoteArgs` gains `--toolchain-change` (cited, `:297-330`)
- `ess/domains/takt.yaml`: `takt.work.Promotion`'s toolchain fields and the refusal, declared before the code (cited, `:5834`)
- `ess/domains/takt.citations.tsv`: the citations the new lines need (inferred)
- `crates/takt-cli/tests/promote_cli.rs`: the rows below (inferred)

## Acceptance

1. `takt promote --dry-run` on a candidate prints a record whose toolchain directory is the one the build used and whose `rustc -vV` field equals that toolchain's `bin/rustc -vV` output byte for byte.
2. With a scratch `RUSTUP_HOME` whose default differs from the previous record's toolchain, `takt promote` exits non-zero naming both toolchains, and the installed binary's sha256 is unchanged. With `--toolchain-change` the same promotion proceeds, and its record names the previous toolchain and the new one.
3. A committed `rust-toolchain.toml` with `channel` is refused, naming the file.
