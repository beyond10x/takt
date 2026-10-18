---
format: aep.planning-md/3
id: product-requirements:takt
kind: product-requirements
status: draft
title: 'takt: secure, autonomous, reliable, deterministic, efficient agentic coding as one OSS binary'
summary: Users (operator agent first), five qualities in priority order with measures, requirements R1-R18 traced to stories and designs, three-month success, orchestration inside one repository in the horizon and across repositories prepared for
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# Product requirements: takt

Status: draft proposal, 2026-10-18. Serves `vision:safe-agentic-coding`. Sources: the owner's interview answers of
2026-10-18, `docs/design/takt.md`, `design:recursive-self-improvement`, `design:continuous-orchestration`, the store's epics and stories.

## 1. Problem

Agentic coding today is neither secure, deterministic, reliable, efficient nor autonomous at once. An agent says a unit
of work is done; nothing proves it. A run has no bound on tokens or hours, and no number says whether it was worth its
cost. A hostile prompt, a hostile repository or a sloppy agent can forge the appearance of done. Human attention is
spent watching runs instead of setting policy.

takt is one open-source binary that runs agentic work on any repository under a protocol: agents choose how to do the
work, the protocol defines what must be true before the work counts, and only validator evidence makes it true.

## 2. Users

| user | what they do with takt | first need |
|---|---|---|
| operator agent (primary) | points takt at repositories, runs scenarios and stories, reads decisions and scorecards, files findings | a machine-readable run interface and decision documents it can act on without a human |
| the operator agent's human owner | sets protocols, grants and priorities; grants what the protocol reserves for a human | protocol-level control over where a human is needed, and numbers instead of logs |
| developer agent | delivers stories through takt under the pinned judge | a judge it cannot edit and a scorecard that decides promotion |
| OSS maintainer (first outside user) | runs takt on their own repository with Claude Code or another CLI agent | install, point, run; proof of done in the pull request |

## 3. Product qualities, in priority order

When two conflict, the higher one wins: takt stops rather than act insecurely, and gives up tokens before it gives up
determinism.

