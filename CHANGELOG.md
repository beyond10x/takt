# Changelog

A release is cut by an annotated tag `<major>.<minor>.<patch>` whose version equals
`[workspace.package] version` in `Cargo.toml` and has a section `## [<version>] — <date>` here;
`.github/workflows/release.yml` checks that, runs `task check` and publishes the binaries.

## [Unreleased]

### Changed

- `--harness claude` runs Claude Code through a metaharness session (`b10x-session-executor`,
  metaharness 0.10.1, `HermeticMode::Strict`) in a scratch copy of the case tree, offered only
  the file tools; the session's change comes back as a proposal, and takt's edit effect writes it
  to the case tree only after the step guard admits it. A proposal naming an action the frontier
  did not offer, a path outside the tree, under `.git`, in the cases directory, on the judge
  surface, reached through a link, or ignored by git, or a file for an action that changes none
  or records a choice, ends the run `StepRefused` with the case tree unchanged. A gap the
  hermetic floor reports holds the proposal back, but the vendor's version against the adapter's
  pin (H9) and undeclared plugins that all ship inside Claude Code (H1a, `@builtin`) are reported
  by row id in the step summary until metaharness declares Claude Code 2.1.304. `takt replay`
  keeps the print-mode harness in its scratch worktree.
- Each judgement step's prompt carries the change the case has made so far: the diff of its
  accepted steps' paths from the base commit, bounded at about 8 KiB, fenced as data and scanned
  when a gates policy is set; or a line that no change has been made, or that it is not known
  (story:step-prompt-omits-the-change-so-far, the trial agent's candidate of the 0.5.0 cycle).
- `takt scorecard --against` prints a `median <metric>` line only for a metric with a per-case
  figure; against a changed `step_tokens` definition it printed the wall-time medians under that
  name.
- The gate's reader fails a line closed when a file of the tree that hands off to a known
  program writes a `PYTHON*` variable, changes directory or writes `sys.path` before it runs
  it, when a word stands between the file and the program, or when the file is neither Python
  nor shell; a `PATH` write does not.
- `--harness` takes `claude`, `codex` or `harness`, each through one metaharness session seam:
  Claude Code (`HermeticMode::Strict`), Codex (codex-cli 0.153.4, resolved on the child's
  `PATH`; hermetic `On`, the verdict in each step's summary, H9 gating; its own sandbox writes
  only to the copy, with no network) and b10x-harness (observed, no per-call decision). Codex's
  home and XDG directories are the session's scratch.
- Every proposal's files and the step's summary are checked before the edit effect writes:
  a credential shape (private key, JWT, `sk-ant-`, `gh?_`, `github_pat_`, `sk-`, a token field)
  or a refusal of `b10x-gates scan-text` under the run's policy refuses the whole proposal,
  naming the file and never the text, and the summary is replaced by a fixed line; a scan that
  cannot run refuses too. With no policy, a `codex` or `harness` proposal is refused.
- The session's scratch copy carries the case tree's git state through a `.git` file naming a
  git directory outside the copy, so Claude Code's start-up git commands answer there as in the
  tree; the case tree's `.git` is never written, and an edit under `.git` is refused.

### Repository

- `b10x-loom-sdk` 0.27.0 → 0.28.0, now a dependency of `b10x-takt`; `metaharness` 0.10.1 through
  the new crate `b10x-session-executor`, which depends on no takt crate; `metaharness-b10x`
  0.10.1 beside it.

## [0.6.0] — 2026-10-20

The minor version moves because a run whose harness step recorded no tokens has unknown tokens
(the `tokens` and `step_tokens` definitions move to version 2, so a stored baseline is judged
again only after `takt promote --rebaseline`), a run records the layers it ran on, and the gate's
reader fails closed on lines 0.5.0 read.

### Changed

- `gate.run` compares, for every Python file a gate's command names, what its top-level imports
  load from each directory that Python imports from (`name.py`, `name.*`, `name/**`, followed
  through the tree's files, at most 64), whether a `PYTHONPATH` entry was set in the binding's
  `env`, on the line, by `export`, in a sourced file or by `env`; a `-c` string's imports the
  same. `cd` and `env -C` move the directory a command runs in; a redirection-only `exec` sets
  the shell's standard input for the commands after it; `taskset`, `chrt`, `unbuffer`,
  `script -c`, `uv run`, `env -S` and `pytest FILE` are read; `xargs` is read and fails closed
  on its input. The eleven shell forms 0.5.0 listed as slipping are closed
  (story:the-gate-reads-every-shell-form). The import walk reads at most 64 Python files per
  command; reaching the cap, or a read file importing by `importlib`, `__import__(`, `exec(` or
  `eval(`, fails closed, naming the file and the form.
- The step guard reads the same forms and is stricter in five places: a shell reading the
  input a redirection-only `exec` set, `python -` with an unknown input, the wrappers above, an
  `env -S` string split into words, and a program the reader does not know given one it knows
  (`doas python3 …`), which fails closed naming both; a file of the tree given one
  (`scripts/gate-exec python3 …`) is compared whole at the base and read as running that
  program with the words after it.

### Added

- A run records its layers, layer name to revision, read from what ran and never typed (ADR
  0101): the `opened` line's `layers` names `takt` (the binary's revision); each harness step's
  line names the harness program's own version as its stream stated it (`version`; Claude
  Code's `claude_code_version` in the `system`/`init` event) and the model (`model`). takt runs
  the harness program in a judgement step and nowhere else, never for its version. `takt
  scorecard` joins the `harness` and `model` layers from the step lines, prints the layers of
  the runs it scores and refuses to score runs whose layers differ, naming the layer, unless
  `--across-layers`; a record without `layers` reads as not recorded, never as a revision.
- `takt report` attributes tokens, cost, wall time and failures by layer, per run and in total,
  each figure naming the record line it came from (`events.jsonl:<n>`); a figure a layer's
  record does not state prints `not reported`, never 0; a run whose layers wrote no records is
  reported as before with the layers column `not recorded`. Trace operations carry their
  record line and the trace header the layers. Reported today: the model (tokens, cost, API
  time), the harness (call durations) and takt (wall time, refusals and errors); the harness's
  own trace, the model's own failures and the runtime's decision records have no reader yet
  and print `not reported`.

- A run whose harness step recorded no tokens, or 0, has unknown tokens, never a sum counting
  the step 0; `takt scorecard --against` and `takt promote` (with `--baseline-only` and
  `--rebaseline`) refuse such a set, exit 1, naming the case and the step, before anything is
  judged or recorded. The metric definitions `tokens` and `step_tokens` move to version 2: a
  baseline saved by 0.5.0 or earlier is compared only after `takt promote --rebaseline`, which
  refuses a stored run whose tokens are now unknown, naming it; `--baseline-only` refuses a store
  that already holds a baseline, so such a store is judged again only from a new store.

### Repository

- `b10x-loom-sdk` 0.27.0 is a dev-dependency of `b10x-takt` (one copy each of Canon and
  engineering-protocols). `tests/loom_parity.rs` shows Loom's governor decides takt's 30 recorded
  case decisions as takt did: the 17 that read no grant are equal, and the 13 that read one differ
  only where the grant is missing (the granted action's status and reasons, and
  `computed_from.authority`). No run behaviour changes.

## [0.5.0] — 2026-10-20

The minor version moves because a run can stop `ActiveTime`, a stored baseline is judged again
only after `takt promote --rebaseline`, a commit identity the forge's scan refuses stops a run at
open, and a gate line the reader cannot read fails closed.

### Changed

- `takt run` and `takt scenario run` through the bot route scan the commit identity with the
  forge's scan when they open or resume a case, before step 1 (for a scenario run, before its
  trial base is pushed): an identity the scan refuses exits 1, naming where the identity came
  from and the refusal, with no step taken and nothing pushed. Before, the scan refused it only
  at the first push. With no policy or no `b10x-gates` to scan with, the run prints
  `not scanned` and opens; `--forge none` scans nothing.
