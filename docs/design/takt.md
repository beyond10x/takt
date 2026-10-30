# Takt: composable agentic work

Status: design of 2026-10-18, revised for ADR 0099, which makes takt an application on loom-sdk (`epic:takt-on-loom-sdk`); the waves that built it are under `docs/waves/`. M1 is done (`docs/runs/2026-10-18-m1-scratch.md`); M2 is built but not yet proven; M3 to M5 are not built. § 4 says what each milestone has; a feature marked *not built* below is design only.

## 1. The target

Safe agentic coding, applied. A YAML description of workflows made of protocols (a software change
is one, from engineering protocols) and building blocks: inner loops, outer loops, validation
loops. A harness such as Claude Code understands the protocols and the blocks and drives a workflow
through the model, in any code repository, ad hoc or from the repository's default. Every step is
validated by a hard gate: Canon evaluates the case over evidence, and AEP is the record.

The result is one binary. Point it at a repository and say "work on this": it pulls the next unit
from the planning store (or takes an intent), opens a case, and runs one iteration, three, or three
hours, by the stop criteria given. Everything is pluggable and composable.

The name: takt time is the pace at which units of work are finished. One run is one takt.

## 2. Foundations

| foundation | what it gives | release |
|---|---|---|
| Loom | the runtime takt runs on: Commission's runtime (`run_until_blocked`) drives a case step by step; the governor (`CanonGovernor`) decides it with Canon over the case's files; an `AgentExecutor` proposes and an `EffectPort` performs; the session executor runs a harness session over metaharness (`loom_sdk::session`, crate `b10x-loom-session-executor`); the `SubstrateRunner` confines a test command | `b10x-loom-sdk` 0.49.1 (`Cargo.toml`) |
| Canon | `protocol/1`; `evaluate` is a pure function from protocol, case snapshot, evidence records, authority and instant to claims (TRUE, FALSE, UNKNOWN), obligations, admissible actions and legitimate outcomes, with an explanation; evidence is bound to the revision it was observed on. takt reaches it through Loom's governor and calls no `b10x_canon::eval` itself (`crates/takt/tests/no_own_evaluation.rs`) | `b10x-canon` 0.7.0 |
| Engineering protocols | `software.change/1` and the other engineering protocols as Canon documents, validated by Canon; assertion providers and the gates command line | `b10x-canon-engineering` 0.13.0 |
| AEP | the planning store: stories, scope, waves derived from scope, evidence on a story | `aep` 0.74.0 |
| metaharness | the harness sessions Loom's session executor starts: Claude Code, Codex or b10x-harness (`--harness claude\|codex\|harness`), each working in a scratch copy of the case tree and proposing its change | `metaharness` 0.23.0 (`Cargo.lock`) |

ADR 0090 (Loom) and ADR 0099 put the runtime in Loom, and the owner's decision of 2026-10-20 has
Loom reach agent harnesses only through metaharness. takt keeps what is its own: validators and
evidence, git and the forge, the planning-store seam, scenarios, scorecard, promotion and
improvement, trace, report and view.

Two rules carried over from the thesis these foundations serve: "Agents choose how to do the work.
The organization defines what must be true before the work counts", and a run completing or a pull
request merging is not completion; completion is earned when the protocol's conditions hold.

## 3. Shape

```
takt run --repo <path> [--intent "<text>" | --case <id>] [--workflow <file>]   --workflow: not built (M3)
         [--iterations N | --hours H | --tokens T | --until <outcome>]   --until: not built

  unit     := intent | frontier(repo)              AEP: ready stories and waves; no model
  case     := open(protocol, unit, worktree)       case files under .engineering/cases/<id>/, Loom's case store
  loop     := pre-check → evaluate → frontier → propose → admit → perform → observation → validator → evidence → evaluate
                one call of run_until_blocked per step, with a budget of one step
                judgement step: the harness proposes ONE admissible action from a scratch copy; takt's effect applies it
                deterministic step: a validator's action (tests, a gate, a pull request, a CI read, a merge on a grant)
             until an outcome is legitimate | an approval is needed | the stop criteria | nothing admissible
  record   := decisions/NNNN.json per evaluation, observations.json, evidence/, events.jsonl; AEP evidence on the story
```

