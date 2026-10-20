---
format: aep.planning-md/3
id: story:a-trial-candidate-lands-in-the-repository-of-its-layer
kind: story
status: draft
title: A trial candidate is built in the repository that owns its layer and re-trialled pinned
summary: the trial agent profile still builds candidates in takt only; ADR 0101 points 3 and 4 move a candidate to its layer's repository and pin it for the re-trial
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:a-trial-records-the-revision-of-each-layer
scope:
- confidence: cited
  path: .agents/trial-agent.md
revision: 2
---
# Story: a-trial-candidate-lands-in-the-repository-of-its-layer

## Outcome

A trial proposal names the repository that owns its layer (ADR 0096's table), and the trial agent
builds its candidate there: a managed worktree of that repository, a `trial-cand/<cycle>-<k>`
branch cut from the revision the trial ran, committed through that repository's bot route. The
re-trial runs takt at the trigger's sha with that candidate pinned in place of the layer's
release, and each run's record names the pinned revision. The scorecard rule does not change. A
proposal for a layer with no repository in beyond10x (a vendor harness, a provider) stays a
finding for the monitor, with no candidate.

## Context

ADR 0101 (accepted 2026-10-20), points 3 and 4: "A proposal names its owning repository by ADR
0096's table, and its candidate is built in a managed worktree of that repository"; "takt's
re-trial runs with the candidate revision of that repository pinned in place of the release; the
scorecard rule is unchanged". Its consequence: "takt's trial agent profile widens from takt-only
candidates to candidates in any layer's repository, through that repository's own merge rules."
`.agents/trial-agent.md` at 0.5.0 still says the opposite: step 7 cuts the candidate "from the
trigger's sha" in takt, § Reports sends "a proposal that concerns another repository" to the
monitor "and not acted on by the trial agent: no finding, no candidate, no branch, no edit in that
repository", and the closing list forbids acting on one. The 0.4.0 cycle's token proposal found
that 4,816 of a step's first-request tokens are the harness's system prompt and tool schemas,
none of it takt's text: the layer below takt is where that cost is decided. Comes from
self-improvement work, so it carries `rsi`.

## Scope

- `.agents/trial-agent.md`: steps 5 to 10, § Reports, § Runs and measurement, the closing list.
- No change to `crates/`; pinning a binary layer is the runner's `PATH`, pinning a library layer
  is a build flag (acceptance 3). The layers in the record are
  story:a-trial-records-the-revision-of-each-layer, which this story depends on.

## Acceptance

1. Step 5: each proposal names its owning repository and layer by ADR 0096's table. A proposal
   whose layer has no repository in beyond10x (a vendor harness's own prompt or tool schemas, a
   provider) goes to the monitor as one message (the layer, the change, the scenario it should
   move, the evidence) and gets no candidate. takt's own settings for a vendor harness (its
   argv, its settings document) stay takt's layer.
2. Steps 6 and 7 for a proposal in another repository: its red predicate runs takt at the
   trigger's sha with that layer at the revision the trial ran; the candidate is a managed
   worktree of that repository on `trial-cand/<cycle>-<k>` cut from that revision, never a branch
   or tree another session works in; one implementor under that repository's `AGENTS.md`, its
   Scope the proposal's paths; the commit goes through that repository's bot route; its message
   is scanned before the commit, as takt's.
3. Step 8, pinning: takt is built at the trigger's sha from its clean worktree, so `--version`
   names that sha and not `-dirty`. A binary layer's candidate is built from its clean worktree
   and put first on the runs' `PATH`; a library layer's candidate is put in by a build-time
   override that leaves takt's tree unchanged (a `cargo --config 'patch…'` on the command line).
   Pre-flight item 5 checks both revisions: takt's `--version`, and the layer's revision as the
   run record will name it. A run whose record names another revision of the pinned layer is
   discarded and run again.
4. The re-trial compares as today: step 2's saved scorecard and the promoted baseline's, no
   scenario worse, the target scenario better on the proposal's metric; the parking rule, the
   budgets and the early stop apply unchanged.
5. Step 9, hand-over: `[CANDIDATE <repository>]` or `[REJECTED <repository>] <branch> <sha>`
   goes to that repository's delivery session through the monitor; takt's dispatcher gets a copy.
   A repository with no running delivery session holds the accepted candidate as a finding in its
   own planning store until one runs; the trial agent writes nothing to that store.
6. Step 10, promotion: only after the candidate merged and released through its own repository's
   process and takt moved its pin to that release; the promotion record names every pinned
   layer's revision.
7. § Reports and the closing list no longer forbid acting on a proposal for another repository;
   they forbid editing a repository outside a managed worktree of the cycle, committing outside
   its bot route, and acting on a repository that is not public.
8. § Limits: one build or `scripts/check-finding` at a time across all repositories of a cycle,
   each under the disk floor of § Limits.
