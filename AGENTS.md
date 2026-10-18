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
| `.agents/operator-agent.md`, `.agents/developer-agent.md` | the two agent roles' profiles a fresh session starts from: what each reads, owns, watches and never does |
| `design:continuous-orchestration` | continuous work and orchestration: planning as a unit, a tree of cases, one deterministic dispatcher, build steps |
| `aep plan artifact waves --kind story --status draft` | what can be worked next |

## Layout

| path | what |
|---|---|
| `crates/takt` | the library `b10x-takt` (`use takt::…`): protocol loading (`protocol`), case files and evaluation (`case`, `decision`, `evidence`), the loop (`run`), the seams and their first adapters (`harness`, `frontier`, `validator`, `forge`), the git reads and writes (`git`) |
| `crates/takt/tests`, `crates/takt-cli/tests` | integration tests and their fixtures |
| `crates/takt-cli` | the binary `takt` |
| `protocols/` | this repository's `protocol/1` documents: story delivery (embedded in the binary as `story.delivery/1`), self-improvement, loop improvement |
| `ess/` | the ESS specification of takt's nouns and their relations |
| `docs/design/` | design documents |
| `docs/waves/`, `docs/runs/`, `docs/handoff/` | wave pages, recorded runs, handoff notes |
| `.engineering/planning` | the AEP planning store; change it only through `aep plan artifact` verbs |
| `.engineering/cases` | in a repository takt runs on: the cases a run writes; never edited by hand (absent here until takt runs on this repository) |

## Commands

```
task check                      # specification, format, lint, tests
task build
aep plan artifact validate
cargo run -p b10x-takt-cli -- run --repo <path> --intent "<text>" --iterations 1
cargo run -p b10x-takt-cli -- run --help     # defaults and exit codes
```

## Rules

- When two product qualities conflict, the order is secure, autonomous, reliable, deterministic,
  efficient (`product-requirements:takt` § 3).
- Evidence comes from validators only, never from a model's text. A new claim needs a validator
  that writes a `canon-evidence/1` record bound to a revision.
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
  makes, held at the base revision (`design:recursive-self-improvement` § Roles). Until
  story:judge-guard lands, the record is the owner's decision recorded in the planning store
  (`product-requirements:takt` § 2), and a revocation is a later owner decision recorded there;
  from story:judge-guard on, both are held in `.engineering/delegations.yaml`, read at the base
  revision, which is itself a judge surface. story:judge-guard enforces it. Adding a path to that
  list is itself a judge-surface change.
- Two agent roles work on takt (`design:recursive-self-improvement` § Roles). The operator agent
  runs takt, reviews, measures, files findings and verifies fixes; it writes no takt code. The
  developer agent delivers stories in waves; it never marks its own fix verified.
- A finding goes into the planning store with an `expected_before` predicate a machine can check.
  Messages between the agents only announce a store change, with a first line
  `[FINDING takt] <ids>`, `[IMPLEMENTED takt] <story> <sha>` or
  `[VERIFIED takt] <story> <scenario> <before> -> <after>`.
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