Each part of the loop and what performs it now:

| part | performed by | held in |
|---|---|---|
| the step | Commission's runtime of loom-sdk: `run_until_blocked`, one call per step | `crates/takt/src/run.rs` (`Run`), crate `b10x-loom-commission` 0.49.1 |
| evaluate, frontier, admit | the governor of Loom (`CanonGovernor`) over the case's files (`CaseFiles`), deciding with Canon; each decision is written as `decisions/NNNN.json` | `crates/takt/src/case.rs`, crate `b10x-loom-governor` 0.49.1 |
| a grant | the case's authority provider (`CaseAuthority`, Loom's `AuthorityProvider`) over `authority.yaml` | `crates/takt/src/case.rs` |
| pre-check | takt itself, ahead of each Loom step: `Shape::only_behind` stops `ApprovalRequired` (exit 4), `Shape::unperformable` stops `Unperformable` (exit 7) | `crates/takt/src/run.rs` |
| propose | the composite executor (`TurnExecutor`, Loom's `AgentExecutor`): the due validator's action, else a judgement step of a session executor (`SessionHarness` over Loom's `SessionExecutor` and metaharness) | `crates/takt/src/run.rs`, `crates/takt/src/harness.rs`, crate `b10x-loom-session-executor` 0.49.1 |
| perform | the composite effect (`TurnEffect`, Loom's `EffectPort`): the edit effect applies a proposal under the step guard; `ValidatorEffect` performs a validator's action, `ChoiceEffect` records a choice | `crates/takt/src/run.rs`, `crates/takt/src/validator.rs` |
| observation | the session's `AgentSession` record, observed into `observations.json` through the governor | `crates/takt/src/run.rs`, `crates/takt/src/case.rs` |
| validator | a validator (`TestsRun`, `GateRun`, `PrOpen`, `CiRead`, `PrMerged`, `ChangeRead`) turns an observation into a `canon-evidence/1` record, which reaches the case through the evidence port | `crates/takt/src/validator.rs` |
| forge | the pull request, CI read and merge: through the repository's bot route (`BotRoute`) for an `origin` on github.com, or through `glab` (`Glab`) for one on a GitLab host, where the pull request is a merge request and its head pipeline is what `ci.read` reads | `crates/takt/src/forge.rs`, `crates/takt/src/forge/gitlab.rs` |
| child process | a confinement adapter on Substrate's contract (`substrate_host::confinement`): `Unconfined`, the test command through Substrate's `local` adapter declared insecure, or `Substrate`, through the first of its `systemd` around `linux`, `linux` and `macos` adapters that serves it | `crates/takt/src/confinement.rs` |

The hard rule: the model never produces evidence. A validator, a program, turns an observation
(a test run's exit status, a commit, a merged pull request) into a `canon-evidence/1` record bound
to the revision it observed. What the harness says is a trace.

### Building blocks of `workflow/1` (not built, M3: today the protocol and inputs are flags and the steps are code)

| block | fields | example |
|---|---|---|
| `protocol` | id, revision, inputs | `software.change/1`, risk `standard`, change_kind `bugfix` |
| `step` deterministic | action, argv | `tests.run` → `cargo test --quiet` |
| `step` judgement | actions (a subset of the protocol's), brief | implement among `tests.write`, `implementation.edit`, `repository.inspect` |
| `loop` | steps, until (a claim, an outcome or a budget) | inner: until `tests.pass`; validation: until the gate is green; outer: the next story until none is ready or the budget is spent |
| `gate` | a `canon-expr/1` assertion and its provider | `tests.unit.failed == 0`; later `pr.merged`, `ci.green` |
| `approval` | capability, approver | `repository.merge` by the operator |

### Seams

Traits in the library: `Harness` (the judgement actor; built: `SessionHarness` for Claude Code, Codex and
b10x-harness, each through Loom's session executor), `Frontier` (the planning store; built: AEP), `Validator` (an
evidence producer, each performing as a Commission effect), `Forge` (pull request, merge, release; built: all but
release, through the repository's bot route, through `glab` on a GitLab host, and the in-memory double `Memory` the
tests use), `Confinement` (built: `none` and `substrate`, which holds the test command only). The `Harness` trait is
still takt's own; story:takt-harness-trait-retired makes each harness a Loom `AgentExecutor`.

`--forge` names the adapter: `bot`, `gitlab` or `none`. Not given, it follows `origin`: `bot` on github.com,
`gitlab` on another host while `glab auth status --hostname <host>` exits 0, else `none`, with what glab said in
one line. Through `gitlab`, git pushes the case's branch over HTTPS with `glab auth git-credential` as its one
credential helper, `glab api` opens the merge request, reads its state and head pipeline and merges it under a
grant, and takt holds no token. The merge request's files keep the bot route's `pull-request.*` names. A GitLab
repository's CI definition, `.gitlab-ci.yml` and `.gitlab/**` at the root, is on the judge surface beside `.github/**`
(`GITLAB_CI`, `GITLAB_DIR`, `crates/takt/src/run.rs`), so a step that edits it is refused; a file outside both that an `include: local:` names is not.

The endpoint b10x-harness's loop is pointed at under `--harness harness` is configured (`EndpointConfig`,
`crates/takt/src/harness.rs`): its base URL, wire, model and credential come from the `--endpoint-*` flags or from
`--endpoint-file`, a TOML file, a flag winning over the file; with neither, it is the Anthropic Messages API on the
user's claude.ai login. A credential is named, never given: a token file with an optional JSON pointer, or a
secret's name (`--endpoint-secret <namespace>/<name>`). The case records the endpoint by reference on its `opened`
event (base URL, wire, model and the credential's `kind`, `subscription_file` with `path` and `pointer` or `secret`
with `namespace` and `name`, never a secret), and only a `--harness harness` run writes that `endpoint` key. A
secret's name is handed to b10x-harness as that name through the session executor's `api_key_secret` (loom 0.39.1);
the harness's loop opens the secret store itself, so the key never enters takt, metaharness or the child's environment (story:harness-endpoint-is-configured).

## 4. Milestones

| m | deliverable | proof | state |
|---|---|---|---|
| M1 one step validated | the kernel (case files, Canon evaluation), `takt run --repo R --intent "…" --iterations 1` on `software.change/1`, the Claude Code adapter (one action per step in a worktree), a `tests.run` validator, the decision document printed | a scratch crate with one failing test: `tests.pass` TRUE, `repository.merge` approval-required, 0 evidence from the model | done 2026-10-18 (`docs/runs/2026-10-18-m1-scratch.md`) |
| M2 a story end to end | the AEP frontier (ready rule as code, `artifact waves`); a protocol for a story with claims `tests.pass`, `gate.green`, `pr.opened`, `ci.green`, `pr.merged` and outcome `accepted`; `approval` for the merge; `--hours`, `--tokens`; AEP evidence written | one ready story of a small repository done with no manual step but the merge approval | built: the frontier (the ready rule over `artifact list`; `artifact waves` is not used), `story.delivery/1` (embedded, then the default for a store story), the merge under a grant `takt approve` gives, `--hours`, `--tokens`, the approval recorded on the story, the run recorded on the story (story:run-record-and-story-evidence, wave 09: `events.jsonl` per case, a `test_result` record on the story and its move to `implemented` when the case is accepted, written on the case branch), and those records reaching the base with the merge (story:records-reach-base, wave 11); proof: scenario 1 reached `accepted` (pull request merged) on 2026-10-18, but on more than the one grant M2 allows: 4 invocations where 2 were required, `B10X_GATES_POLICY` set by hand; the unattended proof is `product-requirements:takt` § 5, first row |
| M3 workflows | the `workflow/1` schema and loader; inner, validation and outer loops as declared blocks; a repository default in `.engineering/workflow.yaml` | the same story from a YAML file, and a three-iteration outer loop | not built (story:workflow-schema-and-loader, story:declared-loops) |
| M4 pluggable | the five traits; a second harness adapter proves the seam; validators from the engineering assertion providers; Substrate optional | the M2 story runs under both adapters with the same decision documents | built: the five traits; three harnesses through Loom's session executor (`--harness claude\|codex\|harness`, story:second-harness-adapter); the gate validator through the assertions library; `--confinement none\|substrate` (story:confinement-seam). Not built: confining the harness session, git, the forge and aep (story:harness-child-confined) |
| M5 prove | three repositories, each a daily `run --hours 2` for a week; units accepted, tokens, human interventions, defects escaped | the numbers | not started (story:week-of-daily-runs) |

Since M2: `story.delivery/2` of the engineering protocols was the default for a store story from
0.16.0 through 0.23.0, and takt's own `story.delivery/3` (`protocols/story-delivery/3.yaml`) is
embedded from 0.19.0. From 0.24.0, a store story opens on `story.delivery/3`
when nothing names a protocol (`crates/takt/src/protocol_defaults.yaml`); a case already open on
`story.delivery/2` resumes on it. On 2026-10-29 five runs on `story.delivery/3`, with Codex and `--forge none`, took one
story of takt's own store through its tests, the gate and the handover to `pr.opened`
(`docs/runs/2026-10-29-self-run-story-delivery.md`). A delivery on this repository with a forge,
pull request opened, CI read and merged, is not proven.

## 5. Not in this design

A hosted service, a console, a database and cross-repository scheduling. Loom's own model executor
as takt's judgement actor, and Loom's CLI, intake router and connectors (`epic:takt-on-loom-sdk`
§ Out of scope). A takt-side substitute for a gap in loom-sdk: a story such a gap blocks stays
blocked.

Still takt's own, besides what § 2 lists:

- the evaluation the loop reads before each Loom step (`CaseDir::evaluate`), for two uses. The two
  pre-checks of § 3 (`Shape::only_behind`, `Shape::unperformable`) are kept by
  approval-record:loop-pre-checks-ahead-of-loom until a change to Loom's documented contract.
  The loop's other reads of it (the frontier line it prints, the claims, `legitimate()`,
  `admissible()` and `approval_required()`, `crates/takt/src/run.rs`) stay until
  story:takt-harness-trait-retired;
- the `Harness` trait and the `harness` module (`crates/takt/src/harness.rs`), until
  story:takt-harness-trait-retired.

Not built yet: confining the harness session, git, the forge and aep; `--confinement substrate`
holds the test command only (story:harness-child-confined).

Continuous work and orchestration inside one repository, planning as a unit, every loop a case in
one tree, one deterministic dispatcher, and cross-repository scheduling as its last step, are
designed in the planning-store artifact `design:continuous-orchestration`
(`.engineering/planning/design/continuous-orchestration.md`).

## 6. Self-improvement

takt is improved by takt: the improvement process is itself modelled as protocols, and the same
loop improves the loop. The design is the planning-store artifact
`design:recursive-self-improvement` (`.engineering/planning/design/recursive-self-improvement.md`),
carried by `epic:recursive-self-improvement`.

| level | protocol | one case is |
|---|---|---|
| L0 work | `story.delivery/1` | one story of a repository |
| L1 improve takt | `self.improvement/1` | one finding reproduced on the promoted baseline, fixed, verified by another run, merged under the authority its surface needs, scored without regression, promoted |
| L2 improve the loop | `loop.improvement/1` | one loop metric below target, an honest change to the loop merged by the owner or a role the owner delegated the grant to on record, the metric improved |

`crates/takt/tests/improvement_protocols.rs` drives cases on both improvement protocols to their
outcomes. The validators for their actions are stories of that epic; until they land, the agents
perform the actions and record the evidence.
