---
format: aep.planning-md/3
id: story:report-sums-cases
kind: story
status: active
title: takt report --cases sums units accepted, tokens and interventions
summary: per repository and UTC day, from events.jsonl and authority.yaml
tags:
- loom-agent
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:a-killed-step-records-its-tokens
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:22:44Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T21:22:44Z", actor: "human:timo", revision: 3}
---
# Story: report-sums-cases

## Outcome

`takt report --cases <dir>...` reads the case directories under each given cases directory and
prints, per repository and UTC day, the units accepted, the tokens the harness reported and the
human interventions, and a total row; a figure no record states is `unknown`, never 0.

## Context

The counting half of story:week-of-daily-runs, which needs a week of scheduled runs on three
repositories before its acceptance can be checked. This story is the command that week will be
read with, and it is what the pod runs of the cluster example are reported with
(`examples/cluster-delivery/README.md`).

- units accepted: cases whose last `stop` line in `events.jsonl` is `Completed` with an accepted
  outcome (`crates/takt/src/events.rs`, `Event::Stop`).
- tokens: the sum over the case's `stop` lines of `tokens`, one per run of the case; a stop line
  whose tokens are unknown (story:a-killed-step-records-its-tokens) makes the sum `unknown`.
- interventions: grants by a person, one per approval in the case's `authority.yaml` whose
  approver is `human:`; a grant by an agent role (story:an-unattended-run-merges-under-a-delegated-grant)
  is not an intervention.
- the day: the UTC date of the run's `opened` line; the repository: the directory the cases
  directory sits in (`<repo>/.engineering/cases`), or the `--cases` argument as given.

`takt report` today reads run traces (`crates/takt/src/report.rs`); `--cases` is a second input
of the same command, not a new command. Defects escaped, the fourth number of the week, stay
with story:week-of-daily-runs.

## Scope

- `crates/takt/src/report.rs`: the case sums
- `crates/takt-cli/src/main.rs`: `ReportArgs`, `--cases`
- new: `crates/takt/tests/report_cases.rs`, with recorded case directories as fixtures

## Acceptance

Over two fixture cases directories (repository `a`: one case accepted in one run, one case with
two runs and one approval; repository `b`: one case stopped on a budget),
`takt report --cases <a>/.engineering/cases <b>/.engineering/cases` exits 0 and prints one row per
repository and day with units accepted, tokens and interventions equal to a hand count of the
fixtures, and a total row equal to their sum; a case whose run recorded no tokens prints its
row's tokens as `unknown`.
