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
> compared. One improvement turn has been accepted, and its records were written by hand. Every
> figure, with its source: [`docs/metrics/`](docs/metrics/README.md).

## Status

0.1.0 is the first release. It holds `takt run`, `approve`, `protocols`, `scenario run`,
`scorecard`, `trace`, `report` and `promote`, with `story.delivery/1` embedded, and prebuilt
binaries for Linux (x86_64, aarch64) and macOS (aarch64). Open: no recorded run has yet taken a story from ready to merged
with the grant as the only manual step; no operating-system sandbox holds the harness or the test
command (the confinement seam is not built); and improvement turns are still recorded by hand.

| milestone | state |
|---|---|
| M1 one step validated | done 2026-10-18 (`docs/runs/2026-10-18-m1-scratch.md`) |
| M2 a story end to end | built: the planning-store frontier, the story protocol, the gate validator, the pull request and CI through the bot route, the merge under a recorded grant, waits on pending checks (wave 08), the run recorded on the story (wave 09), those records reaching the base with the merge (wave 11). Not yet proven: no recorded run has taken a story from ready to merged with the grant as the only manual step |
| unattended scenario 1 | next: accepted with one invocation plus one grant, daily for a week. Of 27 runs at 0a776d2 and later (T-1..9, T-11..28; not counted: T-fetchfail-1..3, refused at the trial fetch, and T-10, a runner race), 23 reached the merge approval in one invocation, 14 of the 15 after round 3 (`docs/metrics/`). Wave 12 merged the fixes for the three wastes those runs show (a second CI, a model step only for the approval, an inspect step before `NoProgress`); no scenario run has measured them yet |
| self-improvement | phase R0: the first improvement turn was accepted on 2026-10-18 from records written by hand (`docs/runs/2026-10-18-improvement-turn-1/`). Built: the scorecard reader (`takt scorecard`, wave 10), the binary naming its revision (wave 11), promotion (`takt promote`), the run trace and report (`takt trace`, `takt report`). Wave 12 merged the fixes of improvement turns 2 and 3; `improvement-turns.jsonl` records neither turn yet. Not built: the judge guard and validators for `self.improvement/1`, in that order |
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
takt scorecard --repo <path> [--against <scorecard.json>]
takt trace --case <case dir> [--out <file>]
takt report --case <case dir> | --trace <file>
takt promote --repo <takt checkout> --scorecard <file> [--dry-run]
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

## Install

From a release, with no Rust toolchain: each version tag publishes `takt-<version>-<target>.tar.gz`
for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`, with a
`SHA256SUMS` file, at <https://github.com/beyond10x/takt/releases>.

```
version=0.1.0 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
"takt-$version-$target/takt" --version
```

On macOS, where `sha256sum` may be missing, check the archive with
`grep " takt-$version-$target.tar.gz\$" SHA256SUMS | shasum -a 256 --check`.
`takt --version` prints the version and the commit the binary was built from.

From source:

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
- for `takt promote`, bubblewrap (`bwrap`) on the `PATH` and rustup's default toolchain: the
  candidate is built and run inside bubblewrap. bubblewrap is Linux only, so on macOS a promotion
  refuses with exit 1 before it judges; `--dry-run` and `--baseline-only`, which build nothing,
  still run

On macOS a run kills what a child process started in the child's process group only; on Linux takt
also adopts and kills what the child orphaned outside it. No command sandboxes the harness or the
test command on either system yet (`story:confinement-seam`).

Built on [Canon](https://github.com/beyond10x/canon) and
[Engineering protocols](https://github.com/beyond10x/engineering-protocols). Licence: Apache-2.0.