- The scorecard gains the metric `step_tokens` (definition version 1), each run's tokens by step
  action. A baseline stored before it has no version for the metric and is refused, so the next
  promotion needs `takt promote --rebaseline` first; a takt that does not know the metric (0.4.0)
  refuses a scorecard saved under it.
- `gate.run` fails closed on a line its reader cannot read with certainty (a glob or brace word
  where it names what runs, an option it does not know, another input a shell reads as its
  script on the same command, standard input from a duplicated descriptor, a redirection form it
  does not know): the gate compares every Python file, shell file and program at the base, so a
  change to any of them stops it, and the summary names why. A gate running a shell script with
  such lines stops on every such change; takt's own `gate-exec` did, through its `if` and `case`
  lines, until it became a Python program (below).
- `gate.run` compares a Python script run through `sh -c`, a shell script, `exec` (its options
  read), or a shell whose standard input the same command redirects or pipes from a tree file
  (`sh < FILE`, `cat FILE | sh`, a descriptor before a redirection read as part of it: `2>&1`,
  `0<`), and what it imports, as it compares one run directly (0.4.0's known limit,
  story:gate-compares-python-run-through-a-shell). Of the 34 shell forms wave 19's review probed,
  26 are closed and 3 run the code under work at the tree's root by decision; of 22 further forms
  the confirm probed, 20 are closed; of 33 redirect forms, 30 are read or fail closed. The 11
  forms that still let a change decide the gate are under Known limits.
