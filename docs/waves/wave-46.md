# Wave 46: a story opens on story.delivery/3, and report --cases sums a repository's days

Stage: **CI green at 75c13eb3, stories implemented, merge next**. Shape: one coordinator and implementor sub-agents
in one tree, on `wave/46` from bf0fa9c6 (release 0.23.0).

## Units

| unit | story | stage |
|---|---|---|
| U1 | story:story-delivery-3-is-the-default, story:a-handover-after-red-is-not-the-next-step | committed (73cd365c) |
| U2 | story:a-turn-pins-the-repository-of-its-forge | Acceptance 4's predicate rewritten in the store; the code was built in wave 40 |
| U3 | story:implementation-prompt-shows-the-tests-path | Acceptance 1 and 2 built in wave 42; Acceptance 3 needs a recorded self-run, left to the trial of 0.23.0 |
| U4 | story:report-sums-cases | committed (8d5bc9ca), review fixes (200ad048) |
| U5 | story:turn-pin-predicate-runs-on-current-takt | the predicate U2 now carries |

U1 and U4 share `crates/takt-cli/src/main.rs`; each was committed apart. `ess/domains/takt.yaml`
and `CHANGELOG.md` took both units' text in c02b514a, and the citations both units moved were
re-pointed in 89fbb0d5.

## Log

- U1 left six tests out of its Scope asserting `story.delivery/2` as the default; the unit
  re-pointed them and `takt run --help` in its commit.
- U4's CLI test failed `tests_have_a_home_of_their_own` until it took the shared `own_home`
  module.
- U2's old Acceptance 4 predicate ended `error` at both revisions it was run at. The reviewer's
  rewrite runs `self.improvement/1` on `.takt/cases` with a stand-in policy file and a refusing
  `gh`, and prints `reproduced` at dcae6b06 and `not_reproduced` at bf0fa9c6.
- The adversarial pass at 89fbb0d5 returned needs-revision with six findings, five on
  `report --cases`: a cases directory given twice counted twice; a directory with no case, or a
  case with no `events.jsonl`, printed a total of 0 tokens; a linked case directory was skipped;
  `human:` with no name counted as an intervention; the `--format json` refusal printed the
  top-level usage. The sixth: README and `docs/design/takt.md` did not name the new default.
  All six were fixed in 200ad048 and a7a3670e; the reviewer's 8 tests went from 3 passed to 8.
- The reviewer's confirm at a7a3670e: approve, all six fixed. Two low items stay: a step's
  unknown tokens on the day `unknown` get no note of their own, filed as
  story:report-cases-notes-skip-the-unknown-day (its predicate exits 3 at a7a3670e); and an
  empty cases root refuses the whole report, kept, since a root with no case is more often a
  wrong path than a repository with no runs yet.
- `task check-static` at a7a3670e: exit 0. The test jobs run in CI on the wave's pull request.
