---
format: aep.planning-md/3
id: approval-record:scan-and-forge-adapters
kind: approval-record
status: approved
title: The scan and the forge become named adapters (b10x-gates one of them), and release 0.15.0
tags:
- adapters
relations:
- decides: story:scan-and-forge-are-adapters
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-27T00:33:20Z", actor: "human:timo", revision: 2}
---
# Approval: the scan and the forge become named adapters, and release 0.15.0

## Decision

The owner, 2026-10-27, relayed by the orchestrator: "Why does takt documentation mention
b10x-gates as some standard thing? this is just specific to the takt repo itself, I want to use
takt for any kind of repo" and "fix those two findings. Create a story each, and then fix it." His
chosen option: scan and forge become adapters, b10x-gates one of them.

The orchestrator's scope, the same day: `scan: gates | none`, `none` only with an explicit
declaration in the run configuration given with `--config`, a non-Claude harness refused when it
runs without a scan and without the declaration, the refusal naming the adapter; `bot` stays the
github.com App route; `gitlab` and `none` are documented peers and nothing outside the `bot`
adapter references b10x-gates; the scan's security property is kept: an unscanned proposal is
never passed silently. One story, one unit, an adversary pass, then release 0.15.0.

## Decisions taken under it

| fork | taken | why |
|---|---|---|
| where `none` is declared | the run configuration file only (`scan:` key); no `--scan` flag | the scope: "only with an explicit declaration in the run configuration and `--config`" |
| the default | `gates`, so a run that sets `B10X_GATES_POLICY` today is unchanged | no run that passes today changes |
| `none` with a policy set | refused before the case opens | one adapter per run; a policy no scan reads would read as a scan that ran |
| the refusal | before the case opens, exit 1, for `codex` and `harness` under `gates` without a policy | the scope; today the same run opens its case and refuses every proposal at its step |
| the `bot` forge under `scan: none` | still scans with b10x-gates: the bot route is that adapter | `bot` stays the App route |
| the `gitlab` forge | scans what it publishes through the run's scan adapter | nothing outside `bot` references b10x-gates |
| where the bot adapter lives | `crates/takt/src/forge/bot.rs`, the scenario run's trial lines included | a test can then hold the literal to two files |
| version | 0.15.0, Breaking: a codex or b10x-harness run with no policy is refused at open | it opened its case before |

## Amended

The owner, 2026-10-27, relayed by the orchestrator, after the scope above: "We're now merging
all changes from takt into the same integration branch, and this branch we push, and from that
one we create PRs ... do not yet merge it, but push the stuff."

The orchestrator, the same day: "once task check on the merged tree is green, merge into
integration/takt and push now (fetch, merge origin/integration/takt, re-run affected suites,
fast-forward push, never force). Do not wait for any remaining review. The 2 security findings
and any adversary findings land on integration/takt as fix commits afterwards (or as stories),
before any PR." Both reviews were in before the merge, so their fixes ride the unit itself.

So the unit is merged with `integration/takt` and pushed there by fast-forward through the bot
route. No pull request, no merge to `main`, no tag and no release are made under this record:
release 0.15.0 is pending until the integration branch is merged. The CHANGELOG entries stay
under `[Unreleased]` and the version stays 0.14.0.
