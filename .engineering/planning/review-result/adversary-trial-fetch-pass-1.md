---
format: aep.planning-md/3
id: review-result:adversary-trial-fetch-pass-1
kind: review-result
status: active
title: Adversary, trial-fetch-needs-no-user-ssh, pass 1
relations:
- reviews: story:trial-fetch-needs-no-user-ssh
revision: 1
---
unit: story:trial-fetch-needs-no-user-ssh, worktree w10-trial-fetch at 8a28b46 (base 9bd9d18) plus one untracked test file
verdict: INFEASIBLE
cases: executed 464→468, red 2
origin: introduced 0 / pre-existing 2 / undecided 0
wrote-outside-worktree: 6 paths in the unit's scratch `adv1/`
needs-coordinator: none

**Cases** (`crates/takt-cli/tests/trial_fetch_adv1.rs`; a `b10x-gates` stand-in runs git as the bot route does at 0.1.21, a stand-in `ssh` records and refuses)

| case | expected | actual |
|---|---|---|
| `a_repository_insteadof_rule_does_not_send_the_trial_fetch_to_the_users_ssh` | fetch from `https://github.com/owner/repo.git`, ssh never started | `ssh://git@github.com/owner/repo.git`, ssh started with BatchMode, exit 1 (red; red on the base too) |
| `the_trial_fetch_child_reaches_no_user_askpass` | `GIT_ASKPASS` empty or both askpass unset | `GIT_ASKPASS=/user/editor-askpass.sh`, `SSH_ASKPASS=/user/desktop-askpass` (red; red on the base too) |
| `a_timed_out_trial_fetch_leaves_no_process_behind` | a `sleep` in its own session gone after the kill | gone, 30.07 s |
| `origin_forms_map_to_one_https_url_and_no_userinfo_travels` | 8 origin forms map to one HTTPS URL; no token travels; non-GitHub or missing origin refused | as expected |

**Gate.** `task check` exit 201 at tests: ess valid, 367 of 367 citations, fmt, clippy clean; `trial_fetch_adv1` 2 passed, 2 failed; `scenario_run` 14, `forge_cli` 3 pass.

**Attacked, not broken.** `SSH_AUTH_SOCK` removed; a repository credential helper is reset by gates (the bot answers `x-access-token`); `core.sshCommand` loses to `GIT_SSH_COMMAND`; `GH_TOKEN`/`GITHUB_TOKEN` passed by design and not read by git; URL derivation correct with userinfo dropped and never printed; `bot -- fetch <URL>` accepted; the 30 s bound kills the group and a process in its own session; half-written refs not constructible; the two hand-re-pointed citations hold; the implementor's three tests assert literal values.

```findings
[
  {"file": "crates/takt-cli/tests/trial_fetch_adv1.rs", "line": 317, "category": "acceptance", "severity": "warning", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "an insteadOf rule in the repository's own git config sends the HTTPS trial fetch to the user's ssh, which reads ~/.ssh config and passphrase-less keys; GIT_ALLOW_PROTOCOL=https on the fetch child closes it"},
  {"file": "crates/takt-cli/tests/trial_fetch_adv1.rs", "line": 349, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "pre-existing", "message": "GIT_TERMINAL_PROMPT=0 leaves the user's GIT_ASKPASS and SSH_ASKPASS reachable by the forge child's git; an empty GIT_ASKPASS closes it"}
]
```
