# Changelog

A release is cut by an annotated tag `<major>.<minor>.<patch>` whose version equals
`[workspace.package] version` in `Cargo.toml` and has a section `## [<version>] — <date>` here;
`.github/workflows/release.yml` checks that, runs `task check` and publishes the binaries.

## [Unreleased]

## [0.2.1] — 2026-10-19

The first published release of the 0.2 line; its changes are those listed under 0.2.0.

- 0.2.0 was tagged but not published: its release gate failed on one test,
  `finding_scripts.rs:1695`, which expected bash's "No such file or directory", while `/bin/sh`
  on the CI runner is dash, which prints "Directory nonexistent". Every write the test attempts
  was still refused; only the wording differed
  (<https://github.com/beyond10x/takt/actions/runs/39266647600>).
- The test accepts either wording (<https://github.com/beyond10x/takt/pull/1>).

## [0.2.0] — 2026-10-19

### Improvement turns

- `takt run --protocol protocols/self-improvement/1.yaml --input finding=<story id> --input
  candidate=<full commit id>` opens a `self.improvement/1` turn on the promoted baseline, the
  promotion store's latest record. The turn is refused at open, nothing written, unless the
  running binary is the promoted one, the candidate leaves the finding's story file,
  `scripts/check-finding` and `scripts/file-finding` as the baseline has them, and the baseline's
  `scripts/check-finding` is one reviewed as confined. Both scripts are read at the baseline,
  never from the case's tree.
- Four validators drive a turn: `finding.file`, `scenario.reproduce`, `scenario.verify` and
  `scorecard.compare`, which reads the candidate's `scorecard-<candidate>.json` under
  `$XDG_STATE_HOME/takt/scorecards/`. A turn ends `unreproducible` when the finding does not
  reproduce on the baseline and `declined` when it still reproduces on the candidate. The merge
  validators and `binary.promote` are not built yet, so no turn reaches `accepted`.
  `--scenarios <dir>` names the scenario checkout a finding's check runs in.
- `loop.improvement/1` cases (`protocols/loop-improvement/1.yaml`) open only with `--input
  loop_baseline=<full commit id>`. `loop.measure` writes a measurement over the recorded
  `self.improvement/1` cases, each figure naming the case ids it was computed from;
  `loop.definition.check` tells whether a candidate redefined a metric of
  `protocols/loop-improvement/metrics.yaml` without keeping the previous definition beside it;
  `loop.compare` writes `improved` or `not_improved`.

### Validators, pull requests and grants

- A merge that changes a judge surface, a path `.engineering/judge-allow.yaml` does not admit
  (here every path but `docs/**`, `README.md` and `CHANGELOG.md`; without the file, every path),
  needs the owner's grant (`human:<id>`), or an `agent:<role>` grant that
  `.engineering/delegations.yaml` names in an unrevoked delegation of
  `repository.merge.judge_surface`; a revoked one grants nothing. Both files are read at the merge
  target's tip as the forge reports it, never from the case's tree, and a change to either
  merges under a `human:` grant alone.
- The judge guard judges what the merge does to its target (`git merge-tree --write-tree`, git
  2.38 or later) and everything the candidate changed since each merge base. A symlink or
  submodule is a judge surface whatever its path. Refused: a merge that conflicts, a case base
  that is no ancestor of the target, a pull request whose base differs from the case's. The
  objects it reads are fetched into a fresh copy and hashed as they arrive.
- `takt approve --by` takes only `human:<id>` or `agent:<role>`, for an intent's case as for a
  story's.
- `pr.open` scans what the push would send before anything leaves: each commit, every full path
  in its trees, every blob (one with a UTF-16 or UTF-32 byte-order mark also decoded), and the
  pull request's title and body. A refused scan pushes and opens nothing; more than 32 MiB is
  refused, never cut.
- Pushes go over HTTPS only (`GIT_ALLOW_PROTOCOL=https`) and push the scanned commit by id. A push
  is refused when the remote's base differs from the tracking ref that narrowed the scan.

### Safety and confinement

- A step cannot replace what the test command's script loads. The step guard reads the script,
  and one level of the scripts it runs, and guards the modules `python -m` runs,
  `sitecustomize.py`, `usercustomize.py`, `*.pth` files, a module named like one the test
  command's Python imports from outside the tree (asked when the case opens), and the executables
  in a tree directory on `PATH`. A script line it cannot read puts every Python file, shell file
  and program of the tree on the judge surface, and the refusal says why.
- `tests.run` and `gate.run` run with a fresh empty `HOME` outside the tree,
  `PYTHONNOUSERSITE=1` and `PYTHONDONTWRITEBYTECODE=1`. A step that changes a writable directory
  on their `PATH`, or one Python imports from outside the tree, stops the run naming the path;
  takt leaves that change in place and writes back only the cargo and rustup configuration files
  it guards.
- `tests.run` starts the test command without the variables that locate a login (the session
  bus, the runtime and XDG directories, gh, git's global configuration, GnuPG, netrc, cloud and
  container clients) and, on Linux, in a user and mount namespace of its own, where `/run/user`
  is empty and the bus sockets and cargo's registry credentials are covered. Where the host
  refuses unprivileged user namespaces, the test command does not run, and the error names the
  kernel setting.
- A harness step that adds, rewrites or removes a file inside an ignored directory that existed
  before it (`__pycache__/`, `target/`) is refused and undone. A file it rewrote is not deleted:
  it is moved, never over another file and through no link, to
  `$XDG_STATE_HOME/takt/set-aside/<case>/step-<n>/` (`~/.local/state/takt/set-aside/` without
  it), and every message about the discarded step names where each file went or why it stayed.
  A set-aside root that cannot be used keeps the run from starting; with neither
  `XDG_STATE_HOME` nor `HOME` set, the first harness step is refused before the harness is
  called. What a validator's command rewrote there is its build output and stays.
- A step or a test command that replaces `.engineering/cases`, or a part on the way to it, with
  a link or something other than a directory is undone: a link is removed as itself, the cases
  directory is put back from the snapshot, and nothing is put back through a link. A case is
  never opened or resumed through a link. When the undo fails, nothing of the run is recorded
  through the link, the case is not let go, and the error, with the run's steps and tokens, goes
  to stderr.

### Findings

- `scripts/check-finding` builds the revision under check in bubblewrap, with a cleared
  environment and a scratch home; without `bwrap` nothing is built and no predicate runs.
- The predicate's command runs in bubblewrap with every host path read-only but the clone and its
  own home, no network, its own PID and IPC namespaces, its own `/tmp` and `/run`, the caller's
  home and passwd home hidden, and no descriptor the caller left open. A process it leaves
  behind ends with the check.
- A predicate may name `healthy_exit` (0 when absent). `not_reproduced` needs that exit; a status
  the predicate names neither as the defect's nor as the healthy one gives `error`, which
  verifies nothing.
- `scripts/file-finding` refuses a predicate whose command never runs `$TAKT`, and an `output`
  shorter than 8 characters.

### Scorecard and promotion

- `takt scorecard --save` takes `--revision` only as the full 40-hex commit id and refuses a run
  whose `opened` line names another revision or the revision `-dirty`, naming the run. Runs that
  name no revision save only when none of the card's runs names one.
- A scenario case's outcome comes from `held-out.txt` beside its record (first line `PASS` or the
  failure): `failed` unless it passed. A scenario case without one is refused.
- `takt scorecard --against` gives no verdict on fewer cases a side than the rule's trials (exit
  1), and `takt promote` refuses such a scenario.
