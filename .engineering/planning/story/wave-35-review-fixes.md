---
format: aep.planning-md/3
id: story:wave-35-review-fixes
kind: story
status: draft
title: The macOS confinement holds what wave 35 says
relations:
- depends_on: story:takt-confines-on-macos
revision: 2
---
## Outcome

The reviewer's findings on wave 35 at 9adc328 (verdict changes-required) are fixed or recorded,
so the macOS confinement holds what story:takt-confines-on-macos and the CHANGELOG say.

## Decisions (2026-10-23, the dispatcher)

| finding | decision |
|---|---|
| H1 Seatbelt profiles allow `job-creation`, `lsopen`, `appleevent-send` and launchd `mach-lookup`, so `launchctl submit` runs a job outside the sandbox | fix: every takt profile (build, test command, check-finding run) denies them |
| M2 the test command reaches the launchd ssh-agent socket | fix: deny connecting to `/private/tmp/com.apple.launchd.*` sockets |
| M3 release.yml ships the darwin binary gated only by Linux checks | fix: darwin.yml becomes callable, release.yml's publish job needs it |
| M4 `DARWIN_USER_TEMP_DIR` and `DARWIN_USER_CACHE_DIR` writable by confined steps | fix: each confined step gets a private `TMPDIR` inside its tree and the two directories are not writable; if a build needs them, darwin.yml says which |
| M5 the test command may write the common `.git` hooks and config | fix: deny writes to `hooks/` and `config` in the common git dir |
| L6 `sweep_orphans_since` does not take `forge::sweeping()` | fix: it takes the lock |
| L7 pid reuse between listing and kill | accepted: macOS pids are sequential, the window is the listing's |
| L8 two unreviewed check-finding digests | fix: keep only `6c39e63b…` |
| L9 finding_scripts admits any `DYLD_FALLBACK_LIBRARY_PATH` and unbounded hex fields | fix: plant a value and assert it is gone; bound the fields |

## Acceptance

1. On macOS: a confined build and test command running `launchctl submit -l t -- /usr/bin/touch "$HOME/escaped"; sleep 2` leaves no `$HOME/escaped` (H1, test in `crates/takt/tests/macos_confinement.rs`).
2. On macOS: a test command connecting to a socket under `/private/tmp/com.apple.launchd.*` fails (M2).
3. On macOS: a confined build writing `$(getconf DARWIN_USER_CACHE_DIR)/planted` fails and the file does not exist (M4); writing the common git dir's `hooks/probe` fails (M5).
4. `release.yml`'s publish job needs a macOS test job (M3).
5. `CONFINED_CHECK_FINDING_SHA256S` holds no `cd0efbd5` and no `0c2b3048` (L8).
6. `task check` and darwin.yml are green.

## Darwin at a5d6878 (run 39921796824, 2026-10-23)

test-cli passed; test-library hit the 60-minute timeout. Failures: 4 in
`cases_root_link.rs:418` (writes beside the tree, denied by M4), 1 in
`ignored_directories_are_guarded_adv3.rs:612` (plants `.git/hooks/x`, denied by M5),
and `a_test_command_connects_to_no_launchd_socket` hung. Decision: the tests change, the
profiles keep M4 and M5; every subprocess in the macOS module gets a bound.
