# Wave 26: the CI budget, no auto-memory, the runs charts on a phone

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/26` cut from
`main` at d02d1c8d, one sub-agent at a time, units in series (A, C, B, D, then the release R), approved by the
orchestrator under the owner's delegation. Release 0.7.0 follows the wave.

## Units

| unit | story | where |
|---|---|---|
| A | story:the-cli-suite-fits-its-ci-budget | `crates/takt-cli/tests/`, the code the slow tests drive, `.github/workflows/check.yml`, `Taskfile.yml` |
| C | story:a-session-step-loads-no-auto-memory | `crates/takt/src/harness.rs`, `crates/session-executor/` |
| B | story:the-runs-charts-read-on-a-phone | `docs/metrics/timeline.py`, `docs/metrics/journey.py`, `website/` |
| D | re-screen of 2026-10-21: r2-02 (CLI help wrapping at 390 dark), r2-03 (about 21 px extra padding in code blocks) | `website/src/css/custom.css`, `website/src/theme/` |
| R | release 0.7.0: ESS pin 0.60.0 -> 0.66.0, CHANGELOG in Keep a Changelog 1.1.0, short release notes, the version | `.github/workflows/check.yml`, `.github/workflows/release.yml`, `CHANGELOG.md`, `Cargo.toml` |

## Log

- 2026-10-21: wave opened from `main` at d02d1c8d; unit A dispatched.
- 2026-10-21: the orchestrator re-screened the deployed site (d02d1c8d): 41 of 44 fixed, three
  new findings. r2-01 (the timeline wave and reference lines run through seven labels at 1440,
  likely from the s1-08 fit) joins unit B; r2-02 and r2-03 become unit D after B. s1-03 goes to
  the docs template owner.
- 2026-10-21: unit A committed in 57c047df: 70-86% of the slow tests' CPU time was sha256 over the
  158.6 MB debug binary with sha2 unoptimised; sha2 now builds at opt-level 3 in dev and test
  (self_improvement_cli 132.0 s -> 34.5 s at 4 threads; 12 library binaries 211 s -> 66 s), and
  check.yml runs check-static, test-cli and test-library as parallel jobs (expected about 13
  minutes); the 45-minute limit stays until three main runs finish under 20. Unit C dispatched.
- 2026-10-21: unit C committed in 2f11b97b: every Claude session the session executor starts
  gets `--restricted`, so Claude Code 2.1.304 loads no auto-memory and opens no cross-session
  inbox; the first request fell by 754-757 tokens in three live pairs. Before unit B,
  `cargo clean -p` of takt's three packages freed 30,672 MiB of `target/` (the orchestrator's
  word). Unit B dispatched with r2-01.
- 2026-10-21: unit B committed in 8048d840: the timeline SVG itself had 29 label crossings at
  1:1 (lines from marker to axis through the header rows; median labels placed clear of labels
  but not of lines); every axis-parallel line now breaks around the labels it would cross (0
  crossings, 390 to 1440 px). The generators embed title, legend and notes as JSON and the site
  sets them as page text, so on a phone only the plot scrolls. I committed it with 4 citations
  unresolved; 0a396f33 re-points them (one digest accepted after reading journey.py:1-15).
  Unit D dispatched.
- 2026-10-21: unit D committed in 0a2d6f31: a wrapped help-list line hangs under its description,
  capped so a 24-character word kept whole still fits (no split flags at any width; capped lines
  only under 600 px); code blocks pad evenly (the template's pre padding stacked on
  Docusaurus's). Next: unit R (release format), then the 0.7.0 bump.
- 2026-10-21: unit R committed in ae84b79d: CHANGELOG.md in Keep a Changelog 1.1.0 form (773 ->
  315 lines, `**Breaking:**` on entries that break a documented contract), `scripts/release-notes`
  writes the GitHub notes and checks the file (gate `changelog` in `.engineering/gates.yaml`),
  release.yml publishes its output. The bump to 0.7.0 committed in ccdc1f1d with ess 0.66
  (`requires: ess 0.66`, CI pin 0.66.0, validate: valid, no warning). Full gate dispatched.
- 2026-10-21: gate at ccdc1f1d (ess 0.66.0 first on PATH): `task check` exit 0, 1615 passed,
  0 failed, 6 ignored in 152 suites; citations 1017/1017; changelog 10 releases. The adversarial
  review dispatched at ccdc1f1d. Stories A, C and B moved to active.
- 2026-10-21: adversarial review at ccdc1f1d: approve on conditions. Release path, CI matrix,
  unit C (every Claude spawn passes `--restricted`; a mutant turns 3 tests red) and units B and D
  clean. Conditions are in CHANGELOG.md: two 0.7.0 changes unmarked as breaking (the Claude
  login read only from the credentials file; proposals refused on a credential shape or a scan),
  an incomplete fail-closed list, three dropped known limits; plus a misspelt Breaking mark
  passing `--check` and a stale comment. Fix round 1 dispatched. Not fixed in this wave:
  metaharness 0.10.1 does not attest `--restricted`.
- 2026-10-21: fix round 1 committed in d2b62b7b: the Claude login read only from
  `$HOME/.claude/.credentials.json` and the proposal refusals are marked breaking, the fail-closed
  list names all seven forms, the 15 dropped known limits are back under their releases,
  `release-notes --check` refuses a misspelt Breaking mark, `LONG_WORD` reaches the CSS as a
  variable. `site-check` 52 checks, 0 failed; clippy clean.
- 2026-10-21: close. story:a-session-step-loads-no-auto-memory and
  story:the-runs-charts-read-on-a-phone implemented on test_result;
  story:the-cli-suite-fits-its-ci-budget stays active until three `main` runs of check.yml finish
  under 20 minutes (then the job limit goes back to 30).
