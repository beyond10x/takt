---
format: aep.planning-md/3
id: story:a-turn-pins-the-repository-of-its-forge
kind: story
status: draft
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
revision: 2
---
# Story: a-turn-pins-the-repository-of-its-forge

## Outcome

An improvement turn records, beside its forge's name, the repository that forge reads `main`
from (`<owner>/<repo>` for the bot route), and the grant records it with the tip. A grant or a
merge observation read from any other repository is refused, naming both.

## Context

Found by the wave 16 re-check at 622e8a0d (docs/waves/wave-16.md) as a residual and inferred
there; the predicate below now shows it. Cited at 00a900ac.

The turn's `forge` file holds a name and nothing else (`crates/takt/src/case.rs:2221-2224`,
`FORGE_FILE`, `case.rs:2430`). `Mainline::recorded` turns `bot` into a bot route for the tree
(`crates/takt/src/improvement.rs:1146-1150`), and `BotRoute::for_tree` reads the `origin`
remote each time it is called (`crates/takt/src/forge.rs:1293-1315`). A resume through another
forge name is refused (`turn_forge_refusal`, `case.rs:2458`); a resume through the same name on
another repository is not. So the tip a grant records can be read from one repository and the
merge observed on another. When the other repository's `main` descends from the recorded tip
(a fork) and already held the candidate before the grant, the turn records `merged`, and
`merged_before_grant` (`improvement.rs:1353`) cannot see it, because it only compares commits.

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
gate, which verifies nothing. Editing the `forge` file after the grant is the same defect by
another route and is not exercised.

## Acceptance

1. A turn opened through the bot route records the repository it read `main` from, and the
   grant records it with the tip: a test.
2. A resume, a grant or a merge observation through a bot route on another repository than the
   one recorded is refused, naming both, and writes nothing: a test.
