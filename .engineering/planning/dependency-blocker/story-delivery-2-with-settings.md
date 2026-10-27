---
format: aep.planning-md/3
id: dependency-blocker:story-delivery-2-with-settings
kind: dependency-blocker
status: open
title: story.delivery/2 with tests_first and gate_on does not exist yet
relations:
- blocks: story:story-delivery-tests-first
- blocks: story:takt-ships-default-protocol-settings
revision: 1
---
# Dependency blocker: story-delivery-2-with-settings

## What blocks

`story:story-delivery-tests-first` and `story:takt-ships-default-protocol-settings` need the built-in `story.delivery/2`
with the inputs `tests_first` and `gate_on`. Protocols are engineering-protocols built-ins since 0.10.0 (takt
`Cargo.toml:13`, `b10x-canon-engineering`). That protocol is engineering-protocols `story:story-delivery-2-with-settings`
(engineering-protocols store, branch `plan/takt-trial-review`).

## What clears it

A takt story run with a fake harness, on a workspace with an uncommitted `[patch]` path override to that working tree,
shows red before the edit and one gate run. engineering-protocols then releases, and takt takes it as a pin-only hop.
