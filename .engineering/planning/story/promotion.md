---
format: aep.planning-md/3
id: story:promotion
kind: story
status: implemented
title: takt promote installs a binary only when the scorecard shows no regression
summary: revision in --version, no scenario worse (one case of margin, n=5), median tokens within the baseline's maximum, one a day, promotion record as next baseline
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scorecard-reader
- supersedes: story:pinned-judge-and-promotion
- depends_on: story:binary-embeds-revision
scope:
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/takt-cli/build.rs
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: inferred
  path: crates/takt/src/promotion.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T00:53:34Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-19T00:53:34Z", actor: "human:timo", revision: 13, decided_on: {"recorded":{"test_result":1}}}
- {from: "active", to: "implemented", at: "2026-10-19T00:53:34Z", actor: "human:timo", revision: 14, decided_on: {"recorded":{"test_result":1}}}
---
# Story: promotion

## Outcome

`takt promote` installs a takt binary for runs only when the candidate's saved scorecard shows no
scenario with a worse outcome than the promoted baseline's (dominance with a margin of one case,
n=5 candidate runs), no scenario whose median of tokens exceeds the baseline's maximum, and no
other promotion in the same UTC day; the binary embeds the git
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

- Promotion → Scorecard: references exactly one, by id (`ess/domains/takt.yaml`, takt.work.Promotion); one-to-one: a scorecard backs at most one promotion. Decided 2026-10-18.
- Promotion → Revision: names exactly one, the one the binary embeds.
- Promotion record: takt.work.Promotion, result takt.work.PromotionResult. Decided 2026-10-18:
  - store: one JSON file per promotion at `$XDG_STATE_HOME/takt/promotions/<revision>.json` (`~/.local/state/takt/promotions/` without `XDG_STATE_HOME`), outside every repository a candidate changes; the latest record by `promoted_at` is the baseline.
  - instant: a field `promoted_at` (RFC 3339, UTC), which the one-a-UTC-day rule reads.
  - kind: `kind: baseline` on a `--baseline-only` record, `kind: promotion` on one that installed a binary (enum takt.work.PromotionKind).

## Scope

