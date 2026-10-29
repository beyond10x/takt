---
format: aep.planning-md/3
id: story:promote-cases-skip-without-an-adapter
kind: story
status: draft
title: The promote cases fail where no Substrate adapter is served, instead of skipping
summary: 20 promote_cli cases fail without DBUS_SESSION_BUS_ADDRESS, which takt's own test environment drops, so a takt run whose checks include the CLI fails on an unrelated change
tags:
- finding
- priority:medium
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# The promote cases fail where no Substrate adapter is served, instead of skipping

## What is wrong

On a host with no user systemd manager reachable (`DBUS_SESSION_BUS_ADDRESS` unset), 20 cases of
`crates/takt-cli/tests/promote_cli.rs` fail with `confinement substrate: no Substrate adapter on
this host ... DBUS_SESSION_BUS_ADDRESS is unset`. Other tests that need the adapter, such as
`local_forge_adv_substrate`, skip there. takt's own test environment
(`confinement::run_test`, the one `--test-cmd` and the local forge's `local_checks` run in) carries
no `DBUS_SESSION_BUS_ADDRESS`, so a takt run on this repository whose checks include the CLI
package fails its checks on a change that does not touch promotion.

Seen in wave 44: U1's `local_checks` ran `cargo test --locked -p b10x-takt-cli` on the local
forge; `promote_cli` gave 67 passed, 20 failed, and 87 passed with the variable set.

## Check

```expected_before
command: env -u DBUS_SESSION_BUS_ADDRESS cargo test --locked -p b10x-takt-cli --test promote_cli
exit: 101
healthy_exit: 0
output: 20 failed
source: crates/takt-cli/tests/promote_cli.rs
```

## Outcome

A promote case that needs the confinement adapter either runs without it (a confinement the case
does not depend on is not asked for) or skips with the reason named, as the local forge's
Substrate case does. The cases that do not depend on confinement pass with the variable unset.

## Acceptance

1. The predicate above exits 0.
2. With `DBUS_SESSION_BUS_ADDRESS` set, `promote_cli` still runs every case it runs today.
