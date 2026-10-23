# Takt

Composable agentic work: one binary that drives a repository's work through engineering protocols,
every step validated by Canon over evidence. This file is for agents working on this repository.
`README.md` is for people.

## Serves

- **O2 — decisions as data, with evidence.** `takt run` executes a Canon protocol (`story.delivery/1`, `self.improvement/1`, crate `b10x-takt`) as data, and every step's claim is admitted only on validator evidence recorded in the case.
- **O3 — any harness, observed and compared.** `--harness claude|codex|harness` drives each harness through one metaharness session of Loom's session executor (`b10x-loom-session-executor`, reached as `loom_sdk::session`), each session's `AgentSession` record observed into the case, and `takt scenario run` with `takt scorecard --against` scores two of them on the same scenarios.
- **O6 — self-improvement, built into all of it.** `self.improvement/1` turns a run's record into a candidate change, and `takt scorecard --against` with `takt promote` re-measures it against the promoted baseline before it lands.

## Start here

| read | for |
|---|---|
| `product-requirements:takt` (`aep plan artifact show product-requirements:takt`) | users, the five qualities and their order, requirements R1–R18 |
| `docs/design/takt.md` | the design: foundations, shape, seams, milestones and what each has built |
| `design:recursive-self-improvement` | how takt improves itself: roles, judge, scorecard, phases |
| `.agents/dispatcher-agent.md`, `.agents/trial-agent.md` | the two sessions' profiles a fresh session starts from: the dispatcher's wave loop, the trial agent's cycle |
| `.agents/planner-agent.md`, `.agents/developer-agent.md`, `.agents/reviewer-agent.md`, `.agents/doc-writer-agent.md` | the four agent roles' profiles: what each reads, owns, watches and never does |
| `design:continuous-orchestration` | continuous work and orchestration: planning as a unit, a tree of cases, one deterministic dispatcher, build steps |
| ADR 0099 (atlas, `architecture/adr/0099-takt-is-an-application-on-loom-sdk.md`), `epic:takt-on-loom-sdk` | takt as an application on loom-sdk: Commission's runtime drives the case, the governor decides, the harness proposes; the epic's stories S1–S5 and the seams, with their gaps in Loom |
| `aep plan artifact waves --kind story --status draft` | what can be worked next |

## Layout

