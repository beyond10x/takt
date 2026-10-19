---
format: aep.planning-md/3
id: design:recursive-self-improvement
kind: design
status: draft
title: 'Recursive self-improvement of takt: takt improves takt, the loop is a protocol, the loop improves itself'
summary: Three levels (work, takt, the loop), four agent roles, a pinned judge, a scorecard; L1 and L2 modelled as protocol/1 documents
relations:
- serves: vision:safe-agentic-coding
- informed_by: epic:composable-agentic-work
revision: 12
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
| owner (a person) | merge approval for judge surfaces, or its delegation to a named role on record; scorecard definitions; priorities | |
| planner agent | the main agent and orchestrator until takt drives this flow itself; planning the store (stories, priorities within the owner's, wave order with the developer), filing findings, deciding what the developer escalates, granting merges only on a reviewer verdict (judge surfaces only under a delegation on record naming the planner), dispatching the reviewer and the doc-writer, reporting to the owner | writes takt code; grants without a reviewer verdict on that exact sha; grants a change that widens authority, its own or any other role's, even one that cites the owner's recorded decision |
| developer agent | moving stories to active, waves, implementation, merges through the bot route, building and installing promoted binaries; README lines that document its own unit's change | marks its own fix verified; edits `docs/**` but its wave pages, the site or the deck |
| reviewer agent | a verdict with cited findings on each grant request's diff at its exact sha; independent code reviews (correctness, security, usability); adversarial passes on request; trials, scenario runs and verification runs (`scripts/check-finding`, `takt scenario run`); measurement against the baseline | writes takt code; grants; files to the store (it returns findings to the planner, who files them); reviews its own work |
| doc-writer agent | `docs/**` (designs' prose in `docs/design/`, run records under `docs/runs/`, `docs/metrics/`, `docs/pitch/`; wave pages are the developer's), the structure of `README.md`, the public site on the `gh-pages` branch (publishing pushes only `gh-pages`, never `main`) | writes takt code; changes the planning store; states a figure no record holds |

How each role runs: the planner is the main session; the developer is its own long-running
session; the reviewer and the doc-writer are started by the planner per task, from their profiles,
and resumed by name. Until takt drives this flow itself, the planner is the main agent: it starts
the reviewer and the doc-writer, and the developer reports to it.

Grants are typed: `takt approve --by human:<id>` or `--by agent:<role>`. The owner may delegate a
grant reserved for the owner, judge surfaces included, to a named role. The delegation is a record
held at the base revision, and it is revocable. The judge accepts a grant on a diff that touches a
judge surface when it is the owner's, or when it is from a role a delegation on record names and
that delegation was not revoked; it refuses every other grant there. story:judge-guard enforces
it; until then the delegation is a record the wave cites (`docs/waves/wave-07.md`: four
judge-surface merges under the operator role's grant on the owner's delegation, before the
operator role was split). The planner grants only on the reviewer's verdict at the exact sha it
grants; a diff that widens authority is granted under the owner's standing grant of 2026-10-18 and named to the owner. The target stays 0
judge-surface merges without the grant the surface requires (`product-requirements:takt` § 3).
Merge authority split by surface is the owner's decision (2026-10-18), and so are delegation on
record (2026-10-18) and the four agent roles, which split the operator role into planner,
reviewer and doc-writer (2026-10-18).

The store carries the content between the agents. A message only says that something changed,
with a fixed first line:

- from the planner: `[FINDING takt] <ids>`, `[GRANT takt] <story> <sha>`,
  `[DECIDED takt] <topic>: <option>`, and
  `[VERIFIED takt] <story> <scenario> <before> -> <after>` on the reviewer's run;
- from the developer: `[IMPLEMENTED takt] <story|wave> <sha>`, `[WAVE takt] <wave> <units>`,
  `[GRANT? takt] <story> <branch> <sha> <paths>`, and `[DECIDE takt] <topic>` with options A, B
  and C, a recommendation and a default;
- from any role: `[ACK takt]`.

`AGENTS.md` § Messages holds them as a table.

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
The guard runs at merge time, in the pinned binary, over `base..head`, and admits a judge-surface
diff only under the owner's grant or a delegated role's (§ Roles).

## Scorecard

Per scenario run: outcome, steps, tokens, refusals, `NoProgress` and errors, wall time. Tokens are
read only from the run's events under the guarded `.engineering/cases/`, never from a `task_spend`
file the candidate could plant. Each scenario is compared against the promoted revision's baseline
for that scenario.

Promotion defaults, changeable by the owner: no scenario's outcome worse; the median of tokens at
most the baseline plus 10 %; at most one promotion a day. A metric's definition is a judge
surface; for one cycle after a change the previous definition's numbers are printed beside the
new ones, so a metric cannot be redefined to look better.


Open question, from the first recorded improvement turn (2026-10-18): at n=3 the token default trips on run-to-run noise. On scenario 1 with an unchanged harness, six takt runs at two revisions spread from 106,003 to 167,249 tokens; the turn's candidate, which changed only help text, had a median 24 % over its baseline because two of its three runs hit the known `repository.inspect` NoProgress (story:no-inspect-for-stateless-harness). `takt scorecard --against` therefore prints n and the minimum, median and maximum per side and flags only worse outcomes (story:scorecard-reader); which token rule promotion applies (a larger n, a spread-aware rule, or tokens per accepted unit with the known waste removed) is decided below.


Outcomes are compared by dominance (story:scorecard-reader, wave 10): per scenario the candidate is worse when, for some outcome rank, its share of cases at that rank or better is below the baseline's; a median rank missed regressions in fewer than half a scenario's cases (68 of 200 seeded worse candidates passed in its adversary pass). Dominance is strict, so a re-run of unchanged code with one flaky outcome, such as the `repository.inspect` NoProgress that scenario 1 hit in 3 of 6 runs, reports worse. How many trials promotion needs, and whether it re-runs or allows a margin, is decided with the token rule above.

Decided 2026-10-18 by the planner, under the owner's standing grant, before story:promotion lands:

- Trials: promotion runs the candidate n=5 times per scenario; the baseline's runs are the ones saved with the promoted revision's scorecard, at least 5.
- Tokens: the candidate is worse on tokens for a scenario when its median exceeds the baseline's maximum. The rule tolerates the spread one unchanged harness shows (106,003 to 167,249 on scenario 1) and still catches a shift of the whole distribution. Tokens are counted per case, every case; known waste is removed by fixing it, not by excluding it from the count.
- Flaky outcomes: dominance keeps its ranks, with a margin of one case per scenario: the candidate is worse at a rank when its count of cases at that rank or better is more than one below the baseline's. At n=5 a single flaky case passes, two cases worse at any rank fail.
- Re-judging: a refused revision is judged again only through `takt promote --rejudge`, on new runs; a scorecard sharing a case with one the revision was refused on is refused, so re-scoring the same runs cannot reverse a refusal (ruled 2026-10-19 at story:promote-binds-scorecard-and-merge; how often a revision may be re-judged is story:rejudge-is-bounded).
- The rule's parameters (n, the margin) are fields of the Scorecard record, so `takt scorecard --against` prints the rule it applied and a later change to the rule is a change to a judge surface.

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
`repository.merge.judge_surface`, which only the owner holds, or a role the owner delegated it to
on record (§ Roles). `scenario.passes_after` needs its
record from a producer independent of the case's work on `different_agent_run`: the run that
delivered the fix cannot verify it.

`loop.improvement/1` has artifacts `loop_baseline`, `loop_candidate` and `loop_finding`.
`loop.merge` needs `loop.definition_honest` (no metric redefined without its previous figures
beside it) and the owner's capability, or a delegated role's; `accepted` needs the metric improved under the candidate,
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
| R0 | the developer agent by waves; the planner plans and grants, the reviewer reviews and runs trials, the doc-writer keeps the docs | now |
| R1 | takt runs the scratch repository's scenarios daily | the first three-month outcome of `product-requirements:takt` § 5: scenario 1 accepted unattended, one invocation plus one grant, daily for a week; blockers story:ci-pending-waits, story:run-record-and-story-evidence |
| R2 | takt delivers takt's non-judge stories under the pinned judge | the hardening stories implemented (story:validator-confinement, story:tree-hygiene, story:authority-hardening: done, wave 06); a scorecard baseline recorded |
| R3 | takt proposes stories from its scorecards; the owner prioritises | two weeks of R2 with no regression |

## Agent profiles

`.agents/planner-agent.md`, `.agents/developer-agent.md`, `.agents/reviewer-agent.md` and `.agents/doc-writer-agent.md` (start reading, owns, watches, loop,
forbidden), each opening with a frontmatter of `name` and `description`, and each linked from
`.claude/agents/` by a relative symlink of the same name.

## Not in this design

A scheduler of its own, a hosted service, cross-repository improvement, and any change to how
Canon evaluates.
