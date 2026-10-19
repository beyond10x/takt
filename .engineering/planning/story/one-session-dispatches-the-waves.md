---
format: aep.planning-md/3
id: story:one-session-dispatches-the-waves
kind: story
status: draft
title: One dispatcher session plans and delivers takt's waves from one checkout
summary: 'the owner''s process change of 2026-10-20: one session, waves of 3-5 stories, sub-agents in a shared tree, one review and one gate per wave'
tags:
- process
relations:
- decomposes: epic:continuous-orchestration
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: one-session-dispatches-the-waves

## Outcome

One agent session, the dispatcher, plans and delivers takt's waves from one checkout: it picks a
wave of three to five stories, dispatches implementor sub-agents into the shared tree, commits
each unit, runs one adversarial review and one full gate per wave, and merges the wave into `main`
on a grant. Its profile is `.agents/dispatcher-agent.md`, and the repository's agent text no
longer describes two long-running sessions, per-story worktrees or a message contract between
sessions.

## Context

The owner's process change of 2026-10-20: until takt drives its own delivery, the fastest way to
ship is one session that takes a wave and dispatches sub-agents on demand, with full gates only
once per wave and one deeper adversarial review before the wave closes. Not from self-improvement
work, so not tagged `rsi`.

## Scope

- `.agents/dispatcher-agent.md` (new; frontmatter `name` and `description` like the other four
  profiles) and its relative symlink `.claude/agents/dispatcher-agent.md`.
- `AGENTS.md`: § Rules, the managed-worktree rule; § Roles, the "runs as" column and the
  paragraph under the table; § Messages, replaced by the rule that a sub-agent's return is the
  message.
- `.agents/planner-agent.md`, `.agents/developer-agent.md`: drop "its own long-running session",
  the per-story worktree, the message tables and the store-worktree recipe; keep their Owns and
  Never.
- `design:recursive-self-improvement` § Roles: the runs-as entries, through `aep` verbs.

## Acceptance

1. `.agents/dispatcher-agent.md` exists with the frontmatter of the other profiles, and
   `.claude/agents/dispatcher-agent.md` is a relative symlink to it.
2. The profile states the wave loop (pick, open, dispatch, commit per unit, build, adversarial
   review with at most two fix rounds, one full gate, close), the implementor brief, how
   compile errors in the shared tree are sorted, how self-improvement work fits the loop, the
   usage and disk limits, the handoff note, and the defaults (wave size, parallel implementors,
   fix rounds, branch, commits, review, pick).
3. `grep -n -i 'worktree' AGENTS.md .agents/planner-agent.md .agents/developer-agent.md` finds no
   rule requiring a managed worktree, and no profile names a second long-running session or the
   `[GRANT? takt]` message contract.
4. Still stated, unchanged in force: evidence from validators only, the bot route, a
   judge-surface grant on a verdict at the exact sha, `rsi` first, spec first, no private names in
   committed text, a wave page's numbers from commands.
5. Committed text names no session, no home or cache path, no person or organization, and
   carries no figure that no command in this repository computes; `b10x-gates scan-text` passes.
6. The merge of the wave that carries this story into `main` is granted by the owner: it widens
   one role's authority (one session holds the planner's grant and the developer's merge).
