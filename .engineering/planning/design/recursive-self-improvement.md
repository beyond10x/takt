---
format: aep.planning-md/3
id: design:recursive-self-improvement
kind: design
status: draft
title: 'Recursive self-improvement of takt: takt improves takt, the loop is a protocol, the loop improves itself'
summary: Three levels (work, takt, the loop), two agent roles, a pinned judge, a scorecard; L1 and L2 modelled as protocol/1 documents
relations:
- serves: vision:safe-agentic-coding
- informed_by: epic:composable-agentic-work
revision: 2
---
# Design: recursive self-improvement of takt

## Problem

takt is improved today by two agents working by hand: one reviews takt and runs it, the other
implements. On 2026-10-18 a review by three independent reviewers produced 30 findings, and two
live trials on a scratch repository found two defects (a pull request opened before any commit;
a story taken from the store running under `software.change/1` instead of `story.delivery/1`).
The findings travelled as chat messages, some of them never delivered, and nothing measured
whether a fix made takt better.

The intent, from the owner: improve takt itself; model the improvement process itself through
takt; and improve the self-improvement.

## Three levels

| level | what is improved | by what | judged by |
|---|---|---|---|
| L0 work | a repository's stories | takt runs under `story.delivery/1` | Canon over validator evidence |
| L1 improve takt | takt's code, protocols and validators | the improvement turn below | the scorecard, non-regression against the promoted revision |
| L2 improve the loop | the scenario corpus, the scorecard's metrics, the review panel, the intake rules, the improvement protocol, the agent profiles | the same turn, over the loop's own record | the loop scorecard |

## Roles

| role | owns | never |
|---|---|---|
| owner (a person) | merge approval for judge surfaces; scorecard definitions; priorities | |
| operator agent | running takt (scenarios, trials), review, measurement, filing findings in this store, verification runs, approving non-judge merges under a recorded grant | writes takt code; grants on a diff that touches a judge surface |
| developer agent | moving stories to active, waves, merges through the bot route, building and installing promoted binaries | marks its own fix verified |

Grants are typed: `takt approve --by human:<id>` or `--by agent:<role>`. The judge refuses an
`agent:` grant on a diff that touches a judge surface. Merge authority split by surface is the
owner's decision (2026-10-18).

The store carries the content between the agents. A message only says that something changed,
with a fixed first line: `[FINDING takt] <ids>`, `[IMPLEMENTED takt] <story> <sha>`,
`[VERIFIED takt] <story> <scenario> <before> -> <after>`.

## One improvement turn

run → record → measure → diagnose → file → deliver → verify → promote.

- A finding carries an `expected_before` predicate a machine can check: a command, run on a named
  scenario, with its expected exit status or output. A finding without one is a note, not a
  finding.
- A finding also names the path of the run record or review that produced it, and carries the
  reference `finding:<key>`; a second filing of a key already in the store is refused. Both come
  from 2026-10-18, when findings arrived as chat messages with no source and no way to tell a
  repeat from a new one.
- A fix is verified when the predicate held on the scenario at the promoted revision and fails at
  the candidate. A fix that does not move its number is declined, and the record says so.

## Scenario corpus

A scenario is a story of the scratch repository at a known seed commit. Each run gets a fresh base
branch `trial/<n>` cut from the seed, and the case's pull request targets it (the forge already
carries a base, `crates/takt/src/forge.rs:79`): nothing is reset by a force-push, and the store on
the fresh branch is at its seed state, so the story is `proposed` again. Scenario 1 is the scratch
repository's `story:remove-a-todo`.

## The judge

The judge is a bundle: a promoted takt binary whose build embeds its git revision in `--version`,
and the protocol file, the gates and assertions files and the planning store, all read from the
promoted base revision outside the case's worktree, never from the candidate branch. Today all
four are read from the tree the harness edits (review findings 1 and 2; `crates/takt/src/validator.rs:193-216`,
`crates/takt-cli/src/main.rs:256-263`); story:validator-confinement moves the gates inputs to the
base revision; story:promotion installs the promoted binary and story:judge-guard makes it read the
protocol and the store from the base and guard the merge.

Judge surfaces are every path except an allow-list held at the base revision. The allow-list
starts as `docs/**`, `README.md`, `CHANGELOG.md`; adding a path to it is a judge-surface change.
The guard runs at merge time, in the pinned binary, over `base..head`.

