---
format: aep.planning-md/3
id: story:test-command-reaches-no-forge-login
kind: story
status: implemented
title: The test command runs with no forge login, keyring or session bus reachable
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
scope:
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T07:39:18Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-19T07:39:18Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-19T14:11:44Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

The test command (and any child of it) runs with a scratch HOME, no `XDG_RUNTIME_DIR`, no `DBUS_SESSION_BUS_ADDRESS` and no access to `/run/user/<uid>/bus`, so `gh`, `git` credential helpers and any keyring client in it find no login of the operator's.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), new finding N1 of the forge and harness fact-check, each link observed: `env -i HOME=$HOME PATH=$PATH gh auth status` still logs in through the keyring with scopes `repo`, `admin:org` and `delete_repo`; the test command runs files the model wrote (`harness.rs:93-95`, `validator.rs:373-376@9cc158b`), so a candidate's test can act on the forge as the operator, including merging its own pull request, which takt then records `merged` (`validator.rs:1517`, the non-open arm). story:confinement-seam's M19 closes HOME and XDG state for the same route; a scratch HOME alone does not close the session bus, which the keyring is reached through.

## Scope

- `crates/takt/src/validator.rs`: the test command's environment and confinement (cited)
- `crates/takt/src/forge.rs`: the environment rules it shares (inferred)

## Acceptance

A test command `sh -c 'gh auth status; echo $DBUS_SESSION_BUS_ADDRESS; ls /run/user/$(id -u)/bus'` run by `takt run --harness none` reports no logged-in account, an empty bus address and no bus socket, while `gh auth status` in the operator's shell still reports the login.
