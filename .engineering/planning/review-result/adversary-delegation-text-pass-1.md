---
format: aep.planning-md/3
id: review-result:adversary-delegation-text-pass-1
kind: review-result
status: active
title: Adversary, judge-surface-delegation-text, pass 1
relations:
- reviews: story:judge-surface-delegation-text
revision: 1
---
unit: story:judge-surface-delegation-text, commit 98d586d on base 8ce2019 (worktree w9-delegation-text, clean)
verdict: NEEDS-CHANGE
cases: executed 9→9, red 0
origin: introduced 5 / pre-existing 4 / undecided 0
wrote-outside-worktree: 7 paths, all in the unit's scratch `adv1/`
needs-coordinator: story:judge-guard still requires a human grant on a judge surface (finding 1); two files changed outside the story's scope (finding 5); two draft-story line citations moved (finding 9)

**Checks.** `cargo test -p b10x-takt --locked --test improvement_protocols` exit 0, 9 passed (loads, validates, compiles both). Both protocols at base and head with `description` keys removed: equal. Expected-before greps: 0 and 0. `scan-text` on the 6 files and the commit message: clean. The commit message's figures match the implementor's log (44 lines, 361 passed, 0 failed). No `ess/` citation into the changed files.

**Holding.** AGENTS.md § Rules, § Owns and § Never say what the decision says (owner, named role, record, revocable, a revoked one grants nothing, judge-guard); none gives the operator more. The § Owns sentence matches the design's Roles table. The number rule is verbatim; the Commands block untouched. `crates/` source, help text, `ess/` and `.claude/` carry no owner-only or person-only rule.

```findings
[
  {"file": ".engineering/planning/story/judge-guard.md", "line": 47, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "every changed text names story:judge-guard as enforcing delegation, but its Outcome and Acceptance refuse every non-human grant on a judge surface with no delegation clause"},
  {"file": ".agents/operator-agent.md", "line": 69, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Loop step 8 grants under any delegation naming the role, without the not-revoked condition or the self-limit, so read literally it lets a widening diff through"},
  {"file": ".agents/operator-agent.md", "line": 41, "category": "judgement", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the self-limit does not define widening (allow-list, guard code, protocol requires), covers only the operator's own authority, and no program checks it"},
  {"file": ".agents/operator-agent.md", "line": 39, "category": "judgement", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "neither the profile nor AGENTS.md says where a delegation record or its revocation is held (design says at the base revision), and no such record exists in the tree"},
  {"file": ".agents/developer-agent.md", "line": 29, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "developer-agent.md and docs/design/takt.md:99 changed outside the story's Scope and the brief's file assignment"},
  {"file": "README.md", "line": 10, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "README says a bot-route merge always waits for a person's grant, which agent grants already contradict"},
  {"file": ".engineering/planning/design/continuous-orchestration.md", "line": 214, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the draft design assigns judge-surface merges to the owner alone, with no delegation"},
  {"file": "crates/takt/tests/improvement_protocols.rs", "line": 288, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the test name, like design line 141, keeps the owner-only wording for the judge-surface capability"},
  {"file": ".engineering/planning/story/first-recorded-improvement-turn.md", "line": 48, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the protocol line citations here and in loop-scorecard.md:58 moved with the description edits"},
  {"file": "protocols/loop-improvement/1.yaml", "line": 123, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the protocol descriptions do not say a revoked delegation grants nothing, and loop.merge does not name story:judge-guard"}
]
```