## Scorecard

Per scenario run: outcome, steps, tokens, refusals, `NoProgress` and errors, wall time. Tokens are
read only from the run's events under the guarded `.engineering/cases/`, never from a `task_spend`
file the candidate could plant. Each scenario is compared against the promoted revision's baseline
for that scenario.

Promotion defaults, changeable by the owner: no scenario's outcome worse; the median of tokens at
most the baseline plus 10 %; at most one promotion a day. A metric's definition is a judge
surface; for one cycle after a change the previous definition's numbers are printed beside the
new ones, so a metric cannot be redefined to look better.

## What earns each claim

A validator earns a claim where the thing is observable: `scenario.reproduced_before`,
`scenario.passes_after`, `scorecard.no_regression`, `fix.merged`, `promoted` (the installed
binary's revision against the promotion record). What no validator can observe is a `decision:`
requirement or human-sourced evidence, labelled as such: escaped defects, the value of a review,
interventions the run's events do not record.

## The loops as protocols

Both loops are `protocol/1` documents, validated and compiled by Canon like any other protocol:

| level | protocol | case | outcomes |
|---|---|---|---|
| L0 | `protocols/story-delivery/1.yaml` | one story | accepted, abandoned |
| L1 | `protocols/self-improvement/1.yaml` | one improvement turn on takt | accepted, declined, unreproducible, abandoned |
| L2 | `protocols/loop-improvement/1.yaml` | one turn on the loop itself | accepted, declined, abandoned |

`self.improvement/1` has artifacts `finding`, `baseline` (the promoted revision), `candidate` (the
fix) and `promotion`, so claims about the baseline survive commits to the candidate. Its input
`surface` (`judge` or `non_judge`) selects the merge action: `candidate.merge` requires the
capability `repository.merge`, `candidate.merge_judge_surface` requires
`repository.merge.judge_surface`, which only the owner holds. `scenario.passes_after` needs its
record from a producer independent of the case's work on `different_agent_run`: the run that
delivered the fix cannot verify it.

`loop.improvement/1` has artifacts `loop_baseline`, `loop_candidate` and `loop_finding`.
`loop.merge` needs `loop.definition_honest` (no metric redefined without its previous figures
beside it) and the owner's capability; `accepted` needs the metric improved under the candidate,
again observed independently.

`crates/takt/tests/improvement_protocols.rs` drives cases on both through their records: accepted
once verified, merged, without regression and promoted; declined when the candidate still
reproduces; a self-verification earns nothing; a judge-surface candidate needs the owner's
capability; a candidate commit keeps the baseline's reproduction; a silent redefinition blocks the
loop merge.

What is not built: the validators for these actions (Rust structs chosen by action name,
`crates/takt/src/validator.rs`, so judge-surface work), and the kernel declaring each producer's
independence in `case.yaml` (the test writes it by hand). In phases R0 and R1 the agents perform
the actions and record their evidence; from R2 takt drives the cases.

## Improving the loop (L2)

The loop scorecard, per L1 turn: the share of findings that became fixes; the share of fixes
verified against declined; the time from finding to verified; tokens per verified fix; escaped
defects (human-sourced). An L2 turn runs once three L1 turns are recorded, then weekly: the lowest
loop metric becomes a finding against the loop itself and goes through the same turn. Every L2
change is a judge-surface change.

## Phases

| phase | who delivers takt's stories | entry |
|---|---|---|
| R0 | the developer agent by waves; the operator agent reviews and runs trials | now |
| R1 | takt runs the scratch repository's scenarios daily | story:story-done-end-to-end implemented |
| R2 | takt delivers takt's non-judge stories under the pinned judge | the hardening stories implemented (story:validator-confinement, story:tree-hygiene, story:authority-hardening); a scorecard baseline recorded |
| R3 | takt proposes stories from its scorecards; the owner prioritises | two weeks of R2 with no regression |

## Agent profiles

`.agents/operator-agent.md` and `.agents/developer-agent.md` (start reading, owns, watches, loop,
forbidden), each with a pointer stub under `.claude/agents/`.

## Not in this design

A scheduler of its own, a hosted service, cross-repository improvement, and any change to how
Canon evaluates.
