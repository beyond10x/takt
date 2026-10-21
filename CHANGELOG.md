# Changelog

This file follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and takt follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Bare tags such as `0.7.0` are
takt's releases.

## [Unreleased]

## [0.7.0] - 2026-10-21

The minor version moves because every harness step runs through a metaharness session, `--harness` takes `codex` and `harness`
beside `claude`, and the gate's reader fails closed on lines 0.6.0 read.

### Changed

- `--harness claude` runs Claude Code through a metaharness session (`HermeticMode::Strict`) in a scratch copy of the case tree, offered only the file tools.
- **Breaking:** A Claude step needs `$HOME/.claude/.credentials.json`; without it the session gets no login (not from `CLAUDE_CONFIG_DIR`, probably not from the macOS Keychain) and probably fails.
- The session's change comes back as a proposal that takt writes to the case tree only after the step guard admits it; a refused one ends the run `StepRefused`.
- A gap the hermetic floor reports holds the proposal back; H9 and H1a are reported by row id in the step summary until metaharness declares Claude Code 2.1.304.
- `takt replay` keeps the print-mode harness in its scratch worktree.
- Each judgement step's prompt carries the change the case has made so far: the diff of its accepted steps from the base commit, about 8 KiB at most, fenced as data.
- `--harness` takes `claude`, `codex` (codex-cli 0.153.4) or `harness` (b10x-harness), each through one metaharness session seam.
- Codex runs hermetic `On` with H9 gating, its sandbox writing only to the copy with no network, its home and XDG directories in the session's scratch.
- **Breaking:** A proposal is refused whole, `StepRefused`, on a credential shape in its files or summary, a `b10x-gates scan-text` refusal or a scan that cannot run; 0.6.0 completed such steps.
- With no gates policy a `codex` or `harness` proposal is refused unscanned; a `claude` one is checked for credential shapes only.
- The session's scratch copy reaches the case tree's git state through a `.git` file naming a git directory outside it; the tree's `.git` is never written.
- On the Claude route a judgement step's prompt names its story's `## Scope` files as file mentions (`@todo.py`), at most 8 files and 64 KiB.
- `b10x-loom-sdk` 0.27.0 → 0.28.0 is a dependency of `b10x-takt`; `metaharness` 0.10.1 comes through the new crate `b10x-session-executor`.
- `task site-check` fails a page on text under 12 px, console errors, failed requests and an ignored stored theme.
- The metaharness session starts Claude Code with `--restricted`, as 0.6.0's print mode did, so no auto-memory: a scenario 1 request is 754-757 tokens smaller than without it in three live pairs.
- CI runs `task check` as three parallel jobs (`task check-static`, `test-cli`, `test-library`); test builds compile `sha2` at opt-level 3.
- CHANGELOG.md follows Keep a Changelog 1.1.0; `scripts/release-notes <version>` writes a release's GitHub notes, and `task check` runs its `--check`.
- takt's specification requires ess 0.66 (`ess/ess-inputs.yaml`), and CI installs ess 0.66.0.

### Fixed

- `takt scorecard --against` prints a `median <metric>` line only for a metric with a per-case figure, no longer wall-time medians as `step_tokens`.
- The site fixes the defects of its 2026-10-20 screen but the AEP card's family styling.
- On the runs page no chart line runs through a label, and on a phone each chart's title and legend stay in view while only the plot scrolls.
- A wrapped entry of a CLI help list continues under its description, and code blocks have the same space above and below as at the sides.

### Security

- **Breaking:** The gate's reader fails a line closed when a tree file handing off to a known program sets `PYTHON*` or a whole environment, changes directory or writes `sys.path`.
- **Breaking:** The reader fails it closed, too, if that file hides such a name (`getattr(`, `import sys as`), takes a word before the program, is not Python or shell, or is unreadable or over 1 MiB.

## [0.6.0] - 2026-10-20

The minor version moves because a run whose harness step recorded no tokens has unknown tokens, a run records the layers it ran
on, and the gate's reader fails closed on lines 0.5.0 read.

### Added

- A run records its layers (ADR 0101): the `opened` line's `layers` names `takt`'s revision, each harness step's line the harness `version` and the `model`.
- `takt report` attributes tokens, cost, wall time and failures by layer, per run and in total, each figure naming its line (`events.jsonl:<n>`).
- In `takt report` a figure no layer's record states prints `not reported`, never 0; trace operations carry their record line and the trace header the layers.

