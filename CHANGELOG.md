# Changelog

A release is cut by an annotated tag `<major>.<minor>.<patch>` whose version equals
`[workspace.package] version` in `Cargo.toml` and has a section `## [<version>] — <date>` here;
`.github/workflows/release.yml` checks that, runs `task check` and publishes the binaries.

## [Unreleased]

## [0.1.0] — 2026-10-19

### Run loop

- `takt run --repo <path>` opens or resumes a case and takes one validated step at a time until a
  stop, bounded by `--iterations`, `--hours` and `--tokens`; `--hours` also ends a harness step,
  the test command or a gate command that runs past it. `--harness claude` (default) or
  `--harness none`, which runs only validators.
- A run takes no model step when only a validator or a grant can advance the case, and a harness
  that keeps nothing between steps is not offered `repository.inspect`.
- Without `--intent` or `--case`, a run resumes the open case of a story being worked, or takes
  the next ready story from the planning store and records it as taken; no model makes the choice.
- A story opens on `story.delivery/1`, embedded in the binary; an `--intent` opens on
  `software.change/1`. `--protocol` takes a built-in id or a `protocol/1` file; `--input
  name=value` sets case inputs. A resumed case keeps its own protocol and inputs.
  `takt protocols` lists the built-ins. A `--case-id` that is no valid branch name is refused.
- Exit codes: 0 completed or nothing ready, 1 refused before any step, 2 arguments do not parse,
  3 a budget, nothing admissible, a refused step or no progress, 4 waiting for an approval, 5 an
  error once the case was open, 6 waited on the forge until the budget ended.
- A run whose pull request still has checks running waits for them inside its budget; out of
  budget it stops as `Waiting`, and the next run resumes the case.
- `takt scenario run --repo <path> <id>` replays a scenario from `.engineering/scenarios.conf`:
  it cuts the next free `trial/<n>` from the seed commit, pushes it and runs the story against it,
  with no hand reset. The trial fetch needs no SSH credential of the user's.

### Validators, pull requests and grants

- `story.delivery/1` makes `tests.pass`, `gate.green`, `pr.opened`, `ci.green` and `pr.merged`
  claims about the implementation revision; `accepted` holds only once the pull request merged.
- `gate.green` comes only from the repository's `engineering-gates/1` document (`--gates`, else
  `.engineering/gates.yaml`) evaluated at the implementation revision.
- With `--forge bot` (the default for an `origin` on github.com) a run pushes the case's branch
  and opens one pull request through the bot route once the change is complete, so CI runs once;
  `pr.opened` and `ci.green` come only from what the forge reports. `pr.open` scans the body
  under the policy in `B10X_GATES_POLICY`.
- A run merges only under a recorded grant of `repository.merge`, else stops at
  `ApprovalRequired` with nothing merged. `takt approve --repo <path> <case> <capability> --by
  <who>` records the grant; the next run resumes the case and merges.
- A grant authorises only the revision of the artifact its action is about, and a case is held by
  one run at a time.
- After the merge, the base's planning store holds the story `implemented` with its `test_result`
  and approval records; above the granted revision only those records ride, each byte-equal to
  the one takt kept for that revision.

### Safety and confinement

- A case refuses to open on a dirty tree; a step commits only the paths it changed; a refused step
  is undone, index included; evidence binds only to a revision whose content it observed.
- Validators run under the case-directory guard; gate and project definitions are read from the
  base revision; child processes inherit the environment without credential variables (a name
  denylist), a timeout and bounded output.
- A step that edits a validator's inputs (the test script, `.cargo/**`, `build.rs`,
  `rust-toolchain*`, the CI workflow) earns no `tests.pass` or `ci.green` from that edit.

### Records

- Each case keeps `events.jsonl`, one line per evaluation, step, evidence record and stop; a step's
  line carries the model, cost, duration and turns the harness reported, absent when unstated.
- A line of `events.jsonl` that is not valid UTF-8 is skipped and named; the rest still reads.
- `takt --version` prints the revision the binary was built from, and every run records it.

### Scorecard and promotion

- `takt scorecard --repo <path>` prints per run outcome, steps, tokens, refusals, `NoProgress`,
  errors and wall time from `events.jsonl`. `--save <dir> --revision <sha>` saves it;
  `--against <file>` compares per scenario under the rule it prints (5 trials, margin of 1 case;
  fewer than 5 cases are not yet refused) and exits 3 when worse.
- `takt promote --repo <path> --scorecard <file>` (Linux only: it needs bubblewrap) installs a
  candidate only when no scenario is worse than the baseline, from a fresh clone built under
  bubblewrap, and records it under `$XDG_STATE_HOME/takt/promotions/`; one a UTC day.
- `--dry-run` writes nothing. `--baseline-only` records the first baseline, with `--revision` and
  `--binary-sha256` for a binary built before `--version` named its revision.

### Reports and traces

- `takt trace --case <dir>` writes a run as one `takt-trace/1` JSON Lines file; token and cost
  figures come from the harness's raw usage, and command text is not copied.
- `takt report --case <dir>` or `--trace <file>` (repeatable) prints wall time per actor, steps,
  model calls, tokens and cost, defects, and what the sources did not record.
- `docs/metrics/` keeps scenario runs, improvement turns and waves as JSON Lines series;
  `timeline.py` and `journey.py` draw them per revision and per run.

### Findings

- `scripts/file-finding` files a finding once per key with an `expected_before` predicate;
  `scripts/check-finding <id>` runs it at a takt revision and prints `reproduced` or
  `not_reproduced`. A predicate may name a repository, a seed commit and a test.

### Repository and site

- `ess/` declares takt's nouns and relations (`ess specify validate`); `task check` fails on a
  stale `read from:` citation there.
- <https://beyond10x.github.io/takt/> describes takt, its results with n and caveats, and its
  roadmap; `/takt/protocols/` draws every protocol from its `protocol/1` file.

### Distribution

- A version tag publishes `takt` for Linux (x86_64, aarch64) and macOS (aarch64) as `.tar.gz`
  with a `SHA256SUMS` file at <https://github.com/beyond10x/takt/releases>.

### Known limits

- `story.delivery/1` does not require a failing test before the implementation
  (story:story-delivery-tests-first).
- The scorecard takes a run's outcome from the run's own record, not from a held-out acceptance
  check (story:scorecard-outcome-from-held-out-check).
- The test command can reach the operator's forge login through the keyring and session bus
  (story:test-command-reaches-no-forge-login).
- A relative or in-tree `B10X_GATES_POLICY` resolves inside the case's tree, where a step can
  rewrite the policy `pr.open` scans with (story:scan-policy-outside-the-tree).
- A step's writes inside a git-ignored directory that existed before it are not detected
  (story:ignored-directories-are-guarded).
- A step that replaces `.engineering/cases` or a case directory with a symbolic link is not undone
  (story:cases-root-link-is-undone).
- Under load, a process a test command started with `setsid` can write into the tree after the
  command returns (story:run-child-sweep-survives-setsid).
- `scripts/check-finding` builds the revision under check with the operator's HOME and
  environment (story:check-finding-builds-confined), and can print `not_reproduced` when the
  predicate's command crashed or never reached the scenario (story:check-finding-positive-control).
