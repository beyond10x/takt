---
format: aep.planning-md/3
id: review-result:config-proxy-scope-round-2
kind: review-result
status: active
title: 'Scope critic, round 2: config-proxy drafts'
tags:
- config-proxy
relations:
- reviews: story:llm-section-is-an-llm-routes-document
- reviews: story:model-names-an-llm-route
- reviews: story:harness-section-is-metaharness-session-request
- reviews: story:run-flags-spell-metaharness-and-llm
- reviews: story:one-run-file-for-host-and-loom-agent
- reviews: story:harness-flags-spell-metaharness-names
revision: 1
---
needs-revision
story:llm-section-is-an-llm-routes-document — the outcome adds a recorded `llm_digest` on the `opened` record (and a new `takt.work.RunOpened` field) that no sentence of the epic asks for; drop it, or have the epic state it as a promise — .engineering/planning/story/llm-section-is-an-llm-routes-document.md:38

**What you read.** 8 artifacts, 7 promises extracted (the run file's `llm` section as an llm.routes/1 document; the `harness` section as metaharness's session request; takt adding only its own keys; model routes configured in the run file; Claude and Codex configured the metaharness way; the same flag names as metaharness and llm; one file for a host run and a LoomAgent). All 7 trace to an item. Commands run: `aep plan artifact show` on the epic and all six stories, and `aep plan artifact graph`. I also grepped `.engineering/planning` for `llm_digest`, which only the three stories carry.

**What you could not establish.**
- The harness story admits six of metaharness's `RunSpec` keys. The epic's outcome reads "a `harness` section that is metaharness's session request". I did not call that a silent narrowing, because the story names it in its Decision and its Out of Scope. It stays a point for the operator to see.
- The story that renames the endpoint flags drops `--subscription-token-env` and uses `--llm-config` instead of llm's `--config`. Both are named in its Decision, so they are not silent narrowings.
- Whether llm itself refuses a literal `auth.secret` (scenario 4 of the llm-section story) is acceptance-lane, so it does not set my verdict.
- Two upstream stories, metaharness's session-request document and Loom's executor overlay, are named in the epic's Out of Scope, so they are not gaps.
- Out of my lane: what happens when `--model-endpoint` and an `llm` section both set the endpoint. No story states the precedence, which is a design-critic question.

```findings
- file: .engineering/planning/story/llm-section-is-an-llm-routes-document.md
  line: 38
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome adds a recorded llm_digest on the opened record (and a new takt.work.RunOpened field) that no sentence of the epic asks for; drop it, or have the epic state it as a promise"
```
