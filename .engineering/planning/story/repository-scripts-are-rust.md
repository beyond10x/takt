---
format: aep.planning-md/3
id: story:repository-scripts-are-rust
kind: story
status: draft
title: The repository's Python programs are Rust commands with clap derive
summary: check-citations, release-notes, gate-exec and the docs/metrics programs ported; no Python under scripts/ or docs/metrics/
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:run-swimlane
- informed_by: story:scenario-timelines-and-scorecard-page
- informed_by: story:bench-runs-in-journey
scope:
- confidence: cited
  path: .agents/developer-agent.md
- confidence: cited
  path: .agents/dispatcher-agent.md
- confidence: cited
  path: .engineering/assertions-project.yaml
- confidence: cited
  path: .github/workflows/release.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/takt-cli/tests/citation_check.rs
- confidence: cited
  path: crates/takt-cli/tests/citation_check_adv1.rs
- confidence: cited
  path: docs/metrics/badges.py
- confidence: cited
  path: docs/metrics/journey.py
- confidence: cited
  path: docs/metrics/timeline.py
- confidence: cited
  path: scripts/check-citations
- confidence: cited
  path: scripts/gate-exec
- confidence: cited
  path: scripts/release-notes
revision: 2
---
# Story: repository-scripts-are-rust

## Outcome

Every program this repository commits and runs is Rust, with a clap derive command line. The
Python programs below are replaced by Rust commands with the same arguments, outputs and exit
codes, and their callers (`Taskfile.yml`, `.github/workflows/release.yml`,
`.engineering/assertions-project.yaml`, the tests, `AGENTS.md` § Commands and the agent profiles)
call the Rust commands. No file under `scripts/` or `docs/metrics/` is Python afterwards.

| today | what it does | callers |
|---|---|---|
| `scripts/check-citations` (`python3 -I`, 578 lines) | each `ess/` `read from:` against its needles and digest; `dump`, `fix`, `accept` | `Taskfile.yml:12,22`, `assertions-project.yaml:17`, `crates/takt-cli/tests/citation_check*.rs`, `AGENTS.md`, `.agents/dispatcher-agent.md`, `.agents/developer-agent.md` |
| `scripts/release-notes` (`python3 -I`, 358 lines) | a release's notes from `CHANGELOG.md`, or `--check` of the whole file | `Taskfile.yml:13,23`, `assertions-project.yaml:24`, `.github/workflows/release.yml:166`, `AGENTS.md` |
| `scripts/gate-exec` (`/usr/bin/python3 -I`, 64 lines) | runs one command of `.engineering/gates.yaml` on a fixed system path | `assertions-project.yaml:5,13,20,27,34` |
| `docs/metrics/badges.py` (115 lines) | rewrites the README badge block from the records | by hand |
| `docs/metrics/journey.py` (899 lines) | overlays recorded scenario runs on one timeline | by hand; retired by story:bench-runs-in-journey or story:run-swimlane |
| `docs/metrics/timeline.py` (548 lines) | draws `docs/metrics/timeline.svg` | by hand; retired by story:scenario-timelines-and-scorecard-page |

## Context

- The rule: anything committed to a beyond10x repository that runs is Rust, with clap derive;
  no Python. The exceptions are a Docusaurus site build and browser code in Vue 3 with
  TypeScript; neither covers these files.
- `journey.py` and `timeline.py` already have retirement stories: `journey.py` by
  story:bench-runs-in-journey (`takt-bench journey`) or story:run-swimlane (`takt-docs`),
  `timeline.py` by story:scenario-timelines-and-scorecard-page (`takt-docs`), all draft.
  Whichever lands first does the port: this story ports, as they are, the programs still Python
  when it starts, and a later retirement story extends the Rust code it finds.
- `gate-exec` is Python, not shell, so the command reader the step guard and the gate share
  takes it whole, compared at the base (its own docstring). A Rust `gate-exec` built from the tree
  under test would run code the step could have changed. Open question for scoping, not decided
  here: build it from the base revision, ship it in the `takt` binary as a subcommand the gate
  calls, or keep a shell-free launcher the guard can read. Whatever is chosen, the step guard
  must still refuse a change to the launcher.
- Not in this story: the test fixtures `crates/takt/tests/fixtures/{forge_commit_graph,plant_commit_bomb,plant_loose_object}.py`,
  which `loop_case_cli.rs` and `loop_scorecard.rs` run through `python3`, and the scenario-1
  fixture, which is the scenario's own subject code. The fixtures fall under the same rule and
  are a follow-up.
- Changing `.agents/**` widens authority (`.agents/planner-agent.md` § Never); the grant names
  it.

```expected_before
host: any checkout of this repository
command: git ls-files scripts docs/metrics | while read -r f; do case "$f" in *.py) echo "$f";; *) head -n1 "$f" | grep -q python3 && echo "$f";; esac; done | grep -q . && echo python-runs-here
exit: 0
healthy_exit: 1
output: python-runs-here
source: .engineering/planning/approval-record/store-replan-2026-10-28.md
```

## Acceptance

1. The predicate above exits 1 and prints nothing at the story's merge.
2. `crates/takt-cli/tests/citation_check.rs` and `citation_check_adv1.rs` pass against the Rust
   `check-citations` with their cases unchanged, so it reports the same citations as the Python
   one did on the same trees.
3. The Rust `release-notes <version>` prints, for every version in `CHANGELOG.md` at the merge,
   the same bytes the Python script printed at the base; `release-notes --check` passes on the
   file and fails, naming the line, on a fixture that breaks Keep a Changelog 1.1.0.
4. Every gate in `.engineering/gates.yaml` gives the same result through the Rust `gate-exec`
   as through the Python one on the merge's tree, and a tool that resolves inside the repository
   is still refused, exit 126.
5. `task check` runs the Rust commands and no `python3`; `grep -rn python3 Taskfile.yml
   .github/workflows .engineering/assertions-project.yaml` prints nothing.

## Scope

- `crates/` (a new crate, or `takt-cli` subcommands, for `check-citations`, `release-notes` and
  `gate-exec`; the `takt-docs` crate for the metrics programs) (inferred)
- `scripts/check-citations`, `scripts/release-notes`, `scripts/gate-exec`: removed (cited)
- `docs/metrics/badges.py`, `docs/metrics/journey.py`, `docs/metrics/timeline.py`: removed (cited)
- `Taskfile.yml`, `.github/workflows/release.yml`, `.engineering/assertions-project.yaml` (cited)
- `crates/takt-cli/tests/citation_check.rs`, `crates/takt-cli/tests/citation_check_adv1.rs` (cited)
- `AGENTS.md`, `.agents/dispatcher-agent.md`, `.agents/developer-agent.md`: the command lines (cited)
