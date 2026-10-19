---
format: aep.planning-md/3
id: evaluation-plan:review-diff-first
kind: evaluation-plan
status: proposed
title: One-stage review against diff-first then full-code review, on units with known defects
relations:
- serves: vision:safe-agentic-coding
- informed_by: epic:recursive-self-improvement
- informed_by: review-result:main-fe93286-areas
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T01:44:34Z", actor: "human:timo", revision: 2}
---
## Question

Does a two-stage code review (stage 1 reads only the diff; stage 2 reads the full code, given stage 1's findings) find more of the defects that matter, per token, than takt's one-stage review (the reviewer reads the diff and the whole tree in one pass)?

## Context

Owner idea of 2026-10-19: "test regular code-review vs first review diff only, then full code". takt's grants rest on a reviewer verdict at the exact sha, and this session's reviews show both failure modes the stages could address: findings rooted outside the diff (the lazy fetch in a partial clone reached through `git cat-file`, loop-scorecard at f43b0f7; the interpreter takt queries versus the one the script runs, turn 4 at 0a29493) and findings inside the diff that a full-tree reading spent tokens before reaching (the path-component scan, scan-before-push at 792cd2e). `aep plan artifact review-value` reports 71 reviews and 345 findings but no cost (`unknown`), so no comparison exists today.

## Arms

- A, one stage (today): the reviewer brief as used for grant requests; diff and full tree in one pass.
- B, two stages: B1 gets `git diff <base>..<sha>` and the story only, no tree access, and returns findings; B2 gets full tree access, the story and B1's findings verbatim, confirms or refutes each and adds its own.
- Both arms: the same model as today's reviewer, the same story text, no builds and no test runs (static reading plus scratch scripts), so the comparison is about reading order, not about probes. Reviewers are not told which defects are known.

## Corpus (ground truth)

Units reviewed this session whose findings were later fixed with a test that failed before the fix, checked out at the reviewed sha:

| unit | sha | known findings (severity) |
|---|---|---|
| scan-before-push | 2df6d4e | N5 empty-tree entry names (medium), N4 head not a commit (medium) |
| scan-before-push | 792cd2e | forbidden literal with `/` passes the per-component scan (medium) |
| loop-scorecard | 76508b7 | H3 object header lying about its length hangs the reader (high), L5 `size + 1` overflow (low), L6 definitions not capped (low) |
| loop-scorecard | f43b0f7 | lazy fetch outlives the deadline (high), lazy fetch runs `uploadpack` from repo config (high) |
| step-guard-sees-test-script-imports | 0a29493 | interpreter query is takt's python, not the script's (blocker) |
| run-trace-and-report | bd0d91f | report prints control characters (medium), trace header carries home paths (medium), unknown figures print 0 (medium) |

Plus planted defects: per unit, one mutant in a line the diff touches and one in code the diff calls but does not touch, each with a failing input, written by an agent that does not review.

## Measures

Per review: recall of known and planted findings (a finding counts when it names the defect's file within 20 lines and the mechanism, judged by a separate matcher agent against the ground-truth list), recall split by in-diff versus outside-diff defects, precision (each unmatched finding fact-checked against the code: confirmed, unreachable, wrong), tokens and wall time (sub-agent usage).

Repetitions: 2 per arm per unit in the pilot-free run (7 units, 28 reviews); the pilot is 2 units x 2 arms x 1 run, to check the matcher and the briefs.

## Decision rule

Adopt B for grant reviews when its recall of high and blocker findings is at least A's, its overall recall is at least A's, and its median tokens are at most 1.2 times A's; adopt B for outside-diff defects only (B2 as a second pass after A) when B finds outside-diff defects A misses at any cost up to 1.5 times; otherwise keep A. The result is recorded as a verification report and, on adoption, as an edit of `.agents/reviewer-agent.md` reviewed like any judge-surface change.

## Limits

No builds in either arm, so runtime probes (the reviewers' strongest evidence this session) are out of the comparison. Seven units is small; the result says which arm to try next, not a law.

## Pilot results (2026-10-19)

## Pilot results (2026-10-19)

Two units, one run per arm, same model as the grant reviewer, no builds. Ground truth in the planner's scratch file; tokens from sub-agent usage.

| unit (sha) | known defects | arm A: found | A tokens | arm B: found | B tokens (B1+B2) | B / A tokens |
|---|---|---|---|---|---|---|
| scan-before-push (792cd2e) | 1 medium, in the diff | 0 of 1 | 131,898 | 0 of 1 | 286,136 | 2.17 |
| loop-scorecard (f43b0f7) | 2 high + 1 low, outside the diff | 1 of 3 (the low) | 134,734 | 3 of 3 | 310,745 | 2.31 |
| total | 4 | 1 of 4 (0 of 2 high) | 266,632 | 3 of 4 (2 of 2 high) | 596,881 | 2.24 |

New defects not in the ground truth, later sent to the developer:
- A on 792cd2e: a stale tracking ref hides commits from the scan (confirmed by the grant reviewer as a known limit); `scan-text` ignores the policy's exceptions (confirmed).
- B on 792cd2e: UTF-16 files carry a forbidden literal past `scan-text` (verified with b10x-gates 0.1.21).
- A on f43b0f7, and B independently: `filed_at` forgeable through a planted loose commit (replayed by both); A also measured `git log` at 482 MB RSS on a 486 KB plant.
- B on f43b0f7: YAML alias expansion unbounded (inferred); `compute()` arm change passes the definition check; shallow-clone walk error (replayed).

Stage 2 refuted 2 of stage 1's 7 findings on 792cd2e and confirmed 10 of 10 on f43b0f7, so arm B's surviving findings carry a second reading. Arm A's findings had none.

Reading against the decision rule: B's high recall is higher (2 of 2 against 0 of 2) but its tokens are 2.24 times A's, above both thresholds (1.2 for grant reviews, 1.5 for the second-pass use), so the rule adopts nothing yet. Two units and one run per arm cannot carry that decision. Next run: the remaining 5 units with 2 runs per arm, and a third arm C, the grant review A followed by stage 2 given A's findings, which tests whether B's gain comes from the diff-first reading or from the second reading.
