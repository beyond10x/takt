---
format: aep.planning-md/3
id: review-result:main-fe93286-areas
kind: review-result
status: active
title: 'Review of main at fe93286: ten area reviewers, fact-checked'
relations:
- reviews: story:cases-root-link-is-undone
- reviews: story:check-finding-builds-confined
- reviews: story:check-finding-positive-control
- reviews: story:git-filter-drivers-do-not-run
- reviews: story:merge-checks-the-pull-request-base
- reviews: story:promote-binds-scorecard-and-merge
- reviews: story:review-of-main-lows
- reviews: story:run-record-survives-a-torn-character
- reviews: story:scorecard-against-needs-the-trials
- reviews: story:test-command-reaches-no-forge-login
- reviews: story:failed-step-tokens-are-recorded
revision: 1
---
needs-revision

Review of takt main at fe93286, 2026-10-19: ten independent reviewers (one per area, a smaller model), each finding then fact-checked against the code by a second reviewer. Verdicts: CONFIRMED, PARTLY, CONFIRMED-UNREACHABLE (the code does it, no reachable trigger), COVERED (an existing story plans it), WRONG.

## Fact-checks

Fact-check of area 10 (protocols and CLI), 8 findings. Copies: (the review scratch directory)

| id | verdict | severity | covered | evidence |
|---|---|---|---|---|
| X10.1 | PARTLY | low (high once judge-guard is the only guard) | judge-guard covers format, owner, delegation; nothing covers who types --by | case.rs:694-696 intent case takes any approver; :460-466 any human:/agent:; takt merges itself via bot route, not takt's grant file (developer-agent.md:42-46) |
| X10.2 | COVERED | low | judge-guard, SIP D6 | no validator owns candidate.merge |
| X10.3 | CONFIRMED | medium | partly (SIP D3 inside a case, confinement-seam M19); bare CLI none | promotion.rs:1016-1096 no is-ancestor/merge-base/grant check |
| X10.4 | COVERED | medium | judge-guard | example paths wrong: run.rs:357 refuses build.rs and .engineering/**; crates/** gets through |
| X10.5 | CONFIRMED-UNREACHABLE | low | loop-scorecard (definitions only); three-turn rule and corpus check in no story | no loop.* validator at fe93286 |
| X10.6 | PARTLY | low | none | merged-then-regressing case has only `abandoned` |
| X10.7 | PARTLY | low | judge-guard | --protocol takes any path; gates claim wrong (validator.rs:565-591 reads at base_commit) |
| X10.8 | CONFIRMED | low | none | intent default affects_security_boundary=false drops Canon's security review (software-change/1.yaml:30-34,224-229@53a221b) |

Fact-check of areas 1 (promotion) and 9 (scripts), 13 findings. Probes: (the review scratch directory)

| id | verdict | severity | covered | evidence |
|---|---|---|---|---|
| P1.1 | PARTLY | low (was high) | no | no --new-session (promotion.rs:832-876); TIOCSTI EIO on this host (legacy_tiocsti=0); /dev/tty readable/writable by confined build under an interactive terminal |
| P1.2 | CONFIRMED | medium | save side only (scorecard-save-checks-binary) | promote judges rows of any scorecard; rows never carry the opened event's binary (events.rs:76, scorecard.rs:337-378) |
| P1.3 | CONFIRMED-UNREACHABLE | low | no | race adds nothing over P1.2 |
| P1.4 | CONFIRMED-UNREACHABLE | low | no | no filter.* in global/system config; checkout already runs filters anyway |
| P1.5 | CONFIRMED | low | partly (promotion-records-its-toolchain C4) | two more crash windows: :644/:645 orphan copy refused at :376-379; :1636/:1637 record without installed binary refused at :1188-1195 |
| P1.6 | CONFIRMED-UNREACHABLE | low | no | paths in ~/.local/state only |
| S9.1 | CONFIRMED | medium (was high) | no | check-finding:293-299: status=101 → not_reproduced (probe) |
| S9.2 | CONFIRMED | high | validator path only (D8); manual runs not | cargo build with full env and real HOME (check-finding:233-234) |
| S9.3 | PARTLY | medium | validator path only (D5) | finding and file-finding read from the tree run in |
| S9.4 | PARTLY | low | no | EXTERNAL pattern (:78) matched before in-repo forms; `takt 0.1.0, crates/missing.rs:9000` passes |
| S9.5 | CONFIRMED-UNREACHABLE | low | no | ess/ holds only .yaml |
| S9.6 | CONFIRMED-UNREACHABLE | low | no | by design; ess/ is a judge surface |
| S9.7 | CONFIRMED | low | no | `# from: Makefile:3` not flagged |

Fact-check of areas 2 (scorecard) and 3 (validator), 15 findings. Experiments: (the review scratch directory)

Key reason most fall: run.rs snapshots and restores .engineering/cases around every harness step (3782-3783, 3899-3925) and validator step (3508-3509, 3549, 3597); merge validator runs only when Canon admits repository.merge with a grant at that revision (case.rs:353-357).

| id | verdict | severity | evidence |
|---|---|---|---|
| T1 | CONFIRMED-UNREACHABLE | low (was high) | 12 real records: every tokenless step is a `none` harness step |
| T2 | CONFIRMED-UNREACHABLE | low | stop lines always carry `stop` (events.rs:147-151) |
| T3 | PARTLY | medium | `takt scorecard --against` gives verdict on fewer cases than trials; promotion refuses (promotion.rs:1656-1690) |
| T4 | CONFIRMED-UNREACHABLE | low | padding needs 0-token accepted cases |
| T5 | CONFIRMED-UNREACHABLE | low | snapshot catches links in-run (run.rs:217-221, 306-338) |
| T6 | WRONG | — | documented rule (scorecard.rs:9-11, 88-91) |
| T7 | CONFIRMED-UNREACHABLE | low | every line written with `at` |
| T8 | CONFIRMED | low | create_new then write_all; truncated file blocks re-save |
| H3.1 | WRONG | — | merge validator only when admissible; grant at revision required |
| H3.2 | CONFIRMED-UNREACHABLE | low | project file read at base; none exists |
| H3.3 | CONFIRMED-UNREACHABLE | low | needs object-store corruption |
| H3.4 | CONFIRMED-UNREACHABLE | low | branch.yaml guarded in-run |
| H3.5 | CONFIRMED | low | checks read by PR number; out-of-loop push between calls; wrong record, not wrong merge |
| H3.6 | WRONG | — | deliberate; next run pushes again |
| H3.7 | WRONG | — | ci.green precondition + sha-pinned merge |

Fact-checker's extra (inferred, untested): the candidate takt binary writes its own trial records and scorecard/promotion trust them as written.

Fact-check of areas 7 (forge) and 8 (harness, events), 16 findings + 2 new. Probes: (the review scratch directory)

| id | verdict | severity | covered | evidence |
|---|---|---|---|---|
| F7.1 | PARTLY | medium (was high) | no | ci_result pass written on listed checks only; merge still blocked by GitHub's required-check refusal (validator.rs:1494-1506) |
| F7.2 | dup of H3.5 | low | — | — |
| F7.3 | CONFIRMED-UNREACHABLE | low | no | only repo-write person can change base |
| F7.4 | WRONG | — | — | bot lines ignore GH_TOKEN (gates src/delivery.rs:189-198 @935e973) |
| F7.5 | PARTLY | low | story:environment-allowlist (draft) | GITHUB_PAT, GH_PAT, PGPASSWORD, MYSQL_PWD, DATABASE_URL pass (is_credential.py) |
| F7.6 | CONFIRMED-UNREACHABLE | low | no | scp form with userinfo is not a working remote |
| F7.7 | CONFIRMED-UNREACHABLE | low | no | nothing runs between rev-parse and push; head re-checked |
| F7.8 | CONFIRMED | low | no | PR POST non-idempotent; number never recorded; next run 422, stalls |
| E8.1 | CONFIRMED | medium | no | read_to_string fails the whole file on a torn multibyte write (torn_utf8.py); run.rs:3011 and scorecard.rs:470 fail |
| E8.2 | CONFIRMED-UNREACHABLE | low | no | real claude -p gives integer usage |
| E8.3 | PARTLY | low | no | 0 enters at scorecard.rs:366 |
| E8.4 | CONFIRMED-UNREACHABLE | low | no | action still checked against offered list |
| E8.5 | CONFIRMED | low | no | summary with `\n\ntakt: take story:x` makes validator.rs:981-992 count 0 commits → no PR (grep_take.sh) |
| E8.6 | dup of F7.5 | low | environment-allowlist | harness.rs:211-212 comment false |
| E8.7 | WRONG | — | — | live: --restricted refused a symlink to outside |
| E8.8 | PARTLY | low | no | last-wins test single line; no invalid-UTF-8 test |
| N1 new | CONFIRMED (links seen live, not end to end) | high | no | `env -i HOME PATH gh auth status` still logs in via keyring with repo, admin:org, delete_repo; the test command runs model-written tests (harness.rs:93-95, validator.rs:373-376); a PR merged that way is recorded merged without grant check (validator.rs:1517) |
| N2 new | CONFIRMED (code) | medium | no | parse(result)? at harness.rs:262 returns before reported.set :263; failed step's tokens never recorded |

## Area reviews, as returned (condensed by the planner)

Area 1: crates/takt/src/promotion.rs and `promote` in main.rs at fe93286 (Haiku reviewer)

- P1.1 high — promotion.rs:832-838 (sandbox()), :881-897 (cargo()), :867 `--dev` — bwrap runs without `--new-session`; the confined build keeps takt's controlling terminal; a candidate build.rs can open /dev/tty and inject keystrokes (TIOCSTI) into the user's shell. `grep new-session` finds nothing. Inferred: --dev binds a tty node and TIOCSTI is enabled on this kernel.
- P1.2 medium — scorecard.rs:613-625 (load), :485-496 (cases), consumed at promotion.rs:1046-1047, :1557-1558 — gate inputs are self-reported: load checks only format; nothing binds rows to .engineering/cases event files; a hand-written scorecard-<HEAD>.json with passing rows is promoted; --baseline-only accepts a forged baseline.
- P1.3 low — promotion.rs:1046-1047 — TOCTOU: the recorded digest's bytes and the judged card come from two separate reads; same for baseline copy (367 vs 1557).
- P1.4 low — promotion.rs:1292 — `git checkout` in the fresh clone runs on the host outside bubblewrap under the user's global git config; a commit's .gitattributes `filter=<name>` runs the user's filter.<name>.smudge. git() helper blocks hooks and fsmonitor, not filter drivers. Inferred.
- P1.5 low — promotion.rs:605-632 (publish), :273-388 (records), :305-310 — a crash leaves `.<name>.tmp-<pid>`, which records refuses; every later promotion refuses. (Already filed as story:promotion-records-its-toolchain, C4.)
- P1.6 low — promotion.rs:1472, :1634 — records and --dry-run JSON carry absolute host paths with the home directory.

Area 2: crates/takt/src/scorecard.rs at fe93286 (Haiku reviewer)

- T1 high — scorecard.rs:366 (also :356, :371) — a step's missing or non-integer `tokens` counts as 0, so unreported usage looks free in the median gate and min/median/max. harness.rs:260 turns no usage into `tokens: None`, run.rs:3944 skips, events.rs:94 omits; or a crafted line `"tokens":"900"`/`9.5`. A baseline with unreported runs gets max 0. Claimed verified.
- T2 medium — scorecard.rs:371, :309-311, :241-243, :215-221 — a stop line with no `stop` string becomes outcome `""`, accepted by is_completed and ranked 1; an errored run (rank 0) is promoted by one bogus line. Real takt writes only listed kinds (run.rs:84-99); needs a tampered record.
- T3 medium — scorecard.rs:657 (`trials` bound, only printed), :663 (`no_record` unused in `against`), :759-767 — `against` judges on fewer cases than the rule's trials: n=1 with 0/1 accepted vs 5/5 baseline passes (5·1 > 1·5 false); n=2 with 1/2 passes. CLI exit 0; promotion protected by enough_trials (promotion.rs:1656-1690); Rule doc at scorecard.rs:166-167 says fewer is a refusal. Claimed verified.
- T4 medium — scorecard.rs:700-711, :730-733 — token median over all candidate cases, no cap on n; padding with zero-token cases pulls it under the saved max (5 at 2000, 5 at 0, max 1000 → median 1000 passes).
- T5 medium — scorecard.rs:444, :469 and events.rs:258 — file_type() does not follow symlinks but is_file()/read_to_string do; events.jsonl symlinked to another case's record double-counts; a symlinked case dir is dropped with no no_record. Reachability inferred (step snapshot restores cases dir).
- T6 low — scorecard.rs:120-128 — outcome is best completed run, tokens summed over all runs; a resumed case counts accepted.
- T7 low — scorecard.rs:316-320, :352-355 — unparsable or missing `at` leaves wall 0 unmarked.
- T8 low — scorecard.rs:591-608 — save creates with create_new then writes; a failed write leaves a truncated file blocking re-save.

Area 3: crates/takt/src/validator.rs at fe93286 (Haiku reviewer)

- H3.1 medium — validator.rs:1517 and :1561 — the `state => (state, false)` arm skips the grant check that the Open arm makes, so a pull request already merged or closed is recorded with no grant; the merged-head artifact still writes `"granted": revision`. Trigger: PR at the revision merged outside takt (forge UI, another actor), no repository.merge grant in authority.yaml. Claimed verified.
- H3.2 medium — validator.rs:484-496 (with gate.run comparison :580-600) — admitted_files drops executables outside the tree (`..` past root, absolute paths), so gate.run never compares them with the base and runs a disk binary whose provenance is not the base commit. Trigger: project file admits `../ci-tools/check` or `/opt/x/check`. Claimed verified; exploitability inferred.
- H3.3 medium — validator.rs:524-526 — file_at treats every `git cat-file -e` failure as "file absent"; for the default project file (:591) absence means gates evaluated with no project file. Trigger: transient git failure reading `.engineering/assertions-project.yaml` at base. Claimed verified; pass outcome inferred.
- H3.4 low — validator.rs:440-452 and :108-113 — base_revision resolves base_commit as any revision expression, never checks full sha or ancestry; base_commit = revision empties judged() diff, tests.run and ci.read record Unchanged. Needs edit to branch.yaml outside the loop (run.rs:3783 snapshots/restores cases dir).
- H3.5 low — validator.rs:1228 — ci.read takes PR head from one read and checks from another (`forge.checks(tree, number)` takes no sha); checks of a newer head can be recorded for the revision. Race inferred.
- H3.6 low — validator.rs:1446-1452 — branch.pushed written before the push; a failed push leaves an unpushed head recorded and later merges refuse "nothing to merge". Claimed verified.
- H3.7 low — validator.rs:1473-1475 — when head equals the revision, PrMerged reads no checks before merging; relies on an earlier ci_result record. Inferred.

Area 4: crates/takt/src/case.rs at fe93286 (Haiku reviewer)

- C4.1 high — case.rs:700-707 — the approver check runs only when the case has a story.yaml; an intent-opened case skips is_approver, so `takt approve … --by agent:takt` (forbidden at 460-461) or any string is written as a grant; frontier::approve (frontier.rs:374) passes --by straight to prepare_grant; grants_at (1344-1351) and the merge observation (validator.rs:1409-1410) check only capability and revision. Reaching a merge on an intent case inferred.
- C4.2 medium — case.rs:317-327, 1802-1812 — approver never re-validated when authority.yaml is read; a hand-written `approver: agent:takt` counts via canon_by and the merge gate.
- C4.3 medium — case.rs:330-334, 736-768, 831-863 — authority.yaml and decisions/<n>.json are unauthenticated; prepare_grant takes the highest-numbered decision as gate status, checks only computed_from; the in-run snapshot (run.rs:3508-3576) covers judgement steps only; edits between runs accepted.
- C4.4 medium — case.rs:1361-1366, 1707-1715, 681-693 — only `implementation` is ever moved (set_revision callers case.rs:1548, run.rs:2783, 4287, main.rs:1243); grant_subject (protocol.rs:148-188) binds improvement capabilities to `candidate`, which nothing moves, so a candidate-bound grant survives later candidate commits.
- C4.5 medium — case.rs:197-203, 1427-1430 (readers run.rs:2859-2862, 3331; validator.rs:440, 567-575, 109-114, 973, 989; forge.rs:1058) — branch.yaml base_commit and base trusted without comparison to an open-time record; `base` not repaired unlike `name` (validator.rs:960-969); merge observation never compares the PR's base. Edits between runs.
- C4.6 low — case.rs:1690-1699 with validator.rs:1410 — grants() reads the revision from in-memory doc; the merge gate compares every grant with the implementation revision regardless of grant_subject, so a candidate-bound grant never satisfies it (fails closed).
- C4.7 low — case.rs:1404-1410 — open() takes the case id from the branch suffix `takt/<other>` without check_id; `takt/a/b` reads .engineering/cases/a/b/branch.yaml.
- C4.8 low — case.rs:1652-1662 (git.rs:164-171) — discard runs `git branch -D <name>` with name from branch.yaml, not compared with Branch::name_of(id).
Reviewer notes: case.rs has no test module; check_id covers open/resume/prepare_grant/current.

Area 5: crates/takt/src/run.rs at fe93286 (Haiku reviewer)

- R5.1 high — run.rs:3925-3926 (judgement step), :3597 (validator path), restore :306-310 — a step or test command replaces .engineering/cases with a symlink; snapshot of a symlinked root is empty; restore bails at :309 ("replaced by a link") and the `?` skips undo()/undo_step, so the link, HEAD, index and the step's commits survive; next run resumes through the link. Trigger: `rm -rf .engineering/cases && ln -s <dir> .engineering/cases` in a step. Claimed verified end to end.
- R5.2 medium — run.rs:2970-2975 (close) — Ok(refusals) from record_accepted dropped; stop stays Completed (accepted) when the store refuses the test_result or move; case then skipped by resumable (case.rs:879-) and take_next (run.rs:2750, :2450), story stays active.
- R5.3 medium — harness.rs:260-261 with run.rs:3943-3945 — reply with no usage gives tokens None; --tokens never stops the run; nothing says the budget is unenforced. (Overlaps T1, E8.2.)
- R5.4 medium — run.rs:3549, :3563, :3592 — after a validator's observation these `?` return without undo, unlike the judgement path (:3899-3990); test-written files survive the run's error.
- R5.5 low — run.rs:1363-1364 (judge_list_at) — any cat-file failure returns an empty repository judge list silently.
- R5.6 low — run.rs:2616-2617, :2673-2674 — steps, tokens, clock reset on resume; budgets bound one invocation, not the case. Intent unclear.
- R5.7 low — run.rs:2741-2749, case.rs:1513-1530 — no tree-level lock; two takt processes on one worktree move each other's HEAD. Inferred.

Area 6: crates/takt/src/git.rs at fe93286 (Haiku reviewer)

- GIT-1 medium — git.rs:149-152 — current_branch uses `rev-parse --abbrev-ref HEAD`, which returns `heads/main` when a tag `main` exists; used in Checkpoint::take_guarding (1105), restore (1366-1377), the case's `left` (case.rs:1404, checkout at case.rs:1489). Restore then points HEAD at refs/heads/heads/main, creates a stray branch and bails. Scratch repo (the review scratch directory) printed `heads/main`.
- GIT-2 medium — git.rs:1364 — restore does `let _ = self.put_back_git_dir(tree)`, dropping GitDirPutBack.failed; post-check 1412-1420 does not inspect .git/config, hooks, info/; restore returns Ok over unrestored .git state; undo paths run.rs:3535, 3865, 3869-3872 don't report.
- GIT-3 low — git.rs:236-256 — has_subject returns Ok(false) on any git log failure (249-251); callers run.rs:2441, 3136 use it for resume dedupe.
- GIT-4 low — git.rs:49-62, 19-25 — REDIRECTING_ENV lacks GIT_NAMESPACE and GIT_ATTR_SOURCE though takt-cli/build.rs:75-76,90-92 says git.rs carries the list; GIT_SSH_COMMAND overrides core.sshCommand=false; GIT_CONFIG_GLOBAL/GIT_CONFIG not scrubbed. Needs takt launched with those vars.
- GIT-5 low — git.rs:272-273, 236-245 — commit() and has_subject() put the revision before `--` unguarded; `--all` answers for the whole repo. Callers pass git-produced shas: latent.
- GIT-6 low — git.rs:523-563, 1057-1064, 1317 — Content::of reads whole files, 64 MiB cap per file, no total cap; symlink_metadata then fs::read follows a swapped symlink.
- GIT-7 low — git.rs:275-305 — commit() reports no paths for a root commit; no consumer today.
- GIT-8 low — git.rs:1029-1053, 40-41 — checkpoint guards only refs/heads and refs/tags; stash, remotes, notes not restored or detected (doc at 940 states it).

Area 7: crates/takt/src/forge.rs at fe93286 (Haiku reviewer)

- F7.1 high — forge.rs:1111-1147 (read at validator.rs:1228-1235; merge gate skipped at validator.rs:1473) — checks() reports Pass from whatever `gh pr checks` lists; no required-check set, so a required check not yet created is invisible; ci_result pass written; merge at head==revision does not re-read checks. No test for a missing required check (tests/forge.rs:564). Trigger: one check passed, a required one not yet created. Code path claimed verified; gh omission inferred.
- F7.2 medium — validator.rs:1213-1230 and forge.rs:1111-1119 — checks read by PR number not sha; head can move between view and checks call. (Same as H3.5.)
- F7.3 medium — forge.rs:956-963 (view_argv omits baseRefName), :944-951 (merge_argv sends only sha), merge :1149-1174 — the PR's base is never read or compared with the base opened. Trigger: PR base edited between open and merge.
- F7.4 medium — forge.rs:388-397 (bot_route_reads keeps GH_TOKEN and GITHUB_TOKEN) with :401-408 (without_credentials) and header :1-13 saying the bot credential never enters takt — a personal GH_TOKEN in the user's shell is passed to gh and b10x-gates, so reads and bot lines can run under the user's identity.
- F7.5 low — forge.rs:370-386 (is_credential suffix deny-list), :396 (any B10X_* admitted) — GITHUB_PAT, GH_PAT, *_KEY, *_AUTH, *_PASS, *_PWD, *_COOKIE pass to children; B10X_*_TOKEN/_SECRET pass via prefix.
- F7.6 low — forge.rs:1218-1226 with :837-849 — without_userinfo returns scp-form `user:tok@host:path` unchanged; for_tree prints it in its refusal.
- F7.7 low — forge.rs:1042-1049, :1037-1040, :858-864 — when the branch tip equals the revision the push is by branch name, so git pushes whatever the branch is at push time.
- F7.8 low — forge.rs:1051-1082 with validator.rs:1043-1059 — PR POST not idempotent, no lookup before retry; a created PR with a non-zero exit leaves the number unrecorded.

Area 8: crates/takt/src/harness.rs and events.rs at fe93286 (Haiku reviewer)

- E8.1 medium — events.rs:258 — read() uses read_to_string, so one invalid UTF-8 byte (a torn multi-byte write on ENOSPC) makes the whole record unreadable and bypasses the torn-line skip logic (~265-300); emit keeps appending.
- E8.2 medium — harness.rs:243-261 (components 249-251) — each usage component is `as_u64().unwrap_or(0)`; absent key, null, float `1200.0` or string becomes 0; tokens (261) sums a partial set with no marker.
- E8.3 medium — events.rs:151 with run.rs:2617, 2674, 2987 — Stop.tokens starts at 0 and is written 0 when no step reported usage; the record cannot say "no figure".
- E8.4 low — harness.rs:243-244 and 159-172 — is_error and subtype of the reply never read; exit-0 JSON with is_error true and an action line becomes Ok(StepReport). Inferred.
- E8.5 low — harness.rs:169 with run.rs:4275, git.rs:437-439, run.rs:3967 — model's summary passed verbatim; newlines reach commit messages (`-m` in commit_paths) as body/trailers; control characters reach the terminal through `said:`.
- E8.6 low — harness.rs:211-220 with forge.rs:370-385 — comment "No other credential reaches it" is false; is_credential is suffix-based and admits AWS_ACCESS_KEY_ID, DATABASE_URL with a password.
- E8.7 low — harness.rs:66-71 — isolation rests on `claude --restricted`; takt does not check whether a committed symlink pointing outside is followed by Read. Inferred.
- E8.8 low — harness.rs:327 — the last-wins test has one candidate line; ClaudeCode::step, usage mapping, missing-summary default untested; events.rs has no tests of torn lines, skipped lines or invalid UTF-8.
Reviewer's non-findings: run.rs ~3754-3800 snapshots and restores .engineering/cases around a step, so the harness cannot forge events lines; action checked against offered list (run.rs:4235).

Area 9: scripts/ at fe93286 (Haiku reviewer)

- S9.1 high — scripts/check-finding:293-299 — not_reproduced whenever exit differs or the fixed string is absent; no positive control that the command reached the scenario, so a crash (exit 101/2) reads as a fixed defect; exit-only predicates allowed by file-finding:184-185.
- S9.2 high — scripts/check-finding:233-234 — cargo build runs candidate build.rs / proc macros / .cargo config with the caller's full environment and real HOME; header isolation (36-53, 265-269) covers only clone and predicate command.
- S9.3 medium — scripts/check-finding:133,146 — finding file, predicate and $here/file-finding read from the working tree, not the judged revision or main; command runs via sh -c.
- S9.4 medium — scripts/check-citations:157-159 — external citations (`# read from: <tool> <ver>, <text>`) exit 0 unverified; a broken line in that form passes. Probe: exit 0.
- S9.5 medium — scripts/check-citations:433 — only *.yaml under ess/ is read; .yml skipped. Probe: 0 counted, exit 0.
- S9.6 low — scripts/check-citations:473-476 — digest compared with the row in the same tree; text and digest changed together pass.
- S9.7 low — scripts/check-citations:70 — WRAPPED flags a continuation only if the path has / or .; `# from: Makefile:3` ignored.

Area 10: protocols/ with protocol.rs and takt-cli main.rs at fe93286 (Haiku reviewer)

- X10.1 high — main.rs:348-378, 177; case.rs:454-466, 694-707 — `takt approve --by` accepts any `human:<name>`/`agent:<name>`; nothing checks owner, delegation record or the actor; judge_surface capability unguarded (judge-guard not built). Trigger: developer-agent runs `takt approve <case> repository.merge.judge_surface --by human:timo`.
- X10.2 high — protocols/self-improvement/1.yaml:18-22, 153-157, 162-176 — `surface` is a caller-supplied input (main.rs:450-508 passes --input through); nothing computes it from the diff; `--input surface=non_judge` on a candidate touching build.rs makes candidate.merge admissible under repository.merge alone.
- X10.3 high — promotion.rs:1016-1096 with main.rs:69-89 — takt promote checks HEAD = scorecard revision, clean tree, non-regressing scorecard; no check the revision is merged into mainline; no takt.promote grant or case record; protocol `promoted` (1.yaml:100-106) and binary.promote (184-194) say built from the merged candidate.
- X10.4 high — protocols/story-delivery/1.yaml:135-148 vs AGENTS.md:65-67 — repository.merge needs only the repository.merge capability; validator.rs:1316-1323, 1269-1294 check grant and CI, not whether the diff touches a judge surface; a story diff touching build.rs or .engineering/** merges under the planner's grant.
- X10.5 medium — protocols/loop-improvement/1.yaml:37-41, 57-70, 110-116, 134-142 — definition_check covers only metric definitions; corpus, review panel, intake rules, protocols, profiles can change while loop.definition_honest stays true; three-turn requirement is prose only.
- X10.6 medium — protocols/self-improvement/1.yaml:153-161, 177-183 — candidate.merge precedes scorecard.compare (its precondition is fix.merged); a regressing candidate reaches mainline; no revert action.
- X10.7 medium — main.rs:462-474, 631-643, 1350-1400; protocol.rs:62-68 — --protocol loads any cwd-relative file, no baseline comparison; validators_of reads .engineering/gates.yaml from the judged repository; contrary to self-improvement 1.yaml:136-138 ("read from the baseline").
- X10.8 medium — main.rs:509-522 — intent runs default risk=standard, change_kind=feature, affects_security_boundary=false, dropping the security-review obligation Canon derives from true.

```findings
[
 {
  "file": "crates/takt/src/run.rs",
  "line": 3926,
  "category": "correctness",
  "severity": "blocker",
  "message": "R5.1: filed as story:cases-root-link-is-undone"
 },
 {
  "file": "crates/takt/src/validator.rs",
  "line": 373,
  "category": "correctness",
  "severity": "blocker",
  "message": "N1: filed as story:test-command-reaches-no-forge-login"
 },
 {
  "file": "scripts/check-finding",
  "line": 233,
  "category": "correctness",
  "severity": "blocker",
  "message": "S9.2: filed as story:check-finding-builds-confined"
 },
 {
  "file": "scripts/check-finding",
  "line": 293,
  "category": "correctness",
  "severity": "warning",
  "message": "S9.1: filed as story:check-finding-positive-control"
 },
 {
  "file": "crates/takt/src/promotion.rs",
  "line": 1047,
  "category": "correctness",
  "severity": "warning",
  "message": "P1.2: filed as story:promote-binds-scorecard-and-merge"
 },
 {
  "file": "crates/takt/src/promotion.rs",
  "line": 1082,
  "category": "correctness",
  "severity": "warning",
  "message": "X10.3: filed as story:promote-binds-scorecard-and-merge"
 },
 {
  "file": "crates/takt/src/scorecard.rs",
  "line": 657,
  "category": "correctness",
  "severity": "warning",
  "message": "T3: filed as story:scorecard-against-needs-the-trials"
 },
 {
  "file": "crates/takt/src/events.rs",
  "line": 258,
  "category": "correctness",
  "severity": "warning",
  "message": "E8.1: filed as story:run-record-survives-a-torn-character"
 },
 {
  "file": "crates/takt/src/forge.rs",
  "line": 253,
  "category": "correctness",
  "severity": "warning",
  "message": "C4.5: filed as story:merge-checks-the-pull-request-base"
 },
 {
  "file": "crates/takt/src/harness.rs",
  "line": 262,
  "category": "correctness",
  "severity": "warning",
  "message": "N2: filed as story:failed-step-tokens-are-recorded"
 },
 {
  "file": "crates/takt/src/promotion.rs",
  "line": 1292,
  "category": "correctness",
  "severity": "note",
  "message": "P1.4: filed as story:git-filter-drivers-do-not-run"
 },
 {
  "file": "various",
  "category": "correctness",
  "severity": "note",
  "message": "confirmed lows: filed as story:review-of-main-lows"
 }
]
```