- takt's own gates run through `scripts/gate-exec`, now a Python program (`/usr/bin/python3 -I`,
  standard library only) the gate compares whole at the base, so they need `/usr/bin/python3`.
  A delivery changing a Python file, shell file or program no gate command runs is judged by
  takt's gate; one changing `gate-exec` or `scripts/check-citations` still stops it, naming the
  file (story:gate-exec-is-readable-by-the-gate).
- `scripts/check-finding` builds and runs one check at a time per cache directory under a `flock`
  on `takt/check-finding/lock`, says when it waits, and refuses, exit 1, before building when that
  file system has less than `CHECK_FINDING_MIN_FREE_GIB` GiB free (20 by default; CI sets 0). It
  reads the free space before it waits and again once it holds the lock.

### Added

- `takt run` and `takt scenario run` take `--active-seconds <n>`: a budget on a run's wall time
  less its waits on the forge, read before each step, so a step under way runs to its end. Over
  it the run stops `ActiveTime`, naming the budget, the active time and the wait time, exit 3,
  and the case stays open. `ActiveTime` is a stop 0.4.0 does not have: a runner that resumes a
  case stopped with 3 resumes this one too, and the resumed run's active time starts again from
  0. A scorecard ranks `ActiveTime` with `NoProgress`, below the budget stops.
- `takt scorecard --against <saved> --steps <action>[,...]` judges the steps a candidate targets:
  a step is better when every candidate case is below the saved side's cheapest such step, or a
  one-sided exact Mann-Whitney U test (ties against the candidate, at least 5 cases a side) rejects
  equality at p <= 1/20; the whole run must stay not worse. Better on its steps but worse on the
  run is refused, exit 3; not worse but not better on a step is exit 4. A saved scorecard without
  the `step_tokens` metric is refused, exit 1.
- `takt replay --case <dir> --step <n> [--times <k>]` runs one judgement step of a recorded case
  again from its recorded state, in a scratch worktree at the revision it started at. It checks the
  rebuilt prompt against the recorded transcript first, when one is found (a mismatch exits 3 and
  runs nothing), records each replay as `takt-replay/1` outside the case and the tree, and leaves
  the case untouched. A replay whose harness failed exits 5, the error in its record.
  A link in the revision's tree, among the case's evidence or decisions, or at its `intent.md` or
  `protocol.yaml` is refused, never followed; `--out` is taken as an absolute path, and inside the
  tree the case lies in it is refused even with `--repo`.

