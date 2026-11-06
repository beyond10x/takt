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
on an engineering protocol (for a story, takt's own `story.delivery/3` unless you name another)
and lets a harness (Claude Code, Codex or b10x-harness) take one
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
the plan.

From 0.27.0 on, every release publishes a linux/amd64 container image,
`ghcr.io/beyond10x/takt:<version>`. A deployment adds only configuration: the repository mounted
at `/workspace`, the run's configuration at `/config/agent.yaml`, the endpoint file beside it and
the token variable the configuration names. What it holds and what a deployment supplies:
[Container image](https://beyond10x.github.io/takt/docs/reference/container-image).

## Status

0.34.0 is the latest release: `takt run --work <key>` names the work item a run delivers, and the run file's `work` list names the sources the next item comes from, in order: `aep`, the planning store, and `command`, a team's own command line printing `takt.work-item/1` documents; a case records the item's key and revision, and its commits carry `Refs: <key>`; and the run file's `deployment_paths` keep a deployment's own untracked files beside the tree out of every step change, take and commit. Since 0.33.0 the gate compares at the case's base what an edit on the case's branch could otherwise use to decide it, a file named glued to a short option or inside a shell's `-c` line, a Taskfile pulled in through `includes:`, a lint group allow added in source, a citation digest and the specification inputs. Since 0.32.0 `takt run` refuses before a case opens a `--test-cmd` holding shell quoting or naming a program the step could not start, and a `--gate` naming no approval-required action of the case's protocol; a resume whose case branch is gone names it and how to recreate it; and a failed git read leaves no git process. Since 0.31.0 a pull request open cut off by a crash is read back on every forge, a command forge without the new `find` command refuses a second open while one stays unanswered, no merge follows a read that finds the pull request merged or closed, and the step guard sees a program rewritten behind a link in a program directory. Since 0.30.1 trials started in one second in one checkout are numbered in the order they start, and the test suite keeps no relative `PATH` entry. Since 0.30.0 the run file's `harness` mapping and the flags `--effort`, `--max-budget-usd`, `--strict-version` and `--context` reach the session, checked by metaharness before any case opens. Since 0.29.0 `takt run --usd` stops a run on the cost its harness reports, counted in micro-dollars, and each pull-request open and merge records whether the forge applied it, refused it, was never asked or may have been, so a lost answer is read back and never sent twice. Since 0.28.0 the token rule of `takt promote` compares tokens per model request within 5 %, so an unchanged binary is not refused on drift, the wait on the forge for CI starts at 2 s and grows to 4.5 s, and a red revision records `gate.deferred` and runs no gate. Since 0.27.0 every release publishes the container image `ghcr.io/beyond10x/takt:<version>`. Since 0.26.0, `takt scorecard` refuses a `--repo` that is not an existing directory, exit 1, instead of printing an empty table or saving an empty scorecard. Since 0.25.0, on a Connectors host with standing grants, `forge: connectors` keys each open and merge, so a lost answer replays instead of writing twice. A merge answered `outcome_unknown` reads the pull request back before any write. A pull request behind its base is brought up to date by takt's own recorded merge commit, checked, then merged. `origin` may be a Connectors git relay on loopback. It runs on Loom 0.52.0, which starts a keyless `--harness harness` session on a loopback endpoint, with engineering-protocols 0.15.2. Since 0.24.0 takt carries no organisation's own delivery route: the `bot` value of `--forge` and the `gates` scan adapter are gone, and a forge that publishes needs `scan: none` in the run file. `forge: connectors` drives a GitHub pull request through a served Connectors instance at `forge_endpoint` with no credential; `takt scenario run` creates `trial/<n>` on `origin` with the operator's own git; and git, and a model endpoint with no credential, take plain `http://` on a loopback host. `takt report --cases` sums units accepted, tokens and human interventions per repository and UTC day, and a store story opens on takt's own `story.delivery/3` unless one is named. Since 0.23.0 a run that names no token budget stops at 3,000,000 tokens; a resumed run opens its pull request once; and under a delegation of `repository.merge` to `agent:takt-delivery` in the target's `.engineering/delegations.yaml` a run merges with no person's step. 0.22.1 moved `libz-sys` to 1.1.31 for RUSTSEC-2026-0418. Since 0.22.0 it stamps every evidence record to the microsecond, so a record ordered after another one in the same second no longer waits for the next second, and takt runs on loom 0.50.0 with Canon 0.8.0 and engineering-protocols 0.14.0. Since 0.21.0, under its `said:` line a harness step lists its tool calls, and a step that committed prints its commit's diff, in colour under `--color`. A documentation edit a test names counts as an attempt. Since 0.20.0 `--forge local` makes a bare repository on this machine the forge: a run opens a request there, runs the checks its run file names and, on a grant, merges into the target branch. Since 0.19.0 takt ships its own `story.delivery/3`, a failed gate or test run reaches the next prompt as what failed and where, and a run stops `RepeatedGateFailure` after three red gates in a row; since 0.18.0 a story case on `story.delivery/2` writes the `tests_change` and
`paths_changed` evidence its tests-first guards read. What exists and what is planned: [Status](https://beyond10x.github.io/takt/docs/status).
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