- `takt promote` refuses a revision that is no ancestor of the checkout's `origin/main` as last
  fetched, and a scorecard holding a run made by another binary than the revision's.
- `--rejudge` judges a refused revision again on a scorecard of new cases; a second refusal is
  recorded beside the first.
- `--rebaseline <file>` replaces the baseline's scorecard with the same runs re-scored under a
  newer metric definition, and appends a record naming the replaced scorecard by its sha256.
- The promotion chain's latest record is anchored by `promotions.head` beside the record
  directory: a deleted or replaced latest record is refused, naming it, `--dry-run` included. A
  store with no head is read on trust, said on stderr, and the head written from the chain (not
  on `--dry-run`).
- `takt promote` no longer refuses a second promotion on the same UTC day: a candidate of
  another revision is judged against the latest record and, when it holds, recorded after it.
  Promotions still run one at a time under the store's lock, and a revision or tree recorded
  already is still refused.

### Repository and site

- `task check` runs 12 property tests with a fixed seed; `task fuzz` runs the cargo-fuzz targets
  `events`, `harness_reply`, `case_id` and `scorecard` for 60 s each on nightly.

### Known limits

- `story.delivery/1` does not require a failing test before the implementation
  (story:story-delivery-tests-first).
- `tests.run` passes on exit 0 alone, so code under test that ends the process with status 0
  before the runner reports earns a pass (story:test-pass-needs-the-runners-report).
- A relative or in-tree `B10X_GATES_POLICY` resolves inside the case's tree, where a step can
  rewrite the policy `pr.open` scans with (story:scan-policy-outside-the-tree).
- A deleted `promotions.head` is read on trust again, and a process running as the same user can
  rewrite a record and the head together.
- A predicate's command still sees a host socket outside `/run`, `/tmp` and the hidden homes, and
  one that needs `~/.cargo` or `~/.rustup` fails.

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