### Fixed

- `takt scorecard --steps`: a harness step that recorded no tokens is unknown, not 0. A targeted
  step unknown on either side is refused, exit 1, naming the case and the step, so a missing
  record no longer judges a candidate better; a saved scorecard records it as `null`.
- `takt run --help`: `--forge none` names the `Unperformable` stop, exit 7, when only a forge
  action is left, instead of saying the harness takes them.
- A step that writes `.engineering/commit-identity.conf` is refused naming the commit identity
  file, no longer as a write into the planning store (0.4.0's known limit,
  story:commit-identity-refusal-names-the-identity-file).

### Repository

- The trial agent's profile (`.agents/trial-agent.md`) adds a pre-flight before each set of runs,
  a token and an active-time cap per run, an early stop once a set's verdict is decided, a rule
  that parks a candidate for the per-step judge instead of re-trialling it, and reports to a
  monitor the owner names.

### Known limits

- Run-level tokens still read a harness step that recorded no tokens as 0, so a missing record
  reads as a saving; promotions wait for story:run-tokens-unknown-is-not-zero, and no promotion
  is made on 0.5.0.
- These 11 shell forms still let a change decide the gate, each an ignored test naming
  story:the-gate-reads-every-shell-form: `PYTHONPATH` set in the line, by `export` or in a sourced
  file; `cd dir && python3 -m x`; `env -C dir python3 -m x`; a script named on `xargs`'s input;
  `cd dir && python3 -c …` and `cd dir && python3 - < x.py`; and `exec 0< FILE; sh`,
  `exec < FILE; sh`, `exec < FILE; bash`, where a redirection-only `exec` sets the shell's input
  for the commands after it. Wrapper programs the reader does not know (`taskset`, `chrt`,
  `unbuffer`, `script -qec`, `uv run`, `env -S`, `pytest`) and `env PYTHONPATH=…` slip too and are
  listed in that story.
- 0.4.0's other known limits still hold: build-time code, and
  story:a-squash-merge-before-the-grant-is-refused, story:a-turn-pins-the-repository-of-its-forge,
  story:a-tipless-judge-grant-names-its-way-past,
  story:a-candidate-stays-reachable-while-a-turn-names-it and
  story:view-joins-own-transcripts-and-writes-a-private-page.

## [0.4.0] — 2026-10-20

The minor version moves because a `DiskLow` stop exits 8, a 0.3.x baseline is judged again only
after `takt promote --rebaseline`, and a run that pushes is refused without a commit identity.

### Changed

- A run that stops `DiskLow` exits 8, not 3, for `takt run` and `takt scenario run`, so a
  runner that resumes a case stopped with 3 no longer resumes a trial that ran out of disk. Its
  case stays open; a trial runner discards it rather than resume it.
- Every commit takt makes (the take, the store's writes on the case's branch, an approval's
  record, a step's change, a candidate's replay) carries one identity as author and committer:
  `--commit-identity '<name> <email>'`, else the one `.engineering/commit-identity.conf`
  (`[identity]` `name` and `email`) names at the commit the case forks from (for
  `takt scenario run`, at `--declared-on`), else `takt <takt@localhost>`. Neither the operator's
  git configuration nor the identity variables in takt's environment reach a commit. The case
  records the identity when it opens; a resume and `takt approve` commit with it, and a resume
  given another one is refused, exit 1.
- A run whose commits leave the machine (the bot route) is refused at open, exit 1, before
  anything is written, when no identity is given or configured; a run under `--forge none` is
  not. A case opened with no identity, which every case opened by 0.3.x is, is refused the same
  way when resumed through the bot route; the refusal names the way past (finish it with
  `--forge none`, or drop its branch and directory).
