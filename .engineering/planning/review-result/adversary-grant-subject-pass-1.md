---
format: aep.planning-md/3
id: review-result:adversary-grant-subject-pass-1
kind: review-result
status: active
title: Adversary, grant-binds-to-action-subject, pass 1
relations:
- reviews: story:grant-binds-to-action-subject
revision: 1
---
unit: story:grant-binds-to-action-subject, worktree w10-grant-subject at 2bb77b9 (base cc879cb) plus one untracked test file
verdict: CONFIRMED
cases: executed 435→441, red 1
origin: introduced 3 / pre-existing 1 / undecided 0
wrote-outside-worktree: the unit's scratch `adv1/`
needs-coordinator: brief decisions 1 and 2 conflict on software.change/1 release.rollback (decided: refuse); the pre-existing red case is fixed here

**Cases** (`crates/takt/tests/grant_subject_adv1.rs`)

| case | expected | actual |
|---|---|---|
| `a_grant_does_not_survive_a_move_of_an_artifact_its_precondition_reads` (precondition `all: [claim left.ok, evidence {kind: review} with no subject]`) | after `right` moves, `mixed.merge` not admissible | admissible: grant bound to `left-1` (red; also red on the base, bound to `implementation-1`) |
| `a_capability_two_actions_on_two_artifacts_require_is_refused_naming_both` | refused naming both | green (red on a mutant disabling the guard) |
| `grants_on_two_artifacts_authorise_each_its_own_action_only` | each grant authorises its own action | green |
| `an_improvement_turn_is_granted_again_after_its_candidate_moves` | re-grant admissible, one decision | green |
| `a_loop_turn_is_granted_at_its_loop_candidate_and_loses_it_when_it_moves` | bound to loop_candidate | green |
| `the_shipped_protocols_bind_each_capability_to_the_artifact_it_merges` | delivery, software.change → implementation; self.improvement → candidate; loop → loop_candidate | green |

**Gate.** `task check` exit 201 at tests (ess, 319 of 319 citations, fmt, clippy pass); without fail-fast 440 passed, 1 failed.

**Attacked, not broken.** `all`/`any`, nested claims, input-only preconditions, two branches with different subjects, `not`: refused or bound correctly. The protocol read at grant time is the case's pinned `protocol.yaml`, guarded around steps. No cross-capability satisfaction or interference. story.delivery/1 binds to implementation; approvals_only, riders and PrMerged unchanged. The 7 accepted citations point at their text.

```findings
[
  {"file": "crates/takt/src/protocol.rs", "line": 210, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "a precondition evidence or order match naming no subject is dropped, so a grant binds to the other subject and survives a move of an artifact the precondition reads, though grant_subject's doc promises a refusal"},
  {"file": "crates/takt/src/protocol.rs", "line": 173, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "disabling the check for two actions on two subjects leaves the unit's suite green; only the adversary case catches it"},
  {"file": "crates/takt/src/protocol.rs", "line": 144, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "software.change/1 release.rollback, which has no precondition, is now refused where it was granted at implementation"},
  {"file": "ess/domains/takt.yaml", "line": 168, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Approval and authority prose (and the docs at frontier.rs:409 and case.rs:351) still tie the grant to the implementation revision or to a new commit, and the new protocol-read claim cites no protocol.rs line"},
  {"file": "crates/takt/tests/grant_subject.rs", "line": 252, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the final grants_at assertion reads an authority captured before the move and passes even if grants_at always returns false"}
]
```
