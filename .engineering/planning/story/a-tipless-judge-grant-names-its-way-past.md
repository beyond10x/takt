---
format: aep.planning-md/3
id: story:a-tipless-judge-grant-names-its-way-past
kind: story
status: draft
title: A judge-surface grant that records no mainline tip names the way past its refusal
summary: the no-tip refusal names no way past and a re-grant is refused
tags:
- finding
- rsi
refs:
- provider: finding
  reference: a-tipless-judge-grant-names-its-way-past
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: a-tipless-judge-grant-names-its-way-past

## Outcome

A judge-surface merge refused because its grant records no tip of `main` says how to get past
the refusal, and the way it names works: `takt approve` given again replaces the tipless grant
with one that records the tip, or the refusal names the step that does. Today the refusal names
nothing to do, and the grant given again is refused.

## Context

Found by the wave 16 review at 05f70c4f (docs/waves/wave-16.md), filed for later. Cited at
00a900ac.

`takt approve` records the tip of `main` with a turn's grant of
`repository.merge.judge_surface` only when the turn recorded a forge when it opened
(`mainline_at_grant`, `crates/takt/src/case.rs:1121`; no `forge` file, no tip, `case.rs:1123`).
A turn opened before the forge file was written, or whose file is gone, gets a grant with no
tip. `candidate.merge_judge_surface` then refuses the merge: "the grant given at … records no
tip of main read when it was given, so whether the candidate reached main after the grant is
not known" (`crates/takt/src/improvement.rs:1366-1370`). The refusal names no way past.

The obvious way past is to grant again, and that is refused too. Right after the tipless grant
it is "already decides" (`case.rs:1066-1077`): the grant applies at the candidate's revision, so
a second grant there is a duplicate. After a resume it is "not at the gate"
(`case.rs:1048-1056`): the grant has made the merge admissible, so the case never stops
`ApprovalRequired` again, which is what that message asks for. The turn is stuck short of
`fix.merged` with no command that moves it.

The predicate runs in a checkout of this repository at the seed and builds a turn world under
`$HOME`: a repository whose base holds this tree's confined `scripts/check-finding` and
`scripts/file-finding`, a finding, a judge allow-list and a candidate that touches
`tools/judge.sh` (a judge surface); a scenario repository; and a promotion record stating the
takt under test as the baseline's binary. The two `scenario_result` records
(`reproduced` at the baseline, `not_reproduced` at the candidate) are written by hand: in
`scripts/check-finding`'s sandbox the turn's own `scripts/check-finding` cannot build a
revision, because rustup's toolchain is under the hidden home. The predicate then removes the
turn's `forge` file, grants, merges the candidate into `main`, resumes (the refusal), and grants
again. Exit 9 says the refusal names no `takt` command and the second grant was refused. Exit 2
says the turn never reached the judge-surface gate, which verifies nothing.

## Acceptance

1. The refusal of a merge under a tipless grant names the command that gets past it: a test.
2. That command works: under a tipless grant, `takt approve` given again (or the command the
   refusal names) records a tip of `main`, and the merge is then judged by ancestry against that
   tip: a test.
