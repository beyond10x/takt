---
format: aep.planning-md/3
id: story:promotion-survives-a-kill
kind: story
status: draft
title: A promotion killed mid-run does not block the next one
summary: a stale .tmp-<pid>, a scorecard copy without its record and a record whose binary was never installed are removed by the next takt promote, which names them and proceeds
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:promotion
- depends_on: story:installed-binary-checked-by-its-digest
scope:
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 6
---
# Story: promotion-survives-a-kill

## Outcome

A promotion killed mid-run does not block the next one. Each state a kill leaves in the store is recovered from by the next `takt promote`, which names what it removed and proceeds, instead of refusing every later promotion:

- a temporary file of a process that is gone;
- a scorecard copy without its record;
- a record whose binary was never installed.

Each recovery does what the same promotion would have done had it failed in-process rather than been killed: `write` removes the copy when the record cannot be published (`crates/takt/src/promotion.rs:1392-1396`), and `unwrite` removes the record and its copy and points the head back when the install fails (`:1404`).

## Context

Split from story:promotion-records-its-toolchain in plan-critic round 1 (review-result:frontier-1-design-round-1).

- Low C4 of the reviewer's verdict on story:promotion at 2d10b7f: a `.<name>.tmp-<pid>` left by `publish()` when takt is killed makes `records()` refuse every promotion (inferred from the code, not observed).
- Finding P1.5 of the review of main at 9cc158b: a kill between publishing the scorecard copy and publishing the record leaves an orphan copy that `records()` refuses; a kill after the record is written and before the install finishes leaves a record whose binary was never installed.

At 338082d:

- `publish` writes `.<name>.tmp-<pid>` and renames it (`promotion.rs:1347-1372`, the head's at `:404-420`).
- `records()` refuses any file that is neither a record nor a scorecard copy (`:654-660`).
- `write` publishes the copy, then the record, then moves the head `promotions.head` to it (`:1384-1388`).
- The record carries the staged binary's `binary_sha256` before it is written (`:2874-2875`), and the install follows the record (`:2876`).
- `installed_names_latest` (`:2008-2069`) then refuses the binary that is still installed: it prints the previous record's revision, and `printed != latest.revision` is refused (`:2060-2067`).

How a never-installed record is told apart from a hand-replaced binary. The latest record carries `binary_sha256`; a record a promotion writes always does (`:2874`). Three cases, read in this order:

1. The installed binary hashes to the latest record's `binary_sha256`: it is installed. Nothing to recover.
2. It hashes to the `binary_sha256` of the record before it: the install never happened. That record is removed.
3. It hashes to neither, or the record before carries no digest: the binary is not one the store installed. The refusal of story:installed-binary-checked-by-its-digest stands, naming the digests, and nothing is removed.

A printed revision decides none of the three, since a replacement can print any revision.

## Ordering

- story:installed-binary-checked-by-its-digest goes first (`depends_on`). It replaces `installed_names_latest`'s `printed != latest.revision` check (`:2060-2067`) with a check by digest for a record that carries one. This story's row 3 adds case 2 above, before that check refuses.
- `crates/takt/src/promotion.rs`, `crates/takt-cli/tests/promote_cli.rs` and `ess/domains/takt.yaml` are shared with story:installed-binary-checked-by-its-digest, which this story follows (`depends_on`). They are also shared with story:promotion-records-its-toolchain, whose hunks are the record's fields and the build's toolchain (`:173-233`, `:1473-1494`, `:1938-1960`). There is no edge to that story, and `aep plan artifact waves` keeps the two apart.
- `ess/domains/takt.yaml` and `ess/domains/takt.citations.tsv` are also wave 39's. Wave 39 only re-points `read from:` citations, far from the promotion rules (`takt.yaml:5655-5665`, `:5834`), so the citations are re-pointed at merge (`python3 -I scripts/check-citations --require-digests`).

## Expected before

On takt at 338082d, holds; after the change, fails:

- In a scratch store holding a valid chain and `.<revision>.json.tmp-<pid>` of a pid that is gone, `takt promote --dry-run` exits non-zero with "not a record", naming that file.

## Scope

- `crates/takt/src/promotion.rs`: `records()`, `publish`, `write`, `installed_names_latest` (case 2 above) and the removal of a never-installed record with the head pointed back (cited, `:654-660`, `:1347-1399`, `:2008-2069`)
- `ess/domains/takt.yaml`: the store's recovery rule, beside the installed-binary rule (inferred, `:5655-5665`)
- `ess/domains/takt.citations.tsv`: the citations the new lines need (inferred)
- `crates/takt-cli/tests/promote_cli.rs`: the rows below (inferred)

## Acceptance

In `crates/takt-cli/tests/promote_cli.rs`, each store state below is prepared as a `kill -9` in its window leaves it, with no kill in the test. The store starts from a valid chain whose latest record P carries `binary_sha256`, with P's binary installed.

1. A `.<name>.tmp-<pid>` whose pid is gone: the next `takt promote` removes the file, names it, and proceeds. One whose process is alive still blocks, naming the pid, and the file stays.
2. A scorecard copy `<revision>.scorecard.json` with no record: the next `takt promote` removes the copy, names it, and proceeds.
3. A record N after P, carrying a `binary_sha256` other than P's, with `promotions.head` naming N (and, as a second case, still naming P), and P's binary still installed. The next `takt promote` names N as never installed and proceeds against P. Afterwards the store holds no `N.json` and no N scorecard copy, `promotions.head` names P, and the store holds the new promotion's record after P.
4. As row 3, but the installed binary is replaced by one that hashes to neither N's nor P's digest. `takt promote` exits non-zero naming both N's digest and the installed one, and the store still holds N with `promotions.head` unchanged.

With `--dry-run`, rows 1 to 3 each name what would be removed, and the store and the head are unchanged.

Before the change, rows 1 to 3 each exit non-zero.
