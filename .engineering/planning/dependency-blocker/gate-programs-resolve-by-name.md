---
format: aep.planning-md/3
id: dependency-blocker:gate-programs-resolve-by-name
kind: dependency-blocker
status: open
title: Gate program resolution by name is not in the assertions library yet
relations:
- blocks: story:gate-commands-find-programs-on-a-system-path
revision: 1
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
