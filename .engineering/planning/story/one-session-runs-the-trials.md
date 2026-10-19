---
format: aep.planning-md/3
id: story:one-session-runs-the-trials
kind: story
status: draft
title: One trial session runs takt's trials, proposes and re-trials candidates, and promotes
summary: 'the owner''s process change of 2026-10-20: trials, scorecards and promotions move to one trial session triggered by a release or a person'
tags:
- process
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: one-session-runs-the-trials

## Outcome

One agent session, the trial agent, runs takt's trials: on a release that touches code, or when a
person asks, it builds the revision, runs the scenarios, collects the rollouts, renders them,
proposes changes in parallel, has each reviewed, builds a candidate per accepted proposal, re-runs
the trials until the candidate is integrated or rejected, and promotes a merged candidate. Its
profile is `.agents/trial-agent.md`.

## Context

The owner's process change of 2026-10-20: trials, scorecards and promotions move out of the wave
dispatcher into one trial session, triggered by a release or by a person, so waves never wait on a
trial and the trial loop runs as often as it takes. Not from self-improvement findings, so not
tagged `rsi`.

## Scope

- `.agents/trial-agent.md` (new; frontmatter `name` and `description` like the other profiles)
  and its relative symlink `.claude/agents/trial-agent.md`.
- `AGENTS.md` § Roles: the trial agent's row (owns trials, scorecards, rollouts and promotions;
  never edits a wave's tree or branch; hands an accepted candidate to the dispatcher as a unit).
- `.agents/dispatcher-agent.md` (story:one-session-dispatches-the-waves): its self-improvement
  section names the trial agent's trigger and the candidate hand-over.

## Acceptance

1. `.agents/trial-agent.md` exists with the frontmatter of the other profiles, and
   `.claude/agents/trial-agent.md` is a relative symlink to it.
2. The profile states the triggers (a release whose diff touches paths outside `docs/**`,
   `website/**`, `.engineering/**`, `README.md`, `CHANGELOG.md`; a person's request), one cycle
   at a time, and the cycle: binary, trials, rollouts, rendering, parallel proposals each with a
   predicate red at the trigger, a review per proposal, a candidate per accepted proposal in its
   own worktree, re-trials under the scorecard's rule with at most three per candidate, then
   hand-over or a filed rejection, and promotion after the merge.
3. It states the limits: sub-agents at once, free disk before a build, scenario pushes through
   the bot route only.
4. Committed text names no session, no home or cache path, no person or organization, and
   carries no figure no command in this repository computes; `b10x-gates scan-text` passes.
5. The merge of the wave that carries this story into `main` is granted by the owner: it adds a
   role and moves promotion to it.
