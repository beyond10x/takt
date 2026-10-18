---
format: aep.planning-md/3
id: design:continuous-orchestration
kind: design
status: draft
title: 'Continuous work and orchestration: planning as a unit, every loop a case, one deterministic dispatcher'
summary: 'A run never runs dry: plan.refill/1 refills the frontier at a low-water mark; root, planner, waves, reviewers and workers are a tree of Canon cases joined by case.dispatch and child.outcome, run by one deterministic dispatcher from files'
relations:
- designs: product-requirements:takt
- serves: vision:safe-agentic-coding
- informed_by: story:declared-loops
- informed_by: design:recursive-self-improvement
revision: 4
---
# Design: continuous work and orchestration as a tree of takt cases

Status: draft, 2026-10-18, revised once after one review (24 findings, all taken). Serves
`product-requirements:takt` R10, R12, R13, R16, R17 and § 6 (orchestration, prepared for).

Two questions, one answer. A bounded run (`--hours 8`) must not end early because the ready
stories ran out while approved work remains unplanned. And the whole system around a run, the
orchestrator, the planner, the reviewers and many implementation workers, should be expressible in
takt's own terms, with one structured way of dispatching through all those loops. The answer:
planning is a unit like any other, every loop is a Canon case, cases form a tree through a
`case.dispatch` action, and one deterministic dispatcher runs the tree from files.

## 1. Where takt stands

| fact | source |
|---|---|
| a run opens one case: `Run::open`, `open_unit`, `open_next` | `crates/takt/src/run.rs:220-289,341` |
| with nothing ready a run prints `frontier: nothing ready` and exits | `crates/takt-cli/src/main.rs:332` |
| on the wave 06 branch the exit codes are 0 completed or nothing ready, 1 refused, 2 bad arguments, 3 a budget or nothing more to do, 4 approval required, 5 error | `fix/2026-10-18-review:crates/takt-cli/src/main.rs:145-160` |
| ready = `proposed`, unblocked, dependencies done; picked in id order | `crates/takt/src/frontier.rs:5-9,164` |
| nothing in takt moves a story `draft -> proposed`; refinement, decomposition, scoping and wave selection happen outside takt | `frontier.rs` (no such move) |
| AEP has no priority field; `aep plan artifact set` changes title, summary, owner, tags, refs and model digest | `aep plan artifact set --help` |
| a case directory holds `case.yaml`, `protocol.ir.json`, `evidence/`, `decisions/`, `branch.yaml`, `story.yaml`, `authority.yaml`, and `run.lock` while a run holds it | `crates/takt/src/case.rs:1-5,36` |
| a crashed run leaves `run.lock` behind; the refusal says to remove it by hand | `case.rs:61-66` |
| `.engineering/cases/` is excluded from every commit takt makes; the take (`aep move --to active`) is swept into the case branch's first commit | `crates/takt/src/git.rs:78-79` |
| a validator runs once per `(kind, subject, revision)` | `run.rs:83-96` (`is_due`) |
| a resumed run counts tokens from 0 | `run.rs:328` |
| story:declared-loops (draft) adds the outer loop: one case per unit, many cases per run, no parent | the story's Context |

## 2. Planning is a unit: `plan.refill/1`

A planning case is opened on an epic. Its outcome is new stories at `proposed`.

| step | kind | action and validator |
|---|---|---|
| pick the epic | deterministic | an epic at `active` (the owner moved it there) with no story at `draft`, `proposed` or `active` that `decomposes` it; ordered by tag `rank:NN`, then id |
| decompose | judgement, role `planner` | `story.draft`, store writes through `aep plan artifact new` only. The step may end the case with outcome `epic.covered` when it finds the epic's acceptance already claimed; that outcome proposes, it moves nothing |
| scope | judgement, role `planner` | `story.scope` |
| critique | judgement, roles `critic-*`, a model other than the planner's | `review-result` records, labelled as model judgement, never as validator evidence |
| check | deterministic | `aep plan artifact validate` exits 0; `aep plan artifact waves` prints no cycle and no `unassessed` story |
| propose | deterministic, or a protocol `approval` | `story.propose`: `draft -> proposed`, recording each story's body and scope digest in the case's evidence |

Two consequences the protocol must carry:

