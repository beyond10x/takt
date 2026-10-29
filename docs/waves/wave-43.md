# Wave 43: the local forge, the dependency pins and takt delivering one of its units

Stage: **closed**. Shape: one coordinator, one managed worktree per unit (`takt-w43-u2` to
`takt-w43-u6`, the local forge in `takt-wave-43`), the integration branch `integration/wave-43`
in `takt-w43-int`, from 609cda19 (the 0.19.0 release head). Implementor builds share one target
directory; a watch stops them under 25 GiB free. Each unit is merged into the integration branch
after its adversary pass, then one gate, one pull request and one release.

The wave makes the local forge, which lets the next wave run as one coordinator with several
takt runs each merging its story into one integration branch, and takes the `rsi` stories first.

## Units

| unit | story | delivered by | stage |
|---|---|---|---|
| U1 | story:local-forge-merges-into-a-target-branch | implementor | merged (ab973d44), 3 adversary passes |
| U2 | loom 0.48.0, metaharness 0.23.0, engineering-protocols 0.13.0, Canon 0.7.0; clears dependency-blocker:metaharness-session-request-document | implementor | merged (3a1412fb); engineering-protocols 0.13.0 and Canon 0.7.0 deferred |
| U3 | story:credential-shape-sees-a-token-after-a-word-character | implementor | merged (8b4f0ec5), 2 adversary passes, 3 rounds |
| U4 | story:codex-step-gets-each-file-once, from the trial candidate cb40b2d6 | implementor | merged (ca044bc9), 2 adversary passes, 3 rounds |
| U5 | story:red-tests-block-withholds-per-line | implementor | merged (d75e3132), 1 adversary pass |
| U6 | story:the-default-confinement-is-named-as-the-default | takt (Codex, no forge) | merged (81d597bf), 2 adversary passes, corrections by an implementor |

Commits the wave makes: one plan commit per unit branch where a story moves, each unit's
commits, one merge per unit into `integration/wave-43`, the closing plan commit, then the pull
request into `main` and the release tag, all through the bot route.

## Log

- U1 adversary pass 1 (reviewer): needs-revision, nine findings, among them `local_checks` run
  unconfined and able to move the hub's refs; all nine fixed, ten mutants killed.
- U1 adversary pass 2: needs-change, 4 red cases of 39: a merge repeated after a lost record or a
  hand merge, a `local_checks` Substrate cannot run accepted at open, a tree sharing object files
  with the hub accepted, and cold builds per checks run.
- U6 attempt 1 stopped `RedBase`: the story's test target does not exist at the base. Attempt 2
  runs with `--continue-past-red-base`.
- U6 attempt 3 stopped `StepRefused` at `gate.run`: a fetch and the 0.19.0 tag, made in another
  worktree of the same repository while the gate ran, moved refs the step's guard charged to the
  step. Resumed on the same case, the gate passed and the run stopped `Unperformable` at
  `pr.opened` (exit 7) with candidate 649bf157, three commits.
- U5 implementor: green, the predicate from exit 9 to 0; in adversary pass 1.
- U2: loom 0.48.0 (metaharness 0.23.0, harness crates 0.22.0) and the image's harness 0.22.0;
  engineering-protocols 0.13.0 and Canon 0.7.0 wait for a loom on Canon 0.7.0: loom 0.48.0's
  governor compiles protocols with Canon 0.6.0, which refuses 0.13.0's `story.delivery/2`.
- U3 implementor: green, the finding's check from exit 101 to 0.
- One target directory shared by several trees ran one tree's test binary for another's build
  (same binary name, rebuilt between build and run); each adversary builds into its own target.
- U1 round 3: the four red cases of pass 2 pass; adversary pass 3 running.
- U5 adversary pass 1: nothing found, one run-level test added; committed 8e439d65 and merged
  into `integration/wave-43` (d75e3132).
- U1 adversary pass 3: needs-change, 3 red cases: a merge by hand that carries a commit on top of
  the head is recorded merged and then not believed; a tree whose object directory is a link
  into the hub is admitted; a checks pass is keyed by head only. In round 4.
