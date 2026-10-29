# Wave 45: a run in a pod stops on tokens, reads its forge through the App and merges under a delegation

Stage: **review fixes green, full gate next**. Shape: one coordinator and implementor sub-agents
in one tree, on the integration branch `integration/goal-a` from 6f7fbd7d (the 0.22.1 release).
The wave readies takt for the cluster example (`examples/cluster-delivery/`): a `LoomAgent` that
runs takt once in a pod, with no `gh` and no person at the merge.

## Units

| unit | story | stage |
|---|---|---|
| U1 | story:a-run-stops-at-a-default-token-budget, story:a-killed-step-records-its-tokens | merged into the branch (4def3240) |
| U2 | story:bot-forge-reads-through-the-app, story:forge-preflight | merged into the branch (4def3240) |
| U3 | story:pr-open-not-idempotent-on-resume | merged into the branch (4def3240) |
| U4 | story:an-unattended-run-merges-under-a-delegated-grant | merged into the branch (4def3240) |

U1 and U2 share `crates/takt-cli/src/main.rs`; U2, U3 and U4 share `crates/takt/src/forge/bot.rs`
and `validator.rs`. The four ran in one tree and were committed together.

## Log

- U1, U2 and U4 ran at once; U3 after U2, since its read goes through U2's `b10x-gates api`.
- U4's first green run left `w40_adv2_regrant` red: U2's preflight refused the bot-forge runs of
  existing CLI tests with no policy. U2 gave those tests a stand-in policy file.
- U1 inverted the last test of `no_action_retried_adv1`: it required a failed try with no usage
  to leave the run's tokens known, and story:a-killed-step-records-its-tokens Acceptance 3 requires
  them unknown.
- U2's preflight replaces Acceptance 4 of story:a-run-refuses-a-scanned-identity-at-open: a bot
  run with no policy no longer opens with its identity not scanned.
- The adversary pass found five cases, none blocking a pod run: a null `tokens` read as absent; a
  policy at mode 0644, through a symbolic link or by a relative path passing the preflight though
  `b10x-gates` refuses it; a pull request with no head repository taken as the case's.
- The security review found two: a grant `agent:takt-delivery` wrote before the owner revoked the
  delegation still merged on a resumed run (blocker), and forge children still received the four
  GitHub token variables though `gh` was gone. Both were fixed; the merge now reads the
  `repository.merge` delegation again for that role.
- A read of 25 commits of this repository's check runs found `total_count` equal to the runs
  listed under both `filter=latest` and `filter=all`, so a re-run job does not trip the bot
  forge's short-page check.
