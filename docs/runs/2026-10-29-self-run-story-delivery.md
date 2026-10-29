# Self-runs: takt delivers one of its own stories, short of the forge

Twelve takt runs, 2026-10-28 and 2026-10-29 (UTC), on one story of takt's own planning store,
`story:duplicate-key-reader-limits`, each in a scratch clone of this repository reset to the same
base before the run. Runs 11 to 15, on `story.delivery/3`, each took the story through its tests,
the repository's gate and the handover, and stopped at the pull request: `Unperformable
(pr.opened; nothing performs it without the forge)`, exit 7, because the runs had `--forge none`.
Run 9, on an earlier build, had stopped there too. No run opened, read or merged a pull request. **A story delivery with a forge on this
repository, pull request opened, CI read and merged under a grant, is not proven by these runs.**

Every figure below is read from a run's `run.log`: its first line (the binary and the base), its
`settings:` line, its `gate.run` step lines and its `stopped:` line. The binary id and the
protocol's source also come from each case's `opened` line in `events.jsonl`. The records are
kept outside the repository, one directory per run: `run.log`, `exit.txt`, `commits.txt`,
`diff.patch`, the case directory and the step transcripts.

## Setup

| item | value | source |
|---|---|---|
| story | `story:duplicate-key-reader-limits`: three duplicate keys of a YAML run configuration (an alias written as a key, a tagged key before the duplicate, a plain key ending in a colon) that the reader does not refuse at the duplicate's line. Its three tests were already in `crates/takt/tests/w40_adv_dup_and_pin.rs`, each `#[ignore]`d with the story's id. | the story; `diff.patch` |
| base | `8b8ac758`, a local commit above `b0dd8047` (wave 40's close) that moves the story to `proposed`; it is not in the public history. The base branch in the clone was `wave/40`. | `run.log` first line; `change.read` lines |
| harness | Codex, `codex-cli 0.165.0`, model `gpt-6.1-terra`; takt's preflight advised that the adapter is pinned to 0.153.4 to 0.160.0 (row H9, a gap, not a refusal) | `opened` line (`harness_program`, `preflight`); `model` in `events.jsonl` |
| command | `takt run --repo <tree> --story story:duplicate-key-reader-limits --harness codex --model gpt-6.1-terra --forge none --test-cmd "cargo test -p b10x-takt --test w40_adv_dup_and_pin --test run_config_dup_adv2 --test run_config_adv1" --iterations 40 --min-free-gib 26 --commit-identity <identity>`, with `--protocol story.delivery/3 --input tests_first=required --input gate_on=after_handover` for runs 6, 8, 9 and 11 to 15; runs 5 and 7 gave a local protocol file in place of `story.delivery/3` | the runner script; `settings:` lines |
| confinement | none: the test command ran on the host, unconfined | `run.log` `case` line |
| gate | `.engineering/gates.yaml` of the base, five gates: specification, citations, changelog, format, lint | `gate.run` lines |
| takt | development builds of this repository, each printing `takt 0.18.0`. Runs 11 to 15 all record the binary `ad3f76fc…-dirty`: built from uncommitted changes above `ad3f76fc`, so the record does not tell those builds apart. They hold wave 41's console. None of them holds the work of waves 40 and 42 that 0.19.0 also releases, such as the failed-test block at the head of the prompt (`6644f844`), the assertion locations (`4d299030`) and the rule that the red run of new tests is no repeated failure (`6679bf09`) | `opened` line `binary`; `git merge-base --is-ancestor` |

## The runs

| run | binary | protocol (source) | stop | exit | steps | tokens | wall | commits |
|---|---|---|---|---|---|---|---|---|
| 4 | `5eed1c1c` | `story.delivery/2` (default) | `Iterations (40)` | 3 | 40 | 1,281,575 | 6m39s | 9 (2 paths) |
| 5 | `5eed1c1c` | `story.delivery/3` (a local file) | interrupted at step 16 by the operator | none | | | | |
| 6 | `5eed1c1c…-dirty` | `story.delivery/3` (flag) | `Declined` | 9 | 11 | 388,639 | 4m28s | 3 (2 paths) |
| 7 | `5eed1c1c…-dirty` | `story.delivery/3` (a local file) | `Declined` | 9 | 14 | 418,897 | 3m58s | 3 (2 paths) |
| 8 | `54a734fc…-dirty` | `story.delivery/3` (flag) | `StepRefused` | 3 | 9 | 333,902 | 2m16s | 2 (2 paths) |
| 9 | `e17d83b8…-dirty` | `story.delivery/3` (flag) | `Unperformable (pr.opened; …)` | 7 | 18 | 699,122 | 3m31s | 4 (4 paths) |
| 10 | `ad3f76fc…-dirty` | `story.delivery/2` (default) | `Declined` | 9 | 9 | 328,193 | 2m10s | 2 (2 paths) |
| 11 | `ad3f76fc…-dirty` | `story.delivery/3` (flag) | `Unperformable (pr.opened; …)` | 7 | 8 | 463,455 | 2m04s | 2 (2 paths) |
| 12 | `ad3f76fc…-dirty` | `story.delivery/3` (flag) | `Unperformable (pr.opened; …)` | 7 | 8 | 527,959 | 2m21s | 2 (3 paths) |
| 13 | `ad3f76fc…-dirty` | `story.delivery/3` (flag) | `Unperformable (pr.opened; …)` | 7 | 10 | 1,234,041 | 3m50s | 3 (3 paths) |
| 14 | `ad3f76fc…-dirty` | `story.delivery/3` (flag) | `Unperformable (pr.opened; …)` | 7 | 13 | 912,996 | 3m36s | 3 (4 paths) |
| 15 | `ad3f76fc…-dirty` | `story.delivery/3` (flag) | `Unperformable (pr.opened; …)` | 7 | 13 | 1,087,139 | 4m07s | 3 (3 paths) |

Tokens are the `stopped:` line's, as the harness reported them; no stop line names a cost. The
commits are those the run's steps made, with how many paths they committed; the commit that takes
the story is not counted. Runs 1 to 3 were on other stories or on 0.17.0 and are not in this
record.

The `gate.run` results, as the step lines give them:

| run | gate results (step: result) |
|---|---|
| 4 | 8, 13, 18, 23, 28, 33 and 38: fail (citations, format) |
| 5 | 8: fail (citations, format); 15: fail (citations) |
| 6, 7, 8 | 8: fail (citations, format); in run 7 also 13: fail (citations) |
| 9 | 8: fail (citations, format); 13: fail (citations); 18: pass |
| 10 | 8: fail (citations) |
| 11, 12 | 8: pass |
| 13 | 10: pass |
| 14 | 8: fail (citations, format); 13: pass |
| 15 | 8: fail (format); 13: pass |

## What a successful run did

Run 15, from its step lines:

1. `tests.run` at the base: pass, with the story's three tests ignored.
2. `tests.write`: removed the three `#[ignore]` lines.
3. `tests.run`: fail, the three tests named; `implementation.edit` became admissible.
4. `implementation.edit`: changed `crates/takt/src/run_config.rs` and `ess/domains/takt.yaml`.
5. `tests.run`: pass.
6. `change.handover`, then `change.read`: the change is ahead of the base and changes test and
   implementation paths.
7. `gate.run` at step 8: fail on format.
8. `implementation.edit` again (`story.delivery/3` admits it after a red gate), `tests.run` pass,
   `change.handover`, `change.read`.
9. `gate.run` at step 13: pass, every gate true.
10. `candidate: d8ffdee5…` printed, then the stop at `pr.opened`.

## Why the earlier runs stopped

- **Runs 4 and 10, `story.delivery/2`.** These builds pinned engineering protocols 0.12.0, whose
  `story.delivery/2` admits `implementation.edit` only while the tests fail. After a red gate
  with green tests it offered only `tests.write`. Run 4 repeated the same gate failure seven
  times until its step budget; run 10 declined. Engineering protocols 0.12.1 and 0.12.2 (0.19.0
  pins 0.12.2) admit the edit after a red gate. `story.delivery/3`, takt's own protocol from
  0.19.0, adds that `change.handover` is not admissible while the tests fail, and its words say
  that the gate runs after the handover.
- **Runs 6 and 7, `Declined`.** The step after the red gate declined. In run 6 the step was offered
  `change.handover` and `tests.write`, and declined with the reason that `implementation.edit`
  was not admissible. In run 7 the citations to fix were in `ess/domains/takt.yaml`, outside the
  story's Scope. From 0.19.0 the prompt says that the gate runs after the handover, and that a
  file the failed gate names is part of the change.
- **Run 8, `StepRefused`.** The edit to `ess/domains/takt.yaml` was refused because the whole file
  held text of a credential's shape. The base already had that text, in a comment at line 1665.
  From 0.19.0 the proposal scan reads what a proposal adds.
- **Run 5** was interrupted by the operator at step 16.

## Seen in the runs

- In runs 11, 13, 14 and 15, an `implementation.edit` step's `said:` line reports that the
  specification and format checks could not run in the harness session: `ess` was unavailable
  and rustup's home could not be written. The gate ran those checks afterwards. This is the
  harness's own text, not evidence. In runs 14 and 15 the first gate then failed on format (in
  run 14 on citations too), and the fix took one more edit and one more gate run.
- None of the six candidates (runs 9 and 11 to 15) was pushed or merged. The story is still
  `draft` in this repository's planning store, and none of the candidates' code is in 0.19.0.

## Not proven by these runs

- A delivery with a forge: `pr.open`, `ci.read` and `repository.merge` under a grant on this
  repository. Every run here had `--forge none`.
- A run of the 0.19.0 release itself: the builds above lack the work of waves 40 and 42.
- A run with another harness or model on this story.
- That the candidates are correct beyond the gate and the three tests: no review verdict is
  recorded here.
