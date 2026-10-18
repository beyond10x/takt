# Takt

**Agentic coding that counts nothing as done until a validator has recorded the evidence.**

<!-- badges begin -->
[![stories implemented: 30 of 75](https://img.shields.io/badge/stories%20implemented-30%20of%2075-yellowgreen)](.engineering/planning)
[![composable agentic work: 21/40 stories](https://img.shields.io/badge/composable%20agentic%20work-21%2F40%20stories-yellowgreen)](.engineering/planning/epic/composable-agentic-work.md)
[![recursive self improvement: 9/19 stories](https://img.shields.io/badge/recursive%20self%20improvement-9%2F19%20stories-yellowgreen)](.engineering/planning/epic/recursive-self-improvement.md)
[![benchmark integration: 0/11 stories](https://img.shields.io/badge/benchmark%20integration-0%2F11%20stories-orange)](.engineering/planning/epic/benchmark-integration.md)
[![terminal frontend: 0/5 stories](https://img.shields.io/badge/terminal%20frontend-0%2F5%20stories-orange)](.engineering/planning/epic/terminal-frontend.md)
[![continuous orchestration: 0/1 stories](https://img.shields.io/badge/continuous%20orchestration-0%2F1%20stories-orange)](.engineering/planning/epic/continuous-orchestration.md)
[![improvement turns: 1 accepted, 1 promoted](https://img.shields.io/badge/improvement%20turns-1%20accepted%2C%201%20promoted-blue)](docs/metrics/improvement-turns.jsonl)
[![scenario runs recorded: 42](https://img.shields.io/badge/scenario%20runs%20recorded-42-blue)](docs/metrics/scenario-runs.jsonl)
[![held-out check: 30 of 30 pass](https://img.shields.io/badge/held--out%20check-30%20of%2030%20pass-brightgreen)](docs/metrics/scenario-runs.jsonl)
[![ESS citations resolved: 421 of 421](https://img.shields.io/badge/ESS%20citations%20resolved-421%20of%20421-brightgreen)](ess/domains)
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
> about 1.8x as much and took 2.4x the time. Later takt revisions were not cheaper. Against Claude Code with its full configuration
> takt used 4.7x fewer tokens, and nearly all of that gap is the isolation, not the protocol.
> takt's case is what the free runs don't give: evidence, grants, a record and named stops.
>
> Caveats: one toy scenario and small n, so these are first results, not a benchmark. In the third
> round no arm pinned a model and takt's moved to another one, so that round's takt cost can't be
> compared. One improvement turn has been accepted, and its records were written by hand. Every
> figure, with its source: [`docs/metrics/`](docs/metrics/README.md).

## Status

| milestone | state |
|---|---|
| M1 one step validated | done 2026-10-18 (`docs/runs/2026-10-18-m1-scratch.md`) |
| M2 a story end to end | built: the planning-store frontier, the story protocol, the gate validator, the pull request and CI through the bot route, the merge under a recorded grant, waits on pending checks (wave 08), the run recorded on the story (wave 09). Not yet proven: no recorded run has taken a story from ready to merged with the grant as the only manual step |
| unattended scenario 1 | next: accepted with one invocation plus one grant, daily for a week. Of 12 timed runs at 0a776d2 and later, 9 reached the merge approval in one invocation (`docs/metrics/`) |
| self-improvement | phase R0: the first improvement turn was accepted on 2026-10-18 from records written by hand (`docs/runs/2026-10-18-improvement-turn-1/`). Not built: the scorecard reader, promotion, the judge guard and validators for `self.improvement/1`, in that order |
| M3 to M5 | workflows as `workflow/1` files, a second harness and the confinement seam, a week of daily runs: not built |
| continuous runs | a run that plans its next stories and works a wave with parallel workers: designed, not built |

The design is `docs/design/takt.md`; its § 4 lists what each milestone has. Scenario runs and
their figures are in `docs/metrics/`.

## Use

```
takt run --repo <path> --intent "make the failing test pass" --iterations 1
takt run --repo <path> --hours 1
takt approve --repo <path> <case> repository.merge --by <who>
takt scenario run --repo <path> <id> --hours 1
takt protocols
```

The first form opens a case on `software.change/1`. The second resumes the open case of a story
being worked, or else takes the next ready story from the repository's planning store and opens
its case on `story.delivery/1`, which is embedded in the binary: tests, gate, pull request, CI and
merge as claims about the implementation revision, `accepted` once the pull request is merged. A
case opens only on a clean tree. `--protocol` takes a built-in id or a `protocol/1` file, read
from the directory you run `takt` in. Bounds: `--iterations`, `--hours`, `--tokens`. The tests are
`--test-cmd` (default `cargo test --quiet`); `.engineering/gates.yaml`, when the repository has
one, adds the gate. `takt run --help` lists every default and the exit codes:

| exit code | the run |
|---|---|
| 0 | completed, or nothing was ready |
| 1 | refused before any step; the reason is on stderr |
| 2 | the arguments do not parse |
| 3 | stopped on a budget, with nothing admissible, on a refused step, or with no progress |
| 4 | waits for an approval (`takt approve`); the next run resumes the case |
| 5 | stopped on an error once its case was open; the case stays open |
| 6 | waited on the forge (pending checks or a pull-request head behind the push) until the budget ended; the next run resumes the case |

`takt scenario run` replays a declared scenario: `.engineering/scenarios.conf` on `main` names
its story and seed commit. The run fetches the remote's `trial/*` branches, cuts the next free
`trial/<n>` from the seed, pushes it (it must not exist yet), and runs the story on it, so its pull
request targets `trial/<n>`. A trial branch is deleted afterwards with `b10x-gates bot
--repository <owner>/<repo> -- delete-branch trial/<n>`, which the bot route admits only while the
branch's tip is merged into the default branch.

## Build

```
cargo install --locked --path crates/takt-cli
takt --version
```

## Requirements

- a harness: Claude Code (`claude` on the `PATH`), or `--harness none` to run validators only
- for story runs, `aep` on the `PATH` and its planning store in the repository
  (`.engineering/project.yaml`) with ready stories
- for pull requests, an `origin` on github.com and `b10x-gates` and `gh` on the `PATH`; another
  host gets no forge, and `--forge none` runs without one; the forge's gates policy comes from
  `B10X_GATES_POLICY`

Built on [Canon](https://github.com/beyond10x/canon) and
[Engineering protocols](https://github.com/beyond10x/engineering-protocols). Licence: Apache-2.0.
