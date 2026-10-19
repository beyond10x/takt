---
format: aep.planning-md/3
id: story:promotion-records-its-toolchain
kind: story
status: draft
title: A promotion record names its toolchain, and a changed toolchain is refused
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
scope:
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
revision: 5
---
## Outcome

A promotion record names the toolchain that built the binary: the toolchain directory and the `rustc -vV` output. `takt promote` refuses a build whose toolchain differs from the previous record's unless the operator passes the change explicitly. A `rust-toolchain.toml` that names a channel is refused, like one that names a `path`. A promotion killed mid-run leaves no `.tmp-<pid>` file that makes `records()` refuse every later promotion.

## Context

From the reviewer's verdict "approve" on story:promotion at 2d10b7f (2026-10-19, logs outside the repository):
- Medium C3: a `RUSTUP_HOME` whose `settings.toml` names 1.88.0 produced an installed binary built by rustc 1.88.0, recorded as a clean promotion (`crates/takt/src/promotion.rs:753-826@2d10b7f`). `~/.rustup/toolchains` and `settings.toml` belong to the operator's user.
- Low C2: a committed `channel = "1.88.0"` was built with 1.99.0 without a word (`:1124-1146`).
- Low C4: if takt itself is killed, a `.<name>.tmp-<pid>` left in the store by `publish()` (`:613`) makes `records()` refuse every promotion (`:292-310`) (inferred from the code, not observed).

The rewrite of the chain's newest record (C6, C8) needs write access to the store and is story:confinement-seam's.

## Scope

- `crates/takt/src/promotion.rs`: the record's toolchain fields, the toolchain check, the stale temporary file (cited by the review)
- `crates/takt-cli/tests/promote_cli.rs`: the refusal rows (inferred)

## Acceptance

With a scratch `RUSTUP_HOME` whose default differs from the previous record's toolchain, `takt promote` exits non-zero naming both toolchains and installs nothing. A committed `rust-toolchain.toml` with `channel` is refused naming the file. A store holding a `.tmp-<pid>` from a process that is gone does not block the next promotion.

## From the review of main at 9cc158b (2026-10-19)

Two more crash windows (finding P1.5): a kill between `publish(copy)` and `publish(record)` (`crates/takt/src/promotion.rs:644-645@9cc158b`) leaves an orphan copy that `records()` refuses (`:376-379`); a kill after the record is written (`:1636`) and before `install` finishes (`:1637`) leaves a record whose binary was never installed, refused at `:1188-1195`. Acceptance, added: after `kill -9` in either window the next promotion proceeds.

## Also (2026-10-19)

`installed_names_latest` checks `binary_sha256` only for a stated baseline (story:baseline-for-a-pre-revision-binary, decision 2). Acceptance, added: when the latest record carries `binary_sha256`, the installed binary's digest is checked whatever its `--version` prints; a replaced binary printing the recorded revision under another digest is refused naming both digests.
