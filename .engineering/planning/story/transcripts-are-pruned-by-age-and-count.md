---
format: aep.planning-md/3
id: story:transcripts-are-pruned-by-age-and-count
kind: story
status: draft
title: takt run removes a repository's transcripts older than 30 days and beyond its 20 newest cases
summary: step transcripts accumulate under the state directory with no retention, up to 16 MiB per step
tags:
- finding
- rsi
refs:
- provider: finding
  reference: transcripts-are-pruned-by-age-and-count
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: transcripts-are-pruned-by-age-and-count

## Outcome

`takt run` bounds the transcripts directory. Before it keeps a step's transcript, it removes,
for the repository it runs on, every case's transcripts older than 30 days and all but the 20
newest cases' transcripts. The case it runs is never removed, and a removal it cannot make is
reported once on standard error without stopping the run.

## Context

Found by the wave 15 close review at 0fae6305 (docs/waves/wave-15.md). `keep_transcript`
(`crates/takt/src/harness.rs:383` at 0fae6305) writes every harness step's output to
`$XDG_STATE_HOME/takt/transcripts/<repo key>/<case>/run-<n>/step-<k>.jsonl`
(`default_transcripts`, `crates/takt/src/run.rs:6084`). One file holds at most `TRANSCRIPT_CAP`,
16 MiB (`harness.rs:103`). Nothing ever removes one. A scenario set of five trials of about ten
harness steps can add up to 800 MiB, and every trial, promotion run and check adds more. The
directory is outside every repository and case directory, so cleaning a case does not reach it.

The limits (30 days, 20 cases a repository) are a proposal; the planner may set others. The
predicate holds under any age limit below about 21 months, any count limit below 41 cases a
repository, and any size limit below 640 MiB that removes the oldest first. It plants 40 cases'
transcripts dated 2025-01-01, 16 MiB each (sparse, so no disk is used), under the repository's
key, then runs one harness step with a `claude` that fails. Exit 9 says the oldest planted
transcript is still there after the run. Exit 0 says the run removed it.

## Acceptance

1. A run that keeps a transcript removes the repository's transcripts of cases older than the age
   limit and beyond the count limit, keeps the current case's and other repositories' newer ones:
   a test on a planted directory.
2. A removal that fails (a file not the user's, a link) is said on standard error and the run goes
   on; nothing outside the transcripts directory is followed or removed: a test.
3. At the candidate, `scripts/check-finding story:transcripts-are-pruned-by-age-and-count
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: set -e; k=$(printf %s "$(pwd -P)" | sha256sum | cut -c1-16); r="$HOME/.local/state/takt/transcripts/$k"; old="$r/takt-20250101T000010Z/run-1/step-2.jsonl"; for i in $(seq 10 49); do mkdir -p "$r/takt-20250101T0000${i}Z/run-1"; truncate -s 16M "$r/takt-20250101T0000${i}Z/run-1/step-2.jsonl"; done; chmod -R go-rwx "$HOME/.local"; find "$r" -mindepth 1 -exec touch -d 2025-01-01T00:00:00 {} +; test -e "$old"; mkdir -p "$HOME/bin"; printf '#!/bin/sh\necho {}\nexit 1\n' > "$HOME/bin/claude"; chmod +x "$HOME/bin/claude"; set +e; PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 3; test ! -e "$old" || { echo "a transcript of 2025-01-01 is still kept, beside $(ls "$r" | wc -l) cases" >&2; exit 9; }
exit: 9
healthy_exit: 0
output: a transcript of 2025-01-01 is still kept
source: docs/waves/wave-15.md
```