- The scorecard's outcome metric is at definition version 3: `Unperformable` ranks with the
  budget stops and a `DiskLow` trial takes no rank. A takt that knows version 2 refuses a
  version 3 scorecard, and a baseline stored under version 2 (any 0.3.x baseline) meets a
  version 3 candidate only after `takt promote --rebaseline`.
- A self-improvement turn is refused at open, exit 1, before anything is written, when the
  mainline's tip already holds its candidate (the candidate or a descendant of it), naming the
  candidate and the tip; open it on a fresh candidate. It is refused the same way when the tip
  cannot be read (the bot route or the network down, no `main`) or whether the tip holds the
  candidate cannot be read, naming the forge and the read's error; open it again once the tip can
  be read. A resumed turn reads no tip at open, and one whose candidate reaches the mainline after
  it opened still resumes.
- A turn resumed through another forge than the one it recorded when it opened is refused, exit
  1, before anything is written, naming both and `--forge <recorded>` as the way past.
- `candidate.merge_judge_surface` records `merged` only for a candidate that reached the mainline
  after the grant. `takt approve` records the mainline's tip with a turn's judge-surface grant,
  read through the forge the turn recorded when it opened; a tip that cannot be read refuses the
  grant with nothing written. A candidate that tip already holds is refused, decided by ancestry,
  not by a committer date; the refusal names the commit where the candidate reached the mainline,
  its committer date and the grant's time. A grant that records no tip, whose tip cannot be
  fetched, or whose tip is not on the mainline now is refused too. A turn opened by 0.3.x recorded
  no forge, so its grant records no tip and its judge-surface merge is refused.
- `gate.run` with a project file compares at the case's pinned base, besides the executables and
  catalogs it admits: every file a gate command takes as an argument; the tool configuration
  (`.cargo/config`, `.cargo/config.toml`, `clippy.toml`, `.clippy.toml`, `rustfmt.toml`,
  `.rustfmt.toml`, `Taskfile.yml`, `Taskfile.yaml`, `Taskfile.dist.yml`, `Taskfile.dist.yaml`, by
  file name at any depth); the `[lints]` and `[workspace.lints]` entries of every `Cargo.toml`;
  and the judge surface `tests.run` is compared on, `rust-toolchain*` and the repository's own
  `.engineering/judge-surface.yaml` included. One that differs, was added or was deleted runs
  nothing and writes no record, and the summary names it. A directory the base holds where an
  argument points is the data a command checks and is not compared.
- `gate.run` also compares at the base what Python loads for a gate's commands: every module,
  compiled module, `*.pth` file and package in a script's own directory or the directory
  `python3 <dir>` runs, and the module and package `python -m a.b` runs, with the `*.pth` files
  beside it, from the directory the command runs in and each `PYTHONPATH` entry the binding sets.
  No other module of the run directory or a `PYTHONPATH` entry is compared: those often hold the
  code the gate checks. A script at the root of the tree, where the gate runs, is compared as `-m`
  is (the script and the root `*.pth` files), and a subdirectory of a script's directory that
  holds a module at any depth is compared whole.

### Added

- A delivery under `--forge none` that stops `Unperformable` at `pr.opened` hands its change over
  as a candidate: its code commits, without takt's take, record and approval commits, are
  replayed on the base branch's tip under the case's identity, kept under
  `refs/takt/candidates/<case-id>`, and the run prints `candidate: <full commit id>`, which a
  `self.improvement/1` turn opens on (`--input candidate=<id>`). With no identity configured, a
  case with no code commit, or a replay that conflicts, it prints `candidate: none:` and why, and
  the ref no longer names a candidate an earlier run of the case made.
