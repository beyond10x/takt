---
format: aep.planning-md/3
id: story:takt-runs-its-tests-on-macos
kind: story
status: active
title: takt's scripts and binary run on macOS where only a Linux assumption stopped them
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 3
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
