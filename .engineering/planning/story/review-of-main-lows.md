---
format: aep.planning-md/3
id: story:review-of-main-lows
kind: story
status: draft
title: Low findings from the review of main at fe93286
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
revision: 1
---
## Outcome

Each confirmed low finding below is fixed or recorded as accepted with a reason, one commit each, a test for each fix.

## Context

From the review of main at fe93286 on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas). These held up under the fact-check at low severity (each needs the operator, an out-of-loop actor or a full disk):

- T8 `scorecard.rs:591-608`: `save` creates then writes; a failed write leaves a truncated file that blocks re-save.
- H3.5 `validator.rs:1213,1228`: checks read by pull request number, not sha; an outside push between the two calls records the new head's checks (the merge stays sha-pinned).
- F7.1 `forge.rs:1124-1146` (medium by the fact-check, here because story:ci-wait-watches-the-check rewrites the read): `ci_result pass` is written on the listed checks only; a required check not yet created is missing; the merge is still blocked by the forge.
- F7.8 `validator.rs:1043-1059`: the pull request POST is not idempotent; a create that exits non-zero leaves the number unrecorded and the next run gets 422.
- E8.5 `run.rs:4272-4276`: the model's summary goes into `git commit -m` verbatim; `\n\ntakt: take story:x` makes the take-commit count 0 (`validator.rs:981-992`) so no pull request opens; control characters reach the terminal.
- E8.8 `harness.rs:326-333`: the last-wins parse test has one action line.
- P1.1 `promotion.rs:832-876`: bubblewrap runs without `--new-session`; the confined build keeps the controlling terminal (TIOCSTI is off on this host, `legacy_tiocsti=0`).
- S9.4 `scripts/check-citations:78,157`: the external-citation pattern matches before the in-repository forms, so `takt 0.1.0, crates/missing.rs:9000` passes unresolved.
- S9.7 `scripts/check-citations:70`: a wrapped `# from: Makefile:3` is not flagged.
- X10.6 `protocols/self-improvement/1.yaml:196-222`: a candidate merged and then regressing has no outcome but `abandoned`.
- X10.8 `crates/takt-cli/src/main.rs:509-522`: an intent run defaults `affects_security_boundary=false`, which drops Canon's security review.
- C4.2 `case.rs:317-327`: `Authority::read` does not re-check approvers; a typed `agent:takt` still grants outside judge-surface merges after story:judge-guard D5.
- C4.3 `case.rs:772-778`: `prepare_grant` trusts the highest-numbered decision file.
- C4.7 `case.rs:1405-1410`: case open takes the parent id from the branch suffix without `check_id`.
- R5.2 `run.rs:2967-2971`: `close` drops `Ok(refusals)`; the stop says accepted while the store refused (the refusals are on record).
- R5.4 `run.rs:3549,3563,3591-3592`: errors after a validator's observation return without undo.
- R5.5 `run.rs:1363-1364`: any `cat-file -e` failure drops the repository's judge-surface list.
- R5.7 `case.rs:1529-1531`: no tree-level lock; two runs in one worktree check out over each other.
- GIT-1 `git.rs:150`: `rev-parse --abbrev-ref` says `heads/<b>` when a tag shares the name; a restore then makes `refs/heads/heads/<b>`.
- GIT-4 `git.rs:49-62`: `GIT_NAMESPACE`, `GIT_ATTR_SOURCE`, `GIT_SSH_COMMAND`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG` are not scrubbed.
- GIT-6 `git.rs:548`: 64 MiB per file with no total cap in `Content::of`.
- P1.5 extra crash windows (`promotion.rs:644/645`, `:1636/1637`): go to story:promotion-records-its-toolchain.

Covered elsewhere: F7.5/E8.6 (suffix credential list) by story:environment-allowlist; GIT-8's `refs/replace` route by story:replace-refs-do-not-steer-takts-reads; N2 (a failed step's tokens) by story:failed-step-tokens-are-recorded; X10.1, X10.2, X10.4, X10.7 by story:judge-guard and story:self-improvement-protocol.

## Scope

- the files named above (cited by the fact-check)

## Acceptance

Each item above has a commit with a test that fails before it, or a line in this story's body saying why it is accepted; `task check` passes.
