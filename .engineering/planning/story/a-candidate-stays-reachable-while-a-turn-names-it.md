---
format: aep.planning-md/3
id: story:a-candidate-stays-reachable-while-a-turn-names-it
kind: story
status: draft
title: A delivery's candidate stays reachable while an open turn names it
summary: deleting the candidate ref on a failed resume lets gc remove the candidate an open turn names
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-candidate-stays-reachable-while-a-turn-names-it
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: a-candidate-stays-reachable-while-a-turn-names-it

## Outcome

A candidate an open improvement turn names stays in the repository for as long as the turn is
open. A later resume of the delivery case that cannot make a new candidate may drop the case's
candidate ref, but the commit the turn was opened on is still reachable, so `git gc` does not
remove it and the turn can still check, merge and promote what it was granted on.

## Context

Found by the wave 16 re-check at 622e8a0d (docs/waves/wave-16.md). A delivery that stops at
`pr.opened` for want of a forge hands its change over as a candidate
(`crates/takt/src/run.rs:7222-7228` at 00a900ac). The candidate is a commit replayed on the
base's tip, and the only thing that names it is `refs/takt/candidates/<case id>`
(`CANDIDATE_REFS`, `run.rs:195-196`). When a later resume of that case cannot make a candidate
(the replay conflicts, no identity), `Run::candidate` deletes that ref so that no ref names a
candidate the refusal calls none (`run.rs:6517-6527`, `update-ref -d` at `run.rs:6521`).

A turn opened on the earlier candidate records it only as an id in its case files
(`TurnOpening::candidate`, `crates/takt/src/case.rs:2376-2380`). No ref holds it. Once the reflog
expires and `git gc` runs, the commit is gone, and the turn's later steps (`scenario.verify`,
`candidate.merge`, `binary.promote`) name an object the repository no longer has. The re-check
saw this on a live probe: `git fsck` reported the candidate unreachable, and after gc
`git cat-file` exited 128.

The predicate builds the delivery world of `crates/takt-cli/tests/delivery_candidate_cli.rs` from
takt's own tree at the seed: the `one-ready` fixture, a promotion record of the binary under
test, and a `claude` double. A delivery stops handing over its candidate. `main` moves by an
unrelated file and the case is resumed, so the candidate is replayed into a commit that only the
ref names. A turn opens on that commit. `main` then moves by a conflicting `work.txt`, and the
case is resumed again, so `candidate: none` and the ref is deleted. Finally the reflog is
expired and `git gc --prune=now` runs. Exit 9 means the turn's candidate is gone. Exit 0 means
it is still there. Exit 2 means the world was not built (no candidate, or no turn), which
verifies nothing.

## Acceptance

1. While a turn that names a candidate is open, the candidate is reachable from a ref takt
   writes when the turn opens (for example `refs/takt/turns/<turn id>`), and that ref is removed
   only when the turn closes. A test opens a turn, makes the delivery's resume fail, runs
   `git gc --prune=now` and finds the commit.
