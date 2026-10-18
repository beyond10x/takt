---
format: aep.planning-md/3
id: story:binary-embeds-revision
kind: story
status: active
title: takt --version and every run record the revision the binary was built from
summary: 'R9: the binary prints and records its git revision, so a promotion is checked against it, not a build note'
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/build.rs
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/events.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T18:27:20Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T18:27:20Z", actor: "human:timo", revision: 5}
---
# Story: binary-embeds-revision

## Outcome

Every takt binary carries the git revision it was built from: `takt --version` prints it, every run's `events.jsonl` records it as the binary's revision, and a promotion record is checked against the revision the installed binary reports, not against a build note.

## Context

product-requirements:takt R9 asks that every run record its pinned inputs, the binary sha first; nothing builds it yet. The first accepted improvement turn (docs/runs/2026-10-18-improvement-turn-1/) showed the gap: the developer role installed takt built at a0b8576 into `~/.cargo/bin/takt`, and `takt --version` printed only `takt 0.1.0` (`crates/takt-cli/src/main.rs`, the Cargo version); `strings` found no a0b8576 in the binary. The turn's `promotion_record` is therefore a build record tied by the binary's sha256 (b7af9ec9…5c56), not the check `self.improvement/1` describes, "the installed takt binary was built from the merged candidate revision" (`protocols/self-improvement/1.yaml`, claim `promoted`).

## Expected before

On takt at a0b8576, holds; after the fix, fails:

- `~/.cargo/bin/takt --version` prints exactly `takt 0.1.0`, with no revision.

## Scope

- `crates/takt-cli/build.rs` or `crates/takt-cli/src/main.rs`: the revision at build time (inferred)
- `crates/takt/src/events.rs`: the binary revision on the `opened` event (inferred)

## Acceptance

A binary built at a commit prints `takt 0.1.0 (<sha>)`, and `(<sha>-dirty)` when built from a tree with uncommitted changes; a run's `opened` event records that revision; a build outside a git checkout prints `(unknown revision)` and never a guessed one.
