---
format: aep.planning-md/3
id: story:the-gate-reads-command-strings-other-programs-split
kind: story
status: draft
title: The gate reads the command strings env -S, flock -c and su -c run
summary: env -S, flock -c, su -c and git -C are not read by the gate's argument reader
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: the-gate-reads-command-strings-other-programs-split

## Outcome

A gate command run through a program that splits a command string itself (`env -S '<line>'`, `flock -c`, `su -c`,
`git -C <dir>`) has the paths in that string compared at the base, or is refused at open, naming the command.

## Context

Found by unit a of wave 55 and its adversary pass at d0e8fba54: story:the-gate-reads-paths-inside-arguments reads the
words of a shell's `-c` line, but not the strings `env -S`, `flock -c` and `su -c` run, nor a directory `git -C` moves
to. The step guard's reader in `crates/takt/src/run.rs` (`read_line`) does not read glued short option tails either.

## Acceptance

- A gate command `env -S 'sh scripts/check.sh'` whose script a step edited is refused before the gate runs, naming the
  script, or refused at open: a test.
- The same for `flock <lock> -c '<line>'`: a test.
