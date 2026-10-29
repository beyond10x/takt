---
format: aep.planning-md/3
id: story:installed-binary-checked-by-its-digest
kind: story
status: draft
title: The installed binary is checked by its recorded digest, whatever it prints
summary: when the latest record carries binary_sha256, a binary printing the recorded revision under another digest is refused naming both
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
scope:
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: ess/domains/takt.yaml
revision: 5
---
# Story: installed-binary-checked-by-its-digest

## Outcome

When the latest promotion record carries `binary_sha256`, `takt promote` checks the installed binary by that digest whatever its `--version` prints. A replaced binary that prints the recorded revision under another digest is refused, naming both digests.

## Context

Split from story:promotion-records-its-toolchain in plan-critic round 1 (review-result:frontier-1-design-round-1). Noted 2026-10-19 after story:baseline-for-a-pre-revision-binary, decision 2.

At 338082d, `installed_names_latest` (`crates/takt/src/promotion.rs:2008-2069`) checks the digest only when the record's revision was stated by the operator (`:2019-2037`) or the binary prints no revision (`:2039-2057`). A binary that prints the recorded revision passes on that alone (`:2060-2067`). story:scorecard-binds-binary-digest is about a scorecard row's digest, another record.

A record without `binary_sha256` (a baseline recorded before records carried one) is still checked by the revision its binary prints; this story changes only the records that carry a digest.

## Ordering

- This story is in wave 40, with story:baseline-runner-in-repo, whose scope (`crates/takt-bench/**`, `Cargo.lock`, `docs/metrics/README.md`) shares no file with this one.
- `crates/takt/src/promotion.rs`, `crates/takt-cli/tests/promote_cli.rs` and `ess/domains/takt.yaml` are shared with story:promotion-survives-a-kill. Both decide `installed_names_latest`. That story goes after this one (its `depends_on` this story): it adds, before this story's digest refusal, the case of a record whose install never happened (the installed binary hashing to the previous record's digest).
- The same three files are shared with story:promotion-records-its-toolchain, whose hunks are the record's fields and the build's toolchain (`promotion.rs:173-233`, `:1473-1494`, `:1938-1960`). There is no edge between the two stories, and `aep plan artifact waves` keeps them in different waves.
- `ess/domains/takt.yaml` and `ess/domains/takt.citations.tsv` are also wave 39's. Wave 39 only re-points `read from:` citations, far from the installed-binary rule (`takt.yaml:5655-5665`) and `takt.work.Promotion` (`:5834`). This story does not wait for wave 39, and whichever merges second re-points the citations at its merge (`python3 -I scripts/check-citations --require-digests`).

## Expected before

On takt at 338082d, holds; after the change, fails:

- The latest record carries `binary_sha256` and no `revision_stated_by`. The binary at the install root prints `takt <version> (<the recorded revision>)` but hashes otherwise. `takt promote --dry-run` passes the installed-binary check: its output holds no "hashes to".

## Scope

- `crates/takt/src/promotion.rs`: `installed_names_latest` (cited, `:2008-2069`)
- `ess/domains/takt.yaml`: the installed-binary rule (cited, `:5655-5665`)
- `ess/domains/takt.citations.tsv`: the citations the changed lines need (inferred)
- `crates/takt-cli/tests/promote_cli.rs`: the rows below (inferred)

## Acceptance

In `crates/takt-cli/tests/promote_cli.rs`, the latest record carrying `binary_sha256`:

1. A replaced binary that prints the recorded revision under another digest makes `takt promote` exit non-zero, naming both digests, and install nothing.
2. The recorded binary itself passes the check.
3. A latest record without `binary_sha256` and a binary printing its revision passes as before.

Before the change, row 1's binary passes.
