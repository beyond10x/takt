---
format: aep.planning-md/3
id: approval-record:cfgproxy-accept-harness-section
kind: approval-record
status: accepted
title: Accepts story harness-section-is-metaharness-session-request with its upstream limits named
tags:
- wave-approval
relations:
- decides: story:harness-section-is-metaharness-session-request
revision: 2
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T01:36:46Z", actor: "human:timo", revision: 2}
---
Decider: operator

Accepts story:harness-section-is-metaharness-session-request for the wave on integration/takt, with its upstream limits
named and accepted as the decision's own:

- metaharness publishes no public session-request document yet (no `deny_unknown_fields` `RunSpec` document, no public
  parse, no public `check_spec` over one). Unblocked by a metaharness story, not yet written.
- Loom's session executor hard-codes the `RunSpec` per kind and takes no caller's partial `RunSpec`. Unblocked by a Loom
  story, not yet written.
- Loom pins llm 0.8.1; the `harness:` mapping cannot reach metaharness's `--effort`, `--max-turns` and similar through
  Loom's executor until the Loom story lands.

Built in this wave: the `harness:` mapping admitting `kind`, `effort`, `max-turns`, `max-budget-usd`, `strict-version`
and `context`, each checked to have arrived at the harness argv or the generated configuration. What stays unbuilt is
recorded in the story itself.

Quoted operator words, 2026-10-27, relayed by the orchestrator: "implement both stories ... merge them into one
integration branch. We're now merging all changes from takt into the same integration branch, and this branch we
push, and from that one we create PRs. Use a cheap haiku to implement that new design and the stories, the epic and
the six draft stories, and then implement them, but do not yet merge it, but push the stuff then."

Not merged to the base branch; no pull request; no release.
