---
format: aep.planning-md/3
id: story:check-finding-run-step-confined
kind: story
status: active
title: check-finding runs the predicate with the host read-only
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/tests/finding_scripts.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: scripts/check-finding
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T06:11:34Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-19T06:11:34Z", actor: "human:timo", revision: 7}
---
## Outcome

`scripts/check-finding` runs a finding's predicate command, and so the candidate `$TAKT` it calls, with the host file system read-only except the check's own scratch directory: the repository, the kept `builds/<sha>/takt`, the protocol sources by their real path and the caller's home cannot be written, and the caller's home cannot be read. A process the command leaves behind cannot reach the next check: the run step has its own PID namespace, and bwrap's status reaches the script through a pipe, not a file on disk.

## Context

From the grant reviews of story:check-finding-builds-confined (wave 13, E1, 2026-10-19). Round 1, finding 2: the run step binds `--dev-bind / /` (`scripts/check-finding:410-421`), so a predicate overwrote another revision's kept `builds/<sha>/takt`, wrote a file into the repository, changed `protocol-sources/<id>/protocol.yaml` through its real path and read a secret planted in the caller's home; the build step is confined, the run step is not, and the candidate binary runs in the run step. Correction 1, finding A: a process left running by an earlier check appended a fake `{"exit-code": 1}` line to the next check's status file by path (`:424`, `:429-430`), and that check printed `reproduced` although its bwrap failed setup (2 of 2). Findings B-D of the same round ride here: the comment at `:416-420` overstates what the exit-code line proves; mutants M8b (the line present but not compared with the status) and M8d (`head` for `tail`) survive; `crates/takt-cli/tests/finding_scripts.rs:1013` still prints lines 2 onward of a multi-line environment value.

## Scope

Derived 2026-10-19 by `story-scoper`, read-only at `wave/2026-10-19-w13` bbe5bdb (`scripts/check-finding`, `finding_scripts.rs` identical to 3224311). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `scripts/check-finding`, the run step — cited
- **Files:** `scripts/check-finding:409-433@3224311` (environment 409-410, comment 411-420, bwrap with `--dev-bind / /` and `--json-status-fd 3` to `$run/bwrap-status` 421-425, status read 426-433) — cited
- **Files:** `scripts/check-finding:361-376@3224311` (fresh home, the protocol-sources bind of the caller's real path) and the header comment at 44-70, which describes the run step as not confined — cited
- **Files:** `crates/takt-cli/tests/finding_scripts.rs:1013-1018@3224311` (`attempts` keeps lines 2+ of a multi-line `env` value); new cases beside `a_bubblewrap_that_fails_before_the_command_runs_gives_no_result` (1141-1199) and `the_predicate_cannot_write_the_callers_protocol_sources` (1267) — cited
- **Symbols:** `environment`, `binds`, `bound`, `status`, `ended` — cited
- **Also likely:** `ess/domains/takt.citations.tsv`: the `takt.work.Finding` citation spans `check-finding:184-399`, which covers the bind block at 361-376 — inferred
- **Confidence:** high
- **Would collide with:** any unit touching `scripts/check-finding` or `crates/takt-cli/tests/finding_scripts.rs`
- **Safety fact:** the kept `builds/<sha>/takt` runs only through `$TAKT` in the run step (`check-finding:409,421-425`); the build step runs cargo, never the binary — level 2, unproven

## Acceptance

A predicate that writes into `~/.cache`, the repository, `builds/<sha>/takt` and a protocol source's real path gets a read-only error on each and the check still gives its result; a predicate that reads a file planted in the caller's home does not see it; a process started by one check and still alive cannot change the next check's result; M8b and M8d are killed.