3. At the candidate, `scripts/check-finding story:a-turn-pins-the-repository-of-its-forge
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
repository: https://github.com/beyond10x/takt
seed: 00a900acfe8aceb09be456bf43d4c4868467cd25
command: top=$PWD; w=$HOME/w; r=$w/repo; s=$w/scen; p=$w/state/takt/promotions; mkdir -p "$r/scripts" "$r/docs/runs" "$r/.engineering/planning/story" "$s/.engineering" "$p" "$w/install" "$w/decoy" "$w/cache"; cp scripts/check-finding scripts/file-finding "$r/scripts/"; printf '# Trial 1\n\nThe probe showed the defect.\n' > "$r/docs/runs/trial-1.md"; printf 'paths:\n  - fix.txt\n' > "$r/.engineering/judge-allow.yaml"; printf '# The probe shows the defect\n\nThe probe prints the defect.\n\n\140\140\140expected_before\nscenario: 1\ncommand: "$TAKT" probe\nexit: 1\noutput: the defect is still here\nsource: docs/runs/trial-1.md\n\140\140\140\n' > "$r/.engineering/planning/story/probe-shows-the-defect.md"; g() { git -c user.name=t -c user.email=t@example.invalid "$@"; }; cd "$r"; g init -q -b main && g add -A && g commit -q -m base; b=$(git rev-parse HEAD); g checkout -q -b fix && echo fix > fix.txt && mkdir tools && printf '#!/bin/sh\nexit 0\n' > tools/judge.sh && chmod +x tools/judge.sh && g add -A && g commit -q -m fix; c=$(git rev-parse HEAD); g checkout -q main; cd "$s" && echo scenario > README.md && g init -q -b main && g add -A && g commit -q -m seed; printf '[scenario "1"]\n\tstory = story:probe-shows-the-defect\n\tseed = %s\n' "$(git rev-parse HEAD)" > .engineering/scenarios.conf && g add -A && g commit -q -m declare; cd "$top"; printf '{"format":"takt-scorecard/1","revision":"%s","named_by":"build","metrics":{},"rule":{"trials":5,"margin":1},"rows":[],"no_record":[],"unstated":{}}' "$b" > "$p/$b.scorecard.json"; printf '{"format":"takt-promotion/1","revision":"%s","tree":"%s","promoted_at":"2026-01-01T00:00:00Z","kind":"baseline","scorecard":"%s","scorecard_file":"/srv/scorecards/scorecard-%s.json","scorecard_sha256":"%s","against":null,"rule":{"trials":5,"margin":1},"worse":0,"binary":null,"binary_sha256":"%s","revision_stated_by":"operator","previous_sha256":null}' "$b" "$(git -C "$r" rev-parse "$b^{tree}")" "$b" "$b" "$(sha256sum "$p/$b.scorecard.json" | cut -d' ' -f1)" "$(sha256sum "$TAKT" | cut -d' ' -f1)" > "$p/$b.json"; t() { XDG_STATE_HOME=$w/state XDG_CACHE_HOME=$w/cache CARGO_INSTALL_ROOT=$w/decoy "$TAKT" "$@" >> "$w/log" 2>&1; }; shown() { sed "s|$HOME|~|g" "$w/log"; }; mkdir -p "$HOME/bin" && printf '%s\n' '#!/bin/sh' 'u=$(printf "%s\n" "$@" | grep "^https://" | head -n 1)' 'case "$*" in *--dry-run*) printf "* %s %s refs/remotes/origin/main\n" 0000000000000000000000000000000000000000 "$(cat "$(dirname "$0")/tip-$(basename "$u" .git)")";; esac' > "$HOME/bin/b10x-gates" && chmod +x "$HOME/bin/b10x-gates"; echo "$b" > "$HOME/bin/tip-one"; cd "$r" && echo "$(g commit-tree "$c^{tree}" -p "$b" -p "$c" -m "the candidate, merged on the other repository's main before the grant")" > "$HOME/bin/tip-two" && cd "$top"; git -C "$r" remote add origin https://github.com/acme/one.git; export PATH="$HOME/bin:$PATH"; t run --intent "the probe no longer shows the defect" --repo "$r" --protocol "$top/protocols/self-improvement/1.yaml" --install-root "$w/install" --case-id turn-1 --harness none --forge bot --commit-identity "t <t@example.invalid>" --test-cmd true --iterations 1 --scenarios "$s" --input finding=story:probe-shows-the-defect --input "candidate=$c"; e=$r/.engineering/cases/turn-1/evidence; now=$(date -u +%Y-%m-%dT%H:%M:%SZ); printf '{"format":"canon-evidence/1","id":"scenario-result-2","kind":"scenario_result","result":"reproduced","subject":"baseline","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-reproduce"}' "$b" "$now" > "$e/0002-scenario-result-2.json"; printf '{"format":"canon-evidence/1","id":"scenario-result-3","kind":"scenario_result","result":"not_reproduced","subject":"candidate","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-verify"}' "$c" "$now" > "$e/0003-scenario-result-3.json"; k() { t run --case turn-1 --repo "$r" --harness none --forge bot --test-cmd true --scenarios "$s" "$@"; }; k --iterations 3; grep -q 'stopped: ApprovalRequired (candidate.merge_judge_surface)' "$w/log" || { shown; echo "the turn did not reach the judge-surface merge's gate" >&2; exit 2; }; t approve --repo "$r" turn-1 repository.merge.judge_surface --by human:owner; echo "forge file: $(cat "$r/.engineering/cases/turn-1/forge")" >> "$w/log"; git -C "$r" remote set-url origin https://github.com/acme/two.git; k --iterations 2; shown; if grep -l '"result": "merged"' "$e"/*.json > /dev/null 2>&1; then echo "merged recorded from acme/two under a grant whose mainline tip was read from acme/one" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: under a grant whose mainline tip was read from acme/one
source: docs/waves/wave-16.md
```
