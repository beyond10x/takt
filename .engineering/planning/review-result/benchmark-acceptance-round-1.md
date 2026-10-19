---
format: aep.planning-md/3
id: review-result:benchmark-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, benchmark-integration stories, round 1
relations:
- reviews: story:bench-task-protocol
- reviews: story:bench-subset-and-budget
- reviews: story:bench-free-arms
- reviews: story:bench-in-scorecard
- reviews: story:bench-run-export
- reviews: story:harbor-agent-adapter
- reviews: story:takt-in-container
- reviews: story:swe-bench-mini-runner
- reviews: story:baseline-runner-in-repo
- reviews: story:scenario-corpus-realistic
revision: 1
---
needs-revision

story:bench-task-protocol — the acceptance joins two outcomes with "and" ("reaches `completed` once the visible tests pass, and offers no forge action"), so one can pass while the other fails — .engineering/planning/story/bench-task-protocol.md:38
story:bench-task-protocol — the acceptance names no harness and no fixture, so it cannot run offline: `--harness` takes only `claude` or `none`, and `none` cannot edit toward green tests — crates/takt-cli/src/main.rs:529
story:bench-subset-and-budget — the acceptance joins two outcomes (the budget stop with an unknown verdict, and the run record listing the dataset version and every task id) — .engineering/planning/story/bench-subset-and-budget.md:39
story:bench-subset-and-budget — "the run record" names no file or field, and "a budget below one task's cost" depends on a live model's spend, so it is neither observable nor offline; takt already has a `--tokens` stop, so the acceptance must name what is new (the dollar budget, the per-run budget, the unknown verdict) — .engineering/planning/story/bench-subset-and-budget.md:39
story:bench-free-arms — the acceptance joins two outcomes (a record per task and arm, and the journey view drawing them) — .engineering/planning/story/bench-free-arms.md:40
story:bench-free-arms — "the journey view draws them" names no checkable output and no pinned subset, and the generator reads only scenario-runs.jsonl today (`RUNS = .../scenario-runs.jsonl`), so the transition is unnamed — docs/metrics/journey.py:21
story:bench-in-scorecard — the acceptance joins "reported worse" and "promotion refuses it" as two outcomes, and the second sits behind story:promotion, which is still draft — .engineering/planning/story/bench-in-scorecard.md:40
story:bench-in-scorecard — the scorecard already reports a scenario worse under dominance, so the acceptance reads the same before the work unless it names how a benchmark task enters the scorecard (a case under `.engineering/cases/` or a bench-runs.jsonl row) and a fixture pair to compare — crates/takt/src/scorecard.rs:11
story:bench-run-export — "bench's validator accepts" names no command, and bench's validator checks its catalog store, not one exported record — the benchmark instrument: crates/bench-cli/src/validate.rs:1
story:bench-run-export — "a missing figure written as unknown, never 0" has no representation in bench's run shape, where `cost_micro_usd` defaults to 0, so the second half of the acceptance has no checkable target and is a second statement — .engineering/planning/story/bench-run-export.md:42; the benchmark instrument: docs/design/bench-entities-v0.1.md:218
story:harbor-agent-adapter — the acceptance joins two outcomes (a verdict from the task's tests, and a takt case directory per task) — .engineering/planning/story/harbor-agent-adapter.md:38
story:harbor-agent-adapter — nothing in it can run offline (Harbor, Docker, the dataset and a model login), no task ids are named for "a pinned subset of 10", and no `--test-cmd` is named, although `bench.task/1` reaches `completed` only on a visible test command (design:32); the story itself defers the adapter interface to the docs — .engineering/planning/story/harbor-agent-adapter.md:26
story:takt-in-container — the acceptance joins "`takt run` completes" and "a search finds nothing" as two outcomes — .engineering/planning/story/takt-in-container.md:42
story:takt-in-container — "a search ... for the login's token finds nothing" is also true before the work and when nothing ever ran, so the acceptance names no state before; a planted canary token that is present in the read-only mount and absent from the case, events and output would make it a transition, and the static musl build named in Expected before is not in the acceptance — .engineering/planning/story/takt-in-container.md:33
story:swe-bench-mini-runner — the acceptance joins three results (a patch file, a harness verdict, usage recorded) and "recorded" names no file, while the outcome promises 50 tasks and the acceptance names 10 unnamed ids — .engineering/planning/story/swe-bench-mini-runner.md:38
story:swe-bench-mini-runner — it needs Docker, the Hugging Face dataset and a model, and it names no offline surface, such as a fixture task graded by a stub harness — .engineering/planning/story/swe-bench-mini-runner.md:38
story:baseline-runner-in-repo — "reproduces a scenario 1 run of any arm from a clean machine" names no comparison for a run of a non-deterministic model, and the runner pushes a trial branch and calls live harnesses, so only "writes the same result fields" is checkable and it is not offline — .engineering/planning/story/baseline-runner-in-repo.md:38; docs/runs/2026-10-18-efficiency-analysis.md:59
story:scenario-corpus-realistic — the acceptance joins an offline-checkable outcome (six scenarios, each script passing on the reference and failing on the seed) with a live outcome (one four-arm round recorded) — .engineering/planning/story/scenario-corpus-realistic.md:38
story:scenario-corpus-realistic — "an acceptance script that passes on a reference solution and fails on the seed" is incoherent for the scenario "a task that should be refused", whose correct result is the untouched seed — .engineering/planning/story/scenario-corpus-realistic.md:20

(Findings block re-serialised as JSON by the planner and messages shortened; the critic returned YAML with the same fields. The private benchmark repository is not named here.)

Out of the acceptance critic's lane, noted for the scope critic: no story imports Terminal-Bench 2.0 or SWE-bench Verified Mini as bench corpora or adds takt as a bench harness (bench's harness kind is the enum claude|codex|b10x, bench-entities-v0.1.md:85); takt export covers takt runs only; no story writes the bench-runs.jsonl schema or the takt-arm rows; no story produces "one comparison per stage" in docs/metrics (epic:22-24); no story selects and pins the dataset version and task ids; bench.task/1 needs a visible --test-cmd and Terminal-Bench's is unconfirmed; "Claude Code isolated" has no Harbor or SWE-bench agent to reuse; bench-free-arms and bench-in-scorecard do not depend on baseline-runner-in-repo.