- The planning store is part of the pinned judge (`design:recursive-self-improvement` § The judge),
  and the judge-surface allow-list is `docs/**`, `README.md`, `CHANGELOG.md`. So today every
  planning case ends in one human merge grant. O6 asks whether `.engineering/planning/story/**`
  leaves the judge surface for an `agent:planner` grant, with lifecycles, `project.yaml`, epics,
  designs, the PRD and the vision staying judge. Until the owner decides, planning costs one human
  merge per case, and "the worker never waits" holds only when that merge lands ahead of need.
- A proposed story is pinned. The delivery lane refuses to take a story whose body or scope digest
  differs from the one `story.propose` recorded. A planner that wants to change a proposed story
  moves it `proposed -> draft` first, which takes it out of the frontier and needs a new
  `story.propose`, with its approval if the protocol has one.

## 3. When to plan: a low-water mark

```
per_story     := median wall time (or tokens) per accepted story, read from case events
                 (story:run-record-and-story-evidence), never from task_spend files a candidate could plant
buffer        := ready stories × per_story
remaining     := hours left (--hours) | tokens left (--tokens) | iterations left × per_story (--iterations)
start planning  when buffer < remaining, or fewer than 2 stories are ready
stop planning   when buffer ≥ 1.5 × remaining, or one wave is planned ahead
```

Before the first per-story figure exists, the rule is "fewer than 2 ready". One wave ahead is the
cap, so planning does not run ahead of what the next delivery will invalidate. If no epic
qualifies (§ 2 pick), the run stops with `FrontierExhausted` and exits 7 (§ 7). It does not invent
work.

## 4. Steering

| level | who | how | takes effect |
|---|---|---|---|
| direction | owner | vision, PRD, epics; move an epic to `active` or out of it | the next planning pick |
| order | owner or operator agent | tag `rank:NN` on epics and stories (§ 7, O3) | the next pick |
| checkpoint | protocol | an `approval` on `wave.start` in `wave.delivery/1` | that wave's case waits at exit 4; others proceed |

Steering without a pause is double buffering: the dispatcher runs wave N while the owner reviews
the planned wave N+1, and the grant for N+1 lands before N drains.

Withdrawal mid-case: before each step a story case checks that its story is still `active` and
that its body and scope digest equal the taken one. If the owner archived it (`active -> archived`
is the only exit the story lifecycle offers besides `implemented`) or changed its body or scope,
the case stops with `UnitWithdrawn`. AEP `revision` is not used for this check, because takt's own
evidence writes bump it.

## 5. Pause and resume

- `takt pause --repo <path> [--at step|unit]` writes `control.yaml` into the root case directory
  (§ 6). The cases directory is already protected from harness writes (`run.rs:570-578`).
- The dispatcher reads it before each pick; a story case reads it before each step. Both stop with
  `Paused`, exit 6.
- SIGTERM: a handler sets the same pause flag, read at step boundaries; nothing is forwarded to the
  harness child (`harness.rs:170-185` runs it with `command.output()` and no handler today).
- Resume is `takt run` again: open cases resume (`main.rs:982` test, "frontier: resumed ...").

## 6. Every loop is a case, and dispatch is an action

| addition | what it is |
|---|---|
| a root case | every `takt run` opens or resumes one root case under `run/1` (budget, horizon, the control file). `takt run` with one story is a root with one child |
| `case.dispatch` | a deterministic step: open a child case `(protocol, unit, role, budget slice, base branch)`. Its effect: a child directory with `parent: <id>`, `worktree: <path>` and `base: <branch>` in `case.yaml`, and a managed worktree for it |
| a `children` artifact on the parent | its revision is the digest of every child's latest decision. A new child decision changes the revision, so `is_due` (`run.rs:83-96`) runs `child.outcome` again |
| `child.outcome` | one summary record per `children` revision: `all_accepted`, or `stopped: <id>=<stop>, ...`. Parent claims are computed from it |
| one cases root | every case of a tree lives under `.engineering/cases/` of the checkout `takt run` was started in, never in a child's worktree. Children work in their worktrees and write their records to the root. The snapshot guard that refuses a step touching the cases directory (`run.rs:570-578`) covers the root path, not only the worktree's own `.engineering/cases` |
| one store writer | lifecycle moves (take, propose, implemented, archived) are made by the dispatcher only, committed to the base branch through the bot route, one at a time. Children never move a story, and a take is no longer swept into a case branch's first commit (`git.rs:78-79`) |
| a `Workspace` seam | creates a managed worktree on dispatch and finishes it (`worktree finish --discard-cache --archive`) on the child's terminal outcome. A sixth seam beside the five in `docs/design/takt.md` § 3 |