| path | what |
|---|---|
| `crates/takt` | the library `b10x-takt` (`use takt::…`): protocol loading (`protocol`), case files, the decision the governor of Loom renders and the evidence records (`case`, `evidence`), the loop (`run`, on Commission's runtime of `loom-sdk`, each harness session through its session executor `b10x-loom-session-executor`), the seams and their first adapters (`harness`, `frontier`, `validator`, `forge`), the git reads and writes (`git`), the run record read back (`scorecard`, `trace`, `report`) |
| `crates/takt/tests`, `crates/takt-cli/tests` | integration tests and their fixtures |
| `crates/takt-cli` | the binary `takt` |
| `protocols/` | this repository's `protocol/1` documents: story delivery (embedded in the binary as `story.delivery/1`), self-improvement, loop improvement |
| `ess/` | the ESS specification of takt's nouns and their relations |
| `docs/design/` | design documents |
| `docs/waves/`, `docs/runs/`, `docs/handoff/` | wave pages, recorded runs, handoff notes |
| `website/` | the public site: Docusaurus pages in `website/docs/`, the landing in `product.json`, and `scripts/generate.mjs`, which writes the status, charts, CLI reference and protocol viewer from the records at build time |
| `.engineering/planning` | the AEP planning store; change it only through `aep plan artifact` verbs |
| `.engineering/cases` | in a repository takt runs on: the cases a run writes; never edited by hand (absent here until takt runs on this repository) |

## Commands

```
task check                      # specification, citations, format, lint, tests
task check-static | test-cli | test-library   # the three parts of check that CI runs as parallel jobs (check.yml)
task build
task site-build                 # the documentation site into website/build
task site-check                 # site-build, then every page at 1440 and 390 px in headless Chrome (website/.site-check/)
aep plan artifact validate
python3 -I scripts/check-citations --require-digests [dump | fix | accept <doc>:<line>...]   # each ess/ `read from:` against its needles and digest (ess/domains/<doc>.citations.tsv); accept records a digest after a person or an agent read the lines, and the commit names the lines
python3 -I scripts/release-notes <version> | --check   # a release's GitHub notes from CHANGELOG.md (Keep a Changelog 1.1.0), or the whole file checked against that form
git add <paths> && b10x-gates bot -- commit -F <file>   # staging is plain git; the route refuses add
b10x-gates scan-text --policy <file> <path>...           # or B10X_GATES_POLICY; no policy found is "not scanned", never "passed"
cargo run -p b10x-takt-cli -- run --repo <path> --intent "<text>" --iterations 1
cargo run -p b10x-takt-cli -- run --help     # defaults and exit codes
```

## Rules

- When two product qualities conflict, the order is secure, autonomous, reliable, deterministic,
  efficient (`product-requirements:takt` § 3).
- Evidence comes from validators only, never from a model's text. A new claim needs a validator
  that writes a `canon-evidence/1` record bound to a revision. One exception, by design: a choice
  record (producer `takt.choice`, such as `handover`) is written by takt and attests which action
  the harness chose at a revision with no file changed, as an approval request does; it never makes a
  claim about the work true (`tests.pass`, `gate.green`, `ci.green`, `pr.merged`), and a protocol
  may gate an action on it but never an outcome on it alone.
- Protocols are data: use `b10x-canon-engineering`'s built-ins, or a `protocol/1` file; never a
  model of a protocol written in Rust. Where a person is needed is the protocol's: an action that
  `requires` a capability waits for a grant (`takt approve`). The governor of Loom decides every
  evaluation over the case's files and supplies no authority, so its decision finds such an
  action approval-required, granted or not. The grant reaches the run as the case's authority
  provider's answer (`CaseAuthority`, `crates/takt/src/case.rs`): it reads `authority.yaml`
  when asked and allows a grant only at the case revision asked about and the revision of its
  subject it was given at. The one gate in Rust is the merge validator's, which merges only under
  a grant of `repository.merge` at the case's revision; add no other.
- A judgement step offers the model only the actions the frontier admits; a step that does more is
  refused and reverted, not accepted.
- Credentials never enter a harness's environment, its prompt or a case record.
- A run's outcome lives in files and the exit code, never only in a process, so a later
  orchestrator can drive takt without a change to it.
- Judge surfaces are every path except `docs/**`, `README.md` and `CHANGELOG.md`. The owner grants
  the merge of a diff that touches a judge surface, or delegates that grant to a named role by a
  record; the delegation is revocable, and a revoked one grants nothing from then on
  (`product-requirements:takt` § 2). The delegation and its revocation are records the owner
  makes (`design:recursive-self-improvement` § Roles). Until
  story:judge-guard lands, the record is the owner's decision recorded in the planning store
  (`product-requirements:takt` § 2), and a revocation is a later owner decision recorded there;
  from story:judge-guard on, both are held in `.engineering/delegations.yaml`, read at the merge
  target's tip as the forge reports it, which is itself a judge surface. story:judge-guard enforces it. Adding a path to that
  list is itself a judge-surface change.
- Four agent roles work on takt (`design:recursive-self-improvement` § Roles): the planner, the
  developer, the reviewer and the doc-writer; the trial agent runs the trials beside them. § Roles
  below says what each owns, what it never does and how it runs. The planner plans and grants and
  writes no takt code; the developer delivers stories in waves and never marks its own fix
  verified; the reviewer reviews and verifies, and never grants or files; the doc-writer
  writes `docs/**` but the wave pages, the README's structure and the public site, and writes no
  takt code; the trial agent runs trials, scenario runs and scorecards, proposes candidates and
  promotes (`takt promote` records and installs), and never edits a wave's tree or branch.
- A story that comes from a self-improvement trial, a self-improvement turn or a review of
  self-improvement work carries the tag `rsi`, and every open `rsi` story is scheduled before any
  story without it: a wave takes the `rsi` stories first, and opens without one only for a unit it
  depends on (owner, 2026-10-19).
- A finding goes into the planning store with an `expected_before` predicate a machine can check.
  A sub-agent's return is its message; the four hand-overs between the dispatcher and the trial
  agent are in § Messages below.
- The dispatcher works in one checkout on its wave branch, with one `target/`; its sub-agents edit
  that tree inside their story's Scope and commit nothing. The trial agent works only in managed
  worktrees (`worktree create --purpose <what>`), one per candidate, each building into its own
  `target/`.
- Every commit, push, pull request and tag goes through the bot route (`b10x-gates bot -- <git
  args>`); API writes through `b10x-gates api`. Pull-request and release bodies pass
  `b10x-gates scan-text` first.
- A number in a commit message or a wave page (passes, tests, tokens, files) is the output of a
  command run on that tree; a figure nobody computed is left out.
- This repository is public. Committed text names no private repository, session or decision id,
  host detail or person.
- Spec first: a story that introduces a noun declares it in `ess/` before code builds on it,
  checked with `ess specify validate --path ess`; every `read from:` citation there names the
  line that says it.

## Roles

From `design:recursive-self-improvement` § Roles, the owner's decision (2026-10-18). Each agent
role's profile is `.agents/<role>-agent.md`, a frontmatter of `name` and `description` and then
the profile; `.claude/agents/<role>-agent.md` is a relative symlink to it.

| role | owns | never | runs as |
|---|---|---|---|
| owner (a person) | merge approval for judge surfaces, or its delegation to a named role on record; scorecard definitions; priorities | | a person |
| planner | the main agent and orchestrator; the planning store: stories, priorities within the owner's, wave order with the developer; filing findings; deciding what the developer escalates; granting merges on a reviewer verdict, judge surfaces only under a delegation on record naming the planner; dispatching the reviewer and the doc-writer; reporting to the owner | writes takt code; grants without a reviewer verdict on that exact sha; grants a widening change without a reviewer verdict or without naming it to the owner (the owner's standing grant of 2026-10-18 covers widening changes) | the dispatcher session |
| developer | moving stories to active, waves and their pages under `docs/waves/`, implementation, merges through the bot route; README lines that document its own unit's change | marks its own fix verified; edits `docs/**` other than its wave pages, the site or the deck | the dispatcher session, which dispatches implementor sub-agents into its tree |
| reviewer | a verdict with cited findings on each grant request's diff at its exact sha; independent code reviews (correctness, security, usability); adversarial passes on request; verification runs of a finding (`scripts/check-finding`) on request | writes takt code; grants; files to the store (it returns findings to the planner, who files them); reviews its own work | a sub-agent started per task from its profile, by the dispatcher or, for a proposal or a candidate, by the trial agent |
| doc-writer | `docs/**` but the wave pages: designs' prose under `docs/design/`, run records under `docs/runs/`, `docs/metrics/`, `docs/pitch/`; the structure of `README.md`; the public site's pages, `website/docs/**` and `website/product.json`, deployed from `main` by `.github/workflows/pages.yml` and the sibling repositories' reusable project-site workflow, while the rest of `website/` changes only on a planner task after a reviewer verdict | writes takt code; changes the planning store; states a figure no record holds | a sub-agent the dispatcher starts per task, from its profile |
| trial agent | trial cycles on a release that touches code or on a person's request: the binary, trials and scenario runs (`takt scenario run`), scorecards and measurement against the baseline, rollouts and their rendering, proposals, candidates in the public repository that owns each proposal's layer (ADR 0101) and their re-trials; the promotion of a merged candidate and the install of its binary (`takt promote`) | edits the dispatcher's checkout, a wave's branch or `main`; edits any repository outside a managed worktree of its cycle or commits outside that repository's bot route; merges its own candidate (it hands an accepted takt candidate to the dispatcher as a unit of the next wave, and another repository's to that repository's delivery session through the monitor); reviews its own proposal or candidate; files to any planning store (it hands a rejection to the dispatcher, or to the other repository's delivery session, which files it) | its own session, in managed worktrees, with proposer, reviewer and implementor sub-agents |

