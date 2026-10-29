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

Point `takt` at a repository and say "work on this". It takes an intent or a story, opens a case
on an engineering protocol and lets a harness (Claude Code, Codex or b10x-harness) take one
admissible step at a time. A program checks each step's result, not the model that made it.
[Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)) decides
what is permitted, what is owed and whether completion was earned.

> Agents choose how to do the work. The protocol defines what must be true before the work counts.

takt is one Rust binary, Apache-2.0, an application on the SDK of
[Loom](https://beyond10x.github.io/loom/) ([GitHub](https://github.com/beyond10x/loom)): Loom's
runtime drives the case, Loom's governor decides, the harness proposes. It gives you:

- evidence bound to a revision, and a merge only under a grant recorded for that revision;
- a case that is files under `.takt/cases` in your repository, ignored by git unless you commit its
  `.gitignore`, which a later run resumes and a reviewer replays;
- a named stop and its own exit code for every end of a run, so a script can drive it;
- a measured self-improvement loop: a fix counts when a check failed before it and passes after.

takt is not a model, a planner, a CI service or a sandbox for the harness. The harness does the work,
[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep)) keeps
the plan, and no container image is published: you build one from `Dockerfile`.

## Status

0.22.1 is the latest release. It moves `libz-sys` to 1.1.31 in the lock, for RUSTSEC-2026-0418. Since 0.22.0 it stamps every evidence record to the microsecond, so a record ordered after another one in the same second no longer waits for the next second, and takt runs on loom 0.50.0 with Canon 0.8.0 and engineering-protocols 0.14.0. Since 0.21.0, under its `said:` line a harness step lists its tool calls, and a step that committed prints its commit's diff, in colour under `--color`. A documentation edit a test names counts as an attempt. Since 0.20.0 `--forge local` makes a bare repository on this machine the forge: a run opens a request there, runs the checks its run file names and, on a grant, merges into the target branch. Since 0.19.0 takt ships its own `story.delivery/3`, a failed gate or test run reaches the next prompt as what failed and where, and a run stops `RepeatedGateFailure` after three red gates in a row; since 0.18.0 a story case on `story.delivery/2` writes the `tests_change` and
`paths_changed` evidence its tests-first guards read, and a bot merge commit names the story and its evidence. What exists and what is planned: [Status](https://beyond10x.github.io/takt/docs/status).
Prebuilt binaries for Linux (x86_64, aarch64) and macOS (aarch64) are on the
[releases](https://github.com/beyond10x/takt/releases) page. Changes: [`CHANGELOG.md`](CHANGELOG.md).

## Documentation

The site is <https://beyond10x.github.io/takt/>.

| to | read |
|---|---|
| run takt for the first time | [Getting started](https://beyond10x.github.io/takt/docs/getting-started) |
| install a release, a build or the image | [Install](https://beyond10x.github.io/takt/docs/guides/install) |
| set a run's budget, harness, endpoint and forge | [Configuration reference](https://beyond10x.github.io/takt/docs/reference/configuration) |
| run it as a Loom agent on Kubernetes | [Deploy on Kubernetes](https://beyond10x.github.io/takt/docs/guides/deploy-on-kubernetes) |
| look up a command or an exit code | [CLI reference](https://beyond10x.github.io/takt/docs/reference/cli) |
| see what a case writes | [Case files](https://beyond10x.github.io/takt/docs/reference/case-files) |
| see what exists today | [Status](https://beyond10x.github.io/takt/docs/status) |

## Build from source

```console
cargo build --locked -p b10x-takt-cli
cargo install --locked --path crates/takt-cli
task check
```

`task check` runs the specification, citations, format, lint and tests. `AGENTS.md` is for agents
working on this repository and carries its commands, layout and rules.

## Licence

Apache-2.0, in [`LICENSE`](LICENSE).