A parent waits because its frontier admits nothing until `child.outcome` writes new evidence. The
loop that already exists does the waiting.

story:declared-loops is the interim step: its outer loop opens many cases with no parent. Once
build step 2 (§ 11) lands, that loop becomes the root case dispatching story children.

## 7. The tree

```
root   run/1               one per takt run; budget, horizon, control.yaml
         dispatches ─┬─ plan.refill/1      per qualifying epic                role: planner (+ critics)
                     ├─ wave.delivery/1    per selected wave                  no judgement role
                     │     approval: wave.start (if the protocol has it)
                     │     dispatches ── story.delivery/1  per story, base = the wave's integration branch
                     │                         inner loops: tests until pass, gate until green
                     │     then: merge the integration branch to the base, wave CI, move stories implemented
                     ├─ review.panel/1     per wave, before the wave merge    roles: adversary, security, critics
                     └─ self.improvement/1 per finding
                           dispatches ── story.delivery/1  role implementer: the candidate
                                     └── a verification child, another agent: scenario.passes_after
```

| detail | rule |
|---|---|
| story children in a wave | their pull requests target the integration branch (`forge.rs` carries `base`). Their outcome there is `integrated`, a variant of `story.delivery/1` (O7). The wave parent moves each story `implemented` after the integration branch merges into the base; until then they stay `active`. A failed wave merge leaves them `active`, so nothing needs a move backwards |
| review panel | its findings are records: each goes through finding intake (story 3 of `epic:recursive-self-improvement`) with an `expected_before`. An open high finding against the wave makes the wave's `review.no_open_high` claim false, which blocks the wave merge. The panel writes nothing to a child's branch |
| self-improvement | the operator agent never writes takt code (`design:recursive-self-improvement` § 1), and `scenario.passes_after` needs a different agent's run (`protocols/self-improvement/1.yaml`). So the case dispatches an implementer child for the candidate and a separate verification child, and records that the two differ |
| approvals | a child at `ApprovalRequired` blocks only its own branch of the tree. `takt approve <child> <capability>` unblocks it. The root writes every pending approval into one file |

## 8. Roles

A role is a harness profile: harness, model, brief, the actions it may take, the paths it may
write. The workflow's judgement step names a role; `protocol/1` keeps actions and capabilities
only.

```
roles:
  planner:     { harness: claude, model: <id>, actions: [story.draft, story.scope], writes: [.engineering/planning/story/**] }
  implementer: { harness: claude, model: <id>, actions: [tests.write, implementation.edit, repository.inspect] }
  adversary:   { harness: <any CLI agent>, actions: [tests.write], writes: [crates/*/tests/**] }
```

- Write confinement per role does not exist today. The refusals are: an action not offered,
  `repository.inspect` with changes, `tests.write` outside the test paths (`run.rs:629-647`).
  `implementation.edit` may write `.engineering/planning/**` or `protocols/`. Build step 5 adds a
  per-action write-path allow-list, checked on the changed paths after each step. The judge guard
  at merge time stays the backstop.
- A second harness is a role setting (R7). Reviewer independence becomes checkable: the review
  case records that its role's model differs from the implementer's.

## 9. The dispatcher

```
loop:
  tree      := cases under the cases root with no terminal outcome
  runnable  := cases whose frontier admits an action, not held by a live process,
               not waiting on an approval or on a child
  order     := (level asc, rank tag, open instant from case.yaml, id)
  pick      := the first runnable cases in that order, up to the free slots per role
  record    := one pick record: runnable set, slot occupancy, the chosen ids, the store sha
  for each pick: one action in the case's worktree (a harness step or a command)
until the root completes | its budget is spent | Paused
```

