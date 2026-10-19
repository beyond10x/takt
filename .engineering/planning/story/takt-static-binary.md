---
format: aep.planning-md/3
id: story:takt-static-binary
kind: story
status: draft
title: A statically linked takt binary is built by a Taskfile target
relations:
- informed_by: review-result:benchmark-acceptance-round-1
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- informed_by: story:prebuilt-binaries-in-ci
revision: 2
---
## Outcome

`task build:static` builds `takt` for `x86_64-unknown-linux-musl`. The binary runs in a benchmark task's container whatever C library the image has, and story:harbor-agent-adapter and story:swe-bench-mini-runner install it there.

## Context

This was split from story:takt-in-container, whose Expected before named this build while its acceptance did not. design:benchmark-integration § How takt fits a benchmark asks for "a statically linked `takt`". The release workflow builds only gnu and darwin targets (`.github/workflows/release.yml:78-82`). The Taskfile has only a native `build` (`Taskfile.yml:17-20`).

Adding the musl target to the release matrix is optional, and it is a separate change if wanted. The check needs the musl target installed (`rustup target add`, network once). Running the binary inside a stock container needs Docker, so it is not part of the acceptance.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -c musl Taskfile.yml` prints 0.

## Scope

- `Taskfile.yml` (inferred)

## Acceptance

After `task build:static`, `file target/x86_64-unknown-linux-musl/release/takt` prints a line matching the regular expression `static(ally|-pie) linked` (`file` 5.48 prints `static-pie linked` for a musl build), where at fb3c3bc `task build:static` is not a task and exits non-zero.
