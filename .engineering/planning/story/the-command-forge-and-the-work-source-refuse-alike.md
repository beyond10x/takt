---
format: aep.planning-md/3
id: story:the-command-forge-and-the-work-source-refuse-alike
kind: story
status: draft
title: The command forge and commit text refuse what the work source refuses
summary: cut forge output, U+2028/9 in commit text, and the tree after a refused --work
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:work-sources
- informed_by: story:the-command-work-source
revision: 1
---
# Story: the-command-forge-and-the-work-source-refuse-alike

## What is wrong

The wave 56 correction round hardened the command work source, and three places with the same input did not follow:

- The command forge (`crates/takt/src/forge/command.rs`) still reads a child's standard output after the runner cut it
  to its last 8 MiB, where the work source now refuses it, naming the limit and the command.
- `crates/takt/src/commit_style.rs` and `crates/takt/src/git.rs` refuse control characters in commit text but take
  U+2028 and U+2029, which a work item's title now may not hold.
- After `--work` is refused on the read path, the tree stays on the base and is not put back where it was, as a refused
  `--story` puts it back.

Found by the wave 56 correction round.

## Outcome

The command forge refuses cut output as the work source does; commit text refuses U+2028 and U+2029; a refused
`--work` leaves the tree where it found it.

## Acceptance

- A forge command printing more than the runner keeps is refused, naming the limit and the command: a test.
- A commit message or subject holding U+2028 is refused: a test.
- A `--work command:<id>` refused after `read` leaves the tree on the branch it started on: a test.
