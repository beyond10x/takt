---
format: aep.planning-md/3
id: story:the-refused-path-proposal-case-holds-on-every-host
kind: story
status: rejected
title: The refused-path proposal case holds on every host
summary: harness_proposes' refused-path case stops at a judge-surface link on the wave's host and passes in CI at the same source
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T02:39:11Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "rejected", at: "2026-11-04T02:39:12Z", actor: "human:timo", revision: 4}
---
# The refused-path proposal case is red on a host where CI is green

## What is wrong

Found by wave 51's unit a, in its red run on unchanged source (base d661e73b1).
`a_scripted_proposal_of_a_refused_path_leaves_the_tree_unchanged`
(`crates/takt/tests/harness_proposes.rs:668`) commits a link `linked` into the scratch tree for every path it tries.
On the wave's host the run stops at step 2 before the harness, so the case's own expectation, a refusal "before the
edit effect", is never reached:

```text
step 1: tests.run {}
  effect: the test command `true`: not run; the case's branch at 78bac467 changed the judge surface since main at 78bac467, pinned at open: linked (a link); no record, the claim stays unknown
step 2: refused before the harness: the judge surface is reached through a link, which takt never follows: linked -> <target>/tmp/proposes-<pid>-parent/elsewhere; no judgement step is taken while one is there
```

The same case passed in CI at 1c3ba5178 (the `check` workflow's test-library job on `main`). Verified: red on the
host with the base source, green in CI at the same source. Not established: which difference between the host and the
runner decides it (inferred candidates: the host's git 2.56.0, the build directory's path).

## Check

```expected_before
command: cargo test --locked -p b10x-takt --test harness_proposes a_scripted_proposal_of_a_refused_path_leaves_the_tree_unchanged
exit: 101
healthy_exit: 0
output: refused before the harness: the judge surface is reached through a link
source: wave 51 unit a red run, a managed worktree of the operator's host
```

## Outcome

The case reaches the refusal it is written for on every host its suite runs on, or it states the host condition it
needs; a link committed by the case is either refused for its own reason or no longer part of every path's fixture.

## Scope

- `crates/takt/tests/harness_proposes.rs:668-720` (cited)
- `crates/takt/src/validator.rs:2205` (the judge-surface comparison that names `linked`) (inferred)

## Duplicate

Duplicates story:a-relative-path-entry-is-no-test-program-directory, filed by wave 50, which names this case and
`decline_adv1::a_decline_after_a_kept_edit_holds_on_resume`. Wave 51's staged gate on `wave/51` at ea4847516 ran
from a shell whose `PATH` had `.` as its fourth entry: both cases red, the second 3 times in 3. On the same build with
every relative entry removed from `PATH`, `harness_proposes` passed 11 of 11 and `decline_adv1` 14 of 14. The git
upgrade named above as a candidate is not the cause. Work it under the earlier draft; this one stays a draft for an
operator to reject.
