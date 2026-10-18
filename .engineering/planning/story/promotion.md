---
format: aep.planning-md/3
id: story:promotion
kind: story
status: draft
title: takt promote installs a binary only when the scorecard shows no regression
summary: revision in --version, median tokens within 10 %, one a day, promotion record as next baseline
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scorecard-reader
- supersedes: story:pinned-judge-and-promotion
revision: 1
---
# Story: promotion

## Outcome

`takt promote` installs a takt binary for runs only when the candidate's saved scorecard shows no
scenario with a worse outcome than the promoted baseline's, the median of tokens at most the
baseline's plus 10 %, and no other promotion in the same UTC day; the binary embeds the git
revision it was built from in `takt --version`; and each promotion writes a promotion record that
names that revision and the scorecard it read, which becomes the next turn's baseline.

## Context

design:recursive-self-improvement § Scorecard, § The judge. `takt --version` prints a bare version
today (clap `version`, `crates/takt-cli/src/main.rs`), so nothing can check which revision an
installed binary was built from. story:scorecard-reader saves scorecards and compares two; this
story decides which saved scorecard is the promoted baseline: the one named by the latest
promotion record. The first baseline is the scorecard of the revision installed when this story
lands, recorded by a first `takt promote --baseline-only` that installs nothing.

## Domain relations

- Promotion → Scorecard: references exactly one, by id (`ess/domains/takt.yaml`, takt.work.Promotion).
- Promotion → Revision: names exactly one, the one the binary embeds.

## Scope

- `crates/takt-cli/src/main.rs` (`promote`, revision in `--version`) (inferred)
- new: `crates/takt-cli/build.rs` (the revision embedded at build) (inferred)
- new: `crates/takt/src/promotion.rs` (the record and the defaults) (inferred)
- `crates/takt/src/lib.rs` (inferred)

## Ordering

- depends_on story:scorecard-reader: promotion reads its saved scorecards; both edit `crates/takt-cli/src/main.rs`, so this story follows it on that file.

## Acceptance

`takt promote` over a saved scorecard whose median tokens exceed the baseline's by more than 10 %
exits non-zero, installs nothing and writes no promotion record.

## Checks

- A promotion that passes writes a record whose revision equals the one the installed `takt --version` prints.
- A second `takt promote` on the same UTC day is refused, naming the first.
- A scorecard with one scenario whose outcome is worse than the baseline's is refused.

## Out of Scope

The merge-time judge guard (story:judge-guard); the loop's own promotion (story:loop-scorecard).
