---
format: aep.planning-md/3
id: story:view-joins-own-transcripts-and-writes-a-private-page
kind: story
status: draft
title: takt view joins each step to its own case's transcript and writes a private page
summary: transcripts joined by path suffix; an existing page keeps its mode; a linked output is written through
tags:
- finding
- process
refs:
- provider: finding
  reference: view-joins-own-transcripts-and-writes-a-private-page
relations:
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: view-joins-own-transcripts-and-writes-a-private-page

## Outcome

`takt view` shows a step's transcript only when it belongs to that step's own case, and writes
its page as a new file readable by its owner only: never through a link, never by keeping an
older file's mode, and never leaving a truncated page behind when the write fails. Its help names
every directory it skips.

## Context

Found by the wave 17 review at ef8609da (docs/waves/wave-17.md), left for later. At 00a900ac:

- The join. A trace operation's own `transcript` field, or the record's, is looked up with
  `Path::ends_with` over every transcript under the directory (`named`,
  `crates/takt/src/view.rs:314`; used at `view.rs:341` and after). An empty field ends every path,
  and a bare `step-2.jsonl` ends every case's step 2. Either one joins the first transcript found,
  another case's, to the operation. The layout fallback is keyed by case, run and step and is
  not affected.
- The page. `write` (`view.rs:443`) opens the output with `create`, `truncate` and `mode(0o600)`
  (`view.rs:452`). The mode applies only to a file it creates, so an existing `0644` page stays
  `0644` with transcripts in it. `open` follows a link, so a linked `--output` writes the page
  wherever the link points. A failed `write_all` leaves the file truncated.
- The help. `ViewArgs` (`crates/takt-cli/src/main.rs:253`) says `.git` and `target` are skipped;
  `SKIPPED_DIRS` (`view.rs:37`) skips `node_modules` too.

The predicate runs one case with a `claude` double, traces it into `x/`, copies its transcripts,
and adds two copies of the trace under other case ids: `y/` with `"transcript":""` and `z/` with
`"transcript":"step-2.jsonl"` on the harness step. It renders onto an existing `0644` page and
through a link, then reads the help. Healthy is "1 of 3 harness steps with a transcript", a page of
mode `0600` (or none written), the link's target left empty, and `node_modules` in the help. Exit 9
lists each defect still there. The truncated page is not in the predicate. A write failure cannot
be made in the check's sandbox without a full file system.

## Acceptance

1. An operation's transcript field joins only a transcript of the same case (and run and step); an
   empty field or a bare file name joins none: a test.
2. The page is written to a new file of mode `0600` beside the output and renamed over it; an
   existing file of another mode ends `0600`, a linked `--output` is refused or replaced, never
   written through, and a failed write leaves the earlier file as it was: tests.
3. `takt view --help` names every directory the walk skips.
4. At the candidate, `scripts/check-finding
   story:view-joins-own-transcripts-and-writes-a-private-page <candidate>` prints
   `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/v/x" "$HOME/v/y" "$HOME/v/z"; printf '%s\n' '{"result":"{\"action\": \"implementation.edit\", \"summary\": \"an edit\"}","usage":{"input_tokens":1}}' > "$HOME/answer.json"; printf '%s\n' '#!/bin/sh' 'echo the work >> work.txt' "cat $HOME/answer.json" > "$HOME/bin/claude"; chmod +x "$HOME/bin/claude"; PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 2 > /dev/null 2>&1; c=$(ls .engineering/cases); "$TAKT" trace --case ".engineering/cases/$c" --out "$HOME/v/x/trace.jsonl" > /dev/null; cp -r "$HOME/.local/state/takt/transcripts" "$HOME/v/"; sed -e "s/$c/takt-20000101T000000Z/g" -e 's/"actor":"harness",/"actor":"harness","transcript":"",/' "$HOME/v/x/trace.jsonl" > "$HOME/v/y/trace.jsonl"; sed -e "s/$c/takt-20000101T000001Z/g" -e 's/"actor":"harness",/"actor":"harness","transcript":"step-2.jsonl",/' "$HOME/v/x/trace.jsonl" > "$HOME/v/z/trace.jsonl"; : > "$HOME/page.html"; chmod 644 "$HOME/page.html"; : > "$HOME/target.html"; ln -s "$HOME/target.html" "$HOME/link.html"; "$TAKT" view "$HOME/v" --output "$HOME/page.html" > "$HOME/view.out" 2>&1; sed "s|$HOME|~|g" "$HOME/view.out"; "$TAKT" view "$HOME/v" --output "$HOME/link.html" > /dev/null 2>&1; d=; grep -q '1 of 3 harness steps with a transcript' "$HOME/view.out" || d="$d a transcript joined to another case;"; [ -s "$HOME/page.html" ] && [ "$(stat -c %a "$HOME/page.html")" != 600 ] && d="$d an existing page kept mode $(stat -c %a "$HOME/page.html");"; [ -s "$HOME/target.html" ] && d="$d the page written through a link;"; "$TAKT" view --help | grep -q node_modules || d="$d help omits node_modules;"; [ -z "$d" ] || { echo "takt view still:$d" >&2; exit 9; }
exit: 9
healthy_exit: 0
output: takt view still:
source: docs/waves/wave-17.md
```
