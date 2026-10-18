---
format: aep.planning-md/3
id: story:agent-profiles
kind: story
status: draft
title: Operator agent and developer agent profiles a fresh session starts from
summary: .agents/operator-agent.md, .agents/developer-agent.md and .claude/agents stubs
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: agent-profiles

## Outcome

Each of the two agent roles has a harness-neutral profile a fresh session starts from:
`.agents/operator-agent.md` and `.agents/developer-agent.md`, each saying what to read at start,
what the role owns, what it watches, its loop, its message first lines and what it must never do,
with a pointer stub under `.claude/agents/`.

## Context

design:recursive-self-improvement § Roles, § Agent profiles. On 2026-10-18 the roles existed only
in two sessions' context: messages were held, two sessions shared one name, and neither role's
rules were written where a new session would read them. story:finding-intake and
story:loop-scorecard add their procedures to the operator profile after this story creates it.

## Scope

- new: `.agents/operator-agent.md` (cited by the design)
- new: `.agents/developer-agent.md` (cited by the design)
- new: `.claude/agents/operator-agent.md`, `.claude/agents/developer-agent.md` (pointer stubs) (inferred)
- `AGENTS.md` (one line linking the profiles) (inferred)

## Ordering

- none.

## Acceptance

Each profile contains every "never" line of its role in the design's roles table, word for word.

## Checks

- `b10x-gates scan-text` over both profiles reports nothing: no session, person, decision id or private repository named.
- Each `.claude/agents/` stub names its `.agents/` profile and holds nothing else.

## Out of Scope

Scheduling the agents; the intake procedure (story:finding-intake); the L2 turn (story:loop-scorecard).
