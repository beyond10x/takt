---
format: aep.planning-md/3
id: story:agent-profiles
kind: story
status: active
title: Operator agent and developer agent profiles a fresh session starts from
summary: .agents/operator-agent.md, .agents/developer-agent.md and .claude/agents stubs
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: .agents/developer-agent.md
- confidence: cited
  path: .agents/operator-agent.md
- confidence: inferred
  path: .claude/agents/developer-agent.md
- confidence: inferred
  path: .claude/agents/operator-agent.md
- confidence: inferred
  path: AGENTS.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T12:07:49Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-18T12:07:49Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
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

Derived 2026-10-18 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `.agents/` (new directory: two harness-neutral profiles) — cited, story:agent-profiles Outcome and Scope (`.engineering/planning/story/agent-profiles.md:17-20,31-32`), design § Agent profiles (`.engineering/planning/design/recursive-self-improvement.md:158-161`)
- **Files:** `.agents/operator-agent.md` (new) — cited, story line 31 and design line 160
- **Files:** `.agents/developer-agent.md` (new) — cited, story line 32 and design line 160
- **Files:** `.claude/agents/operator-agent.md`, `.claude/agents/developer-agent.md` (new, pointer stubs) — inferred: the design cites the `.claude/agents/` directory (line 161), but the story itself marks these filenames inferred (line 33); neither `.agents/` nor `.claude/` exists in the tree (`ls` fails)
- **Also likely:** `AGENTS.md` (one link line, probably in the Layout table at `AGENTS.md:7-14`) — inferred, story line 34 marks it inferred, and the line position is my guess
- **Symbols:** none; no code symbol is named or needed — cited, the Acceptance (story line 42) and Checks (lines 46-47) are about text only
- **Documents:** the whole story is documents. The "never" lines to copy word for word are `.engineering/planning/design/recursive-self-improvement.md:40-41`; the text must pass `b10x-gates scan-text` (story line 46, `AGENTS.md:35-36`) — cited
- **Confidence:** high — the story and the design name both profile paths, and the acceptance is entirely about documents
- **Would collide with:** any unit that writes `.agents/operator-agent.md` (story:finding-intake and story:loop-scorecard both do, and both `depends_on` this story: `finding-intake.md:11`, `loop-scorecard.md:12`); any unit editing `AGENTS.md`'s Layout or Rules lines; any unit creating `.claude/` — cited (edges), inferred (`AGENTS.md`/`.claude/`)
- **Safety fact:** no code in the tree reads `.agents/` or `.claude/`, so adding them changes no runtime behaviour. Step 2: `git grep -n -e '\.agents/' -e '\.claude'` over `crates`, `Taskfile.yml` and the tree matches only planning-store text. Unproven.

## Ordering

- none.

## Acceptance

Each profile contains every "never" line of its role in the design's roles table, word for word.

## Checks

- `b10x-gates scan-text` over both profiles reports nothing: no session, person, decision id or private repository named.
- Each `.claude/agents/` stub names its `.agents/` profile and holds nothing else.

## Out of Scope

Scheduling the agents; the intake procedure (story:finding-intake); the L2 turn (story:loop-scorecard).