- U6 adversary pass 1 on takt's candidate: needs-change. takt's gate (specification, citations,
  changelog, format, lint) and the story's test command passed, but two existing tests the
  candidate broke ran in neither: `scan_adapter_cli` asserts the old wording, and the new
  `run_console.rs` lacks its own home. `--help` lost `[default: none]`; the wording rested on a
  string replace. Corrections by an implementor.
- U2 adversary pass 1: needs-change, 3 red cases and a stale changelog promise (a strict-version
  refusal takt never makes); the blocker's clearing claimed an `effort` refusal metaharness does
  not make. Fixed in the unit with the doc-writer's site pages; merged (3a1412fb).
- U1 round 4: the three red cases of pass 3 pass; committed and merged (ab973d44). On the merged
  tree effects 7, no_own_evaluation 3, tests_have_a_home_of_their_own 3, local_forge 14 and
  local_forge_adv3_holder 3 pass.
- U6 corrections: green, `--help` says `[default: none]` again, the library carries the default
  reason; adversary pass 2 running.
- U3 adversary pass 1: needs-change: without the word-boundary skip the credential reader is
  quadratic on a long word run (1 MiB in 422 s, 12 ms before). A lower-case hex `sk-` key joined
  to a letter stays not found, by decision: the price of `risk-…` words staying shown.
- U6 adversary pass 2: no red case; committed as one commit carrying takt's three and the
  corrections (81ac10f3), merged (81d597bf).
- 0.19.0 released; its release check accepted five ignored adversary cases on the condition
  that each names its story; the two that named wave units now do, and
  story:unit-tests-run-steps-over-the-operators-path is recorded for wave 44.
- U4 adversary pass 1: needs-change, 5 red cases: a `CLAUDE.md` beside an `AGENTS.md` is never
  given to Codex even when it holds its own rules; binary, mode-only and CRLF test files count
  as shown whole. Decisions: `CLAUDE.md` is left out only when `AGENTS.md` holds its text or it
  points there, and given when named in the run file; a test file outside the scope keeps its
  hunks. In round 2.
- U4 round 2: the five red cases pass with their assertions unchanged; the discard case can now
  fail. Adversary pass 2: needs-change, 2 red cases of 149: a one-line `CLAUDE.md` naming
  `AGENTS.md` and adding a rule counts as a pointer, and a git-quoted path's hunk marks the
  binary file before it shown whole. In round 3.
- Two findings filed for later waves, each with a predicate that exits 9 on the integration
  branch: story:a-ref-moved-outside-the-gate-is-not-its-own (U6 attempt 3) and
  story:takt-gate-runs-the-tests-a-change-can-break (U6's candidate). Codex's step 4 in U6 said
  the specification tool and the formatter were unavailable in its sandbox; that is the model's
  text, no record, and is not filed.
- U4 round 3: both red cases pass; committed (a6b232a9) and merged (ca044bc9). Every unit is
  on `integration/wave-43`; the full gate runs on it.
- Full gate 1 on ca044bc9: 48 test binaries ran, 576 tests passed and 1 failed:
  `forge.rs` `gitlab_tokens_and_glab_s_token_key_are_credential_shapes` still asserted that
  a token joined to a word (`xglpat-…`) is not found, which U3 changes on purpose. The
  assertion now expects it found; the gate reruns with every test binary run.
- Full gate 2 on ca044bc9 with that assertion changed, every test binary run: 304 suites, 2569
  passed, 0 failed, 10 ignored, each ignore naming its story or its limit; specification,
  citations (1351 resolved, 0 unresolved), changelog, format and lint exit 0. ESS 0.76.0 rides
  along (task:ess-0-76-0). The five stories are implemented on that gate.
- PR CI: `local_forge_adv_substrate` failed on its runner (git 2.56.0): the push to the hub
  by its path wrote `refs/remotes/origin/takt/<case>`, which the step guard charged to
  `pr.open`. git 2.55.0 here writes no such ref, and the test skips itself here for want of a
  Substrate adapter, so the local gate passed it without running it. Fixed in 6191610b: the
  push puts back the tracking refs it wrote.
