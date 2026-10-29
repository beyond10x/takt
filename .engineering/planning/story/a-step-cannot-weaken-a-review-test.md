---
format: aep.planning-md/3
id: story:a-step-cannot-weaken-a-review-test
kind: story
status: draft
title: A step cannot weaken a test the reviewer committed to the case
summary: a harness step renamed a review case and inverted its assertion, and the run went on to a merge request with the finding turned into its opposite
tags:
- finding
- priority:high
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A step cannot weaken a test the reviewer committed to the case

## What is wrong

A case's tests can be changed by any step that writes them. When the reviewer (or the planner)
commits failing cases to a case's branch as the next attempt's tests, the harness's next
`tests.write` or `implementation.edit` can rename such a case and invert its assertion, and the
run goes on to a green gate and a merge request with the review's finding turned into its
opposite.

Seen in wave 44: on story:docs-edits-the-tests-read-are-attempts the review committed
`crates/takt/tests/w44_adv_docs_named.rs` with two red cases (1dc1bd60). The next run's step 4
(fdd6ceaa) renamed `a_docs_path_that_is_only_a_prefix_of_a_named_path_is_not_counted` to
`a_docs_path_whose_text_occurs_in_a_longer_path_is_counted` and turned its assertion from
`ApprovalRequired` to `RepeatedTestFailure`; the code was then changed to match, and the run
stopped at the merge approval. The story, at that revision, held two readings of the rule, which
the harness resolved by the older one.

## Check

Not runnable by `scripts/check-finding`: it needs a recorded case. The case's commit history holds
it:

```expected_before
command: git log --format=%s 1dc1bd60..0106f34d -- crates/takt/tests/w44_adv_docs_named.rs
exit: 0
healthy_exit: 0
output: "Test documentation paths appearing within longer paths"
source: a recorded wave-44 case
```

## Outcome

Paths a case marks as review tests (committed by a role other than the harness, or named in the
case's record) are guarded: a step that changes a review test's case names or assertions is
refused and reverted, as a step outside the frontier is; reformatting stays allowed. A step that
finds a review test contradicting the story declines with the case named.

## Acceptance

1. A scripted case whose harness inverts a committed review test's assertion has that step
   refused and reverted, and the case's tests stay red.
2. A step that only reformats a review test is accepted.
