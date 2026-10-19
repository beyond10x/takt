---
format: aep.planning-md/3
id: story:review-of-main-lows
kind: story
status: draft
title: Low findings from the review of main at 9cc158b
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
revision: 7
---
## Outcome

Each confirmed low finding below is fixed or recorded as accepted with a reason, one commit each, a test for each fix.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas). These held up under the fact-check at low severity (each needs the operator, an out-of-loop actor or a full disk):

- T8 `scorecard.rs:591-608`: `save` creates then writes; a failed write leaves a truncated file that blocks re-save.
- H3.5 `validator.rs:1213,1228`: checks read by pull request number, not sha; an outside push between the two calls records the new head's checks (the merge stays sha-pinned).
- F7.1 `forge.rs:1124-1146` (medium by the fact-check, here because story:ci-wait-watches-the-check rewrites the read): `ci_result pass` is written on the listed checks only; a required check not yet created is missing; the merge is still blocked by the forge.
- F7.8 `validator.rs:1043-1059`: the pull request POST is not idempotent; a create that exits non-zero leaves the number unrecorded and the next run gets 422.
- E8.5 `run.rs:4272-4276`: the model's summary goes into `git commit -m` verbatim; `\n\ntakt: take story:x` makes the take-commit count 0 (`validator.rs:981-992`) so no pull request opens; control characters reach the terminal.
- E8.8 `harness.rs:326-333`: the last-wins parse test has one action line.
- P1.1 `promotion.rs:832-876`: bubblewrap runs without `--new-session`; the confined build keeps the controlling terminal (TIOCSTI is off on this host, `legacy_tiocsti=0`).
- S9.4 `scripts/check-citations:78,157`: the external-citation pattern matches before the in-repository forms, so `takt 0.1.0, crates/missing.rs:9000` passes unresolved.
- S9.7 `scripts/check-citations:70`: a wrapped `# from: Makefile:3` is not flagged.
- X10.6 `protocols/self-improvement/1.yaml:196-222`: a candidate merged and then regressing has no outcome but `abandoned`.
- X10.8 `crates/takt-cli/src/main.rs:509-522`: an intent run defaults `affects_security_boundary=false`, which drops Canon's security review.
- C4.2 `case.rs:317-327`: `Authority::read` does not re-check approvers; a typed `agent:takt` still grants outside judge-surface merges after story:judge-guard D5.
- C4.3 `case.rs:772-778`: `prepare_grant` trusts the highest-numbered decision file.
- C4.7 `case.rs:1405-1410`: case open takes the parent id from the branch suffix without `check_id`.
- R5.2 `run.rs:2967-2971`: `close` drops `Ok(refusals)`; the stop says accepted while the store refused (the refusals are on record).
- R5.4 `run.rs:3549,3563,3591-3592`: errors after a validator's observation return without undo.
- R5.5 `run.rs:1363-1364`: any `cat-file -e` failure drops the repository's judge-surface list.
- R5.7 `case.rs:1529-1531`: no tree-level lock; two runs in one worktree check out over each other.
- GIT-1 `git.rs:150`: `rev-parse --abbrev-ref` says `heads/<b>` when a tag shares the name; a restore then makes `refs/heads/heads/<b>`.
- GIT-4 `git.rs:49-62`: `GIT_NAMESPACE`, `GIT_ATTR_SOURCE`, `GIT_SSH_COMMAND`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG` are not scrubbed.
- GIT-6 `git.rs:548`: 64 MiB per file with no total cap in `Content::of`.
- P1.5 extra crash windows (`promotion.rs:644/645`, `:1636/1637`): go to story:promotion-records-its-toolchain.

Covered elsewhere: F7.5/E8.6 (suffix credential list) by story:environment-allowlist; GIT-8's `refs/replace` route by story:replace-refs-do-not-steer-takts-reads; N2 (a failed step's tokens) by story:failed-step-tokens-are-recorded; X10.1, X10.2, X10.4, X10.7 by story:judge-guard and story:self-improvement-protocol.

## Scope

- the files named above (cited by the fact-check)

## Acceptance

Each item above has a commit with a test that fails before it, or a line in this story's body saying why it is accepted; `task check` passes.

## From the review of run-report at 95aa293 (2026-10-19)

- `crates/takt/src/report.rs:583-605@95aa293`: a `|` in a case id or run name adds a markdown table column; failing input `"case":"case|x"`.
- `crates/takt/src/trace.rs` (`escape`): only the current `$HOME` becomes `~`; a record made under another home keeps its home path (`HOME=/nonexistent takt report` printed 3 lines with a home path).
- `crates/takt/src/trace.rs` (`escape`): Cf format characters such as U+202E are not escaped (inferred from `char::is_control`, not run).

## From the review of scan-before-push at bc74652 (2026-10-19, granted)

- warning `crates/takt/src/forge.rs:1020@bc74652`: when the remote's base has moved past `refs/remotes/origin/<base>`, every push is refused, the merge push included, though the dry-run fetch has already downloaded the remote base's commit. Decision: exclude what is reachable from the fetched remote sha instead of refusing.
- note `forge.rs:1422-1444`: a byte-order mark is looked for at byte 0 only; a mid-file or odd-offset UTF-16 literal passes (same reader as the documented no-mark limit).
- note `forge.rs:973-987`: a force-pushed remote base fails closed with gates' generic refusal, not the two shas.

## From the review of baseline-for-a-pre-revision-binary at a27d02b (2026-10-19, granted)

- note `crates/takt-cli/src/main.rs:122@a27d02b`: `takt promote --help`'s exit-1 text omits the stated-record digest check and the refusal of runs not stated by the operator.
- note `crates/takt/src/promotion.rs:1380`: a malformed `binary.txt` is refused as "no binary.txt beside its record"; the reason in `card.unstated` is unused.
- note `crates/takt/src/scorecard.rs:575`: an unstated case is in both `no_record` and `rows` of a saved scorecard.
- note `crates/takt/src/scorecard.rs:135`: a run's agent can plant a well-formed `binary.txt` under `.engineering/cases/` (`run.rs:357`).

## From the adversarial review of main at bd20f7b (2026-10-19, fact-checked)

Confirmed, low (review-result:main-ae680d5-adversarial):
- A7.4 `forge.rs:1131`: a `skipping` check counts as pass; only skipped checks give `ci_result pass` (reach needs the repository's own workflow to skip).
- A1.2 `trace.rs:1254-1259` with `main.rs:379`: `report --trace` prints a header's `format` raw in its error.
- A1.4, A1.5 `trace.rs:1127`, `:1235`: `--steps` follows links; `--steps` and `--trace` are read whole with no size or regular-file check.
- A1.7 `report.rs:235`: takt time clamps to 0 instead of unknown when measured exceeds wall.
- A1.8 `trace.rs:911-928`: `merge_steps` is O(calls x lines).
- A8.1 `run.rs:4286-4298`: a NUL or a 128 KiB summary makes the commit spawn fail and the run end on Err with no refusal.
- A8.2 `trace.rs:785`: a refused `repository.merge` is labelled "approval-only call".
- A8.3 `run.rs:4258`, `:4266`: a newline in a changed path forges a line on `out`.
- A9.3 `scripts/check-citations:69-71`: `_read from:`, `xread from:` and homoglyphs are neither flagged nor counted.
- A9.7 `scripts/check-citations:171`, `:329-333`: a repository symlink is followed outside and its lines printed.
- A10.1 `validator.rs:1517`, `:1446`: a PR merged outside takt is recorded merged accurately, but the approval, test_result and move commits stay on the case branch.
- A10.4 `run.rs:3084`: the move to implemented is tried after a refused test_result write.
- A10.5 `validator.rs:378-392`: the test_result record names no test command.
- A10.6 `case.rs:1733-1743`: `has_evidence` ignores the result, so a flaky fail is never re-run at that revision.
- A4.1 `case.rs:1102-1114`: `approvals_only` does not require each grant's approval record to reach the base (operator-only).
- A4.2 `frontier.rs:394-405`: approve commits the record before writing the grant; a failed write needs a new approval (fails closed).
- A6.3 `promotion.rs:531-552`: promotion's `git()` bypasses `git::command`'s environment scrub (operator environment only).
- A3.7 `scorecard.rs:481`: `no_record` is never read by promotion; a trial without a record drops out of n silently.
- A2.4 `promotion.rs:832-846`: bubblewrap passes inherited file descriptors into the confined build (operator-opened fds only).

## From the release night of 0.1.0 (2026-10-19): grant reviews, fact-checked lows

- `crates/takt-cli/build.rs:244` (follow-ups c1 review, W1): `--ignore-submodules=all` reads a submodule at another commit as clean; `=dirty` also keeps its filter from running and still reports it; no test distinguishes them.
- `ess/domains/takt.yaml:467` (follow-ups c1, N1): the `-dirty` text omits the submodule exclusion `build.rs:213-214` documents.
- `crates/takt/src/git.rs:1511` (follow-ups c1, N2): a conflict at a non-UTF-8 path now errors in `change.read`; the ESS states that limit only for `tests.run` and `ci.read`.
- `crates/takt/src/run.rs:4628-4632` (wave-12 close review, W1): no test pins that a step discarded at the run's end carries its call report; the `Late` double in `hours_bounds_the_run_end.rs:308` has no `last_call`.
- `ess/domains/takt.yaml:1414` (wave-12 close, N3): `BenchTaskId` cites a draft story's title line, weaker than the old citation; the claim still holds through `benchmark-integration.md:37`.
- `scripts/check-finding:438-444` (E1 review, #7): a command that exits 0 but prints the defect's output reads `not_reproduced`.
- `crates/takt/src/promotion.rs:670-679` (E2 c3 review, W1): a v1 baseline with a real held-out failure can never be judged against again (re-score refused, v1 not compared, `--baseline-only` refused); no real store is in that state.
- `crates/takt/tests/properties.rs:253` (propfuzz review, N2): no property for an ended unreadable line that no `skipped` line names.
- `crates/takt/tests/properties.rs:396` (propfuzz, N3): median-within-bounds cannot catch an unsorted median; ignoring the margin is killed only by the scorecard tests.
- `fuzz/Cargo.toml:17-18` (propfuzz, N6): `fuzz/` is its own workspace and `task check` never builds it, so an API change breaks the targets unseen.
- `.github/workflows/check.yml:89-90` (CI fix review): the cache fill `aep plan artifact validate` also gates store validity on every pull request and the release gate.
- `crates/takt/src/validator.rs:296-330` (colour fix review): `summarise_tests` would hide failed test names behind escape codes if a test command passes `-- --color always`; the counts still parse.
- `.github/workflows/check.yml:99` (CI fix 2 review): the bwrap smoke test covers less than takt's sandbox (`--unshare-pid`, `--proc`, `--dev`, `--tmpfs`); it fails first at loopback, which is what it guards today.
- `docs/site/publish-protocols.sh` (site review): still pushes `gh-pages`, while the site now deploys from `main`.

## Later lows (2026-10-19, wave 13 and 14 reviews)

- `crates/takt/src/loop_scorecard.rs:34` (loop-scorecard review at 17431f3, info): `FIGURES_CODE` names only `loop_scorecard.rs`; a candidate changing a figure's input computed in `events.rs`, `case.rs` or `git::parse_commit` changes the loop's figures without marking any definition changed. Fixed when a change to the token reading in `events.rs` marks the token-based definitions changed.
- `crates/takt/src/events.rs:335` (torn-character review at 090c241): unreadable lines in a run that ends in an unended last line are skipped together, so a bad middle line can drop unnamed (`A\n\xc3\n{"x` reads as `[A]`); only the last of a run of bad lines must be named. The repair rule is unchanged since 27417e1.
- `crates/takt/tests/run_record_torn.rs:216` (torn-character review): the acceptance's `takt scorecard` exit is checked through the library, not the CLI; the reviewer ran the CLI by hand (exit 0).
- `fuzz/fuzz_targets/scorecard.rs:95-109` (wave-13 close review): the short-side branch does not check which side was short.
- `crates/takt/src/promotion.rs:78` `MAINLINE = "main"`, `crates/takt-cli/src/main.rs:268` (`--declared-on` defaults to `main`), `scripts/check-finding:109` (`declared_on=main`): a repository whose mainline is not `main` is unsupported (CI fix 4 review).
