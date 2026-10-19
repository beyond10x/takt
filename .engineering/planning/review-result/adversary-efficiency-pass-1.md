---
format: aep.planning-md/3
id: review-result:adversary-efficiency-pass-1
kind: review-result
status: active
title: Adversary, efficiency unit, pass 1
relations:
- reviews: story:pr-opens-when-change-is-done
revision: 1
---
# Adversary pass 1: the efficiency unit

Unit at 22a8389 (impl/efficiency, from 8b0afc5): story:pr-opens-when-change-is-done,
story:no-model-step-for-approval, story:no-inspect-for-stateless-harness. Verdict: NEEDS-CHANGE, 4
red cases in `crates/takt/tests/efficiency_adv1.rs`; 530 → 534 executed.

Attacked without a break: a choice step that changes files (untracked, mode change; the .git,
HEAD, refs, index and ignored-file guards run before the recorder); non-choice evidence by naming
an action in story-delivery; replaying a choice record at a later revision; `pr.open` twice at one
revision; `ci.read` without a pull request; a false approval stop in story-delivery; the stateless
filter; ESS prose against the code; software.change/1 unchanged.

```findings
[
  {
    "file": "protocols/story-delivery/1.yaml",
    "category": "correctness",
    "severity": "warning",
    "message": "change.handover is offered at the seed, so pr.open runs before the first implementation commit: a harness hands over at the seed; pr.open is taken at the base revision before any commit (acceptance of story:pr-opens-when-change-is-done)",
    "line": 129
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "choice-kind rule inferred in Rust also matches observation kinds: a protocol reads ci_result without a result and declares ci.read effect read; with no forge validator the harness names ci.read and takt writes ci_result as takt.choice, and the case is accepted",
    "line": 237
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "approval stop missed when another producer of the claim is blocked by a case input: merged is produced by merge.normal (approval-required) and merge.judge (blocked by input surface); the harness is called instead of stopping ApprovalRequired",
    "line": 349
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "choice kind read without a subject is offered, then the step errors: a protocol reads kind ack without a subject; ack.give is offered to a stateless harness and the run ends on an error after the step",
    "line": 2031
  },
  {
    "file": "AGENTS.md",
    "category": "correctness",
    "severity": "note",
    "message": "AGENTS.md still says every claim needs a validator: change.handed_over is decided by a takt.choice record while the rules say evidence comes from validators only",
    "line": 49
  }
]
```
