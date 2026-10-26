---
format: aep.planning-md/3
id: story:image-cargo-home-is-writable
kind: story
status: implemented
title: cargo in the image writes its caches under the user's HOME by default
summary: no CARGO_HOME in the final image; HOME outside /home
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: cited
  path: Dockerfile
- confidence: cited
  path: README.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-26T16:16:36Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: image-cargo-home-is-writable

## Outcome

`cargo` in takt's image writes its registry and git caches to a directory the run's user owns,
by default, with no environment the deployment has to set.

## Context

- The rust base image sets `CARGO_HOME=/usr/local/cargo`, owned by root; under Loom the root is
  also read-only. The trial set `CARGO_HOME` and `CARGO_TARGET_DIR` under the workspace volume by
  hand in its wrapper (run 7).
- Loom's `Command` variant sets `HOME` to `/var/lib/agent`, a writable emptyDir; a plain
  `docker run` keeps the image's `HOME`, `/var/lib/takt`. Both are outside `/home`, which the
  shared security scan refuses in any committed path.

## Acceptance

1. The final image sets no `CARGO_HOME`, so cargo uses `$HOME/.cargo`; `RUSTUP_HOME` is not needed
   at run time.
2. In the built image, as uid 10001 with `HOME` set to an empty writable directory, `cargo new`
   and `cargo build` of a crate with no dependencies exit 0 and write only under that `HOME`
   and the crate.
3. The image's user home is `/var/lib/takt`, not under `/home`.

## Scope

- `Dockerfile` (final stage), `README.md` § Container.