3. At the candidate, `scripts/check-finding story:a-tipless-judge-grant-names-its-way-past
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
repository: https://github.com/beyond10x/takt
seed: 00a900acfe8aceb09be456bf43d4c4868467cd25
command: top=$PWD; w=$HOME/w; r=$w/repo; s=$w/scen; p=$w/state/takt/promotions; mkdir -p "$r/scripts" "$r/docs/runs" "$r/.engineering/planning/story" "$s/.engineering" "$p" "$w/install" "$w/decoy" "$w/cache"; cp scripts/check-finding scripts/file-finding "$r/scripts/"; printf '# Trial 1\n\nThe probe showed the defect.\n' > "$r/docs/runs/trial-1.md"; printf 'paths:\n  - fix.txt\n' > "$r/.engineering/judge-allow.yaml"; printf '# The probe shows the defect\n\nThe probe prints the defect.\n\n\140\140\140expected_before\nscenario: 1\ncommand: "$TAKT" probe\nexit: 1\noutput: the defect is still here\nsource: docs/runs/trial-1.md\n\140\140\140\n' > "$r/.engineering/planning/story/probe-shows-the-defect.md"; g() { git -c user.name=t -c user.email=t@example.invalid "$@"; }; cd "$r"; g init -q -b main && g add -A && g commit -q -m base; b=$(git rev-parse HEAD); g checkout -q -b fix && echo fix > fix.txt && mkdir tools && printf '#!/bin/sh\nexit 0\n' > tools/judge.sh && chmod +x tools/judge.sh && g add -A && g commit -q -m fix; c=$(git rev-parse HEAD); g checkout -q main; cd "$s" && echo scenario > README.md && g init -q -b main && g add -A && g commit -q -m seed; printf '[scenario "1"]\n\tstory = story:probe-shows-the-defect\n\tseed = %s\n' "$(git rev-parse HEAD)" > .engineering/scenarios.conf && g add -A && g commit -q -m declare; cd "$top"; printf '{"format":"takt-scorecard/1","revision":"%s","named_by":"build","metrics":{},"rule":{"trials":5,"margin":1},"rows":[],"no_record":[],"unstated":{}}' "$b" > "$p/$b.scorecard.json"; printf '{"format":"takt-promotion/1","revision":"%s","tree":"%s","promoted_at":"2026-01-01T00:00:00Z","kind":"baseline","scorecard":"%s","scorecard_file":"/srv/scorecards/scorecard-%s.json","scorecard_sha256":"%s","against":null,"rule":{"trials":5,"margin":1},"worse":0,"binary":null,"binary_sha256":"%s","revision_stated_by":"operator","previous_sha256":null}' "$b" "$(git -C "$r" rev-parse "$b^{tree}")" "$b" "$b" "$(sha256sum "$p/$b.scorecard.json" | cut -d' ' -f1)" "$(sha256sum "$TAKT" | cut -d' ' -f1)" > "$p/$b.json"; t() { XDG_STATE_HOME=$w/state XDG_CACHE_HOME=$w/cache CARGO_INSTALL_ROOT=$w/decoy "$TAKT" "$@" >> "$w/log" 2>&1; }; shown() { sed "s|$HOME|~|g" "$w/log"; }; t run --intent "the probe no longer shows the defect" --repo "$r" --protocol "$top/protocols/self-improvement/1.yaml" --install-root "$w/install" --case-id turn-1 --harness none --forge none --test-cmd true --iterations 6 --scenarios "$s" --input finding=story:probe-shows-the-defect --input "candidate=$c"; e=$r/.engineering/cases/turn-1/evidence; now=$(date -u +%Y-%m-%dT%H:%M:%SZ); printf '{"format":"canon-evidence/1","id":"scenario-result-2","kind":"scenario_result","result":"reproduced","subject":"baseline","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-reproduce"}' "$b" "$now" > "$e/0002-scenario-result-2.json"; printf '{"format":"canon-evidence/1","id":"scenario-result-3","kind":"scenario_result","result":"not_reproduced","subject":"candidate","subject_revision":"%s","observed_at":"%s","producer":"takt.scenario-verify"}' "$c" "$now" > "$e/0003-scenario-result-3.json"; k() { t run --case turn-1 --repo "$r" --harness none --forge none --test-cmd true --scenarios "$s" "$@"; }; k --iterations 6; grep -q 'stopped: ApprovalRequired (candidate.merge_judge_surface)' "$w/log" || { shown; echo "the turn did not reach the judge-surface merge's gate" >&2; exit 2; }; rm "$r/.engineering/cases/turn-1/forge"; t approve --repo "$r" turn-1 repository.merge.judge_surface --by human:owner; cd "$r" && g update-ref refs/heads/main "$(g commit-tree "$c^{tree}" -p main -p "$c" -m "the candidate, merged after the grant")" && cd "$top"; k --iterations 3; t approve --repo "$r" turn-1 repository.merge.judge_surface --by human:owner; again=$?; shown; if grep 'records no tip of main' "$w/log" | grep -qv 'takt '; then if [ "$again" -ne 0 ]; then echo "a grant that records no tip of main names no way past, and the grant given again is refused" >&2; exit 9; fi; fi
exit: 9
healthy_exit: 0
output: names no way past, and the grant given again is refused
source: docs/waves/wave-16.md
```
