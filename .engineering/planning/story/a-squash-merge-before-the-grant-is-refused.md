---
format: aep.planning-md/3
id: story:a-squash-merge-before-the-grant-is-refused
kind: story
status: draft
title: A candidate squash-merged into main before its grant is refused like one merged by ancestry
summary: the ancestry check does not see a candidate whose change was squashed into main before the grant
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-squash-merge-before-the-grant-is-refused
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: a-squash-merge-before-the-grant-is-refused

## Outcome

`candidate.merge_judge_surface` records nothing when the candidate's change was already on
`main` when the grant was given, however it got there: a squash, a rebase or a cherry-pick, not
only a merge of the candidate commit itself. A grant authorises no change already made.

## Context

Found by the wave 16 review at 05f70c4f (docs/waves/wave-16.md), filed for later. Cited at
00a900ac.

`merged_before_grant` (`crates/takt/src/improvement.rs:1353`) decides whether the candidate
reached `main` before the grant by ancestry only: the tip recorded with the grant must be on
`main` now (`improvement.rs:1389`), and it refuses only when the candidate commit is that tip or
an ancestor of it (`improvement.rs:1396`). A squash of the candidate makes a new commit with the
candidate's tree and does not make the candidate an ancestor. So an owner can grant after the
change is already on `main`, a later merge of the candidate commit itself (a merge that changes
nothing) makes `main` hold the candidate, and the turn records `merged` under that grant.

The predicate runs in a checkout of this repository at the seed and builds a turn world under
`$HOME`: a repository whose base holds this tree's confined `scripts/check-finding` and
`scripts/file-finding`, a finding, a judge allow-list and a candidate that touches
`tools/judge.sh` (a judge surface); a scenario repository; and a promotion record stating the
takt under test as the baseline's binary. The two `scenario_result` records
(`reproduced` at the baseline, `not_reproduced` at the candidate) are written by hand: in
`scripts/check-finding`'s sandbox the turn's own `scripts/check-finding` cannot build a
revision, because rustup's toolchain is under the hidden home. Then `main` gets the candidate's
tree as one new commit (the squash), the owner grants, `main` gets a merge of the candidate
commit, and the turn resumes. Exit 9 says a `merge_observation` `merged` was written. Exit 2
says the turn never reached the judge-surface gate, which verifies nothing.

## Acceptance

1. A judge-surface merge is refused when, at the tip recorded with the grant, `main` already
   holds the candidate's change but not the candidate commit (a squash; a rebase of the
   candidate's commits), the refusal naming the commit of `main` that holds it: a test for each.
2. A candidate whose change is not on `main` at the grant and is merged after it is still
   recorded `merged`: the existing tests stay green.
