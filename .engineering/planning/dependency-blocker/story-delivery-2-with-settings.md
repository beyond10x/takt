---
format: aep.planning-md/3
id: dependency-blocker:story-delivery-2-with-settings
kind: dependency-blocker
status: open
title: story.delivery/2 with tests_first and gate_on does not exist yet
relations:
- blocks: story:story-delivery-tests-first
- blocks: story:takt-ships-default-protocol-settings
revision: 2
---
# Dependency blocker: story-delivery-2-with-settings

## What blocks

`story:story-delivery-tests-first` and `story:takt-ships-default-protocol-settings` need the built-in `story.delivery/2`
with the inputs `tests_first` and `gate_on` and the `change.complete` claim. Protocols are engineering-protocols
built-ins since 0.10.0 (takt `Cargo.toml:13`, `b10x-canon-engineering`). That protocol is engineering-protocols
`story:story-delivery-2-with-settings` (engineering-protocols store, branch `plan/takt-trial-review`).

## What clears it

Upstream state: that story's fixtures under `fixtures/story-delivery-2/` pass on its working tree, and takt's
workspace builds against that tree through an uncommitted `[patch]` path override. takt's `gate_deferral` and
`protocol_defaults` scenarios are then the end-to-end check. engineering-protocols releases, and takt takes it as a
pin-only hop.