Until takt drives this flow itself, one session, the dispatcher (`.agents/dispatcher-agent.md`),
holds the planner's and the developer's roles in one checkout: it picks each wave, dispatches
implementor sub-agents into the shared tree, commits each unit, starts the reviewer and the
doc-writer as sub-agents, grants on the reviewer's verdict and merges the wave. Under the
developer's role it writes a fix of under 20 lines itself, reviewed in the wave's adversarial pass
like any unit; the planner's "writes takt code" binds the planner's role alone. The trial agent
(`.agents/trial-agent.md`) is the one other session; no wave waits on it.

## Messages

The store carries the content. Inside the dispatcher's session there are no messages between
agents: a sub-agent's return (its changed paths, its commands and their output, what is left) is
the message, and the dispatcher acts on it. Between the dispatcher and the trial agent there are
four hand-overs, each a fixed first line. In a candidate's and a rejection's, `<repository>` is
the repository that owns the candidate's layer (ADR 0101; `.agents/trial-agent.md` step 5), and
a finding body follows the first line (`.agents/planner-agent.md` § Intake). The dispatcher files
it when `<repository>` is takt. For another repository the hand-over goes to that repository's
delivery session through the monitor, and the dispatcher's copy is information: it files nothing
and merges nothing (`.agents/dispatcher-agent.md` § Self-improvement in this loop).

| first line | sent by | to | after |
|---|---|---|---|
| `[TRIAL takt] <tag> <sha>` | dispatcher | trial agent | tagging a release whose diff against the latest promotion record's revision touches a path outside `docs/**`, `website/**`, `.engineering/**`, `README.md` and `CHANGELOG.md` |
| `[CANDIDATE <repository>] <branch> <sha> <verdict> <scorecard>` | trial agent | for takt, the dispatcher; for another repository, its delivery session through the monitor, a copy to the dispatcher | a candidate accepted on its re-trials and `not_reproduced` at its sha, with the reviewer's verdict at that sha |
| `[REJECTED <repository>] <branch> <sha>` | trial agent | for takt, the dispatcher; for another repository, its delivery session through the monitor, a copy to the dispatcher | a candidate rejected on its re-trials or its check, or its promotion refused |
| `[MERGED takt] <branch> <sha>` | dispatcher | trial agent | merging a wave that carries the candidate into `main` and pushing `main`, at that merge's sha; the trial agent promotes the candidate's own sha |
