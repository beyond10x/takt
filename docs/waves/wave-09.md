# Wave 09: the run record, riding grants, delegation text, citations, scenario help

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed**. Approval: the operator role approved this selection (`run-record-and-story-evidence` first, as the head of the self-improvement chain); each judge-surface merge needs the operator role's grant of its exact commit, except `judge-surface-delegation-text`, which merges on the owner's recorded decision (main at 3ad0bd9: `product-requirements:takt` § 2 and R6). Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:run-record-and-story-evidence` | `vision:safe-agentic-coding` | the case directory holds `events.jsonl` (one line per evaluation, step, evidence record and stop); a tests pass record on a story unit becomes a `test_result` record in the store at its `observed_at`; an accepted case moves the story to `implemented` | `crates/takt/src/run.rs` (cited); `events.rs`, `frontier.rs`, `lib.rs`, `tests/run_record.rs` (inferred) | yes |
| `story:approval-riders-are-grants` | `vision:safe-agentic-coding` | a commit riding on a granted revision adds only approval records matching `authority.yaml` grants, one per grant | `crates/takt/src/case.rs` (cited); a test (inferred) | yes |
| `story:judge-surface-delegation-text` | `vision:safe-agentic-coding` | `AGENTS.md` § Rules, the operator profile and both improvement protocols state delegation on record | `AGENTS.md` § Rules, `.agents/operator-agent.md`, `protocols/self-improvement/1.yaml`, `protocols/loop-improvement/1.yaml` (cited) | yes |
| `story:citation-check-in-task-check` | `vision:safe-agentic-coding` | `task check` fails on a stale `read from:` citation and passes on main | `Taskfile.yml`, `scripts/check-citations`, `AGENTS.md` Commands (inferred) | yes |
| `story:scenario-run-help-exit-codes` | `vision:safe-agentic-coding` | `takt scenario run --help` lists no `frontier: nothing ready`; `scripts/check-finding` gives `reproduced` at 0a776d2 and `not_reproduced` at the fix | `crates/takt-cli/src/main.rs`, the scenario run's `after_help` (inferred) | yes |

## Selection path

`aep plan artifact waves --kind story --status draft --format text` (aep 0.74.0) places these five in its waves 1, 2, 5 and 6, because it colours greedily across the whole draft set. Pairwise, the five share one file: `AGENTS.md`, edited by `judge-surface-delegation-text` (§ Rules) and `citation-check-in-task-check` (the Commands block). The coordinator splits that file by section and checks it with `git merge-tree` before the second merge. `run.rs` is in the scope of five draft stories; only `run-record-and-story-evidence` takes it here.

## Left out

- `story:step-cannot-forge-its-validators`: `run.rs`; first unit of wave 10 (the operator role's priority, security).
- `story:case-scoped-step-guard`, `story:no-inspect-for-stateless-harness` with `story:no-model-step-for-approval` (one unit), `story:scan-before-push`: `run.rs` or `validator.rs`; later waves, `run.rs` split by function where `git merge-tree` is clean.
- The chain after this wave: `story:scorecard-reader`, `story:promotion`, `story:judge-guard`, `story:self-improvement-protocol`.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at eb54f3a |
| model budget | not stated; N = 2 |
| agent file | `AGENTS.md` read |
| gates policy for `scan-text` | the bot route's (`--policy` or `B10X_GATES_POLICY`) |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) for each code unit; `judge-surface-delegation-text` is documents and protocol text, verified by the coordinator and one adversary pass. Every merge into `wave/2026-10-18-w9` waits for its grant.

## The commits this wave makes

The opening store commit, each unit's commits, the merges into `wave/2026-10-18-w9` (each under a grant), the closing store commit, the merge into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: opened on the operator role's approval; the five stories moved to active.
- 2026-10-18: opening commit 8ce2019; five trees from 8ce2019; aep:implementor dispatched to each; briefs at `<scratch>/brief.md` with shared invariants. Decision: run-record writes the store on the case branch only; carrying test_result and the move to the base is a follow-up (sent to the operator role).
- 2026-10-18: judge-surface-delegation-text committed at 98d586d (also .agents/developer-agent.md:29 and the docs/design/takt.md L2 row, which kept the owner-only rule; 361 passed); adversary pass dispatched.
- 2026-10-18: citation-check-in-task-check committed at 564d6d8 (scripts/check-citations, ess/domains/takt.citations.tsv, task check step; 319 of 319 resolve; 373 passed); adversary pass 1 dispatched.
- 2026-10-18: scenario-run-help-exit-codes committed at 447423c (+12 −1 in main.rs, scenario_help 2 cases; 363 passed); verified by the coordinator on the built binary, no adversary pass for a help-text change; [GRANT?] sent.
- 2026-10-18: approval-riders-are-grants committed at bd31a66 (rider files read from the commit and matched to grants at the bound revision; approval_riders 0→5, red 5; 366 passed); adversary pass 1 dispatched. Unapplied: frontier-source.patch (frontier.rs builds the approval source by hand; frontier.rs is run-record's file).
- 2026-10-18: scenario-run-help-exit-codes granted for 447423c and merged at 67ab6ce; the operator role verified the finding: scripts/check-finding reproduced at 0a776d2, not_reproduced at 447423c.
- 2026-10-18: judge-surface-delegation-text adversary pass 1 NEEDS-CHANGE (loop step 8 looser than Owns/Never; widening undefined; where the delegation is held unsaid; README and a test name keep the owner-only wording), recorded as review-result:adversary-delegation-text-pass-1; correction 1 dispatched; judge-guard story text, a design line and two draft-story citations sent to the operator role.
- 2026-10-18: judge-surface-delegation-text correction 1 at 50e695a (Loop step 8, widening path list, where the delegation is held, revoked grants nothing, README, a test name), verified by the coordinator; merged at df8cb09 on the owner's recorded decision (main 3ad0bd9), merge-tree against citation-check clean.
- 2026-10-18: citation-check-in-task-check adversary pass 1 NEEDS-CHANGE (9 red: markers outside comment lines skipped, section and file citations without needles, heading substring match, range end unbounded, fix re-points onto a longer span, CRLF, non-UTF-8 traceback), recorded as review-result:adversary-citation-check-pass-1; correction 1 dispatched.
- 2026-10-18: approval-riders-are-grants adversary pass 1 NEEDS-CHANGE (1 red: a real approval record with change.review/outcome added rides, and the store reads it as a review outcome), recorded as review-result:adversary-approval-riders-pass-1; correction 1 dispatched.
- 2026-10-18: run-record-and-story-evidence committed at 44b20a5 (events.jsonl; test_result and the move to implemented at accepted; takt run and scenario run wired; ess RunEventLine and ten event structs; run_record 0→10, run_record_cli 0→1; 372 passed); adversary pass 1 dispatched.
- 2026-10-18: citation-check-in-task-check correction 1 at d6d558e (11 of 11 adv1 green; section and file citations need needles; exact headings; range-end rule; fix bounded and line-ending preserving; prose exception for `read from the `; 387 passed); pass 1 outcome fixed; adversary pass 2 dispatched.
- 2026-10-18: approval-riders-are-grants correction 1 at 63f5d6e (unknown fields refused, every field compared with what approve writes, actor by shape; 6 of 6 adv1 green; 372 passed); pass 1 outcome fixed; adversary pass 2 dispatched.
- 2026-10-18: citation-check-in-task-check adversary pass 2 NEEDS-CHANGE (10 red: the prose exception hides a citation, fences, setext headings, duplicate headings, earliest last-needle match, fix rewriting prose), recorded as review-result:adversary-citation-check-pass-2; correction 2 dispatched, the coordinator verifies it.
- 2026-10-18: run-record-and-story-evidence adversary pass 1 NEEDS-CHANGE (4 red: a judgement step can write a test_result into the store (pre-existing, no guard on .engineering/planning or evidence); a store write the record does not name; a duplicate after a lost line; a torn line ends accepted runs in an error), recorded as review-result:adversary-run-record-pass-1; correction 1 dispatched.
- 2026-10-18: approval-riders-are-grants adversary pass 2 INFEASIBLE (2 red notes: CRLF front matter refused, whitespace before the fence read), recorded as review-result:adversary-approval-riders-pass-2; actor shape and deny_unknown_fields checked against aep 0.74.0; correction 2 dispatched, the coordinator verifies it.
- 2026-10-18: citation-check-in-task-check correction 2 at ec0dc96, verified by the coordinator (task check exit 0, 319 of 319, 404 passed); pass 2 outcome fixed; [GRANT?] sent.
- 2026-10-18: approval-riders-are-grants correction 2 at cc8a4e3 (CRLF front matter, untrimmed blob read via a new git::blob), verified by the coordinator (382 passed); pass 2 outcome fixed; [GRANT?] sent.
- 2026-10-18: citation-check-in-task-check not granted for ec0dc96 (the reviewer: a range citation is checked only at its ends, so a changed interior line passes; 15 main.rs citations stale against the wave tip). The wave branch merged into the unit at 2aeec1a; correction 3 dispatched (a digest of the cited text per line citation, fix only moves, an accept mode), the coordinator verifies it.
- 2026-10-18: run-record-and-story-evidence correction 1 at b0bfaaa (store guard on .engineering/planning and evidence for judgement steps; record before print, both-or-neither store commit; duplicate check by events and commit subject; torn-line skip; AEP_ACTOR=agent:takt; 380 passed); pass 1 outcome fixed; adversary pass 2 dispatched.
- 2026-10-18: approval-riders-are-grants granted for cc8a4e3 and merged; three low reviewer findings folded into story:records-reach-base.
- 2026-10-18: citation-check-in-task-check correction 3 at 25d2978 (a sha256 digest per line citation, fix only moves identical text, accept mode; the reviewer's interior-line case now fails), the wave branch merged in at 2aeec1a and 8b78c49, 53 moved citations re-pointed (8bc49cb); verified by the coordinator: task check exit 0, 432 passed; [GRANT?] sent.
- 2026-10-18: citation-check-in-task-check granted for 8bc49cb and merged; the reviewer accepted the known limit (digests freeze today's text).
- 2026-10-18: run-record-and-story-evidence adversary pass 2 NEEDS-CHANGE (7 red: a store directory planted as a link and a store moved through project.yaml pass the guard (pre-existing); modes; a FIFO hangs the snapshot; an unreadable file leaves the step; a panicking harness; a torn repair), recorded as review-result:adversary-run-record-pass-2; correction 2 dispatched, the coordinator verifies it.
- 2026-10-18: run-record-and-story-evidence correction 2 at fe2e092 (guard on all of .engineering/ but cases/ by git's view and a snapshot with modes; non-regular files by kind; panics; torn repairs), wave branch merged in at e442295, citations at f6056dd (111 moved, 12 by hand, 48 rows, 57 digests accepted after reading); verified by the coordinator: task check exit 0, 461 passed; pass 2 outcome fixed; [GRANT?] sent.
- 2026-10-18: run-record-and-story-evidence granted for f6056dd and merged at 1425689.
- 2026-10-18: closed. Wave gate at 1425689: `task check` exit 0 (ess valid; 367 of 367 in-repository citations resolve with digests; fmt, clippy 0; `cargo test` 55 lanes, 461 passed, 0 failed). Five stories implemented, each with a Landed section and a test_result.

## Follow-ups

- story:records-reach-base (wave 11): test_result and the move to implemented as riders before the merge; `CaseDir::resume` reads the rider rule; the approval-riders reviewer's three low findings; the move run without `AEP_ACTOR`.
- story:step-cannot-forge-its-validators (wave 10): a store directory that is already a link at the base; a case-variant `.Engineering/`; the store checked through git only during a validator observation; a test of a judgement step writing `gates.yaml`.
- story:scorecard-reader (wave 10): the run event lacks the four token components, sub-second wall time, the base revision on `opened` and the observed revision on validator steps.
