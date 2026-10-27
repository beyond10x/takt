---
format: aep.planning-md/3
id: approval-record:cfgproxy-d65-delivery-rule
kind: approval-record
status: accepted
title: Records the delivery rule D65 for the remaining run-config units
tags:
- wave-approval
relations:
- decides: epic:run-config-proxies-llm-and-metaharness
revision: 2
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T02:31:48Z", actor: "human:timo", revision: 2}
---
Decider: operator

Records the delivery rule for this wave's remaining units, as relayed by the orchestrator on 2026-10-27 (decision D65,
brief § The process rule 8, changed 02:26Z):

"Config proxy: start the remaining 5 stories now, in parallel (one haiku implementor per story, own tree), as soon as
each story's depends_on is merged (not after its reviews). Each implementor commits at green tests+clippy; each unit
merges into integration/takt and is pushed at green (fetch, merge origin/integration/takt, re-run affected suites,
fast-forward push, never force). Opus adversary and security-reviewer run afterwards, in parallel, against
integration/takt; fixes land there before any PR. My 01:46Z 'one cargo tree at a time' order is withdrawn: builds run in
parallel inside 40G for takt; disk-guard covers the floor. Still no PR, no merge to main."

Coordinator's reading, recorded here so the store carries it: the depends_on edges of the five stories form a chain
(story 2 after 1; 3 after 2; 4 after 1, 2, 3; 5 after 2, 4; 6 after 5). No two of the five can start at once without
breaking those edges, and they share files (main.rs, Cargo.toml, ess/ess-inputs.yaml). So the five run as a pipeline:
each unit starts the moment its own depends_on are merged, not in parallel. Merges are at green tests and clippy.

Still no pull request, no merge to the base branch, no tag, no release.