| # | quality | definition | measured by |
|---|---|---|---|
| 1 | secure | threat model covers a hostile agent (prompt-injected or adversarial), a sloppy agent, secrets, and a hostile repository | count of claims established without validator evidence (target 0); judge-surface diffs merged without a human grant (target 0); adversary and security-review findings open per release |
| 2 | autonomous | the human touch points are a protocol property, set by whoever owns the protocol (end user, the agent, or the operator agent's human owner); takt hard-codes none | human touches per accepted unit, measured against what the protocol requires; a touch the protocol did not require counts as a defect |
| 3 | reliable | a started unit ends in a named outcome or a named stop, never a silent hang | share of runs ending in `accepted` or a named stop; `NoProgress` and error stops per 100 runs |
| 4 | deterministic | same decision (Canon is pure); replayable from records without the model; stable outcomes across reruns; pinned inputs | replay of every recorded case yields identical claims (target 100%); scenario rerun agreement (proposed target ≥ 9 of 10); every run records binary sha, protocol, gates, model and harness version |
| 5 | efficient | cost per accepted unit across four figures | tokens, wall time, money and human minutes per accepted unit, per scenario, against the promoted baseline |

## 4. Requirements

| id | requirement | acceptance (observable) | covered by |
|---|---|---|---|
| R1 | Every claim in a decision document comes from a validator's evidence record | a case whose harness text asserts a claim with no evidence record evaluates that claim UNKNOWN | epic:composable-agentic-work acceptance |
| R2 | The judge (binary, protocol, gates, store) is read from the promoted base, never the candidate | a candidate that edits the protocol is judged by the base protocol | story:judge-guard |
| R3 | Confinement: the harness writes only inside its worktree, never `.git`, the case records or the judge | a harness write to `.git/hooks` is refused and the run stops naming it | story:confinement-seam, hardening stories |
| R4 | Secrets never reach the harness's environment, prompt or the run records | a run with a token in the parent environment shows no token in the harness env or `events.jsonl` | no story yet |
| R5 | A hostile repository's hooks, test scripts and configs run confined, with no access to credentials | a repository whose test script reads `~/.config` gets nothing | no story yet |
| R6 | Human touch points are declared in the protocol (`approval`, typed grants), configurable per repository and per protocol | the same story runs with a merge grant under one protocol and without under another | story-delivery protocol, story:merge-under-authority (partial) |
| R7 | Any CLI agent works behind the Harness trait; Claude Code first | the same scenario yields the same decision documents under two harnesses | story:second-harness-adapter |
| R8 | `takt replay <case>` re-evaluates a recorded case from its records without a model | replay claims equal the recorded decision for every case in the corpus | no story yet |
| R9 | Every run records its pinned inputs: binary sha, protocol revision, gates file digest, harness and model version | `events.jsonl` of any run carries all five | partly story:run-record-and-story-evidence |
| R10 | Bounded runs: `--iterations`, `--hours`, `--tokens`, and loops declared in `workflow/1` | a run stops at the first bound reached, with a stop naming it | run.rs today, story:declared-loops, story:workflow-schema-and-loader |
| R11 | Scorecard per scenario with tokens, wall time, money and human minutes per accepted unit, against the promoted baseline | `takt scorecard` prints all four figures, each traceable to records or marked unknown | story:scorecard-reader (tokens, time); money and human minutes: no story yet |
| R12 | Next unit from the planning store, or an intent | `takt run` with no intent takes the next ready story | story:frontier-next-ready-story |
| R13 | Orchestration-ready: a run is addressable from outside (stable CLI exit codes, a machine-readable decision and stop, a per-case lock), so a later orchestrator can drive many repositories without a takt change | an external script runs takt on two repositories in parallel and reads both outcomes from files | the per-case lock `run.lock` (built); exit codes 0–5 (built on the wave 06 branch); design:continuous-orchestration § 6, § 9 (root case, dispatcher state in files); the rest: no story yet |
| R14 | Install and first run for an outside maintainer in one documented page | a maintainer with Rust and Claude Code reaches a first decision document from the README alone | no story yet |
| R15 | takt improves itself through its own protocols (`self.improvement/1`, `loop.improvement/1`) | one promotion decided by the scorecard with no regression | epic:recursive-self-improvement |
| R16 | Continuous work: a bounded run plans when its ready stories hold less work than the budget left, and ends for lack of work only with a named stop when no approved epic has unplanned work | a `--hours 2` run on a store with one ready story and one `active` epic with no stories plans that epic and delivers at least one story it planned; with no such epic it stops `FrontierExhausted` | design:continuous-orchestration § 2–3; no story yet |
| R17 | Steer and pause without losing work: a store edit takes effect at the next pick, `takt pause` stops at the next step boundary, `takt run` resumes, and a crashed run loses no case | the dispatcher killed mid-wave; the next `takt run` resumes every open case from files, with no lock removed by hand | design:continuous-orchestration § 4–5, § 9; no story yet |
| R18 | One repository's whole delivery as a tree of cases: planner, wave, reviewers and parallel implementation workers are child cases dispatched by protocol, each with its own role, worktree and budget | a wave of three stories runs as three parallel child cases under one root, merges through one integration branch, and every child's decision documents sit under that root | design:continuous-orchestration § 6–9, build steps 2–5; story:declared-loops (interim, serial) |

## 5. Success at about three months

| outcome | measured by | source today |
|---|---|---|
| self-delivery | takt delivers its own non-judge stories under the pinned judge (RSI phase R2) | design:recursive-self-improvement § 9 |
| daily runs | three repositories, one bounded run per day for a week, with units accepted, the four efficiency figures, human touches and defects escaped | story:week-of-daily-runs |
| outside users | at least one repository outside beyond10x runs takt and keeps the decision documents | no story yet |
| wave delivery through takt | AEP waves in other beyond10x repositories are delivered by takt runs instead of by hand | design:continuous-orchestration build steps 1–4; no story yet beyond story:declared-loops |

## 6. Not now, prepared for

| item | status |
|---|---|
| orchestration across repositories | designed as the last build step of design:continuous-orchestration; not built in this PRD's horizon. Orchestration inside one repository is in the horizon (R16–R18). No decision may close the cross-repository step off: R13 keeps every run addressable from outside, and no state lives only in a process |
| hosted service, console, database | not now; records stay files in the repository |
| a model runtime of takt's own | not planned; the harness is an adapter |

## 7. Open questions

| # | question |
|---|---|
| Q1 | Numeric targets for tokens, wall time and money per unit: set from the first week's baseline (story:week-of-daily-runs), or fixed now? |
| Q2 | Scenario rerun agreement: is ≥ 9 of 10 the target? |
| Q3 | License for the OSS release |
| Q4 | Which harness is second after Claude Code |
