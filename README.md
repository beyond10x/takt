# Takt

**Agentic coding that counts nothing as done until a validator has recorded the evidence.**

<!-- badges begin -->
[![stories implemented: 52 of 160](https://img.shields.io/badge/stories%20implemented-52%20of%20160-orange)](.engineering/planning)
[![composable agentic work: 28/91 stories](https://img.shields.io/badge/composable%20agentic%20work-28%2F91%20stories-orange)](.engineering/planning/epic/composable-agentic-work.md)
[![recursive self improvement: 18/40 stories](https://img.shields.io/badge/recursive%20self%20improvement-18%2F40%20stories-yellowgreen)](.engineering/planning/epic/recursive-self-improvement.md)
[![benchmark integration: 1/17 stories](https://img.shields.io/badge/benchmark%20integration-1%2F17%20stories-orange)](.engineering/planning/epic/benchmark-integration.md)
[![terminal frontend: 0/5 stories](https://img.shields.io/badge/terminal%20frontend-0%2F5%20stories-orange)](.engineering/planning/epic/terminal-frontend.md)
[![continuous orchestration: 0/1 stories](https://img.shields.io/badge/continuous%20orchestration-0%2F1%20stories-orange)](.engineering/planning/epic/continuous-orchestration.md)
[![improvement turns: 3 accepted, 3 promoted](https://img.shields.io/badge/improvement%20turns-3%20accepted%2C%203%20promoted-blue)](docs/metrics/improvement-turns.jsonl)
[![scenario runs recorded: 68](https://img.shields.io/badge/scenario%20runs%20recorded-68-blue)](docs/metrics/scenario-runs.jsonl)
[![held-out check: 54 of 56 pass](https://img.shields.io/badge/held--out%20check-54%20of%2056%20pass-brightgreen)](docs/metrics/scenario-runs.jsonl)
[![ESS citations resolved: 578 of 578](https://img.shields.io/badge/ESS%20citations%20resolved-578%20of%20578-brightgreen)](ess/domains)
<!-- badges end -->

Point `takt` at a repository and say "work on this". It takes an intent, or the next ready story
from the planning store, opens a case on a protocol, and lets a harness such as Claude Code take
one admissible step at a time. Each step's result is checked by a program, not by the model that
made it. [Canon](https://github.com/beyond10x/canon) decides what is permitted, what is owed and
whether completion was earned. The model never does.

> Agents choose how to do the work. The protocol defines what must be true before the work counts.

One open-source binary, Apache-2.0. Site: <https://beyond10x.github.io/takt/>.

## What you get when you run takt

| | |
|---|---|
| **Evidence bound to a revision** | Tests, gates and CI are read by validators, which write a record naming the commit they saw. Done is computed from those records. What the harness says is a trace. |
| **Merges only under a grant** | A merge waits for a grant recorded for the exact revision (`takt approve`); without one it is refused. On a judge surface the grant is the owner's, or a role's the owner delegated it to on record. |
| **A replayable case record** | Every case is files in the repository: each decision, step and evidence record, and `events.jsonl` with their times. A later run resumes from it. A reviewer can replay it. |
| **Named stops** | Budgets for iterations, hours and tokens. Every run ends on a named stop with its own exit code, so a script or an orchestrator can drive it. |
| **A measured self-improvement loop** | takt improves takt in turns. A fix counts when a check reproduced the defect before it and not after it, on the same scenario, and a release is promoted only if no scenario regresses. |

## Results so far

> **Scenario 1**, a 31-line change to a toy CLI, 2026-10-18. Every counted run of every arm
> passed a held-out check no agent saw.
>
> | arm | n | median tokens | median wall time | median cost |
> |---|---|---|---|---|
> | Claude Code, isolated like takt's harness, plus a shell | 6 | 92,772 | 55.5 s | $0.14 |
> | takt at 0a776d2 | 3 | 122,438 | 133 s | $0.25 |
> | Claude Code, the operator's full configuration, first round | 3 | 581,024 | 65 s | $0.48 |
>
> The isolated free agent was cheaper and faster: against its six runs, takt at 0a776d2 cost
> about 1.8x as much and took 2.4x the time. Later takt revisions cost the same or more: a0b8576, the
> binary the first improvement turn promoted, took a median 122,361 tokens, 127 s and $0.25 over
> 5 runs. Against Claude Code with its full configuration takt used 4.7x fewer tokens, and nearly
> all of that gap is the isolation, not the protocol.
> takt's case is what the free runs don't give: evidence, grants, a record and named stops.
>
> Caveats: one toy scenario and small n, so these are first results, not a benchmark. In the third
> round no arm pinned a model and takt's moved to another one, so that round's takt cost can't be
> compared. Three improvement turns have been accepted, their records written by hand. Every
> figure, with its source: [`docs/metrics/`](docs/metrics/README.md).

## Status

0.11.0 is the latest release. It holds `takt run`, `approve`, `protocols`, `scenario run`,
`scorecard`, `trace`, `report`, `view`, `promote` and `replay`, with `story.delivery/1` and `self.improvement/1`
embedded, and prebuilt binaries for Linux (x86_64, aarch64) and macOS (aarch64). A harness step
runs through a metaharness session with Claude Code, Codex or b10x-harness (`--harness`). Since 0.1.0 it
has gained the judge guard and the self-improvement protocol. A scorecard is now bound to the
binary that made its runs, and the promotion chain is anchored. A finding's check runs confined.
Promotions run one at a time. And a case is guarded against links in `.engineering/cases`,
against changes in ignored directories, and against the test command reaching a login. Details:
[`CHANGELOG.md`](CHANGELOG.md). Validators record an improvement turn's evidence now; the
three turns so far were in scratch trials. Open: no recorded run has yet taken a story from
ready to merged with the grant as the only manual step, and no operating-system sandbox holds
the harness (`--confinement substrate` holds the test command only).

| milestone | state |
|---|---|
| M1 one step validated | done 2026-10-18 (`docs/runs/2026-10-18-m1-scratch.md`) |
| M2 a story end to end | built: the planning-store frontier, the story protocol, the gate validator, the pull request and CI through the bot route, the merge under a recorded grant, waits on pending checks (wave 08), the run recorded on the story (wave 09), those records reaching the base with the merge (wave 11). Not yet proven: no recorded run has taken a story from ready to merged with the grant as the only manual step |
| unattended scenario 1 | next: accepted with one invocation plus one grant, daily for a week. Of 27 runs at 0a776d2 and later (T-1..9, T-11..28; not counted: T-fetchfail-1..3, refused at the trial fetch, and T-10, a runner race), 23 reached the merge approval in one invocation, 14 of the 15 after round 3 (`docs/metrics/`). Wave 12 merged the fixes for the three wastes those runs show (a second CI, a model step only for the approval, an inspect step before `NoProgress`) |
| self-improvement | phase R0: three improvement turns were accepted on 2026-10-18 and 2026-10-19 from records written by hand (`docs/metrics/improvement-turns.jsonl`). Built: the scorecard reader (`takt scorecard`, wave 10), the binary naming its revision (wave 11), promotion (`takt promote`), the run trace and report (`takt trace`, `takt report`), the judge guard (wave 14), and `self.improvement/1` driven by validators: part 1 in 0.2.1 (a turn opens and ends `unreproducible` or `declined`), part 2 in 0.2.2 (a turn reaches `accepted` on validators' records, judged by the binary that opened it). On 2026-10-19 takt drove three turns to `accepted` and promoted their candidates, in scratch trials: a scratch promotion store and install root, `--forge none`, `main` fast-forwarded by the operator (`docs/runs/2026-10-19-self-improvement-trials/README.md`). Not yet: a turn promoted in the installed takt's own store, and reading the protocol and the planning store from the promoted base (`story:judge-reads-protocol-and-store-from-base`) |
| M3 to M5 | workflows as `workflow/1` files, a second harness and the confinement seam, a week of daily runs: not built |
| continuous runs | a run that plans its next stories and works a wave with parallel workers: designed, not built |

The design is `docs/design/takt.md`; its § 4 lists what each milestone has. Scenario runs and
their figures are in `docs/metrics/`.

## Use

```
takt run --repo <path> --intent "make the failing test pass" --iterations 1
takt run --repo <path> --hours 1
takt approve --repo <path> <case> repository.merge --by human:<id>
takt scenario run --repo <path> <id> --hours 1
takt protocols
takt scorecard --repo <path> [--against <scorecard.json>]
takt trace --case <case dir> [--out <file>]
takt report --case <case dir> | --trace <file>
takt promote --repo <takt checkout> --scorecard <file> [--dry-run]
```

What each command does:
[Run a story](https://beyond10x.github.io/takt/docs/guides/run-a-story),
[Scenario run](https://beyond10x.github.io/takt/docs/guides/scenario-run),
[CLI reference](https://beyond10x.github.io/takt/docs/reference/cli),
[Exit codes](https://beyond10x.github.io/takt/docs/reference/exit-codes).

## Install

Prebuilt binaries for Linux (x86_64, aarch64) and macOS (aarch64), with checksums
(0.7.0 to 0.9.0 have no macOS binary):
[Install a release](https://beyond10x.github.io/takt/docs/getting-started#install-a-release). From source:

```
cargo install --locked --path crates/takt-cli
takt --version
```

## Requirements

A harness, `aep` for story runs, `b10x-gates` and `gh` for pull requests (`glab` instead of
`gh` on a GitLab host), bubblewrap for `takt promote`: [What it needs](https://beyond10x.github.io/takt/docs/getting-started#what-it-needs).

`--harness harness` points b10x-harness at the Anthropic Messages API on your claude.ai login
(`~/.claude/.credentials.json`) unless `--endpoint-base-url`, `--endpoint-wire`,
`--endpoint-model` and `--endpoint-token-file` with `--endpoint-token-pointer` say otherwise,
or `--endpoint-file`, a TOML file of `base_url`, `wire`, `model` and a `[credential]` table; a
flag wins over the file. A credential is named, never given: a token file's path, or a secret's
name in your secrets store (`--endpoint-secret <namespace>/<name>`), handed to b10x-harness
by name, which the harness resolves; takt reads no secret. Your
claude.ai token goes to the default base URL only, so another base URL needs a credential of
its own; a base URL is `https://`, or `http://` to a loopback host, with no userinfo, query or
fragment.

takt commits as the identity the repository names in `.engineering/commit-identity.conf`
(`[identity]` with `name` and `email`), or as `--commit-identity '<name> <email>'`, never as
your git configuration. With neither it commits as `takt <takt@localhost>`, and a run through
the bot route, which pushes those commits, is refused before its case opens.

The forge follows `origin`: `--forge bot` on github.com, `--forge gitlab` on any other host
while `glab auth status --hostname <host>` exits 0, else `--forge none`, with the reason in
one line. Through `gitlab`, git pushes over HTTPS with `glab auth git-credential` as its one
credential helper, and the merge request is opened, read and merged through `glab api`;
`--forge gitlab` without `glab` or its login is refused before the case opens. A scenario run
creates its trial base only through the bot route: it refuses `--forge gitlab`, and on another
host it runs with `--forge none`.

A `--forge none` delivery whose change reads ahead stops `Unperformable` at `pr.opened`, exit 7,
and prints `candidate: <full commit id>`: its code commits, without takt's take, record and
approval commits, replayed on the base branch's tip under that identity and named by
`refs/takt/candidates/<case>`, the revision a self-improvement turn opens on
(`--input candidate=<id>`). With no identity configured it makes none and names the setting.

bubblewrap is Linux only, so on macOS a promotion refuses with exit 1 before it judges;
`--dry-run` and `--baseline-only`, which build nothing, still run. On macOS a run kills what a
child process started in the child's process group only; on Linux takt also adopts and kills
what the child orphaned outside it. `--confinement substrate` holds the test command only;
nothing sandboxes the harness session yet (`story:harness-child-confined`).

## Built on

takt is an application on [Loom](https://github.com/beyond10x/loom)'s `b10x-loom-sdk`, pinned
in `Cargo.toml` at the Loom tag 0.39.1 (ADR 0099, `epic:takt-on-loom-sdk`):

- Commission's runtime drives each step of a case (`run_until_blocked`).
- Loom's governor decides the case with [Canon](https://github.com/beyond10x/canon) over the
  case's files, and takt's `CaseAuthority` answers it from `authority.yaml` when a step needs a
  grant.
- Each harness step runs in a metaharness 0.15.0 session started by Loom's session executor, the
  crate `b10x-loom-session-executor` 0.39.1 (`loom_sdk::session`). The harness proposes a change
  from a scratch copy, and takt's edit effect applies it under the step guard.

takt keeps its validators and their evidence, git and the forge, the planning-store seam,
scenarios, the scorecard, promotion, trace, report and view. The protocols come from
[Engineering protocols](https://github.com/beyond10x/engineering-protocols). What runs where
in the loop: [`docs/design/takt.md`](docs/design/takt.md) § 3. Licence: Apache-2.0.
