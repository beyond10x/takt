---
title: Read a case
sidebar_position: 5
description: After a run, find out from the case directory what happened, step by step, why it stopped and what the model was told, with jq, takt scorecard and takt view.
lede: Everything a run did is in its case directory and in the transcripts beside it, so the questions after a run are answered by reading files, not by asking the model.
source: crates/takt/src/case.rs, crates/takt/src/events.rs; takt scorecard, trace and view --help at 0.14.0; a story case of takt 0.14.0 on a scratch repository, 2026-10-26; the layout of a 12-run case from a cluster trial of takt 0.13.1, read, not copied
---

# Read a case

A case is the directory `.engineering/cases/<case>/` in the repository. Every run of the case
appends to it, and every file is written by takt, a validator or `takt approve`, never by hand.
What each file holds is in [Case files](../reference/case-files.md). This page is about the
questions you ask after a run.

## What it holds after one story run

The story run of [Run a story](./run-a-story.md), seven steps:

```console
$ cd .engineering/cases/story-add-sub-20261026T231852Z
$ ls -1
artifacts
branch.yaml
case.yaml
commit-identity
decisions
events.jsonl
evidence
intent.md
observations.json
protocol.ir.json
protocol.yaml
python-modules.json
story.yaml
$ ls -1 evidence
0001-test-result-1.json
0002-gate-result-2.json
0003-test-result-4.json
0004-gate-result-5.json
0005-handover-6.json
0006-change-record-7.json
```

A case grows with its runs. One case from a cluster trial held twelve runs: 12 `opened` lines, 101
steps, 101 decision files, 70 evidence records (20 test results, 20 gate results, 15 handovers and
15 change records), 20 gate bundles under `artifacts/`, and 12 endings, 5 `stop` lines and 7
`error` lines. The layout was the same as above.

## Which steps ran, and who took each

`events.jsonl` is the run record, one JSON object per line
([events.jsonl](../reference/events-jsonl.md)):

```console
$ jq -r 'select(.event=="step") | "\(.step) \(.by) \(.action)"' events.jsonl
1 validator tests.run
2 validator gate.run
3 harness implementation.edit
4 validator tests.run
5 validator gate.run
6 harness change.handover
7 validator change.read
```

`by` is `validator` for takt's own checks and `harness` for a model's step. A harness step's line
also names its model, tokens, turns and the transcript.

## What evidence there is

```console
$ jq -r 'select(.event=="evidence") | "\(.step) \(.kind) \(.result)"' events.jsonl
1 test_result pass
2 gate_result pass
4 test_result pass
5 gate_result pass
6 handover null
7 change_record ahead
```

Evidence comes only from validators. The model's account of its own work is in the step's
`summary`, and nothing reads it as evidence
([Evidence and the judge](../concepts/evidence-and-the-judge.md)).

## Why it stopped

```console
$ jq -c 'select(.event=="stop")' events.jsonl
{"at":"2026-10-26T23:19:17.223529Z","step":7,"event":"stop","stop":"Unperformable (pr.opened; nothing performs it without the forge)","steps":7,"tokens":75939,"completed":false}
```

A run that ended on an error writes an `error` line instead. Each `opened` line starts a run, so
the number of runs is the number of `opened` lines. What each stop means and what the next run
does: [How a run ends](../concepts/how-a-run-ends.md).

## Why an action was or was not taken

`decisions/NNNN.json` holds one evaluation by the governor: every action's status
(`admissible`, `approval-required` or `blocked`), and for a blocked one the claims that block it:

```console
$ jq -c '.actions["pr.open"]' decisions/0003.json
{"reasons":[{"claim":"change.handed_over","value":"unknown"},{"claim":"change.made","value":"unknown"}],"status":"blocked"}
```

## Every run of the repository at once

Back in the repository's root:

```console
$ takt scorecard --repo .
1 cases, 1 runs under .engineering/cases; definitions: errors v1, no_progress v1, outcome v3, refusals v1, step_tokens v2, steps v1, tokens v2, wall v1
case                                scenario  outcome        stop           held_out  steps  tokens  refusals  no_progress  errors  wall_s  notes
story-add-sub-20261026T231852Z      -         Unperformable  -              -         7      75939   0         0            0       24.870  -
  story-add-sub-20261026T231852Z#1  -         Unperformable  Unperformable  -         7      75939   0         0            0       24.870  -
layers  takt 9293a4b28f13a62545e51be1bca1327f552dacfb; harness <version>; model example-model  (1 run)
```

(The harness version and model name were replaced.) One row per case, then one per run.

## What the model was told and did

A harness step's transcript is outside the repository, because it holds what the model read:

```text
$XDG_STATE_HOME/takt/transcripts/<repo-key>/<case>/run-<r>/step-<n>.jsonl
```

Without `XDG_STATE_HOME`, that is `~/.local/state/takt/transcripts/`. The step's line in
`events.jsonl` names the path below that directory. `takt view` draws runs on one time axis and
opens a step's transcript on a click. It reads `takt-trace/1` files and transcripts laid out as
above under one directory:

```sh
mkdir runs
takt trace --case .engineering/cases/story-add-sub-20261026T231852Z --out runs/add-sub.trace.jsonl
cp -r ~/.local/state/takt/transcripts/<repo-key> runs/
takt view runs --output takt-view.html
```

```console
written: runs/add-sub.trace.jsonl (8 operations, takt-trace/1)
written: takt-view.html (1 runs, 8 operations, 2 of 2 harness steps with a transcript)
```

The page is one offline file, written with mode 0600, because it holds the transcripts. Where the
time and tokens went: [Read a run report](./read-a-run-report.md).

:::caution[On a cluster]

Under Loom, `HOME` is an emptyDir, so the transcripts go when the pod goes. Copy them out before
the pod is replaced. A run-state directory you can name is planned
([Known limitations](../known-limitations.md#takt)).

:::