| property | rule |
|---|---|
| deterministic | the order is a pure function of the case files, the store and the tags. Slot occupancy depends on which children finished first, so it is recorded per pick, and a replay re-derives each pick from its record |
| budgets | a parent gives each child a slice. Tokens are cumulative per case from `events.jsonl`, so a resumed or re-dispatched child does not start at 0 again (today: `run.rs:328`). The root's `budget.within` sums its children |
| stalls | a child that stops on a budget, `NoProgress` or `StepRefused` is re-dispatched once with a 1.5× slice; a second stop files a finding and the parent moves on. A judgement step decides only where the protocol names one |
| locks | `run.lock` records the holder's pid and the dispatcher's id. The dispatcher reclaims a lock whose pid is dead and records the reclaim as an event. Without this a restart finds every in-flight child held (`case.rs:61-66`) and `runnable` is empty |
| crash | the dispatcher keeps nothing in memory that is not in the case files, so a restart rebuilds the tree from them |
| parallelism | slots per role, for example 4 implementers, 1 planner, 2 reviewers. Inside a wave, scopes are disjoint (`aep plan artifact waves`) |

## 10. Where judgement is allowed

| decision | by |
|---|---|
| which dispatch is admissible | Canon, from the protocol |
| which admissible action runs next | the dispatcher's order; no model |
| decomposition, scope, implementation, review findings | a role's judgement step |
| a stalled child | the § 9 rule; a judgement step only where the protocol names one |
| direction, epics, judge-surface merges | the owner, as protocol approvals |

A model in the dispatcher seat would put the highest-value decision where it is hardest to judge,
against qualities 1 (secure) and 4 (deterministic).

## 11. Gaps and build order

| step | gives | needs |
|---|---|---|
| 1 | outer loop over stories, serial (story:declared-loops) | in the store |
| 2 | root case, `parent` and `worktree` in `case.yaml`, `case.dispatch`, the `children` artifact, `child.outcome`, the `Workspace` seam | 1 |
| 3 | dispatcher as the single store writer; lock with pid and reclaim; cumulative tokens per case; pick records | 2 |
| 4 | `wave.delivery/1` with parallel `story.delivery/1` children on an integration branch; the `integrated` outcome (O7) | 3 |
| 5 | roles as harness profiles; per-action write-path allow-list; `review.panel/1` | 2; story:second-harness-adapter for mixed harnesses |
| 6 | `plan.refill/1`, digest-pinned proposals, the low-water mark, `rank:NN` ordering | 3; O6 for agent-granted planning merges |
| 7 | `takt pause`, SIGTERM handler; stops `Paused` (6), `FrontierExhausted` (7), `UnitWithdrawn` (3) | 2 |
| 8 | the root across repositories | 4, 5, 6 (PRD § 6) |

Steps 1 to 4 earn the PRD success row "wave delivery through takt".

## 12. Risks

- Busywork: a planner told to keep a buffer full invents stories. It decomposes only `active`
  epics, and `FrontierExhausted` is a legitimate end.
- Hidden cost: planning and review tokens and minutes count in "per accepted unit" (PRD § 3).
- Stale plans: one wave ahead at most (§ 3); at each wave boundary the planner re-scopes the
  remaining `proposed` stories whose scope paths changed, through `proposed -> draft`.
- Judge bypass: a planned story that touches a judge surface still needs a human merge grant.
  Pinning by digest (§ 2) stops an edit after approval.

## 13. Open for the owner

| # | question | default if unanswered |
|---|---|---|
| O1 | may an agent move `draft -> proposed`, or only with a grant? | a protocol setting; the shipped `plan.refill/1` needs a grant |
| O2 | may the planner create epics? | no |
| O3 | priority in AEP, or the `rank:NN` tag? | the tag now; a typed field in AEP later, so every tool reads one order |
| O4 | does takt become the orchestrator, or does an outside orchestrator drive takt cases? | build the case tree inside takt and keep the dispatcher replaceable |
| O5 | one dispatcher per machine, or per repository? | per repository; across repositories is step 8 |
| O6 | does `.engineering/planning/story/**` leave the judge surface for an `agent:planner` grant? | no; each planning case costs one human merge |
| O7 | the wave child's outcome: an `integrated` variant of `story.delivery/1`, or a separate protocol? | a variant: same claims, `repository.merge` targets the integration branch |
