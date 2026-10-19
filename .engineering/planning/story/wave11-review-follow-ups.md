---
format: aep.planning-md/3
id: story:wave11-review-follow-ups
kind: story
status: implemented
title: Close the low findings from the wave 11 grant reviews
relations:
- depends_on: story:records-reach-base
- depends_on: story:binary-embeds-revision
- serves: vision:safe-agentic-coding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T20:13:55Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T20:13:56Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T04:04:08Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

The four low findings from the wave 11 grant reviews are closed: the fail-closed stop of story:records-reach-base has a test that fails for no other reason, its path read uses the path-safe git call, and the revision build script of story:binary-embeds-revision runs no repository filter and counts a root `.cargo/` as compiled in.

## Context

From the reviewer's verdicts on 2026-10-18 (both approve, both granted). They are kept in a story of their own because wave 11 moves the two stories they came from, and an edit to those files on main would conflict with the wave's moves.

From story:records-reach-base at e46d9d0:

- Low: `crates/takt/tests/records_reach_base_adv2.rs:462` passes when nothing merges for any reason; it should assert `Stop::StepRefused` containing "not both written".
- Nit: `changes()` reads paths through `git::run`, whose doc says it is never for paths (`crates/takt/src/case.rs:1242` against `crates/takt/src/git.rs:129-131`). No bypass was found.

From story:binary-embeds-revision at 85c98bf:

- Low: a filter named in `.git/info/attributes` still runs its clean command during `cargo build` (`crates/takt-cli/build.rs:35-37, 184-188`; observed with git 2.55). The reviewer's tested fix: for each driver from `git config --name-only --get-regexp '^filter\.'`, pass `-c filter.<x>.clean= -c filter.<x>.process=`.
- Low: only `crates/` and `protocols/` count as compiled in (`build.rs:18`), so an untracked root `.cargo/config.toml` leaves the revision clean (observed). That it can change the binary through rustflags or `[patch]` is inferred, not tested.

## Scope

- `crates/takt/tests/records_reach_base_adv2.rs`
- `crates/takt/src/case.rs`: `changes`
- `crates/takt-cli/build.rs`

## Acceptance

The adv2 test fails when the merge is refused for a reason other than the missing records; `changes()` reads paths with the path-safe git call; a build with a filter driver configured in `.git/info/attributes` runs none of its commands; an untracked root `.cargo/config.toml` makes `takt --version` print `-dirty`.

## Landed (wave 12)

Read from `git diff --name-only 7297af6^1 7297af6` (the unit's merge into the wave branch) at the close of wave 12 (wave branch head 63a9d02). The lows of the wave 11 and wave 12 grant reviews: build.rs runs no filter and counts a root .cargo, paths read byte for byte, pr.open counts from refs/heads/<base>, conflicts outside .engineering read ahead, long minted ids skipped, check_id before a branch name becomes a path, version_cli against the embedded revision.

- `crates/takt-cli/build.rs`
- `crates/takt-cli/tests/case_id_branch_name_cli.rs`
- `crates/takt-cli/tests/case_id_branch_name_cli_adv2.rs`
- `crates/takt-cli/tests/revision_build.rs`
- `crates/takt-cli/tests/revision_build_adv1.rs`
- `crates/takt-cli/tests/version_cli.rs`
- `crates/takt/src/case.rs`
- `crates/takt/src/git.rs`
- `crates/takt/src/run.rs`
- `crates/takt/src/validator.rs`
- `crates/takt/tests/case_id_branch_name.rs`
- `crates/takt/tests/case_id_branch_name_adv1.rs`
- `crates/takt/tests/case_id_branch_name_adv2.rs`
- `crates/takt/tests/pr_opens_when_done.rs`
- `crates/takt/tests/pr_opens_when_done_adv1.rs`
- `crates/takt/tests/records_reach_base_adv2.rs`
- `crates/takt/tests/run_record.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
