---
format: aep.planning-md/3
id: story:judge-surface-delegation-text
kind: story
status: implemented
title: Agent rules and protocols state the owner's delegation of judge-surface grants
summary: AGENTS.md, the operator profile and both improvement protocols carry the delegation-on-record rule of the owner decision at 3ad0bd9
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: .agents/developer-agent.md
- confidence: cited
  path: .agents/operator-agent.md
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/takt/tests/improvement_protocols.rs
- confidence: cited
  path: docs/design/takt.md
- confidence: cited
  path: protocols/loop-improvement/1.yaml
- confidence: cited
  path: protocols/self-improvement/1.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T14:01:46Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T14:01:46Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-18T15:23:35Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: judge-surface-delegation-text

## Outcome

The rules an agent reads say what the owner decided: a grant reserved for the owner, judge surfaces included, may be delegated to a named role by a revocable record. `AGENTS.md` § Rules, the operator profile and the two improvement protocols no longer say that only a person, or only the owner, may grant a judge-surface merge.

## Context

Owner decision recorded at 3ad0bd9: product-requirements:takt § 2 (delegation on record) and R6, design:recursive-self-improvement § Roles and § Judge. A freshness review of 2026-10-18 found these files still carrying the old rule:

- `AGENTS.md` § Rules: "An agent never grants the merge of a diff that touches a judge surface; a person does."
- `.agents/operator-agent.md`, § Never: "grants on a diff that touches a judge surface".
- `protocols/self-improvement/1.yaml:164` and `protocols/loop-improvement/1.yaml:14,121`: only the owner holds the judge-surface capability.

The enforcement of a delegation record is story:judge-guard; this story changes text only. It also carries four lines the developer role parked for `AGENTS.md` (the bot-route commit and scan-text commands, and the rule that a number in a commit message or wave page is the output of a command run on that tree); the two Commands lines go with story:citation-check-in-task-check, which owns the Commands block, and the rule goes here.

Every file in scope is a judge surface, and the change widens who may grant judge-surface merges, including the operator role; its merge cites the owner's recorded decision, 3ad0bd9, and is not merged on the operator role's grant alone.

## Expected before

On takt at b997031, holds; after the change, fails:

- `grep -c "a person does" AGENTS.md` prints 1.
- `grep -c "grants on a diff that touches a judge surface" .agents/operator-agent.md` prints 1.

## Scope

- `AGENTS.md`: § Rules (cited)
- `.agents/operator-agent.md`: § Never, § Owns (cited)
- `protocols/self-improvement/1.yaml` (cited)
- `protocols/loop-improvement/1.yaml` (cited)

## Acceptance

`AGENTS.md` § Rules, `.agents/operator-agent.md` and both improvement protocols state the delegation rule of product-requirements:takt § 2, naming the record, its revocation and story:judge-guard as its enforcement, and none of them says that only a person or only the owner may grant a judge-surface merge; `task check` passes.

## Landed (wave 09)

Read from `git diff --stat 8ce2019 50e695a` at the close of wave 09 (wave branch head 1425689). Merged on the owner's recorded decision, not on the operator role's grant. Also changed: `.agents/developer-agent.md`, `README.md`, the L2 row of `docs/design/takt.md`, one test name.

- `.agents/developer-agent.md`
- `.agents/operator-agent.md`
- `AGENTS.md`
- `README.md`
- `crates/takt/tests/improvement_protocols.rs`
- `docs/design/takt.md`
- `protocols/loop-improvement/1.yaml`
- `protocols/self-improvement/1.yaml`
