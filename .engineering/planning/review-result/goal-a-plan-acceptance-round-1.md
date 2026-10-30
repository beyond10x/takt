---
format: aep.planning-md/3
id: review-result:goal-a-plan-acceptance-round-1
kind: review-result
status: active
title: Plan critic acceptance, goal A cluster-delivery set, round 1
relations:
- reviews: story:a-run-stops-at-a-default-token-budget
- reviews: story:a-killed-step-records-its-tokens
- reviews: story:bot-forge-reads-through-the-app
- reviews: story:forge-preflight
- reviews: story:pr-open-not-idempotent-on-resume
- reviews: story:fenced-forge-dispatch
- reviews: story:an-unattended-run-merges-under-a-delegated-grant
- reviews: story:report-sums-cases
revision: 1
---
needs-revision

story:a-killed-step-records-its-tokens — the acceptance is three independent bullets, and the third ("the scorecard treats unknown as unknown") has no scorecard output named, so it cannot be checked. The goal-critical check is also missing: that a failed or killed step's reported tokens count toward `--tokens` and stop the run at the budget. — `.engineering/planning/story/a-killed-step-records-its-tokens.md:32-36`

story:fenced-forge-dispatch — the acceptance exercises only the `repository.merge` `unknown` case. The Outcome's `pr.open` outcome and its `applied`, `refused` and `not_attempted` classes have no check, and the acceptance does not say where "the run records … outcome `unknown`" is read (the `events.rs` step event named in Scope, or a case file). — `.engineering/planning/story/fenced-forge-dispatch.md:39`

story:an-unattended-run-merges-under-a-delegated-grant — the Outcome makes the grant conditional on "a reviewer verdict at that revision", but the acceptance has only a passing reviewer double and a revoked delegation. No fixture shows a failing or missing verdict leaving the merge ungranted, so the reviewer condition is never checked. — `.engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md:51`

story:a-run-stops-at-a-default-token-budget — the acceptance is one sentence holding independent outcomes: the default stop, the flag override, the run-file key, and refusing `tokens: 0` or `tokens: x` with exit 2. The Outcome's "`takt run --help` states the default" has no check at all. — `.engineering/planning/story/a-run-stops-at-a-default-token-budget.md:50-57`

story:forge-preflight — the acceptance ends "with both present and no `gh` on `PATH`, the preflight passes", and "passes" names no observable. It should say that a case directory opens and the run gets past `forge_of`, which is what its own "Expected before" predicate checks. — `.engineering/planning/story/forge-preflight.md:65`

What I read: all 8 ids (`story:a-run-stops-at-a-default-token-budget`, `story:a-killed-step-records-its-tokens`, `story:bot-forge-reads-through-the-app`, `story:forge-preflight`, `story:pr-open-not-idempotent-on-resume`, `story:fenced-forge-dispatch`, `story:an-unattended-run-merges-under-a-delegated-grant`, `story:report-sums-cases`) in full with `aep plan artifact show`, plus `decision-blocker:unattended-merge-grant`, `aep plan artifact kinds` and `aep plan artifact lifecycle story`. In the tree I grepped `Tokens (`, the exit codes, `merge_observation`, the `run_config.rs` keys and `"gh".to_owned()`, which prints 2 as `story:bot-forge-reads-through-the-app` says.

What I could not establish: none for acceptance. `story:bot-forge-reads-through-the-app`, `story:pr-open-not-idempotent-on-resume` and `story:report-sums-cases` I approve on acceptance. Out of my lane, which did not set the verdict:
- `story:an-unattended-run-merges-under-a-delegated-grant` still lists the cleared `decision-blocker:unattended-merge-grant` under Ambiguities.
- `story:fenced-forge-dispatch` and `story:pr-open-not-idempotent-on-resume` both claim the shared read of open pull requests, and the second says "one unit delivers both".
- `story:a-killed-step-records-its-tokens` decomposes `epic:recursive-self-improvement`, not the goal's parent epic `epic:takt-as-a-loom-agent`.

```findings
- file: .engineering/planning/story/a-killed-step-records-its-tokens.md
  line: 32
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance is three independent bullets, the third ('the scorecard treats unknown as unknown') names no observable scorecard output, and none checks that a killed or failed step's reported tokens count toward --tokens and stop the run, which is what the goal needs"
- file: .engineering/planning/story/fenced-forge-dispatch.md
  line: 39
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance exercises only repository.merge with outcome unknown; the Outcome's pr.open outcome and the applied, refused and not_attempted classes have no check, and where the outcome is recorded (events.jsonl step event or a case file) is not named"
- file: .engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md
  line: 51
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the Outcome conditions the grant on a reviewer verdict at the revision, but the acceptance has only a passing reviewer double and a revoked delegation, so no fixture shows a failing or missing verdict leaving the merge ungranted"
- file: .engineering/planning/story/a-run-stops-at-a-default-token-budget.md
  line: 50
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance is one sentence holding independent outcomes (default stop, flag override, run-file key, refusal of tokens: 0 and tokens: x with exit 2) and has no check for the Outcome's 'takt run --help states the default'"
- file: .engineering/planning/story/forge-preflight.md
  line: 65
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'the preflight passes' names no observable; it should say a case directory opens and the run gets past forge_of when both are present and no gh is on PATH"
```
