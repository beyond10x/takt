---
format: aep.planning-md/3
id: review-result:config-proxy-acceptance-round-1
kind: review-result
status: active
title: 'Acceptance critic, round 1: config-proxy drafts'
tags:
- config-proxy
relations:
- reviews: story:llm-section-is-an-llm-routes-document
- reviews: story:model-names-an-llm-route
- reviews: story:harness-section-is-metaharness-session-request
- reviews: story:run-flags-spell-metaharness-and-llm
- reviews: story:one-run-file-for-host-and-loom-agent
revision: 1
---
needs-revision
story:llm-section-is-an-llm-routes-document — scenarios 1, 2 and 4 expect "the case's opened record names the document's digest", but no drafted story adds a digest to `takt.work.RunOpened` (the Decision adds `llm` only to `RunConfiguration`), so the record has no field to read it from — .engineering/planning/story/llm-section-is-an-llm-routes-document.md:62
story:llm-section-is-an-llm-routes-document — scenario 5 says a run file without `llm` "sets the same run as takt 0.14.0 does" and names no record or output to compare, and a conformance scenario cannot run 0.14.0 — .engineering/planning/story/llm-section-is-an-llm-routes-document.md:70
story:model-names-an-llm-route — scenario 1 reads the endpoint's base URL, wire and upstream model from "the step's recorded `AgentSession`", but loom 0.43.0's `AgentSessions` row holds none of them (it has session_id, kind, commission_id, frontier_id, vendor_session_id, agent, continues, action, files, state); the endpoint is recorded on `RunOpened.endpoint` (`takt.work.HarnessEndpoint`) — .engineering/planning/story/model-names-an-llm-route.md:62 (loom 0.43.0 ess/domains/session.yaml:203-230)
story:model-names-an-llm-route — no scenario names a harness kind, yet takt points an endpoint only at `Kind::B10x` (`endpoint_reference` filters on it), so scenarios 1 and 4 cannot be run, and the outcome for `--model <route>` under claude or codex is unstated — .engineering/planning/story/model-names-an-llm-route.md:62 (crates/takt/src/harness.rs:491)
story:model-names-an-llm-route — scenario 5 says the endpoint file and `--endpoint-*` "behave as in takt 0.14.0" and names no output to compare — .engineering/planning/story/model-names-an-llm-route.md:70
story:harness-section-is-metaharness-session-request — scenarios 1 and 2 read effort and turns from "the step's recorded `AgentSession`", which has no such fields, and scenario 2 names no other observation (no argv or stub record) for "60 turns, not 40" — .engineering/planning/story/harness-section-is-metaharness-session-request.md:72 (loom 0.43.0 ess/domains/session.yaml:203-230)
story:harness-section-is-metaharness-session-request — scenario 4 expects `{kind: codex, max-turns: 60}` to exit 1 "before any case opens" with the refusal cited at `launch.rs:920`, but that refusal is raised in codex's `plan_launch` (`launch.rs:732`) when the session starts, and `check_spec` (`builder.rs:1860-1976`) has no max-turns check, so the case is already open — .engineering/planning/story/harness-section-is-metaharness-session-request.md:75 (metaharness 0.19.0 crates/metaharness-codex/src/launch.rs:919-927)
story:run-flags-spell-metaharness-and-llm — scenario 1 exercises 4 of the 10 new flags (`--model-endpoint`, `--model-wire`, `--max-turns`, `--effort`) and scenario 2 one of the nine aliases, so `--subscription-token-*`, `--api-key-secret`, `--max-budget-usd` and `--strict-version` can break while every item passes — .engineering/planning/story/run-flags-spell-metaharness-and-llm.md:60
story:one-run-file-for-host-and-loom-agent — scenario 4 passes on "an accepted result or a named stop", which any outcome except a crash satisfies, so it cannot tell a working mounted-files run from a refused one — .engineering/planning/story/one-run-file-for-host-and-loom-agent.md:66
story:one-run-file-for-host-and-loom-agent — scenario 3 joins two outcomes (Loom's admission refuses, takt refuses) and names no key of `takt.yaml` that can hold a literal credential, since the run file's only secret-bearing member is the `llm` document already covered by story 1's scenario 4 — .engineering/planning/story/one-run-file-for-host-and-loom-agent.md:64

What I read: 5 of 5 ids in full with `aep plan artifact show <id>`. I checked the acceptance citations against llm 0.16.0 `routes.rs`, loom 0.43.0 `ess/domains/session.yaml` and the 0.44.0 CHANGELOG, metaharness 0.19.0 `launch.rs` and `builder.rs`, and takt `harness.rs` and `ess/domains/takt.yaml`. I did not run `aep plan artifact kinds` or `lifecycle`, and I did not run the CLI validator.

What I could not establish:
- Whether a scenario can start a kind cluster inside `task check` (story 5 scenario 4). That is a test-environment question I did not take up here.
- Whether llm's `Routes::target` handles aliases at `routes.rs:1675`. It is a Decision question, not acceptance.
- Out of my lane, not counted in the verdict (design and parallel-safety):
  - Stories 3 and 4 wait on unreleased metaharness and Loom work.
  - Story 3 introduces a `harness` section that takes claude and codex kinds beside takt's b10x-only endpoint path.

```findings
- file: .engineering/planning/story/llm-section-is-an-llm-routes-document.md
  line: 62
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenarios 1, 2 and 4 expect the case's opened record to name the document's digest, but no drafted story adds a digest to takt.work.RunOpened (the Decision adds llm only to RunConfiguration), so there is no field to read it from"
- file: .engineering/planning/story/llm-section-is-an-llm-routes-document.md
  line: 70
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scenario 5 says a run file without llm sets the same run as takt 0.14.0 does and names no record or output to compare; a conformance scenario cannot run 0.14.0"
- file: .engineering/planning/story/model-names-an-llm-route.md
  line: 62
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenario 1 reads the endpoint's base URL, wire and upstream model from the step's recorded AgentSession, but loom 0.43.0's AgentSessions row holds none of them; the endpoint is recorded on RunOpened.endpoint (takt.work.HarnessEndpoint)"
- file: .engineering/planning/story/model-names-an-llm-route.md
  line: 62
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no scenario names a harness kind, yet takt points an endpoint only at Kind::B10x (harness.rs:491), so scenarios 1 and 4 cannot be run as written and the outcome for --model <route> under claude or codex is unstated"
- file: .engineering/planning/story/model-names-an-llm-route.md
  line: 70
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scenario 5 says the endpoint file and --endpoint-* behave as in takt 0.14.0 and names no output to compare"
- file: .engineering/planning/story/harness-section-is-metaharness-session-request.md
  line: 72
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenarios 1 and 2 read effort and turns from the step's recorded AgentSession, which has no such fields, and scenario 2 names no other observation (argv or stub record) for 60 turns, not 40"
- file: .engineering/planning/story/harness-section-is-metaharness-session-request.md
  line: 75
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenario 4 expects {kind: codex, max-turns: 60} to exit 1 before any case opens with the refusal at launch.rs:920, but that refusal is raised in plan_launch (launch.rs:732) when the session starts and check_spec has no max-turns check, so the case is already open"
- file: .engineering/planning/story/run-flags-spell-metaharness-and-llm.md
  line: 60
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scenario 1 exercises 4 of the 10 new flags and scenario 2 one of the nine aliases, so --subscription-token-*, --api-key-secret, --max-budget-usd and --strict-version can break while every item passes"
- file: .engineering/planning/story/one-run-file-for-host-and-loom-agent.md
  line: 66
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenario 4 passes on an accepted result or a named stop, which any outcome except a crash satisfies, so it cannot tell a working mounted-files run from a refused one"
- file: .engineering/planning/story/one-run-file-for-host-and-loom-agent.md
  line: 64
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scenario 3 joins two outcomes (Loom's admission refuses, takt refuses) and names no key of takt.yaml that can hold a literal credential, since the run file's only secret-bearing member is the llm document already covered by story 1's scenario 4"
```
