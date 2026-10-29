---
format: aep.planning-md/3
id: dependency-blocker:engineering-gates-see-the-change
kind: dependency-blocker
status: cleared
title: engineering gates cannot see the change since the base or be not applicable
relations:
- blocks: story:the-run-shows-each-repository-obligation
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-28T19:04:19Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1,"verification":1}}, executor: "agent:planner"}
---
# Dependency blocker: engineering-gates-see-the-change

## What blocks

`story:the-run-shows-each-repository-obligation` needs an `engineering-gates/1` gate that can test the change since the
base (`git.changed`), declare when it applies (`applies_when`), and report `not_applicable`. That is
engineering-protocols `story:a-gate-sees-the-change-and-may-not-apply` (engineering-protocols store, branch
`plan/takt-trial-review`).

## What clears it

Upstream state: that story's fixtures `git-changed` and `applies-when` and its `not_applicable_is_never_pass` test
pass on its working tree, and takt's workspace builds against that tree through an uncommitted `[patch]` path
override. takt's obligations scenarios are then the end-to-end check. engineering-protocols releases, and takt takes
it as a pin-only hop.

## Cleared

Cleared 2026-10-28 by engineering-protocols 0.12.0 (tag 0.12.0, commit 2bb0eae5). Its assertions library reads a gate's `applies_when` and reports `NotApplicable` (`crates/canon-engineering-assertions/src/lib.rs`), and its providers serve `git.changed` (`crates/assertion-providers/src/git.rs`). takt pins that tag (`Cargo.toml:14`) and has `Status::NotApplicable` (`crates/takt/src/obligations.rs:27`).