- `takt view <dir> [--output <file>]` writes one offline HTML page of the `takt-trace/1` runs
  under a directory (`takt-view.html` in the current directory by default): one lane per run on
  a shared time axis, each operation a mark naming its step, action and outcome, a click opening
  the step's transcript (its head and tail past 256 KiB). The page holds its style, script and
  data inline and fetches nothing. A directory with no trace is refused, exit 1, and nothing is
  written. The page's style and layout derive from the MIT-licensed Context Lens dashboard,
  notice in `viewer/LICENSE-context-lens`; it embeds Vue, MIT, notice in
  `viewer/vendor/VUE-LICENSE`.

### Fixed

- `takt run` and `takt scenario run` on an empty home (a CI runner, a container, a new machine)
  no longer have their first harness step refused for `~/.local`: the transcripts directory is
  made when the run opens, and a run that cannot make it keeps no transcript and says so on
  standard error.
- With `PYTHONUSERBASE`, a `PYTHONPATH` entry or `XDG_STATE_HOME` placing the transcripts inside a
  Python import directory, no step is refused for the transcripts takt writes; a step that writes
  anything else there still is.
- A file or a link where the transcripts directory goes is named as the transcripts directory,
  not as the set-aside directory, and the directories takt made before such a failure no longer
  get the next step refused.

### Repository

- takt's repository configures the identity its commits carry in
  `.engineering/commit-identity.conf`, and lists `scripts/**` on its judge surface.
- The agent profiles describe one dispatcher session that runs the waves and one trial agent
  that runs the trials (`.agents/dispatcher-agent.md`, `.agents/trial-agent.md`).

### Known limits

- Build-time code (a build script, a procedural macro) is not closed by comparing files:
  `gate.green` does not vouch for it, and the merge of such a change stays the owner's to grant.
- A candidate squashed, rebased or cherry-picked onto `main` before the grant is not seen by the
  ancestry check, so a later merge of the candidate commit can be recorded `merged`
  (story:a-squash-merge-before-the-grant-is-refused).
- A turn records its forge by name only, and the bot route reads whatever `origin` names when it
  is called, so a grant's tip and the merge can be read from two repositories
  (story:a-turn-pins-the-repository-of-its-forge).
- A judge-surface merge refused because its grant records no tip names no way past, and the
  grant given again is refused (story:a-tipless-judge-grant-names-its-way-past).
- A resume that cannot make a candidate deletes the case's candidate ref, and nothing else keeps
  the commit an open turn was opened on, so `git gc` can remove it
  (story:a-candidate-stays-reachable-while-a-turn-names-it).
- `takt view` joins a transcript by path suffix, so an empty or bare `transcript` field can show
  another case's transcript; a page written over an existing file keeps that file's mode, and a
  linked `--output` is written through
  (story:view-joins-own-transcripts-and-writes-a-private-page).
- A step refused for writing `.engineering/commit-identity.conf` is told it wrote into the
  planning store (story:commit-identity-refusal-names-the-identity-file).
- A gate that runs Python through a shell (`sh -c "python3 …"`, or a shell script that runs it)
  has neither the script nor what it imports compared at the base, so a change to them does not
  stop the gate (story:gate-compares-python-run-through-a-shell). The same holds for a module
  `python3 -c` imports from the directory the command runs in.

## [0.3.0] — 2026-10-20

The minor version moves because `takt promote` now refuses scorecards that 0.2.x accepted.

### Changed

- `takt promote` judges a candidate against the latest record and also against the start of
  its series: the latest `--baseline-only` record, or the latest `--rebaseline` under a newer
  metric definition. A scenario worse than either is refused, exit 3, naming the record and the
  scenario. In 0.2.x each promotion was judged only against the one before it, so promotions
  that were each one case worse could add up. A `--rebaseline` that only adds a scenario keeps
  the series start, and the added scenario is judged against that `--rebaseline`
  (story:promotion-judged-against-the-series-baseline).
