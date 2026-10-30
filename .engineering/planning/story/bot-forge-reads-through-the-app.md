---
format: aep.planning-md/3
id: story:bot-forge-reads-through-the-app
kind: story
status: implemented
title: The bot forge reads pull requests and checks through the App, not gh
summary: a pod has no gh login; reads go through b10x-gates api GET
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:35:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T21:35:59Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-30T00:45:24Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: bot-forge-reads-through-the-app

## Outcome

The bot forge reads a pull request's state and the checks on its head through `b10x-gates api`
(`GET`), under the same GitHub App it writes with, so a run where `gh` is absent or has no login
(a cluster pod) reads, waits for checks and merges as a run on the operator's machine does.

## Context

The bot forge writes through `b10x-gates bot` and `b10x-gates api` but reads through `gh`:
`view_argv` (`gh pr view --json state,mergedAt,headRefName,headRefOid,baseRefName`,
`crates/takt/src/forge/bot.rs:393`) and `checks_argv` (`gh pr checks --json bucket`,
`bot.rs:409`), called from `pull_request` (`bot.rs:566`) and `checks` (`bot.rs:594`). The takt
image ships no `gh` (`Dockerfile`, the final stage's `apt-get install` line), and a pod has no
`gh` login, so a pod run with `forge: bot` can open a pull request and then fail its first read.
`b10x-gates api` takes `--method GET` with no `--input` (`b10x-gates api --help`, 0.1.24).

The reads become `GET /repos/<owner>/<repo>/pulls/<n>` (`state`, `merged`, `head.ref`,
`head.sha`, `base.ref`) and `GET /repos/<owner>/<repo>/commits/<head>/check-runs` with
`GET .../commits/<head>/status`, mapped to the `Checks` buckets the forge has today: a failed,
cancelled or timed-out run or a failure or error status fails; a queued or in-progress run, a
pending status, or no check at all is pending; success, neutral and skipped pass.
story:forge-preflight then needs `b10x-gates` and the policy, and no longer `gh`.

## Expected before

On takt at 421f4303, holds; after the change, fails:

- `grep -c '"gh".to_owned()' crates/takt/src/forge/bot.rs` prints 2.

## Scope

- `crates/takt/src/forge/bot.rs`: `view_argv`, `checks_argv`, `pull_request`, `checks` (cited)
- `crates/takt/tests/`: the forge's read tests with a `b10x-gates` double (inferred)
- `website/docs/reference/configuration.md`: the bot row of the forge table (cited, line 336 at 0.22.1)

## Acceptance

With a `b10x-gates` double first on `PATH` that answers the two `GET` paths from fixture files and
no `gh` on `PATH`, the bot forge reads a pull request as `Open` with its head branch, sha and
base, and reads its checks as `Pass`, `Pending` and `Fail` for the three fixture sets above; the
double's call log shows `--method GET` for each read and no `gh` was run.
