---
format: aep.planning-md/3
id: story:takt-runs-its-tests-on-macos
kind: story
status: active
title: takt's scripts and binary run on macOS where only a Linux assumption stopped them
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T13:22:17Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T13:22:17Z", actor: "human:timo", revision: 3}
---
## Outcome

takt's own scripts and binary run on macOS where nothing but a Linux-only assumption stopped them:
the finding scripts run under bash 3.2, takt reads its running binary without `/proc`, and the
tests that plant a non-UTF-8 name or a wrapped or inherited interpreter exercise their guard on
APFS and the macOS layout. Linux behaviour is unchanged.

## Context

The darwin CI job (`.github/workflows/darwin.yml`, wave/28) at 6ec2c58, run 39604433453 on
macos-15: test-library 997 passed, 140 failed, 5 ignored; test-cli 317 passed, 160 failed, 1
ignored. Causes counted from the job log:

| failures | cause | here |
|---|---|---|
| 47 | bash 3.2 has no `declare -A` | yes |
| 39 | `/proc/self/exe` does not exist | yes |
| 7 of 17 | non-UTF-8 names refused by APFS (EILSEQ); wrapper and interpreter tables | yes |
| 160 | aep 0.74.0 cannot publish its protocol cache on macOS | no: aep's fix |
| 37 | bubblewrap missing, promotion refuses | no: macOS confinement is the owner's decision |
| 10 of 17 | lock timing and other single causes | no |

## Acceptance

1. `bash -n` and the finding-script tests pass on Linux with no `declare -A` left in
   `scripts/check-finding` and `scripts/file-finding`.
2. On Linux takt reads its binary from `/proc/self/exe` as before; on other platforms from
   `std::env::current_exe()`, an error naming the path when it cannot.
3. The darwin job on the wave's last unit shows none of the failures this story names, counted
   per cause; `task check` on Linux stays green.

## Out of scope

Any test skip, relaxed refusal or longer timeout; macOS confinement; the release.

## Decisions (2026-10-23, the dispatcher)

The unit's implementor left three failures it could not settle inside this story's scope. Each is
decided here before U2 starts.

| test | macOS cause | decision |
|---|---|---|
| `a_non_utf8_name_planted_is_refused_and_removed` | APFS refuses the name itself (EILSEQ), so the test cannot plant it | U2 asserts the platform's refusal on macOS: planting fails with EILSEQ and nothing is created; the guard's own refusal stays asserted on Linux. No skip |
| `the_hidden_alias_bomb_…` | `setrlimit(RLIMIT_AS)` returns EINVAL on macOS | U2: the memory bound on macOS comes from a limit the kernel enforces there, or the step refuses by name; the bound is never dropped silently |
| `the_test_command_reaches_no_login_of_the_operator` | no namespace hides the logins on macOS | U2: Seatbelt denies the reads (story:takt-confines-on-macos, row `without_login`) |

U1's commit 3e1f9e0 left these for U2 as well, each passing vacuously or reading a Linux path on
macOS: `getent` in `scripts/check-finding` and the `finding_scripts` tests; `SOCKET_PROBE`'s reads
of `/proc/self/ns` and `/proc/net`; `finding_scripts`' read of `/proc/self/fd`; the `/proc`-based
`alive()` checks in the `hours_bounds_*` tests, `step_cannot_forge_its_validators.rs` and
`trial_fetch_adv1.rs`.