- `takt promote`, `takt scorecard --against` and `scorecard.compare` cover every scenario
  either side has. Two sides with no scenario case (`no scenario case`) and a scenario only the
  candidate has (`not in the saved scorecard`) are refused, and `scorecard.compare` records
  nothing for them. In 0.2.x both counted as no regression. A new scenario joins the corpus only
  through `takt promote --rebaseline`, with a scorecard that holds every case of the stored one
  and adds the new scenario whole. `--baseline-only` refuses a scorecard with no scenario case
  (story:the-judge-requires-the-whole-corpus).
- A run whose open obligation can only be decided by an action nobody performs in it (`pr.open`
  and the merge under `--forge none`, `gate.green` with no gates document) now stops
  `Unperformable`, exit 7, instead of stopping `NothingAdmissible` with exit 3 or offering the
  model edits until `NoProgress`.
- A wait on something local (a saved scorecard, a promoted binary, a merge into the local
  `main`) stops `Waiting`, exit 6, right after the step that observed it, naming the file it
  reads. 0.2.x polled every 30 s and wrote a decision record for each poll. Waits on the forge
  still poll. Under `--forge none` the merge validators name the local `main` and say nothing
  of a forge (story:a-local-wait-exits-waiting).
- `takt run --install-root` on a resumed turn is accepted when it names the root the turn was
  opened with; another root is still refused. In 0.2.x any `--install-root` on a resume was
  refused.

### Added

- The `Unperformable` stop, `stopped: Unperformable (<claim>; nothing performs it without <the
  forge, the gates document>)`. No judgement step is taken and the case stays open. In a
  scorecard it ranks with the budget stops
  (story:forge-none-stops-when-only-forge-actions-remain,
  story:delivery-refuses-an-obligation-nobody-performs).
- A run's stop line in `events.jsonl` records `completed`, whether its stop is a completion. A
  scorecard takes that word for a stop it does not know, so an unknown stop never counts as
  completed. A line without the field is scored by the stops the reading binary knows.
- `takt run` and `takt scenario run` check free space before every step on the file system of
  the tree and, when `CARGO_TARGET_DIR` is set, of that directory. Under `--min-free-gib`
  (default 10, `0` turns the check off) the run stops `DiskLow`, exit 3, naming the path, its
  free space and the threshold; the case stays open. A scenario with a `DiskLow` trial on
  either side gets no verdict until a fresh set without one is saved
  (story:runs-check-free-disk).
- Each harness step's transcript is kept as
  `$XDG_STATE_HOME/takt/transcripts/<repo>/<case>/run-<r>/step-<n>.jsonl`, outside the case
  directory, mode `0600`, capped at 16 MiB (head and tail kept, the cut marked), also for a step
  that errors. Nothing is overwritten. The step's line in `events.jsonl` names the path relative
  to the transcripts directory, or why none was kept. No validator reads a transcript. Claude
  now runs with `--output-format stream-json --verbose` (story:step-transcripts-are-kept).
- takt's repository has a gates document, `.engineering/gates.yaml`, with four gates
  (specification, citations, format, lint) whose commands run through `scripts/gate-exec`. A
  `story.delivery/1` run on takt can now reach `gate.green` and `pr.open`
  (story:takt-has-a-gates-document).

### Fixed

- `takt scenario run` without the bot route numbers a trial with 22 digits: the instant to the
  second and a random 8-digit nonce, passing over a number the checkout knows. Runs from fresh
  clones of one origin no longer all cut `trial/0`, so their cases go into one scorecard. With
  the bot route the number is still one past the highest `trial/<n>` known
  (story:trial-numbers-are-unique-across-checkouts).
- `takt scorecard --help` names the held-out check, not `events.jsonl` alone, as where a
  scenario run's outcome is read from.

### Repository

- The wall-clock tests that failed only on a loaded host hold under load, each bound still below
  the time its mutant takes (story:timing-tests-hold-under-load).

### Known limits

- `Unperformable` ranks with the budget stops, so a `--forge none` run that stopped `NoProgress`
  under 0.2.x ranks one higher against an older baseline.
