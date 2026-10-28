---
format: aep.planning-md/3
id: story:bench-subset-pinned
kind: story
status: draft
title: 'Each benchmark''s pinned subset is a committed file: the dataset version, the task ids drawn from it, and each task''s visible test command'
relations:
- informed_by: review-result:benchmark-acceptance-round-1
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:baseline-runner-in-repo
revision: 2
---
## Outcome

`docs/bench/subsets/terminal-bench-2.yaml` and `docs/bench/subsets/swe-bench-verified-mini.yaml` each record:

- the dataset and its version: a commit or a dataset revision id, never a branch name;
- the full task list at that version;
- the seed and the command that drew the subset;
- ten task ids drawn from that list;
- for each task, its visible test command or `none`, and its dollar budget;
- the budget for a whole run.

The runners read their tasks and budgets from these files, and every row a run writes names the version. A benchmark run can then be repeated, and nobody at takt picks which tasks it covers.

The draw and its check are `takt-bench draw-subset`, a subcommand of the `takt-bench` binary (crate `crates/takt-bench`, story:baseline-runner-in-repo, clap derive).

## Context

design:benchmark-integration § How takt fits a benchmark ("Each benchmark run names its dataset version and task ids") and § Problem ("tasks ... from sources nobody at takt chose"). No story pinned either until now: story:harbor-agent-adapter and story:swe-bench-mini-runner said "a pinned subset of 10" without naming one.

The draw is random, with the seed recorded, over a task list that is recorded too. Re-drawing is therefore offline and shows that the ten were not hand-picked. The generator and its seed handling are the subcommand's own, fixed in the crate, so a re-draw on another machine gives the same ids. Reading the task list from the dataset at its version needs network, once, when the file is written. Whether the recorded list matches the dataset is a live check, and it is not part of the acceptance.

Terminal-Bench copies its grading tests in after the agent finishes. Whether a task exposes a visible test command is unconfirmed, so it is recorded per task, and `none` is a real value. For `none`, the runners pass `--test-cmd true`, and the task's rows say so (story:bench-runs-schema). SWE-bench exposes no test command per instance, because its FAIL_TO_PASS tests are the held-out acceptance.

The per-task budget feeds `--usd` (story:bench-subset-and-budget).

The draw was planned as `docs/bench/draw-subset.py`. Running code committed to this repository is Rust with clap derive (approval-record:store-replan-2026-10-28), so it is a `takt-bench` subcommand instead.

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/bench/subsets` exits non-zero.

## Scope

- `docs/bench/subsets/terminal-bench-2.yaml` (inferred)
- `docs/bench/subsets/swe-bench-verified-mini.yaml` (inferred)
- `crates/takt-bench/src/main.rs`: the `draw-subset` subcommand (inferred)
- `crates/takt-bench/tests/draw_subset.rs` (inferred)

## Acceptance

`takt-bench draw-subset --check docs/bench/subsets/terminal-bench-2.yaml docs/bench/subsets/swe-bench-verified-mini.yaml` exits 0 when it re-draws each file's ten task ids from the task list and seed that file records and finds them equal to the ids recorded, and exits 1 naming the file when one recorded id is changed (`crates/takt-bench/tests/draw_subset.rs`, under `task test-library`), where at fb3c3bc neither file exists.
