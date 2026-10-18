---
format: aep.planning-md/3
id: story:four-agent-roles
kind: story
status: implemented
title: 'Four agent roles: planner, developer, reviewer, doc-writer'
summary: the operator role splits into planner (main agent), reviewer and doc-writer beside the developer; profiles, stubs, AGENTS.md and the design agree
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: .agents/developer-agent.md
- confidence: cited
  path: .agents/doc-writer-agent.md
- confidence: cited
  path: .agents/planner-agent.md
- confidence: cited
  path: .agents/reviewer-agent.md
- confidence: cited
  path: .claude/agents/planner-agent.md
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: protocols/self-improvement/1.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T15:28:12Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T15:28:12Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-18T15:28:12Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: four-agent-roles

## Outcome

Four agent roles work on takt, each with a profile a fresh session starts from and a repo-local agent that loads it: the planner (the main agent and orchestrator until takt drives this flow itself), the developer, the reviewer and the doc-writer. Each role's ownership and its "never" lines are written in `AGENTS.md`, its profile and design:recursive-self-improvement § Roles, and all four run: the planner as the main session, the developer as its own session, the reviewer and the doc-writer started by the planner per task.

## Context

Owner decision of 2026-10-18: "we should maybe also have 3 roles here: dev, planner, reviewer, doc-writer (4 actually) - make this happen and make sure all those 4 different roles are running ... the planner will become the main agent, orchestrator leading all of this (until this whole flow later is driven by takt itself)". Until now two roles were written down (`.agents/operator-agent.md`, `.agents/developer-agent.md`, story:agent-profiles): one operator role planned, reviewed, verified, measured and wrote docs, and the owner decided earlier the same day that docs changes are done by that session with sub-agents rather than through the developer's waves.

The split: the planner plans, files findings, decides what the developer escalates and grants merges on a reviewer verdict; the reviewer reviews each grant request at its exact sha, runs independent and adversarial reviews, trials and verification runs; the doc-writer owns `docs/**`, README structure, the public site and the pitch deck. The developer is unchanged. The messages the agents have used in practice (`[WAVE]`, `[GRANT?]`, `[GRANT]`, `[DECIDE]`, `[DECIDED]`, `[ACK]`) are written down beside the three the design named.

Every file in scope is a judge surface; the change is the owner's own request, and the planner delivers it.

## Expected before

On takt at main, holds; after the change, fails:

- `ls .agents/reviewer-agent.md .agents/doc-writer-agent.md .agents/planner-agent.md` exits non-zero.

## Scope

- `AGENTS.md`: the roles rule, the messages, Start here (cited)
- `.agents/planner-agent.md`, `.agents/reviewer-agent.md`, `.agents/doc-writer-agent.md` (new), `.agents/developer-agent.md`, `.agents/operator-agent.md` (retired) (cited)
- `.claude/agents/*.md` stubs (cited)
- design:recursive-self-improvement § Roles (cited)

## Acceptance

`AGENTS.md`, the four profiles and design:recursive-self-improvement § Roles name the same four roles with the same owns and never lines, `.agents/operator-agent.md` is gone, each role has a `.claude/agents/` stub, and on the day of the change one task was run under each role: a reviewer verdict on a grant request, a doc-writer change under `docs/`, the developer's wave work and the planner's grant.
