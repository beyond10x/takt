---
format: aep.planning-md/3
id: dependency-blocker:gate-programs-resolve-by-name
kind: dependency-blocker
status: cleared
title: Gate program resolution by name is not in the assertions library yet
relations:
- blocks: story:gate-commands-find-programs-on-a-system-path
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-28T19:04:20Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"verification":1}}, executor: "agent:planner"}
---
# Dependency blocker: gate-programs-resolve-by-name

## What blocks

`story:gate-commands-find-programs-on-a-system-path` leaves open whether its bare-name default lives in takt or in the
assertions library. That decision is now taken in engineering-protocols `story:gate-commands-name-a-program-portably`
(engineering-protocols store, branch `plan/takt-trial-review`):
- the library owns program resolution, a `program` name resolved on a `search_path`;
- the runtime may supply a default `search_path` for a project that declares none;
- takt supplies the fixed system path.

takt's acceptance (a bare `sh` with no `env` runs) therefore needs that binding.

## What clears it

Upstream state: that story's named tests pass on its working tree, including the runtime-supplied default
search path. takt's workspace builds against that tree through an uncommitted `[patch]` path override, and takt's
`gate/bare-program-name-runs` scenario is then the end-to-end check. engineering-protocols releases, and takt takes it
as a pin-only hop.

## Cleared

Cleared 2026-10-28 by engineering-protocols 0.12.0 (tag 0.12.0, commit 2bb0eae5). Its providers resolve a bare `program` name on a `search_path`, and take a runtime-supplied default when the project admits none (`crates/assertion-providers/src/process.rs`, the program resolution). takt pins that tag (`Cargo.toml:14`); supplying the fixed system path is story:gate-commands-find-programs-on-a-system-path's work.
