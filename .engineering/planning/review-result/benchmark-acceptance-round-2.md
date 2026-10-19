---
format: aep.planning-md/3
id: review-result:benchmark-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, benchmark-integration stories, round 2
relations:
- reviews: story:takt-static-binary
- reviews: story:bench-run-export
- reviews: story:bench-nouns-in-ess
- reviews: story:takt-in-container
- reviews: story:baseline-runner-in-repo
- reviews: story:bench-runs-in-journey
revision: 1
---
needs-revision

story:takt-static-binary — the acceptance requires `file` to print "statically linked", but `file` 5.48 prints "static-pie linked" for an `x86_64-unknown-linux-musl` binary built with rustc 1.99.0, so a correct build fails it; the string must match what `file` prints, or the check must use another command — .engineering/planning/story/takt-static-binary.md:36
story:bench-run-export — the Outcome and summary name the private benchmark repository by its short name, where every other story says "the organization's benchmark instrument" — .engineering/planning/story/bench-run-export.md:23
story:bench-nouns-in-ess — the summary and acceptance name the private repository by its short name, and the Context cites a file path inside it — .engineering/planning/story/bench-nouns-in-ess.md:41
story:bench-run-export — the acceptance runs `takt export --bench <case dir>` with none of the arm, case, harness and budget arguments the Context says the entry requires and `takt export` takes, and "the instrument's seed shape" is defined only in the other repository, so the output has no committed expected entry to check against — .engineering/planning/story/bench-run-export.md:48
story:bench-nouns-in-ess — `ess specify validate --path ess` does not check "each relation ... named or `UNMAPPED:`" (a comment convention) or "every `read from:` resolving" (that is `scripts/check-citations`), so two of the three clauses name no command that could fail — .engineering/planning/story/bench-nouns-in-ess.md:41; AGENTS.md:38
story:takt-in-container — the acceptance does not require the run to complete or the reply to be recorded, so a run that stops on an error before writing the double's reply also leaves `grep` with exit 1; require `takt run` to exit 0 with the echoing double's step in `events.jsonl` — .engineering/planning/story/takt-in-container.md:50
story:baseline-runner-in-repo — "for every arm of T, C, I and X" uses arm codes that neither the story nor the tree defines (grep over `docs` and `.engineering/planning` finds them only here), so the `<arm>` values are not checkable — .engineering/planning/story/baseline-runner-in-repo.md:44
story:bench-runs-in-journey — the Outcome promises one lane per run in `journey.svg` grouped by arm, but the acceptance reads only a line in `journeys.jsonl`, so the drawing and the grouping are never checked — .engineering/planning/story/bench-runs-in-journey.md:37

(Recorded by the planner with the private repository's short name and quoted phrases removed from findings 2 and 3, and the critic's out-of-lane notes left out: the store is public. The findings block is the critic's.)

```findings
[
 {"file": ".engineering/planning/story/takt-static-binary.md", "line": 36, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance requires `file` to print \"statically linked\", but `file` 5.48 prints \"static-pie linked\" for an x86_64-unknown-linux-musl binary built with rustc 1.99.0, so a correct build fails it"},
 {"file": ".engineering/planning/story/bench-run-export.md", "line": 23, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the Outcome (line 23) and summary (line 7) name the private benchmark repository by its short name instead of \"the organization's benchmark instrument\""},
 {"file": ".engineering/planning/story/bench-nouns-in-ess.md", "line": 41, "category": "acceptance", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "the summary (line 7) and acceptance (line 41) name the private repository by its short name and the Context (line 26) cites a file path inside it"},
 {"file": ".engineering/planning/story/bench-run-export.md", "line": 48, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance runs `takt export --bench <case dir>` with none of the arm, case, harness and budget arguments the Context says the entry requires, and \"the instrument's seed shape\" has no committed expected entry to check against"},
 {"file": ".engineering/planning/story/bench-nouns-in-ess.md", "line": 41, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "`ess specify validate --path ess` does not check \"each relation named or UNMAPPED\" or \"every read from: resolving\" (scripts/check-citations does), so two of the three clauses name no command that could fail"},
 {"file": ".engineering/planning/story/takt-in-container.md", "line": 50, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the acceptance does not require the run to complete or the double's reply to be recorded, so a run that stops on an error before writing the reply also leaves grep with exit 1"},
 {"file": ".engineering/planning/story/baseline-runner-in-repo.md", "line": 44, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "\"for every arm of T, C, I and X\" uses arm codes that neither the story nor the tree defines, so the <arm> values are not checkable"},
 {"file": ".engineering/planning/story/bench-runs-in-journey.md", "line": 37, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the Outcome promises one lane per run in journey.svg grouped by arm, but the acceptance reads only a line in journeys.jsonl, so the drawing and the grouping are never checked"}
]
```