2. A resume that cannot make a candidate still leaves no ref of the delivery case naming the
   stale candidate (wave 16's N2 holds). A test.
3. At the candidate, `scripts/check-finding story:a-candidate-stays-reachable-while-a-turn-names-it
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
repository: https://github.com/beyond10x/takt
seed: 00a900acfe8aceb09be456bf43d4c4868467cd25
command: set -u; T="$SCENARIO"; W="$HOME/w"; R="$W/repo"; mkdir -p "$W/bin" "$W/cache" "$W/install" "$W/state/takt/promotions"; cp -r "$T/crates/takt/tests/fixtures/one-ready" "$R"; grep -v '^protocols:' "$R/.engineering/project.yaml" > "$W/p.yaml"; mv "$W/p.yaml" "$R/.engineering/project.yaml"; printf '[package]\nname = "scratch"\n' > "$R/Cargo.toml"; printf 'format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    text: The repository has its manifest.\n    assertions:\n      - %s\n' "'fs.file.exists(\"Cargo.toml\")'" > "$R/.engineering/gates.yaml"; printf '[identity]\n\tname = The Bot\n\temail = bot@example.invalid\n' > "$R/.engineering/commit-identity.conf"; mkdir -p "$R/scripts"; cp "$T/scripts/check-finding" "$T/scripts/file-finding" "$R/scripts/"; chmod 755 "$R/scripts/"*; G="git -c user.name=t -c user.email=t@example.invalid"; (cd "$R" && $G init -q -b main && $G add -A && $G commit -q -m seed); B=$(git -C "$R" rev-parse HEAD); TR=$(git -C "$R" rev-parse "$B^{tree}"); S="$W/state/takt/promotions"; printf '{"format":"takt-scorecard/1","revision":"%s","named_by":"build","metrics":{},"rule":{"trials":5,"margin":1},"rows":[],"no_record":[],"unstated":{}}' "$B" > "$S/$B.scorecard.json"; SS=$(sha256sum "$S/$B.scorecard.json" | cut -d' ' -f1); BS=$(sha256sum "$TAKT" | cut -d' ' -f1); printf '{"format":"takt-promotion/1","revision":"%s","tree":"%s","promoted_at":"2026-01-01T00:00:00Z","kind":"baseline","scorecard":"%s","scorecard_file":"/srv/scorecards/scorecard-%s.json","scorecard_sha256":"%s","against":null,"rule":{"trials":5,"margin":1},"worse":0,"binary":null,"binary_sha256":"%s","revision_stated_by":"operator","previous_sha256":null}' "$B" "$TR" "$B" "$B" "$SS" "$BS" > "$S/$B.json"; printf '%s\n' '{"result":"{\"action\": \"implementation.edit\", \"summary\": \"the double\"}","usage":{"input_tokens":1}}' > "$W/e.json"; printf '%s\n' '{"result":"{\"action\": \"change.handover\", \"summary\": \"the double\"}","usage":{"input_tokens":1}}' > "$W/h.json"; printf '%s\n' '#!/bin/sh' 'case "$2" in' "  *\"- change.handover:\"*) if [ -e work.txt ]; then cat $W/h.json; else echo the work >> work.txt; cat $W/e.json; fi ;;" "  *) echo the work >> work.txt; cat $W/e.json ;;" 'esac' > "$W/bin/claude"; printf '#!/bin/sh\necho refused >&2\nexit 1\n' > "$W/bin/b10x-gates"; cp "$W/bin/b10x-gates" "$W/bin/gh"; chmod 755 "$W/bin/"*; export XDG_STATE_HOME="$W/state" XDG_CACHE_HOME="$W/cache" CARGO_INSTALL_ROOT="$W/install" PATH="$W/bin:$PATH"; main_gets() { BL=$(printf '%s\n' "$2" | git -C "$R" hash-object -w --stdin); rm -f "$W/idx"; GIT_INDEX_FILE="$W/idx" git -C "$R" read-tree main; GIT_INDEX_FILE="$W/idx" git -C "$R" update-index --add --cacheinfo "100644,$BL,$1"; NT=$(GIT_INDEX_FILE="$W/idx" git -C "$R" write-tree); N=$($G -C "$R" commit-tree "$NT" -p main -m "main moves: $1"); git -C "$R" update-ref refs/heads/main "$N"; }; "$TAKT" run --repo "$R" --protocol story.delivery/1 --intent "Deliver the work" --test-cmd true --forge none --iterations 20 --min-free-gib 0 > "$W/deliver.out" 2>&1; D=$(ls "$R/.engineering/cases"); main_gets other.txt "other work"; "$TAKT" run --repo "$R" --case "$D" --test-cmd true --forge none --iterations 20 --min-free-gib 0 > "$W/resume1.out" 2>&1; C=$(sed -n 's/^candidate: \([0-9a-f]\{40\}\)$/\1/p' "$W/resume1.out"); [ -n "$C" ] || { sed "s|$HOME|~|g" "$W/resume1.out" | tail -5; echo "setup: the delivery handed over no candidate" >&2; exit 2; }; echo "the delivery's candidate, replayed on main: $C"; "$TAKT" run --repo "$R" --protocol self.improvement/1 --input finding=story:ready-one --input candidate="$C" --case-id turn-1 --install-root "$W/install" --harness none --forge none --test-cmd true --iterations 0 --min-free-gib 0 > "$W/turn.out" 2>&1; grep -q "$C" "$R/.engineering/cases/turn-1/case.yaml" || { sed "s|$HOME|~|g" "$W/turn.out" | tail -5; echo "setup: no turn opened on the candidate" >&2; exit 2; }; echo "turn-1 opened on $C"; main_gets work.txt "conflicting work"; "$TAKT" run --repo "$R" --case "$D" --test-cmd true --forge none --iterations 20 --min-free-gib 0 > "$W/resume2.out" 2>&1; grep '^candidate' "$W/resume2.out" | sed "s|$HOME|~|g"; git -C "$R" reflog expire --expire=now --all; git -C "$R" gc -q --prune=now; if ! git -C "$R" cat-file -e "$C^{commit}" 2>/dev/null; then echo "the candidate the open turn turn-1 names is gone from the repository after gc" >&2; exit 9; fi; echo "the candidate turn-1 names is still held"
exit: 9
healthy_exit: 0
output: the candidate the open turn turn-1 names is gone from the repository after gc
source: docs/waves/wave-16.md
```
