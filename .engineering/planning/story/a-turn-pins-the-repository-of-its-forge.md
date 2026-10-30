---
format: aep.planning-md/3
id: story:a-turn-pins-the-repository-of-its-forge
kind: story
status: implemented
title: A turn records the repository its forge reads, and its merge reads the same one
summary: the forge file records only a name, so the bot route reads whatever origin names at the call
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-turn-pins-the-repository-of-its-forge
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests/self_improvement_cli.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/forge/bot.rs
- confidence: cited
  path: crates/takt/src/improvement.rs
- confidence: inferred
  path: crates/takt/tests/self_improvement.rs
- confidence: cited
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: ess/domains/takt.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T19:03:57Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-28T19:03:58Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-30T02:33:48Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":3,"verification":2}}}
---
# Story: a-turn-pins-the-repository-of-its-forge

## Outcome

An improvement turn records, beside its forge's name, the repository that forge reads `main`
from (`<owner>/<repo>` for the bot route), and its judge-surface grant records that repository
with the tip. A resume, a grant or a merge observation whose mainline is read from another
repository than the one recorded is refused, naming both, with nothing written.

## Context

The finding's origin is its predicate below: no run record or review holds it. An earlier body
named a wave page as its source; that page holds no such finding, and the predicate's `source`
now names this story. Cited at 338082d8.

The turn's `forge` file holds a name and nothing else: `open_turn` writes it
(`crates/takt/src/case.rs:3611-3614`), `FORGE_FILE` and `turn_forge` read it back
(`case.rs:3840-3863`). `Mainline::recorded` turns `bot` into a bot route for the tree
(`crates/takt/src/improvement.rs:1149-1159`), and `BotRoute::for_tree` reads the `origin` remote
each time it is called (`crates/takt/src/forge/bot.rs:130-152`, the repository taken from the URL
at `:131-148`); the CLI builds its forge the same way (`crates/takt-cli/src/main.rs:3032`). The
grant reads the tip through that route (`mainline_at_grant`, `case.rs:1267-1281`) and records it
as `AuthorityDecision.mainline_tip` (`case.rs:666-672`, written at `:1237-1250`), with no
repository. A resume through another forge name is refused (`turn_forge_refusal`,
`case.rs:3870-3880`, called at `crates/takt-cli/src/main.rs:1748-1758`); a resume through the same
name on another repository is not. So the tip a grant records can be read from one repository and
the merge observed on another (the observation reads its tip at `improvement.rs:1533`). When the
other repository's `main` descends from the recorded tip (a fork) and already held the candidate
before the grant, the turn records `merged`, and `merged_before_grant` (`improvement.rs:1356`,
called at `:1544-1556`) cannot see it, because it only compares commits.

The predicate runs in a checkout of this repository at the seed. It builds the turn world the
self-improvement CLI tests build, under `$HOME`: this tree's confined finding scripts, a
finding, a judge allow-list, a candidate touching `tools/judge.sh`, a scenario repository, a
promotion record for the takt under test. The two `scenario_result` records are written by
hand, because the turn's own `scripts/check-finding` cannot build a revision inside
`scripts/check-finding`'s sandbox (rustup's toolchain is under the hidden home). The bot route
is a stand-in: a `b10x-gates` on `PATH` that answers the route's `fetch --dry-run` with a tip
per repository (`acme/one` at the baseline; `acme/two` at a merge of the candidate made before
the grant) and its object fetch with success, since the commits are already local. The turn
opens with `--forge bot` on `origin` `acme/one`, the owner grants (the tip is read from
`acme/one`), `origin` is moved to `acme/two`, and the turn resumes. Exit 9 says a
`merge_observation` `merged` was written. Exit 2 says the turn never reached the judge-surface
gate, which verifies nothing.

The protocol file the predicate passes is the seed's (`protocols/self-improvement/1.yaml`, gone
from `main` since takt reads its protocols from engineering-protocols; the clone at the seed
still holds it). Every validator now runs `check_at_open` before the first step
(`crates/takt/src/run.rs:9301-9309`); only the gate validator overrides it
(`crates/takt/src/validator.rs:2290`), and it returns at once for a protocol with no `gate.run`
action (`validator.rs:2291-2295`, `protocol_has_action` at `:4786`). `self.improvement/1` has
none, so the stand-in `b10x-gates` needs to answer nothing more at open.

## Decision