3. At the candidate, `scripts/check-finding story:a-squash-merge-before-the-grant-is-refused
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
repository: https://github.com/beyond10x/takt
seed: 00a900acfe8aceb09be456bf43d4c4868467cd25
command: top=$PWD; w=$HOME/w; r=$w/repo; s=$w/scen; p=$w/state/takt/promotions; mkdir -p "$r/scripts" "$r/docs/runs" "$r/.engineering/planning/story" "$s/.engineering" "$p" "$w/install" "$w/decoy" "$w/cache"; cp scripts/check-finding scripts/file-finding "$r/scripts/"; printf '# Trial 1\n\nThe probe showed the defect.\n' > "$r/docs/runs/trial-1.md"; printf 'paths:\n  - fix.txt\n' > "$r/.engineering/judge-allow.yaml"; printf '# The probe shows the defect\n\nThe probe prints the defect.\n\n\140\140\140expected_before\nscenario: 1\ncommand: "$TAKT" probe\nexit: 1\noutput: the defect is still here\nsource: docs/runs/trial-1.md\n\140\140\140\n' > "$r/.engineering/planning/story/probe-shows-the-defect.md"; g() { git -c user.name=t -c user.email=t@example.invalid "$@"; }; cd "$r"; g init -q -b main && g add -A && g commit -q -m base; b=$(git rev-parse HEAD); g checkout -q -b fix && echo fix > fix.txt && mkdir tools && printf '#!/bin/sh\nexit 0\n' > tools/judge.sh && chmod +x tools/judge.sh && g add -A && g commit -q -m fix; c=$(git rev-parse HEAD); g checkout -q main; cd "$s" && echo scenario > README.md && g init -q -b main && g add -A && g commit -q -m seed; printf '[scenario "1"]\n\tstory = story:probe-shows-the-defect\n\tseed = %s\n' "$(git rev-parse HEAD)" > .engineering/scenarios.conf && g add -A && g commit -q -m declare; cd "$top"; printf '{"format":"takt-scorecard/1","revision":"%s","named_by":"build","metrics":{},"rule":{"trials":5,"margin":1},"rows":[],"no_record":[],"unstated":{}}' "$b" > "$p/$b.scorecard.json"; printf '{"format":"takt-promotion/1","revision":"%s","tree":"%s","promoted_at":"2026-01-01T00:00:00Z","kind":"baseline","scorecard":"%s","scorecard_file":"/srv/scorecards/scorecard-%s.json","scorecard_sha256":"%s","against":null,"rule":{"trials":5,"margin":1},"worse":0,"binary":null,"binary_sha256":"%s","revision_stated_by":"operator","previous_sha256":null}' "$b" "$(git -C "$r" rev-parse "$b^{tree}")" "$b" "$b" "$(sha256sum "$p/$b.scorecard.json" | cut -d' ' -f1)" "$(sha256sum "$TAKT" | cut -d' ' -f1)" > "$p/$b.json"; t() { XDG_STATE_HOME=$w/state XDG_CACHE_HOME=$w/cache CARGO_INSTALL_ROOT=$w/decoy "$TAKT" "$@" >> "$w/log" 2>&1; }; shown() { sed "s|$HOME|~|g" "$w/log"; }; t run --intent "the probe no longer shows the defect" --repo "$r" --protocol "$top/protocols/self-improvement/1.yaml" --install-root "$w/install" --case-id turn-1 --harness none --forge none --test-cmd true --iterations 6 --scenarios "$s" --input finding=story:probe-shows-the-defect --input "candidate=$c"; e=$r/.engineering/cases/turn-1/evidence; now=$(date -u +%Y-%m-%dT%H:%M:%SZ); printf '{"format":"canon-evidence/1","id":"scenario-result-2","kind":"scenario_result","result":"reproduced","subject":"baseline","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-reproduce"}' "$b" "$now" > "$e/0002-scenario-result-2.json"; printf '{"format":"canon-evidence/1","id":"scenario-result-3","kind":"scenario_result","result":"not_reproduced","subject":"candidate","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-verify"}' "$c" "$now" > "$e/0003-scenario-result-3.json"; k() { t run --case turn-1 --repo "$r" --harness none --forge none --test-cmd true --scenarios "$s" "$@"; }; k --iterations 6; grep -q 'stopped: ApprovalRequired (candidate.merge_judge_surface)' "$w/log" || { shown; echo "the turn did not reach the judge-surface merge's gate" >&2; exit 2; }; cd "$r" && g update-ref refs/heads/main "$(g commit-tree "$c^{tree}" -p main -m "the candidate, squashed")" && cd "$top"; t approve --repo "$r" turn-1 repository.merge.judge_surface --by human:owner; cd "$r" && g update-ref refs/heads/main "$(g commit-tree "$c^{tree}" -p main -p "$c" -m "the candidate, merged after the grant")" && cd "$top"; k --iterations 3; shown; if grep -l '"result": "merged"' "$e"/*.json > /dev/null 2>&1; then echo "merged recorded under a grant given after the candidate's change was squashed into main" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: after the candidate's change was squashed into main
source: docs/waves/wave-16.md
```
