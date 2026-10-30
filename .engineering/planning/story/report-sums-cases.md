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
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/report_cases_cli.rs
- confidence: cited
  path: crates/takt/src/report.rs
- confidence: inferred
  path: crates/takt/src/scorecard.rs
- confidence: cited
  path: crates/takt/tests/report_cases.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: inferred
  path: examples/cluster-delivery/README.md
- confidence: inferred
  path: website/docs/guides/read-a-run-report.md
revision: 9
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
  outcome (`crates/takt/src/events.rs`, `Event::Stop`); the unit counts on the day of the run
  that wrote that `stop` line.
- tokens: the sum over the case's `step` lines of `tokens`, by the rule `takt scorecard` reads
  them with (`crates/takt/src/scorecard.rs`, `runs`, shared rather than written twice): a
  harness step line with no count (story:a-killed-step-records-its-tokens) or a count of 0 makes
  the sum `unknown`; a validator's step line carries no count and adds 0; `skipped` lines and
  lines after a run's `stop` are not read. The `stop` line's `tokens` is not read: it is always
  a number (`Event::Stop { tokens: u64 }`) and counts an unreported step as 0.
- interventions: decisions by a person, one per entry in the case's `authority.yaml` whose
  approver is `human:`, granted or denied; a grant by an agent role
  (story:an-unattended-run-merges-under-a-delegated-grant) is not an intervention.
- the day: the UTC date of the run's `opened` line; the repository: the directory above the
  cases root, `<repo>/.takt/cases` or the legacy `<repo>/.engineering/cases` (`CASE_ROOTS`,
  `crates/takt/src/case.rs`), or the `--cases` argument as given.
- output: text. A scenario case's held-out result does not change "accepted" here.

`takt report` today reads run traces (`crates/takt/src/report.rs`); `--cases` is a second input
of the same command (one more member of `ReportArgs`' `input` group), not a new command. Defects
escaped, the fourth number of the week, stay with story:week-of-daily-runs.

## Scope

- `crates/takt/src/report.rs`: the case sums
- `crates/takt-cli/src/main.rs`: `ReportArgs`, `--cases`
- new: `crates/takt/tests/report_cases.rs`, with recorded case directories as fixtures

## Acceptance

Over two fixture cases directories (repository `a`, under `<a>/.engineering/cases`: one case
accepted in one run, one case with two runs and one approval by `human:`; repository `b`, under
`<b>/.takt/cases`: one case stopped on a budget), `takt report --cases <a>/.engineering/cases
<b>/.takt/cases` exits 0 and prints one row per repository and day with units accepted, tokens
and interventions equal to a hand count of the fixtures, and a total row equal to their sum.
Repository `b`'s case holds a harness `step` line with no `tokens`, beside a harness step line
with a count and a `stop` line whose `tokens` is that count: `b`'s row and the total row print
tokens as `unknown`, never the stop line's number, while `a`'s row, whose validator step lines
carry no `tokens`, prints its sum. A grant by `agent:takt-delivery` in `a` is not counted as an
intervention.
