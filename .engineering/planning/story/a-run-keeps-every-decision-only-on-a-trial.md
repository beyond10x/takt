---
format: aep.planning-md/3
id: story:a-run-keeps-every-decision-only-on-a-trial
kind: story
status: active
title: A run keeps every decision only on the trial records profile
summary: '--records default|trial: default keeps the latest decision, trial and scenario/improvement cases keep all'
tags:
- runtime-records
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/events.rs
- confidence: cited
  path: crates/takt/src/replay.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:44:47Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-27T01:44:47Z", actor: "human:timo", revision: 4}
---
# Story: a-run-keeps-every-decision-only-on-a-trial

## Outcome

A run's records profile, `--records default|trial` or the run configuration's `records`, decides how many of a
case's decisions it keeps. With `default` a case keeps one `decisions/NNNN.json`, the latest; with `trial` it keeps
every one, as today. Scenario runs, `self.improvement/1` and `loop.improvement/1` cases always keep every one. The
`opened` record names the profile.

## Context

- The owner, 2026-10-27: "the state files are probably that verbose because we need it on that verbosity level
  because of the evals and the trials. So maybe we need just some better defaults and then only enable this high
  verbosity during the trials."
- A trial case on a downstream repository held 9 decisions of 90,752 bytes, 67% of the case's 134,529 bytes; each is
  a full `canon-decision/1`, about 10 KB, most of it `explanation`.
- Only the latest decision is read by `takt approve`, by resume and by the pull request's body
  (`crates/takt/src/case.rs:1884-1910`, `case.rs:1932-1969`, `validator.rs:2351-2361`). Every decision is read by
  `takt replay` (`replay.rs:218-225`, `replay.rs:519-524`) and by the loop scorecard (`loop_scorecard.rs:819-850`).
- The sequence continues from the highest number on disk (`case.rs:2816-2835`), so keeping only the latest keeps the
  numbering.
- The governor of Loom renders a decision and reads none back; Canon evaluates from protocol, case, evidence and
  authority only (canon `AGENTS.md:22-26`).

## Acceptance

Each item is a named test, run by `task check`.

1. `records_profile::default_keeps_the_latest_decision`: a run of three evaluations with no `--records` leaves one
   file under `decisions/`, numbered 0003, and `takt approve` and resume read it.
2. `records_profile::trial_keeps_every_decision`: the same run with `--records trial` leaves 0001 to 0003.
3. `records_profile::config_key_and_flag`: `records = "trial"` in the run configuration keeps every decision;
   `--records default` wins over it; any other value is refused before the case opens.
4. `records_profile::scenario_and_improvement_cases_keep_every_decision`: a `takt scenario run` case and a
   `self.improvement/1` case keep every decision whatever `--records` says.
5. `records_profile::opened_names_the_profile`: the `opened` line carries `records`.
6. `records_profile::replay_of_a_dropped_decision_is_refused`: `takt replay` of a step whose decision a `default`
   run did not keep exits non-zero naming the profile and `--records trial`.

## Scope

- `crates/takt/src/case.rs`: keeping the latest decision (cited)
- `crates/takt/src/run_config.rs`, `crates/takt-cli/src/main.rs`: the key and the flag (cited)
- `crates/takt/src/run.rs`, `crates/takt/src/events.rs`: the profile on the run and on `opened` (cited)
- `crates/takt/src/replay.rs`: the refusal (cited)

## What each profile keeps

Measured on the trial of 2026-10-27, one case and two runs on a downstream repository:

| kind | where | files | bytes | default | trial |
|---|---|---|---|---|---|
| decisions, latest | case | 1 | 11,471 | kept | kept |
| decisions, earlier | case | 8 | 79,281 | dropped | kept |
| python-modules.json (no Python answered) | case | 1 | 4,945 | not written (story:python-modules-are-kept-only-when-a-python-answers) | not written |
| rest of the case (case, protocol, intent, branch, identity, evidence, events) | case | 11 | 38,832 | kept | kept |
| transcripts | state directory | 5 | 820,205 | kept | kept |

The run wrote 954,734 bytes, 86% of them transcripts. Both profiles keep transcripts: they are outside the tree,
and a failed step's transcript is the only record of what it did. With `default` the case is about 50 KB in 12
files. Dropping the transcripts of accepted steps (603,064 bytes here) is not part of this story.