One rule for every forge: the merge observation's repository must equal the repository the
grant recorded, and the local `main` (`--forge none`) counts as no repository. So a grant given
before grants recorded a repository authorises no judge-surface merge through the bot route
(a new grant records it), and a turn whose `forge` file names `bot` without a repository (opened
before this change) is not refused at resume but is held by its grant's record. The `forge`
file keeps its one-line form readable.

## Scope

- `ess/domains/takt.yaml`: `takt.work.Case` fields `forge` and `authority` (the recorded
  repository and the grant's), before code; `ess/domains/takt.citations.tsv` for their
  citations
- `crates/takt/src/case.rs`: `open_turn` (`:3611-3614`), `FORGE_FILE` and `turn_forge`
  (`:3840-3863`), `turn_forge_refusal` (`:3870`), `AuthorityDecision` (`:672`), the grant and
  `mainline_at_grant` (`:1237-1281`)
- `crates/takt/src/improvement.rs`: `Mainline::recorded` (`:1149`), `judge_surface_refusal`
  (`:1303`) and `merged_before_grant` (`:1343-1356`), the merge observation (`:1520-1556`)
- `crates/takt/src/forge/bot.rs`: `BotRoute::for_tree` and its `repository` (`:130-152`)
- `crates/takt-cli/src/main.rs`: the resume refusal (`:1748-1758`), the mainline built for the
  run (`:1777-1780`), the bot forge (`:3032`)
- `crates/takt-cli/tests/self_improvement_cli.rs`, `crates/takt/tests/self_improvement.rs`
  (inferred)

`crates/takt/src/improvement.rs` is also story:a-squash-merge-before-the-grant-is-refused's;
that story is not in this wave.

## Out of scope

- Which repository a turn may open on: any the bot route serves, as now.
- A forge other than `bot` and `none`.

## Acceptance

1. A turn opened through the bot route on `origin` `acme/one` writes `acme/one` into its
   `forge` file beside `bot`, and its judge-surface grant writes `acme/one` into
   `authority.yaml` beside `mainline_tip`: a test in
   `crates/takt-cli/tests/self_improvement_cli.rs` reads both files.
2. With `origin` moved to `acme/two` after the open, `takt run --case` through the bot route and
   `takt approve` of the judge-surface capability each exit 1, standard error naming `acme/one`
   and `acme/two`, and leave `state_of` of the repository (every file under the cases directory
   with its bytes, the refs and `HEAD`) as it was: a test in
   `crates/takt-cli/tests/self_improvement_cli.rs`, one case per command.
3. With the grant given on `acme/one` and the `forge` file then edited to name `bot acme/two`
   (or `none`), the turn's next run writes no `merge_observation` `merged`; its summary names
   the grant's repository and the one read now, and the case's evidence directory holds no new
   file: a test in `crates/takt-cli/tests/self_improvement_cli.rs`.
4. At the candidate, `scripts/check-finding story:a-turn-pins-the-repository-of-its-forge
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
repository: https://github.com/beyond10x/takt
seed: bf0fa9c6b23ce9ab9b75f83595f55097b7f1e05e
command: top=$PWD; w=$HOME/w; r=$w/repo; s=$w/scen; p=$w/state/takt/promotions; mkdir -p "$r/scripts" "$r/docs/runs" "$r/.engineering/planning/story" "$s/.engineering" "$p" "$w/install" "$w/decoy" "$w/cache"; cp scripts/check-finding scripts/file-finding "$r/scripts/"; printf '# Trial 1\n\nThe probe showed the defect.\n' > "$r/docs/runs/trial-1.md"; printf 'paths:\n  - fix.txt\n' > "$r/.engineering/judge-allow.yaml"; printf '# The probe shows the defect\n\nThe probe prints the defect.\n\n\140\140\140expected_before\nscenario: 1\ncommand: "$TAKT" probe\nexit: 1\noutput: the defect is still here\nsource: docs/runs/trial-1.md\n\140\140\140\n' > "$r/.engineering/planning/story/probe-shows-the-defect.md"; g() { git -c user.name=t -c user.email=t@example.invalid "$@"; }; cd "$r"; g init -q -b main && g add -A && g commit -q -m base; b=$(git rev-parse HEAD); g checkout -q -b fix && echo fix > fix.txt && mkdir tools && printf '#!/bin/sh\nexit 0\n' > tools/judge.sh && chmod +x tools/judge.sh && g add -A && g commit -q -m fix; c=$(git rev-parse HEAD); g checkout -q main; cd "$s" && echo scenario > README.md && g init -q -b main && g add -A && g commit -q -m seed; printf '[scenario "1"]\n\tstory = story:probe-shows-the-defect\n\tseed = %s\n' "$(git rev-parse HEAD)" > .engineering/scenarios.conf && g add -A && g commit -q -m declare; cd "$top"; printf '{"format":"takt-scorecard/1","revision":"%s","named_by":"build","metrics":{},"rule":{"trials":5,"margin":1},"rows":[],"no_record":[],"unstated":{}}' "$b" > "$p/$b.scorecard.json"; printf '{"format":"takt-promotion/1","revision":"%s","tree":"%s","promoted_at":"2026-01-01T00:00:00Z","kind":"baseline","scorecard":"%s","scorecard_file":"/srv/scorecards/scorecard-%s.json","scorecard_sha256":"%s","against":null,"rule":{"trials":5,"margin":1},"worse":0,"binary":null,"binary_sha256":"%s","revision_stated_by":"operator","previous_sha256":null}' "$b" "$(git -C "$r" rev-parse "$b^{tree}")" "$b" "$b" "$(sha256sum "$p/$b.scorecard.json" | cut -d' ' -f1)" "$(sha256sum "$TAKT" | cut -d' ' -f1)" > "$p/$b.json"; t() { XDG_STATE_HOME=$w/state XDG_CACHE_HOME=$w/cache CARGO_INSTALL_ROOT=$w/decoy "$TAKT" "$@" >> "$w/log" 2>&1; }; shown() { sed "s|$HOME|~|g" "$w/log"; }; mkdir -p "$HOME/bin" && printf '%s\n' '#!/bin/sh' 'u=$(printf "%s\n" "$@" | grep "^https://" | head -n 1)' 'case "$*" in *--dry-run*) printf "* %s %s refs/remotes/origin/main\n" 0000000000000000000000000000000000000000 "$(cat "$(dirname "$0")/tip-$(basename "$u" .git)")";; esac' > "$HOME/bin/b10x-gates" && chmod +x "$HOME/bin/b10x-gates" && printf "%s\n" "#!/bin/sh" "echo no gh here >&2" "exit 1" > "$HOME/bin/gh" && chmod +x "$HOME/bin/gh"; echo "$b" > "$HOME/bin/tip-one"; cd "$r" && echo "$(g commit-tree "$c^{tree}" -p "$b" -p "$c" -m "the candidate, merged on the other repository's main before the grant")" > "$HOME/bin/tip-two" && cd "$top"; git -C "$r" remote add origin https://github.com/acme/one.git; printf "{}" > "$HOME/policy.json" && chmod 600 "$HOME/policy.json"; export B10X_GATES_POLICY="$HOME/policy.json" PATH="$HOME/bin:$PATH"; t run --intent "the probe no longer shows the defect" --repo "$r" --protocol self.improvement/1 --install-root "$w/install" --case-id turn-1 --harness none --forge bot --commit-identity "t <t@example.invalid>" --test-cmd true --iterations 1 --scenarios "$s" --input finding=story:probe-shows-the-defect --input "candidate=$c"; e=$r/.takt/cases/turn-1/evidence; now=$(date -u +%Y-%m-%dT%H:%M:%SZ); printf '{"format":"canon-evidence/1","id":"scenario-result-2","kind":"scenario_result","result":"reproduced","subject":"baseline","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-reproduce"}' "$b" "$now" > "$e/0002-scenario-result-2.json"; printf '{"format":"canon-evidence/1","id":"scenario-result-3","kind":"scenario_result","result":"not_reproduced","subject":"candidate","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-verify"}' "$c" "$now" > "$e/0003-scenario-result-3.json"; k() { t run --case turn-1 --repo "$r" --harness none --forge bot --test-cmd true --scenarios "$s" "$@"; }; k --iterations 3; grep -q 'stopped: ApprovalRequired (candidate.merge_judge_surface)' "$w/log" || { shown; echo "the turn did not reach the judge-surface merge's gate" >&2; exit 2; }; t approve --repo "$r" turn-1 repository.merge.judge_surface --by human:owner; echo "forge file: $(cat "$r/.takt/cases/turn-1/forge")" >> "$w/log"; git -C "$r" remote set-url origin https://github.com/acme/two.git; k --iterations 2; shown; if grep -l '"result": "merged"' "$e"/*.json > /dev/null 2>&1; then echo "merged recorded from acme/two under a grant whose mainline tip was read from acme/one" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: under a grant whose mainline tip was read from acme/one
source: .engineering/planning/story/a-turn-pins-the-repository-of-its-forge.md
```
