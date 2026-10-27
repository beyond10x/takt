---
format: aep.planning-md/3
id: dependency-blocker:scan-and-forge-adapters-not-on-integration
kind: dependency-blocker
status: open
title: The forge adapter seam is not on integration yet
relations:
- blocks: story:a-command-forge-runs-the-configured-commands
- blocks: story:the-gitlab-forge-can-call-the-api-with-a-named-credential
- blocks: story:a-connectors-forge-adapter
revision: 1
---
# Dependency blocker: scan-and-forge-adapters-not-on-integration

## What blocks

`story:a-command-forge-runs-the-configured-commands`, `story:the-gitlab-forge-can-call-the-api-with-a-named-credential`
and `story:a-connectors-forge-adapter` are adapters on the `Forge`/`Runner` seam of `story:scan-and-forge-are-adapters`.
That story is active, and its code and its store record live only on the branch `story/scan-forge-adapters`. That
branch rewrites `crates/takt/src/forge.rs` and `crates/takt/src/forge/gitlab.rs`, so the forge adapters cannot start
on integration, and their depends_on edge cannot be recorded yet.

## What clears it

`story/scan-forge-adapters` is merged into `integration/takt`, and `story:scan-and-forge-are-adapters` is in that
store. At that point each of the three stories gets `depends_on: story:scan-and-forge-are-adapters`, and this blocker
moves to cleared.
