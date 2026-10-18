---
format: aep.planning-md/3
id: design:benchmark-integration
kind: design
status: draft
title: 'Benchmark integration: realistic scenarios first, then Terminal-Bench 2.0 and SWE-bench Verified Mini through the organization's benchmark instrument'
summary: own realistic scenarios first; then the two public benchmarks as bench corpora, takt as one harness, benchmark tests as held-out acceptance, four arms per task
relations:
- designs: product-requirements:takt
- serves: vision:safe-agentic-coding
revision: 2
---
# Design: benchmark integration

## Problem

takt's evidence so far comes from one toy scenario: a 31-line change to a small CLI, where every arm passed its held-out check 3 of 3, so the check could not tell the arms apart (docs/metrics/README.md). Claims about takt (evidence, authority, less supervision, cost per accepted unit) need tasks hard enough that agents fail some of them, from sources nobody at takt chose. Two public benchmarks fit, and our own corpus must grow first.

## Two stages

1. **Our own corpus, realistic first.** More scenarios on the scratch-repository model (story:scenario-corpus): multi-file features, a bug whose fix is tested by a hidden test, a refactor that must keep behaviour, a change that needs the gate, a task that should be refused. Each with a held-out acceptance script, run under the four arms of the baseline method (takt; Claude Code with its operator configuration; Claude Code isolated as takt isolates it; Codex). This stage needs no new runtime and comes first.
2. **External benchmarks, later.**
   - **Terminal-Bench 2.0** (tbench.ai; run by the Harbor harness: `harbor run -d terminal-bench/terminal-bench-2 -a <agent>`): each task has an English instruction, a test script that checks completion and an oracle solution, and runs in its own Docker container. Agents plug in as Harbor adapters.
   - **SWE-bench Verified Mini** (50 tasks, a random subset of SWE-bench Verified; Hugging Face `MariusHobbhahn/swe-bench-verified-mini`, listed on the HAL leaderboard): a model writes a patch for a repository issue inside a Docker environment, and the SWE-bench harness grades the patch with the task's tests.

## The organization's instrument: the organization's benchmark instrument

The organization's benchmark instrument already types what a comparison needs (harness, framework, corpus, case, arm, run, fact, study; `docs/design/bench-entities-v0.1.md` there) and judges runs with a deterministic grader into a fact table that counts what held and what did not, without scoring. takt does not build a second instrument: takt is one harness in bench's harness-by-harness subject, takt exports each run in bench's run shape, and Terminal-Bench 2.0 and SWE-bench Verified Mini become bench corpora. The bench-side work (importing the two benchmarks as corpora, a takt arm, the studies) is filed in bench's own store when it starts; the takt-side work is the stories of epic:benchmark-integration.

## How takt fits a benchmark

- **The benchmark's tests are the held-out acceptance.** takt never sees them, as the baseline method's `acceptance.sh` is never shown to an arm. takt's own claims come from what the task exposes (a visible test command, the repository's tests).
- **A protocol for a benchmark task**, `bench.task/1`: claims `tests.pass` on the visible test command, no forge, no pull request, no merge; outcome `completed` when the visible tests pass and the harness says it is done, `abandoned` on a stop. Defined as protocol data, not Rust.
- **takt runs inside the task's container**, as the agent the harness invokes: a statically linked `takt`, the harness CLI (`claude`), and the harness's login mounted read-only for the run, never copied into a case record, a prompt or a trace (product-requirements:takt R4, R5).
- **The same tasks under the four arms**, so a benchmark result is a comparison, not a score: Harbor and the SWE-bench tooling already ship agents for free-running harnesses where they exist; takt is the added agent.
- **Raw records, as for the scenario runs**: per task and arm the raw usage, wall time, the harness's verdict, the case directory and events, appended to `docs/metrics/bench-runs.jsonl`; the journey view draws them (story:harness-journey-timeline).
- **A pinned subset, a budget.** Each benchmark run names its dataset version and task ids, so it can be repeated; a token and dollar budget per task and per run stops it rather than overspending (R10).

## What it decides later

- Which benchmark tasks join the RSI scorecard's scenario set, so promotion is judged on them too (design:recursive-self-improvement § Scorecard).
- Whether takt registers as a public Harbor agent.

## Not in this design

Leaderboard submission; benchmarks outside software engineering; a hosted runner. Network egress from task containers follows the benchmark's own rules.

Sources: Terminal-Bench documentation (tbench.ai, harbor-framework/terminal-bench); SWE-bench Verified (swebench.com/verified, openai.com/index/introducing-swe-bench-verified); SWE-bench Verified Mini (hal.cs.princeton.edu/swebench_verified_mini; evalscope docs). Read 2026-10-18; the commands are checked against the official docs when the stories start.
