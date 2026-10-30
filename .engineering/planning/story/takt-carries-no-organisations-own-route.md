---
format: aep.planning-md/3
id: story:takt-carries-no-organisations-own-route
kind: story
status: implemented
title: takt carries no organisation's own delivery route
summary: Remove --forge bot, the bot route of scenario run and the private-policy gates scan; a public agent drives no organisation's App
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T03:08:03Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T03:08:03Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-30T09:40:58Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":5}}}
---
# takt carries no organisation's own delivery route

## Context

takt is a public coding agent. `--forge bot` (`crates/takt/src/forge/bot.rs`, `BotRoute`) drives
one organisation's own GitHub App through `b10x-gates bot` and `b10x-gates api`, under a policy
file named by `B10X_GATES_POLICY` that only that organisation holds; `takt scenario run` creates
its trial base only through that route, and the `gates` scan adapter (`crates/takt/src/scan/gates.rs`)
scans under the same private policy. None of it can work for anyone else, and a forge that holds
an App key beside the harness session contradicts the rule that the agent never sees a secret
(the forge's writes belong behind a relay that holds the credential: the connectors forge,
story:connectors-forge-speaks-github-at-an-endpoint). Owner's decision, 2026-10-30.

## Outcome

No code path, flag value, run-file value, environment variable, document page or example of takt
names or drives that organisation's bot route, its App, `b10x-gates` or `B10X_GATES_POLICY`.
`--forge` offers `none`, `local`, `gitlab`, `command` and `connectors`. `takt scenario run`
creates its trial base without the bot route (on the scenario checkout's `origin` with the
operator's own git, or locally under `--forge none`), keeping trial numbers unique as before.
The scan seam keeps `none` and any adapter that reads no organisation's private policy. The
delegated merge grant (`agent:takt-delivery`, `.engineering/delegations.yaml`) stays: it names
no organisation.

## Acceptance

1. `--forge bot` is refused by clap as an invalid value (exit 2), and `takt run --help` lists
   no `bot`.
2. A run file with `forge: bot` is refused before the case opens, naming the file and line.
3. `takt scenario run` creates `trial/<n>` and runs a scenario without `b10x-gates` on `PATH`
   (a CLI test with a bare `origin`).
4. The predicate below exits 1, its `healthy_exit`.
5. `task check` passes; the ESS specification declares no bot forge and its citations resolve.

```expected_before
repository: https://github.com/beyond10x/takt
seed: be894749a84001bf8f4d69e7ea9c5657893c4522
command: git grep -n -i -E 'forge bot|forge: bot|b10x-gates|B10X_GATES_POLICY|BotRoute|bot route' -- crates protocols website/docs examples README.md ess > /dev/null && { echo ORG_ROUTE_IN_TAKT; exit 0; }; "$TAKT" run --help | grep -q -w bot && { echo ORG_ROUTE_IN_TAKT; exit 0; }; exit 1
exit: 0
healthy_exit: 1
output: ORG_ROUTE_IN_TAKT
source: docs/waves/wave-46.md
```

## Scope

- crates/takt/src/forge/bot.rs (removed), crates/takt/src/forge.rs, crates/takt/src/scan/gates.rs
- crates/takt-cli/src/main.rs (forge argument, run file, scenario run, preflight)
- crates/takt/tests, crates/takt-cli/tests (tests of the bot forge removed; tests that used it as
  a stand-in moved to `local` or `none`)
- ess/domains/takt.yaml, website/docs/**, examples/**, README.md, CHANGELOG.md