- The `lint` gate runs `cargo clippy`, which compiles and runs build code a step wrote; the gate
  cannot vouch for that code, and the merge of such a change stays guarded by the owner.

## [0.2.3] — 2026-10-19

- A Claude step is offered only the tools it may use: `claude` gets `--tools
  Read,Edit,Write,Glob,Grep` beside `--allowedTools`, both from one constant in
  `crates/takt/src/harness.rs` (<https://github.com/beyond10x/takt/pull/3>). Before, Claude Code
  offered all 20 of its built-in tools. Measured on scenario 1 with one fixed prompt, the first
  request of a step dropped from 12,554 prompt tokens to 5,550
  (story:claude-step-offers-tools-it-never-allows).
- `MultiEdit` is no longer in `--allowedTools`: Claude Code 2.1.303 has no such tool.

## [0.2.2] — 2026-10-19

### Improvement turns

- A `self.improvement/1` turn now reaches `accepted` on validators' records:
  `candidate.merge` or `candidate.merge_judge_surface` records the candidate merged,
  `scorecard.compare` no regression, and `binary.promote` the candidate promoted
  (<https://github.com/beyond10x/takt/pull/2>).
- `self.improvement/1` is built in (`--protocol self.improvement/1`, listed by `takt protocols`),
  and a turn opens and resumes only on that embedded document; another document with its id is
  refused. `candidate.deliver` is gone: the candidate is given at open.
- The merge validators record `merged` when the case holds the action's grant and the candidate
  is an ancestor of the mainline, `main` as the forge reports it or the local `main` under
  `--forge none`; they observe the merge and do not make it. A `candidate.merge_judge_surface`
  grant counts only as `repository.merge` does for a judge surface: the owner's (`human:<id>`),
  or a role's that an unrevoked delegation in `.engineering/delegations.yaml` names, read at
  that mainline tip.
- `binary.promote` records `promoted` only from a promotion record of the candidate, no refusal,
  whose `binary_sha256` equals the sha256 of `<install root>/bin/takt`, the install root
  recorded when the turn opened.
- The case input `surface` is computed at open, from the baseline's
  `.engineering/judge-allow.yaml`, over baseline..candidate and each merge base..candidate. A
  given `surface` that differs is refused, naming the first judge-surface path.
- A turn records the sha256 of the binary that opens it, and every resume is refused before any
  step unless the running binary hashes to it. `takt promote` keeps the binary it replaces at
  `$XDG_STATE_HOME/takt/judges/<sha256>` and names it; the turn is finished with that binary.
- `takt run --install-root <dir>` names where a turn reads the promoted binary (else
  `CARGO_INSTALL_ROOT`, then `CARGO_HOME`, then `~/.cargo`). It is recorded canonical at open;
  a root that is missing or no directory opens no turn, and a resumed case refuses the option.
- A turn opened without `--intent` takes its finding's title as its intent. The finding is read
  on the mainline, so one filed after the last promotion opens a turn. A turn writes nothing to
  the planning store, and `takt approve` on a turn writes the case's grant only.

### Run loop

- `--gate` defaults to the protocol's first action that requires a capability and that the
  case's inputs do not block for good: `repository.merge` for a delivery or a software change,
  `candidate.merge` or `candidate.merge_judge_surface` for a turn.
- A step's line in `events.jsonl`, its tokens included, is written before the revision update
  that can end the run.

### Findings

- `scripts/check-finding` runs git with `GIT_NO_REPLACE_OBJECTS=1`, so a replace ref in a
  scenario checkout cannot stand in for a seed.

### Repository

- The loop-case test's forged-commit precondition no longer depends on the host's commit-graph
  or gc settings.

### Known limits

- A turn's judge check is the running binary checking itself: a binary built without it would
  resume the turn and judge it.
- Under `--forge none` the operator's local `main`, and the `.engineering/delegations.yaml` it
  holds, are trusted: whoever moves `main` decides what counts as merged.

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
