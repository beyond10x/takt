---
format: aep.planning-md/3
id: story:run-trace-and-report
kind: story
status: active
title: takt trace writes one takt-trace/1 file per run; takt report prints where its time and tokens went
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:step-records-per-call-usage
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T22:43:23Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T22:43:23Z", actor: "human:timo", revision: 3}
---
## Outcome

`takt trace --case <dir>` writes the run as one `takt-trace/1` JSON Lines file: a header (run, arm, harness, model, revision, binary, task, start, end, stop, tokens in four components, cost, acceptance) and one operation per line (order, step, actor, intent, kind, action, start, end, tokens, cost, outcome, paths, evidence observed or declared, pattern label). `takt report --case <dir>` prints, from that trace, the seconds and share per actor (harness, validator, forge, takt, human, idle), tokens and cost per model call, steps, resumes, acceptance, every refusal, NoProgress, error and pattern label, and a list of what the sources did not record. Every token and cost figure comes from the raw usage; command text is classified, never copied, so a trace is publishable. Traces are committed under `docs/metrics/traces/<run>.jsonl`.

## Context

From the owner's ask of 2026-10-19 (several reporters over one trace, the journey image on the public site, a report per run, more self-improvement on the site, the beyond10x docs style with a sidebar) and the read-only design written for it the same day. The run records today are made by hand (`docs/runs/2026-10-18-trial-1-worktree.md`, `docs/runs/2026-10-18-efficiency-analysis.md`), and `docs/metrics/journey.py` classifies steps in Python (`journey.py:73-103`, `:553-570`). The two-level vocabulary settles the UNMAPPED notes on `takt.work.Trace` (`ess/domains/takt.yaml`): `intent` is story:normalized-harness-trace's set (inspect, change, verify, research, wait, coordinate) and `kind` is `takt.work.JourneyKind`. A trace is a file per run, not part of the case directory, because free-running arms have no case. Runs recorded before story:step-records-per-call-usage are read with an optional `--steps <shim file>`. story:week-of-daily-runs' `takt report --cases` becomes a sum over the same module.

## Scope

- `crates/takt/src/trace.rs` (new): `takt-trace/1` from `events.jsonl` (inferred)
- `crates/takt/src/report.rs` (new): attribution and defects (inferred)
- `crates/takt/src/lib.rs`, `crates/takt-cli/src/main.rs`: the two subcommands (inferred)
- `ess/domains/takt.yaml`: `Trace`, `TraceOperation` fields and the vocabulary (cited)
- `crates/takt/tests/trace.rs` (new): fixtures from the T-24 and trial-1 `events.jsonl`, paths scrubbed (inferred)
- `docs/metrics/README.md`: the format (inferred)

## Acceptance

On trial 1's case `takt report` prints validators 107 s, model calls 130 s and other 5 s, the figures of `docs/runs/2026-10-18-trial-1-worktree.md`; on T-24's case the trace holds 18 operations, 2 `idle` operations of 30 s and 3 model calls with four token components; for both, the trace's token sum equals `takt scorecard`'s for the same case; no trace line contains a home-directory path.
