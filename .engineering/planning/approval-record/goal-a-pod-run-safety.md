---
format: aep.planning-md/3
id: approval-record:goal-a-pod-run-safety
kind: approval-record
status: approved
title: 'The pod-run safety wave: budget stop, forge safety, report --cases, and the next release'
tags:
- loom-agent
relations:
- decides: story:bench-subset-and-budget
- decides: story:a-run-stops-at-a-default-token-budget
- decides: story:forge-preflight
- decides: story:fenced-forge-dispatch
- decides: story:report-sums-cases
- decides: story:a-turn-pins-the-repository-of-its-forge
- decides: task:ess-0-77-0-and-loom-0-50-1
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-29T21:22:41Z", actor: "human:timo", revision: 2}
---
# Approval: the pod-run safety wave, and the next release

## Decision

The orchestrator, under the owner's grant (class C: wave, bot commits, pull request, merge,
release, dependency upgrade), 2026-10-29: "G3 first, before any pod run. Add a hard budget stop
for a pod run: `story:bench-subset-and-budget`, `--usd` / token cap. Default cap: 3,000,000
tokens per run, which stops the run and records why. Reason: the 2026-10-26 S7 hour used
17,200,642 tokens and accepted 0." "G4. Forge safety in the pod: story:fenced-forge-dispatch,
story:forge-preflight, story:a-turn-pins-the-repository-of-its-forge." "G6. `takt report --cases`
(story:week-of-daily-runs) prints units accepted, tokens and interventions." "Run G2, G3, G4 and
G6 as one takt wave, or two if the 4-unit cap forces it. Move ESS to 0.77.0 and loom to 0.50.1
in that wave. Release takt."

## How it is cut

- The default token cap is its own story, story:a-run-stops-at-a-default-token-budget, delivered
  in one unit with story:bench-subset-and-budget's `--usd`.
- story:week-of-daily-runs needs a week of scheduled runs before its acceptance can be checked;
  its counting command is story:report-sums-cases, on which it now depends.
- Wave 1 (4 units): the budget unit; story:forge-preflight; story:fenced-forge-dispatch;
  story:report-sums-cases. The pins ride along as task:ess-0-77-0-and-loom-0-50-1. Wave 2:
  story:a-turn-pins-the-repository-of-its-forge, which concerns improvement turns, not the
  delivery runs a pod makes.
- The stories the second kind-cluster trial filed (approval-record:wave-38-trial-s7-defects) are
  in the store under their own ids and implemented since release 0.14.0 and 0.15.0; none is filed
  again.
