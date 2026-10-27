---
format: aep.planning-md/3
id: approval-record:live-trial-setup
kind: approval-record
status: accepted
title: A live trial installs the released takt on its own PATH and clones fresh each time
summary: release asset on the trial PATH only, a fresh clone per trial, approve on the case branch
relations:
- decides: epic:downstream-story-delivery
revision: 3
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T17:55:33Z", actor: "human:timo", revision: 3}
---
Decider: the operator's delegate for the live trials, 2026-10-27 16:03Z, recorded by the trial-review
coordinator.

How a live trial of takt on todo-app-example is set up, decided after the trial agent named three gaps
in run-a-story's trial section (no install step, whether a clone is reused, which branch `takt approve`
needs):

- **The released binary, on the trial's `PATH` only.** The trial installs the release asset (archive
  checked against `SHA256SUMS`) into a directory of its own and puts that directory first on the
  trial's `PATH`. No other session's `takt` changes.
- **A fresh clone per trial.** Trials 2 and 3 clone again. A trial leaves its clone on the case's branch
  with the case directory in it, and `takt scenario run` needs a clean tree.
- **Approve as the stop says.** `takt approve` runs in the clone the run stopped in, on the case's branch
  `takt/scenario-1-trial-<n>`, clean.

The docs follow this: website/docs/guides/run-a-story.md § "A trial on a GitHub repository" and
website/docs/guides/install.md, at 0.15.1.
