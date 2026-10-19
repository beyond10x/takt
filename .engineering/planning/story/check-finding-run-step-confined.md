---
format: aep.planning-md/3
id: story:check-finding-run-step-confined
kind: story
status: draft
title: check-finding runs the predicate with the host read-only
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
## Outcome

`scripts/check-finding` runs a finding's predicate command, and so the candidate `$TAKT` it calls, with the host file system read-only except the check's own scratch directory: the repository, the kept `builds/<sha>/takt`, the protocol sources by their real path and the caller's home cannot be written, and the caller's home cannot be read. A process the command leaves behind cannot reach the next check: the run step has its own PID namespace, and bwrap's status reaches the script through a pipe, not a file on disk.

## Context

From the grant reviews of story:check-finding-builds-confined (wave 13, E1, 2026-10-19). Round 1, finding 2: the run step binds `--dev-bind / /` (`scripts/check-finding:410-421`), so a predicate overwrote another revision's kept `builds/<sha>/takt`, wrote a file into the repository, changed `protocol-sources/<id>/protocol.yaml` through its real path and read a secret planted in the caller's home; the build step is confined, the run step is not, and the candidate binary runs in the run step. Correction 1, finding A: a process left running by an earlier check appended a fake `{"exit-code": 1}` line to the next check's status file by path (`:424`, `:429-430`), and that check printed `reproduced` although its bwrap failed setup (2 of 2). Findings B-D of the same round ride here: the comment at `:416-420` overstates what the exit-code line proves; mutants M8b (the line present but not compared with the status) and M8d (`head` for `tail`) survive; `crates/takt-cli/tests/finding_scripts.rs:1013` still prints lines 2 onward of a multi-line environment value.

## Scope

- `scripts/check-finding`: the run step's bwrap arguments and the status channel (cited)
- `crates/takt-cli/tests/finding_scripts.rs`: the new cases and the assertion message (cited)

## Acceptance

A predicate that writes into `~/.cache`, the repository, `builds/<sha>/takt` and a protocol source's real path gets a read-only error on each and the check still gives its result; a predicate that reads a file planted in the caller's home does not see it; a process started by one check and still alive cannot change the next check's result; M8b and M8d are killed.