Derived 2026-10-18 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt-cli` — cited, the story's Context names `crates/takt-cli/src/main.rs` for `--version`
- **Files:** `crates/takt-cli/src/main.rs:16-25` (clap `version`, line 19) — cited, the bare version the story replaces
- **Files:** `crates/takt-cli/src/main.rs:27-37` (`enum Commands`) and `:189-228` (dispatch in `run`) — inferred, where a `Promote` subcommand and its `--baseline-only` flag go
- **Files:** new `crates/takt-cli/build.rs` (the git revision embedded at build) — inferred, absent today; Cargo picks a `build.rs` up without a manifest change
- **Files:** new `crates/takt/src/promotion.rs` (the promotion record, the no-worse / median within the baseline's maximum / one-per-UTC-day rules) — inferred, absent today
- **Files:** `crates/takt/src/lib.rs:8-17` (`pub mod promotion;`) — inferred, the module list every kernel module is declared in
- **Symbols:** `takt.work.Promotion`, `takt.work.PromotionId` — cited, already declared in `ess/domains/takt.yaml:413` and `:1063-1090` ("Declared ahead of the code (story:promotion)")
- **Also likely:** new `crates/takt-cli/tests/promote_cli.rs` — inferred, from the `*_cli.rs` pattern in `crates/takt-cli/tests/`; it is where the Acceptance and the three Checks run against `CARGO_BIN_EXE_takt`
- **Also likely:** `ess/domains/takt.yaml:1063-1090` — inferred, the Promotion comment would move from "declared ahead of the code" to code citations, as each wave's domain-document step has done (`docs/waves/wave-05.md:116`)
- **Documents:** `README.md:16-20` (`cargo install --locked --path crates/takt-cli`, `takt --version`) — inferred, only if installing moves behind `takt promote`
- **Confidence:** medium — the story names `main.rs` and `--version` and the noun is already declared in ESS; the kernel module, the build script and where the record is stored are guesses, and `crates/takt/src/scorecard.rs` (story:scorecard-reader), which this story reads, does not exist in the tree
- **Would collide with:** any unit that adds or changes a `takt-cli` subcommand in `crates/takt-cli/src/main.rs`, and any unit that adds a kernel module to `crates/takt/src/lib.rs` — cited, the story's Ordering names the `main.rs` overlap with story:scorecard-reader, and story:judge-guard names the same file
- **Safety fact:** nothing in the tree reads `takt --version` output: the only version source is clap's `version` at `crates/takt-cli/src/main.rs:19`, and `git grep -- '--version\|CARGO_PKG_VERSION' crates` finds no reader, so adding the revision breaks no existing caller — step 2, unproven

Where the promotion record is stored was decided 2026-10-18 (Domain relations: under `$XDG_STATE_HOME/takt/promotions/`, outside every repository a candidate changes); in a tree a candidate edits, a candidate could plant its own baseline, so the security reviewer should still prove a candidate cannot write there.

Not established: what "installs" means (cargo install, copy, Taskfile task); the saved scorecard's format and location (story:scorecard-reader); whether a `promoted` validator in `validator.rs` is in scope (`protocols/self-improvement/1.yaml:184-194` declares `binary.promote`); whether `crates/takt-cli/Cargo.toml` needs dependencies.

Typed entries: `crates/takt-cli/src/main.rs` (cited); `crates/takt-cli/build.rs`, `crates/takt/src/promotion.rs`, `crates/takt/src/lib.rs`, `crates/takt-cli/tests/promote_cli.rs`, `ess/domains/takt.yaml`, `README.md` (inferred).

## Ordering

- depends_on story:scorecard-reader: promotion reads its saved scorecards; both edit `crates/takt-cli/src/main.rs`, so this story follows it on that file.

## Acceptance

`takt promote` over a saved scorecard where one scenario's median tokens exceed the baseline's
maximum, or where one scenario has two cases worse than the baseline at some outcome rank, exits
non-zero, installs nothing and writes no promotion record.

## Checks

- A promotion that passes writes a record whose revision equals the one the installed `takt --version` prints.
- A second `takt promote` on the same UTC day is refused, naming the first.
- A scorecard with one scenario whose outcome is worse than the baseline's is refused.

## Out of Scope

The merge-time judge guard (story:judge-guard); the loop's own promotion (story:loop-scorecard).

## From the scorecard-reader review

From the reviewer verdict on story:scorecard-reader at 1c3cf0f (approve):

- medium: `takt scorecard --against <file>` checks neither where the saved scorecard is nor where it came from (`crates/takt/src/scorecard.rs:584-597`, `:558`): a copy renamed `scorecard-aaaaaaa.json` with every outcome set to `NoProgress` printed `against 97623a6` and `not worse`; `--save` accepts a directory inside `--repo`, which a harness step could write and the case commit. Promotion therefore reads the baseline it compares against from a pinned place outside the repository the candidate changes, requires the file's `revision` field to equal its name and the promoted revision, and refuses a `--save` or `--against` path inside the repository.
- low: a case and its scenario are taken from the directory name and the `case` field on `opened` is not read (`scorecard.rs:309-349`, `:295`); a case directory that is a link is dropped without being listed (`:416`, `:440`). Promotion counts only cases whose `opened` names them, and lists every directory it skipped.
- decided 2026-10-18, design:recursive-self-improvement § Scorecard: n=5 candidate runs per scenario; worse on tokens when the candidate's median exceeds the baseline's maximum; dominance with a margin of one case per scenario; no re-run of an unchanged candidate; n and the margin recorded in the Scorecard. The ESS entry's UNMAPPED token rule is mapped to these fields in this story.
