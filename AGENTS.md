# Takt

Composable agentic work: one binary that drives a repository's work through engineering protocols,
every step validated by Canon over evidence. This file is for agents working on this repository.
`README.md` is for people.

## Start here

| read | for |
|---|---|
| `product-requirements:takt` (`aep plan artifact show product-requirements:takt`) | users, the five qualities and their order, requirements R1–R18 |
| `docs/design/takt.md` | the design: foundations, shape, seams, milestones and what each has built |
| `design:recursive-self-improvement` | how takt improves itself: roles, judge, scorecard, phases |
| `.agents/planner-agent.md`, `.agents/developer-agent.md`, `.agents/reviewer-agent.md`, `.agents/doc-writer-agent.md` | the four agent roles' profiles a fresh session starts from: what each reads, owns, watches and never does |
| `design:continuous-orchestration` | continuous work and orchestration: planning as a unit, a tree of cases, one deterministic dispatcher, build steps |
| `aep plan artifact waves --kind story --status draft` | what can be worked next |

## Layout

| path | what |
|---|---|
| `crates/takt` | the library `b10x-takt` (`use takt::…`): protocol loading (`protocol`), case files and evaluation (`case`, `decision`, `evidence`), the loop (`run`), the seams and their first adapters (`harness`, `frontier`, `validator`, `forge`), the git reads and writes (`git`), the run record read back (`scorecard`, `trace`, `report`) |
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
task build
task site-build                 # the documentation site into website/build
task site-check                 # site-build, then every page at 1440 and 390 px in headless Chrome (website/.site-check/)
aep plan artifact validate
python3 -I scripts/check-citations --require-digests [dump | fix | accept <doc>:<line>...]   # each ess/ `read from:` against its needles and digest (ess/domains/<doc>.citations.tsv); accept records a digest after a person read the lines
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
  `requires` a capability waits for a grant (`takt approve`). The one gate in Rust is the merge
  validator's, which merges only under a grant of `repository.merge` at the case's revision; add
  no other.
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
  developer, the reviewer and the doc-writer. § Roles below says what each owns, what it never
  does and how it runs. The planner plans and grants and writes no takt code; the developer
  delivers stories in waves and never marks its own fix verified; the reviewer reviews, runs and
  measures, and never grants or files; the doc-writer writes `docs/**` but the wave pages, the
  README's structure and the public site, and writes no takt code.
- A finding goes into the planning store with an `expected_before` predicate a machine can check.
  Messages between the agents only announce a store change; their first lines are in § Messages
  below.
- Work in a managed worktree (`worktree create --purpose <what>`), build into its own `target/`.
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
| planner | the main agent and orchestrator; the planning store: stories, priorities within the owner's, wave order with the developer; filing findings; deciding what the developer escalates; granting merges on a reviewer verdict, judge surfaces only under a delegation on record naming the planner; dispatching the reviewer and the doc-writer; reporting to the owner | writes takt code; grants without a reviewer verdict on that exact sha; grants a widening change without a reviewer verdict or without naming it to the owner (the owner's standing grant of 2026-10-18 covers widening changes) | the main session |
| developer | moving stories to active, waves and their pages under `docs/waves/`, implementation, merges through the bot route, building and installing promoted binaries; README lines that document its own unit's change | marks its own fix verified; edits `docs/**` other than its wave pages, the site or the deck | its own long-running session |
| reviewer | a verdict with cited findings on each `[GRANT? takt]` diff at its exact sha; independent code reviews (correctness, security, usability); adversarial passes on request; trials, scenario runs and verification runs (`scripts/check-finding`, `takt scenario run`); measurement against the baseline | writes takt code; grants; files to the store (it returns findings to the planner, who files them); reviews its own work | started by the planner per task, from its profile, and resumed by name |
| doc-writer | `docs/**` but the wave pages: designs' prose under `docs/design/`, run records under `docs/runs/`, `docs/metrics/`, `docs/pitch/`; the structure of `README.md`; the public site's pages, `website/docs/**` and `website/product.json`, deployed from `main` by `.github/workflows/pages.yml` and the sibling repositories' reusable project-site workflow, while the rest of `website/` changes only on a planner task after a reviewer verdict | writes takt code; changes the planning store; states a figure no record holds | started by the planner per task, from its profile, and resumed by name |

Until takt drives this flow itself, the planner is the main agent: it starts the reviewer and the
doc-writer, and the developer reports to it.

## Messages

The store carries the content; a message only announces that the store changed. Its first line is
one of these, and nothing else. This table is the contract; each profile under `.agents/` carries
a copy of it, and a copy that differs from it is wrong:

| first line | sent by | to | after |
|---|---|---|---|
| `[FINDING takt] <ids>` | planner | developer | filing findings in the store |
| `[IMPLEMENTED takt] <story\|wave> <sha>` | developer | planner | merging a story's fix or a wave |
| `[VERIFIED takt] <story> <scenario> <before> -> <after>` | planner | developer | a reviewer's verification run that moved the number |
| `[WAVE takt] <wave> <units>` | developer | planner | opening a wave and its page under `docs/waves/` |
| `[GRANT? takt] <story> <branch> <sha> <paths>` | developer | planner | a merge ready but for its grant; `<paths>` are the paths its diff touches |
| `[GRANT takt] <story> <sha>` | planner | developer | granting that merge on the reviewer's verdict at that sha |
| `[DECIDE takt] <topic>` | developer | planner | a question it escalates; the lines below the first give options A, B and C, a recommendation and a default |
| `[DECIDED takt] <topic>: <option>` | planner | developer | recording the decision in the store |
| `[ACK takt]` | any | the sender | receiving a message |
