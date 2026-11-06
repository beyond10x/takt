# Wave 56: work sources, the aep and command sources

Stage: **proposed**. `wave/56` branches from fedfbc28c, the release of 0.33.0, and merges `store/close-wave-55` (the
close-out of wave 55) first, as 407ff3e56. Skill version 0.26.0 (aep:implementing, wave mode). Approved: pending.
Release: 0.34.0.

## Selection

epic:work-sources holds the shape the owner approved (approval-record:work-sources-shape): a run's work comes from the
tracker a team plans in, `--work <key>` naming one item and the run file's `work` list the sources of the next ready
one. Wave 56 is its first half, with story:git-excluded-files-are-not-step-changes, accepted for this wave by
approval-record:wave-56-excluded-files. Wave 57 is story:work-from-jira-github-and-gitlab and
story:claim-and-report-on-a-tracker, after an issue-tracker profile and the Jira mapping land in connectors-definitions.

The three work-source stories are drafts; the go accepts them.

What the design draft took as new is in the code already, and stays as it is: the case keeps the item's text as
`intent.md` with its sha256 as the revision and reads no store on resume (`crates/takt/src/case.rs`, `run.rs`), the
`Refs:` trailer (`crates/takt/src/commit_style.rs`), and children started without credentials (`frontier.rs`,
`forge.rs`).

## Units

| unit | story | objective | scope | depends on |
|---|---|---|---|---|
| a | story:work-items-in-the-ess-model | `takt.work.Source`, `Key`, `Item` and the widened `Unit` declared and validated | `ess/` only | |
| b | story:work-flag-and-the-aep-source | `--work <key>`, the `work` list, `aep` as today, case ids for tracker keys, the key in the case and the trailer | `frontier.rs`, `run_config.rs`, `case.rs`, `commit_style.rs`, `main.rs` | a |
| c | story:the-command-work-source | `work_commands` read and select, `takt.work-item/1` JSON | `run_config.rs`, `main.rs`, new `work/command.rs` | b |
| d | story:git-excluded-files-are-not-step-changes | paths the run file names as the deployment's are not step changes | `run_config.rs`, `git.rs`, `run.rs` | |

Order a, b, c, d in one tree: b and c collide on `run_config.rs` and `main.rs` (`aep plan artifact waves`), d on
`run_config.rs`.

## Case ids for tracker keys

`aep` keeps `story-<name>-<instant>`. Another source mints `<source>-<slug>-<instant>`: the key's id lowercased, each run
of characters outside `[a-z0-9._]` one `-`, at most 48 characters, then `-` and 8 hex digits of the sha256 of the full
key whenever the slug lost anything. The id passes `check_id` (no `/`, no `..`, a valid branch name under `takt/`), is
the same on a case-insensitive file system, and the full key is recorded in the case beside it.

## Gate plan

As wave 55: `task check-static`, `cargo test --locked -p b10x-takt-cli --no-fail-fast`, then the library's targeted
test targets, the build cache discarded between the CLI and the library stage and after the last; the whole library
suite on the pull request's CI.