```findings
[
 {"file": ".engineering/planning/story/bench-task-protocol.md", "line": 38, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins two outcomes with 'and' (reaches completed once the visible tests pass, and offers no forge action)"},
 {"file": "crates/takt-cli/src/main.rs", "line": 529, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "bench-task-protocol: the acceptance names no harness and no fixture, so it cannot run offline"},
 {"file": ".engineering/planning/story/bench-subset-and-budget.md", "line": 39, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins two outcomes (budget stop with unknown verdict; run record listing dataset version and task ids)"},
 {"file": ".engineering/planning/story/bench-subset-and-budget.md", "line": 39, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "'the run record' names no file or field; a budget below one task's cost depends on a live model's spend"},
 {"file": ".engineering/planning/story/bench-free-arms.md", "line": 40, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins two outcomes (a record per task and arm; the journey view drawing them)"},
 {"file": "docs/metrics/journey.py", "line": 21, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "bench-free-arms: 'the journey view draws them' names no checkable output and no pinned subset"},
 {"file": ".engineering/planning/story/bench-in-scorecard.md", "line": 40, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins 'reported worse' and 'promotion refuses it'"},
 {"file": "crates/takt/src/scorecard.rs", "line": 11, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "bench-in-scorecard reads the same before the work unless it names how a benchmark task enters the scorecard and a fixture pair"},
 {"file": "the benchmark instrument: crates/bench-cli/src/validate.rs", "line": 1, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "bench-run-export: 'bench's validator accepts' names no command; the validator checks its catalog store"},
 {"file": ".engineering/planning/story/bench-run-export.md", "line": 42, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "'a missing figure written as unknown' has no representation in bench's run shape (cost_micro_usd defaults to 0)"},
 {"file": ".engineering/planning/story/harbor-agent-adapter.md", "line": 38, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins two outcomes (a verdict from the task's tests; a takt case directory per task)"},
 {"file": ".engineering/planning/story/harbor-agent-adapter.md", "line": 26, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "nothing runs offline; no task ids named for the pinned 10; no --test-cmd named"},
 {"file": ".engineering/planning/story/takt-in-container.md", "line": 42, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins 'takt run completes' and 'a search finds nothing'"},
 {"file": ".engineering/planning/story/takt-in-container.md", "line": 33, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the token search is also true before the work; a planted canary would make it a transition; the musl build is not in the acceptance"},
 {"file": ".engineering/planning/story/swe-bench-mini-runner.md", "line": 38, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins three results and 'recorded' names no file; outcome 50 tasks vs acceptance 10 unnamed ids"},
 {"file": ".engineering/planning/story/swe-bench-mini-runner.md", "line": 38, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "needs Docker, the dataset and a model; names no offline surface"},
 {"file": ".engineering/planning/story/baseline-runner-in-repo.md", "line": 38, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "'reproduces a scenario 1 run from a clean machine' names no comparison for a non-deterministic run; only 'writes the same result fields' is checkable"},
 {"file": ".engineering/planning/story/scenario-corpus-realistic.md", "line": 38, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance joins an offline outcome (six scenarios) with a live one (a four-arm round)"},
 {"file": ".engineering/planning/story/scenario-corpus-realistic.md", "line": 20, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "'passes on a reference and fails on the seed' is incoherent for 'a task that should be refused'"}
]
```