### Changed

- **Breaking:** `takt scorecard` prints the layers of the runs it scores and refuses runs whose layers differ, naming the layer, unless `--across-layers`.
- **Breaking:** A run whose harness step recorded no tokens, or 0, has unknown tokens (0.5.0's known limit); `takt scorecard --against` and `takt promote` refuse such a set, exit 1.
- **Breaking:** The metrics `tokens` and `step_tokens` move to definition version 2: a baseline of 0.5.0 or earlier is compared only after `takt promote --rebaseline`.
- `b10x-loom-sdk` 0.27.0 is a dev-dependency; Loom's governor decides takt's 30 recorded case decisions as takt did but where a grant is missing.

### Security

- `gate.run` compares what a gate command's Python files import from every import directory, a `PYTHONPATH` set anywhere on the line included.
- `gate.run` reads `cd`, `env -C`, a redirection-only `exec`, `taskset`, `chrt`, `unbuffer`, `script -c`, `uv run`, `env -S` and `pytest FILE`: 0.5.0's known limit of 11 forms and wrappers closes.
- **Breaking:** The gate's import walk reads at most 64 Python files a command; the cap, `importlib`, `__import__(`, `exec(`, `eval(` or `xargs` input fails closed.
- **Breaking:** The step guard reads the same forms and fails closed in five more places (`doas python3 …` among them), refusing a step that changes any Python file, shell file or program.

## [0.5.0] - 2026-10-20

The minor version moves because a run can stop `ActiveTime`, a stored baseline is judged again only after `takt promote --rebaseline`,
a commit identity the forge's scan refuses stops a run at open, and a gate line the reader cannot read fails closed.

### Added

- `takt run` and `takt scenario run` take `--active-seconds <n>`, a budget on wall time less forge waits; over it the run stops `ActiveTime`, exit 3, the case open.
- `takt scorecard --against <saved> --steps <action>[,...]` judges the steps a candidate targets: better on them but worse on the run exits 3, not better exits 4.
- `takt replay --case <dir> --step <n> [--times <k>]` runs a recorded judgement step again in a scratch worktree and records each replay as `takt-replay/1`.
- `takt replay` exits 3, running nothing, when the rebuilt prompt differs from the recorded transcript, and 5 when the harness failed; it follows no link.

### Changed

- **Breaking:** `takt run` and `takt scenario run` through the bot route scan the commit identity at open; an identity the forge's scan refuses exits 1 before step 1.
- **Breaking:** The scorecard gains the metric `step_tokens` (definition version 1); a baseline stored before it is refused until `takt promote --rebaseline`.
- takt's own gates run through `scripts/gate-exec`, now a Python program (`/usr/bin/python3 -I`, standard library only) that the gate compares whole at the base.
- **Breaking:** `scripts/check-finding` runs one check at a time per cache directory and refuses, exit 1, under `CHECK_FINDING_MIN_FREE_GIB` (20) GiB free.
- The trial agent's profile adds a pre-flight, a token and an active-time cap per run, and an early stop once a set's verdict is decided.
- Known limit: run-level tokens still read a harness step that recorded no tokens as 0, so a missing record reads as a saving; no promotion is made on 0.5.0.
- Known limit: 11 shell forms still let a change decide the gate, 6 of them `PYTHONPATH` set on the line, by `export` or in a sourced file, and `cd dir && python3` with `-m x`, `-c …` or `- < x.py`.
- Known limit: the other 5 are `env -C dir python3 -m x`, a script on `xargs`'s input, `exec 0< FILE; sh`, `exec < FILE; sh` and `exec < FILE; bash` (story:the-gate-reads-every-shell-form).
- Known limit: wrapper programs the reader does not know (`taskset`, `chrt`, `unbuffer`, `script -qec`, `uv run`, `env -S`, `pytest`) and `env PYTHONPATH=…` slip too.
- Known limit: 0.4.0's known limits on build-time code, squash merges, the forge's repository, tipless grants, candidate refs and `takt view` still hold.

### Fixed

- `takt scorecard --steps` reads a harness step that recorded no tokens as unknown, not 0; a targeted step unknown on either side is refused, exit 1.
- `takt run --help`: `--forge none` names the `Unperformable` stop, exit 7, when only a forge action is left.
- A step that writes `.engineering/commit-identity.conf` is refused naming the commit identity file, no longer as a write into the planning store (0.4.0's known limit).

### Security

- **Breaking:** `gate.run` fails closed on a line its reader cannot read with certainty (a glob, an unknown option or redirection, a second script input), naming why.
- `gate.run` compares a Python script run through `sh -c`, a shell script, `exec` or a shell reading a tree file, and what it imports, as it compares one run directly (0.4.0's known limit).

## [0.4.0] - 2026-10-20

The minor version moves because a `DiskLow` stop exits 8, a 0.3.x baseline is judged again only after `takt promote --rebaseline`,
and a run that pushes is refused without a commit identity.

### Added

- A `--forge none` delivery that stops `Unperformable` at `pr.opened` hands its change over as a candidate, `refs/takt/candidates/<case-id>`, printing its id.
- `takt view <dir> [--output <file>]` writes one offline HTML page of the `takt-trace/1` runs under a directory, one lane per run on a shared time axis.
- `takt view`'s page derives from the MIT-licensed Context Lens dashboard and embeds Vue (MIT); the notices are in `viewer/`.

### Changed

- **Breaking:** A run that stops `DiskLow` exits 8, not 3, for `takt run` and `takt scenario run`; its case stays open.
- **Breaking:** takt's commits carry `--commit-identity`, else `.engineering/commit-identity.conf`, else `takt <takt@localhost>`, never the operator's git identity.
- **Breaking:** A run through the bot route is refused at open, exit 1, without a commit identity, and so is the resume of a case opened by 0.3.x.
- **Breaking:** The scorecard's outcome metric is at definition version 3; a version 2 baseline (any 0.3.x one) meets a candidate only after `takt promote --rebaseline`.
- **Breaking:** A self-improvement turn is refused at open, exit 1, when the mainline's tip already holds its candidate or the tip cannot be read.
- **Breaking:** A turn resumed through another forge than the one it recorded is refused, exit 1, naming `--forge <recorded>` as the way past.
- **Breaking:** `candidate.merge_judge_surface` records `merged` only for a candidate that reached the mainline after the grant; a 0.3.x turn's such merge is refused.
- Known limit: build-time code (a build script, a procedural macro) is not closed by comparing files, so the merge of such a change stays the owner's to grant.
- Known limit: a candidate squashed or rebased onto `main` before the grant can still be recorded `merged` (story:a-squash-merge-before-the-grant-is-refused).
- Known limit: a turn records its forge by name only, so a grant's tip and the merge can be read from two repositories (story:a-turn-pins-the-repository-of-its-forge).
- Known limit: a judge-surface merge refused because its grant records no tip names no way past (story:a-tipless-judge-grant-names-its-way-past).
- Known limit: nothing keeps the commit an open turn was opened on once a resume deletes its candidate ref (story:a-candidate-stays-reachable-while-a-turn-names-it).
- Known limit: `takt view` joins a transcript by path suffix and writes through a linked `--output` (story:view-joins-own-transcripts-and-writes-a-private-page).
- Known limit: a step refused for writing `.engineering/commit-identity.conf` is told it wrote into the planning store (story:commit-identity-refusal-names-the-identity-file).
- Known limit: a gate running Python through a shell (`sh -c "python3 …"`, a shell script) has neither the script nor its imports compared (story:gate-compares-python-run-through-a-shell).
- Known limit: nor is a module that `python3 -c` imports from the directory the command runs in.
- takt's repository configures its commit identity in `.engineering/commit-identity.conf` and lists `scripts/**` on its judge surface.
- The agent profiles describe one dispatcher session that runs the waves and one trial agent that runs the trials.

### Fixed

- `takt run` and `takt scenario run` on an empty home no longer have their first harness step refused for `~/.local`: the transcripts directory is made at open.
- No step is refused for the transcripts takt writes when `PYTHONUSERBASE`, `PYTHONPATH` or `XDG_STATE_HOME` places them inside a Python import directory.
- A file or a link where the transcripts directory goes is named as such, and the directories made before that failure no longer get the next step refused.

### Security

- `gate.run` with a project file compares at the case's base every file a gate command takes as an argument, the tool configuration, `[lints]` and the judge surface.
- `gate.run` compares at the base what Python loads for a gate's commands: the modules, `*.pth` files and packages of a script's directory, of `-m` and of `PYTHONPATH`.

## [0.3.0] - 2026-10-20

The minor version moves because `takt promote` now refuses scorecards that 0.2.x accepted.

### Added

- The `Unperformable` stop: no judgement step is taken, the case stays open, and a scorecard ranks it with the budget stops.
- A run's stop line in `events.jsonl` records `completed`; a scorecard takes that word for a stop it does not know, so an unknown stop never counts as completed.
- **Breaking:** `takt run` and `takt scenario run` stop `DiskLow`, exit 3, under `--min-free-gib` (default 10, `0` off) free on the tree's or `CARGO_TARGET_DIR`'s disk.
- Each harness step's transcript is kept under `$XDG_STATE_HOME/takt/transcripts/`, mode `0600`, capped at 16 MiB; no validator reads it.
- takt's repository has a gates document, `.engineering/gates.yaml`, with four gates run through `scripts/gate-exec`, so a delivery on takt reaches `pr.open`.

### Changed

- **Breaking:** `takt promote` judges a candidate against the latest record and against the start of its series; a scenario worse than either is refused, exit 3.
- **Breaking:** `takt promote`, `scorecard --against` and `scorecard.compare` cover every scenario of either side; none, or one only the candidate has, is refused.
- **Breaking:** A new scenario joins the corpus only through `takt promote --rebaseline`, and `--baseline-only` refuses a scorecard with no scenario case.
- **Breaking:** A run whose open obligation only an action nobody performs can decide stops `Unperformable`, exit 7, not `NothingAdmissible`, exit 3.
- A wait on something local (a saved scorecard, a promoted binary, a merge into the local `main`) stops `Waiting`, exit 6, right after the step that observed it.
- `takt run --install-root` on a resumed turn is accepted when it names the root the turn was opened with.
- Claude runs with `--output-format stream-json --verbose`.
- Known limit: `Unperformable` ranks with the budget stops, so a `--forge none` run that stopped `NoProgress` under 0.2.x ranks one higher against an older baseline.
- Known limit: the `lint` gate runs `cargo clippy`, which compiles and runs build code a step wrote, so the merge of such a change stays guarded by the owner.

### Fixed

- `takt scenario run` without the bot route numbers a trial with 22 digits (the second and a random nonce), so fresh clones no longer all cut `trial/0`.
- `takt scorecard --help` names the held-out check, not `events.jsonl` alone, as where a scenario run's outcome is read from.
- The wall-clock tests that failed only on a loaded host hold under load.

## [0.2.3] - 2026-10-19

### Changed

- A Claude step gets `--tools Read,Edit,Write,Glob,Grep` beside `--allowedTools`; a step's first request fell from 12,554 to 5,550 prompt tokens on scenario 1.

### Fixed

- `MultiEdit` is no longer in `--allowedTools`: Claude Code 2.1.303 has no such tool.

## [0.2.2] - 2026-10-19

### Added

- A `self.improvement/1` turn reaches `accepted` on validators' records: the candidate merged, no scorecard regression, the candidate promoted.
- `self.improvement/1` is built in: `--protocol self.improvement/1`, listed by `takt protocols`.
- The merge validators record `merged` when the case holds the grant and the candidate is an ancestor of the mainline; they observe the merge, never make it.
- `binary.promote` records `promoted` only from a promotion record of the candidate whose `binary_sha256` is that of `<install root>/bin/takt`.
- `takt run --install-root <dir>` names where a turn reads the promoted binary, else `CARGO_INSTALL_ROOT`, then `CARGO_HOME`, then `~/.cargo`.
- `takt promote` keeps the binary it replaces at `$XDG_STATE_HOME/takt/judges/<sha256>` and names it, so a turn is finished with that binary.

### Changed

- **Breaking:** A turn opens and resumes only on the embedded `self.improvement/1`; another document with its id is refused.
- **Breaking:** The case input `surface` is computed at open from the baseline's `.engineering/judge-allow.yaml`; a given `surface` that differs is refused.
- **Breaking:** A turn records the sha256 of the binary that opens it, and every resume by another binary is refused before any step.
- **Breaking:** `--gate` defaults to the protocol's first action that requires a capability and that the case's inputs do not block for good.
- A `candidate.merge_judge_surface` grant counts as `repository.merge` does for a judge surface: the owner's, or a role's an unrevoked delegation names.
- A turn opened without `--intent` takes its finding's title as its intent, read on the mainline; a turn writes nothing to the planning store.
- Known limit: a turn's judge check is the running binary checking itself, so a binary built without it would resume the turn and judge it.
- Known limit: under `--forge none` the operator's local `main`, and the `.engineering/delegations.yaml` it holds, decide what counts as merged.

### Removed

- **Breaking:** The action `candidate.deliver`: a turn's candidate is given at open.

### Fixed

- A step's line in `events.jsonl`, its tokens included, is written before the revision update that can end the run.
- The loop-case test's forged-commit precondition no longer depends on the host's commit-graph or gc settings.

### Security

- `scripts/check-finding` runs git with `GIT_NO_REPLACE_OBJECTS=1`, so a replace ref in a scenario checkout cannot stand in for a seed.

## [0.2.1] - 2026-10-19

The first published release of the 0.2 line; its changes are those listed under 0.2.0, which was tagged but not published.

### Fixed

- `finding_scripts.rs` accepts dash's "Directory nonexistent" beside bash's "No such file or directory", which failed 0.2.0's release gate on CI.

## [0.2.0] - 2026-10-19

### Added

- `takt run --protocol protocols/self-improvement/1.yaml --input finding=<story id> --input candidate=<commit>` opens a turn on the promoted baseline.
- A turn is refused at open unless the running binary is the promoted one and the candidate leaves the finding's story and both finding scripts unchanged.
- Validators `finding.file`, `scenario.reproduce`, `scenario.verify` and `scorecard.compare` end a turn `unreproducible` or `declined`; none reaches `accepted` yet.
- `--scenarios <dir>` names the scenario checkout a finding's check runs in.
- `loop.improvement/1` cases open with `--input loop_baseline=<commit>`; `loop.measure`, `loop.definition.check` and `loop.compare` write their records.
- A predicate may name `healthy_exit`, 0 when absent.
- `takt promote --rejudge` judges a refused revision again on a scorecard of new cases; a second refusal is recorded beside the first.
- `takt promote --rebaseline <file>` re-scores the baseline's runs under a newer metric definition and records the replaced scorecard's sha256.
- `promotions.head` anchors the promotion chain's latest record: a deleted or replaced latest record is refused; a store with no head is read on trust.
- `task check` runs 12 property tests with a fixed seed; `task fuzz` runs four cargo-fuzz targets for 60 s each on nightly.

### Changed

- **Breaking:** A merge that changes a judge surface (every path but `docs/**`, `README.md` and `CHANGELOG.md`) needs the owner's grant or a delegated role's.
- `.engineering/judge-allow.yaml` and `.engineering/delegations.yaml` are read at the merge target's tip, and a change to either merges under a `human:` grant alone.
- **Breaking:** The judge guard needs git 2.38 or later (`git merge-tree --write-tree`) and refuses a conflict, a base off the target or a pull request on another base.
- **Breaking:** `takt approve --by` takes only `human:<id>` or `agent:<role>`.
- **Breaking:** `scripts/check-finding` says `not_reproduced` only for the predicate's healthy exit, and `error` for a status the predicate does not name (0.1.0's known limit).
- **Breaking:** `scripts/file-finding` refuses a predicate whose command never runs `$TAKT`, and an `output` shorter than 8 characters.
- **Breaking:** `takt scorecard --save` takes `--revision` only as the full 40-hex commit id and refuses a run whose `opened` line names another or `-dirty`.
- **Breaking:** A scenario case's outcome comes from `held-out.txt` beside its record, `failed` unless its first line is `PASS`; a case without one is refused (0.1.0's known limit).
- **Breaking:** `takt scorecard --against` gives no verdict, exit 1, on fewer cases a side than the rule's trials, and `takt promote` refuses such a scenario.
- **Breaking:** `takt promote` refuses a revision that is no ancestor of `origin/main` as last fetched, and a scorecard holding a run made by another binary.
- `takt promote` no longer refuses a second promotion on the same UTC day; promotions still run one at a time under the store's lock.
- Known limit: `story.delivery/1` does not require a failing test before the implementation (story:story-delivery-tests-first).
- Known limit: `tests.run` passes on exit 0 alone, so code under test that exits 0 before the runner reports earns a pass (story:test-pass-needs-the-runners-report).
- Known limit: a relative or in-tree `B10X_GATES_POLICY` resolves inside the case's tree, where a step can rewrite it (story:scan-policy-outside-the-tree).
- Known limit: a deleted `promotions.head` is read on trust again, and a process running as the same user can rewrite a record and the head together.
- Known limit: a predicate's command still sees a host socket outside `/run`, `/tmp` and the hidden homes, and one that needs `~/.cargo` or `~/.rustup` fails.

### Security

- **Breaking:** `pr.open` scans every commit, path and blob the push would send and the pull request's title and body; over 32 MiB is refused, never cut.
- **Breaking:** Pushes go over HTTPS only (`GIT_ALLOW_PROTOCOL=https`), push the scanned commit by id, and are refused when the remote's base moved.
- A step cannot replace what the test command's script loads: its modules, `sitecustomize.py`, `usercustomize.py`, `*.pth` files and the tree's `PATH` executables.
- **Breaking:** `tests.run` and `gate.run` run with a fresh empty `HOME` outside the tree, `PYTHONNOUSERSITE=1` and `PYTHONDONTWRITEBYTECODE=1`.
- A step that changes a writable directory on their `PATH`, or one Python imports from outside the tree, stops the run naming the path.
- **Breaking:** `tests.run` starts without the variables that locate a login, in its own user and mount namespace on Linux; without user namespaces it does not run (0.1.0's known limit).
- A harness step that changes a file in an ignored directory that existed before it is refused and undone, a rewritten file moved to `$XDG_STATE_HOME/takt/set-aside/` (0.1.0's known limit).
- A step or a test command that replaces `.engineering/cases` with a link or a non-directory is undone, and a case is never opened or resumed through a link (0.1.0's known limit).
- **Breaking:** `scripts/check-finding` builds the revision under check in bubblewrap with a cleared environment and a scratch home; without `bwrap` nothing is built (0.1.0's known limit).
- A predicate's command runs in bubblewrap: host paths read-only but the clone and its home, no network, its own PID and IPC namespaces, `/tmp` and `/run`.

## [0.1.0] - 2026-10-19

### Added

- `takt run --repo <path>` opens or resumes a case and takes one validated step at a time until a stop, bounded by `--iterations`, `--hours` and `--tokens`.
- `--hours` also ends a harness step, the test command or a gate command that runs past it.
- `--harness claude` (default) or `--harness none`, which runs only validators.
- A run takes no model step when only a validator or a grant can advance the case.
- Without `--intent` or `--case`, a run resumes the open case of a story being worked, or takes the next ready story from the planning store.
- A story opens on the embedded `story.delivery/1`, an `--intent` on `software.change/1`; `--protocol` takes a built-in id or a `protocol/1` file.
- `--input name=value` sets case inputs, a resumed case keeps its own protocol and inputs, and `takt protocols` lists the built-ins.
- Exit codes: 0 completed or nothing ready, 1 refused before any step, 2 bad arguments, 3 a budget or no way on, 4 approval, 5 an error, 6 a forge wait.
- A run whose pull request still has checks running waits for them inside its budget, else stops `Waiting`; the next run resumes the case.
- `takt scenario run --repo <path> <id>` replays a scenario of `.engineering/scenarios.conf` on the next free `trial/<n>` cut from its seed commit.
- `story.delivery/1` makes `tests.pass`, `gate.green`, `pr.opened`, `ci.green` and `pr.merged` claims; `accepted` holds only once the pull request merged.
- `gate.green` comes only from the repository's `engineering-gates/1` document (`--gates`, else `.engineering/gates.yaml`) at the implementation revision.
- With `--forge bot` (the default for an `origin` on github.com) a run pushes the case's branch and opens one pull request through the bot route.
- `pr.opened` and `ci.green` come only from what the forge reports; `pr.open` scans the body under the policy in `B10X_GATES_POLICY`.
- A run merges only under a recorded grant of `repository.merge`, else stops `ApprovalRequired`; `takt approve --repo <path> <case> <capability> --by <who>` records it.
- A grant authorises only the revision of the artifact its action is about, and a case is held by one run at a time.
- After the merge, the base's planning store holds the story `implemented` with its `test_result` and approval records.
- A case refuses to open on a dirty tree; a step commits only the paths it changed; a refused step is undone, index included.
- Evidence binds only to a revision whose content it observed; gate and project definitions are read from the base revision.
- Child processes inherit the environment without credential variables, with a timeout and bounded output.
- A step that edits a validator's inputs (the test script, `.cargo/**`, `build.rs`, `rust-toolchain*`, the CI workflow) earns no `tests.pass` or `ci.green` from it.
- Each case keeps `events.jsonl`, one line per evaluation, step, evidence record and stop; a step's line carries what the harness reported.
- A line of `events.jsonl` that is not valid UTF-8 is skipped and named; the rest still reads.
- `takt --version` prints the revision the binary was built from, and every run records it.
- `takt scorecard --repo <path>` prints per run outcome, steps, tokens, refusals, `NoProgress`, errors and wall time from `events.jsonl`.
- `takt scorecard --save <dir> --revision <sha>` saves it; `--against <file>` compares per scenario (5 trials, margin of 1 case) and exits 3 when worse.
- `takt promote --repo <path> --scorecard <file>` (Linux, bubblewrap) installs a candidate when no scenario is worse than the baseline, one a UTC day.
- `takt promote --dry-run` writes nothing; `--baseline-only` records the first baseline.
- `takt trace --case <dir>` writes a run as one `takt-trace/1` JSON Lines file; token and cost figures come from the harness's raw usage.
- `takt report --case <dir>` or `--trace <file>` prints wall time per actor, steps, model calls, tokens, cost, defects and what the sources did not record.
- `docs/metrics/` keeps scenario runs, improvement turns and waves as JSON Lines series, drawn by `timeline.py` and `journey.py`.
- `scripts/file-finding` files a finding once per key with an `expected_before` predicate; `scripts/check-finding <id>` prints `reproduced` or `not_reproduced`.
- `ess/` declares takt's nouns and relations, and `task check` fails on a stale `read from:` citation there.
- <https://beyond10x.github.io/takt/> describes takt, its results with n and caveats, and its roadmap, and draws every protocol from its file.
- A version tag publishes `takt` for Linux (x86_64, aarch64) and macOS (aarch64) as `.tar.gz` with a `SHA256SUMS` file.

### Changed

- Known limit: `story.delivery/1` does not require a failing test before the implementation (story:story-delivery-tests-first).
- Known limit: a relative or in-tree `B10X_GATES_POLICY` resolves inside the case's tree, where a step can rewrite it (story:scan-policy-outside-the-tree).
- Known limit: the scorecard takes a run's outcome from the run's own record, not from a held-out acceptance check (story:scorecard-outcome-from-held-out-check).
- Known limit: the test command can reach the operator's forge login through the keyring and session bus (story:test-command-reaches-no-forge-login).
- Known limit: a step's writes inside a git-ignored directory that existed before it are not detected (story:ignored-directories-are-guarded).
- Known limit: a step that replaces `.engineering/cases` or a case directory with a symbolic link is not undone (story:cases-root-link-is-undone).
- Known limit: under load, a process a test command started with `setsid` can write into the tree after it returns (story:run-child-sweep-survives-setsid).
- Known limit: `scripts/check-finding` builds the revision under check with the operator's HOME and environment (story:check-finding-builds-confined).
- Known limit: `scripts/check-finding` can print `not_reproduced` when the predicate's command crashed or never reached the scenario (story:check-finding-positive-control).

[Unreleased]: https://github.com/beyond10x/takt/compare/0.7.0...HEAD
[0.7.0]: https://github.com/beyond10x/takt/compare/0.6.0...0.7.0
[0.6.0]: https://github.com/beyond10x/takt/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/beyond10x/takt/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/beyond10x/takt/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/beyond10x/takt/compare/0.2.3...0.3.0
[0.2.3]: https://github.com/beyond10x/takt/compare/0.2.2...0.2.3
[0.2.2]: https://github.com/beyond10x/takt/compare/0.2.1...0.2.2
[0.2.1]: https://github.com/beyond10x/takt/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/beyond10x/takt/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/beyond10x/takt/releases/tag/0.1.0
