---
title: Run a story
sidebar_position: 3
description: Prepare a repository for story delivery, let takt take the next ready story or the one you name, follow its steps to a pull request, and grant the merge.
lede: takt delivers one story of the planning store per case, through tests and gates it runs itself, and the merge waits for your grant.
source: takt run --help and takt approve --help at 0.14.0; crates/takt/src/frontier.rs; crates/takt/src/forge/local.rs; crates/takt/src/run_config.rs (local_checks); crates/takt-cli/tests/local_forge_cli.rs; crates/takt/src/console.rs (reads, diff, harness_figures, LINE_WIDTH, READS_SHOWN, DIFF_SHOWN), run.rs (said_line, committed_diff, the harness step line, candidate, CANDIDATE_REFS); crates/takt/tests/console_reads_and_diffs.rs; crates/takt/tests/w44_adv_console_listing.rs; crates/takt-cli/src/main.rs (LoopArgs, console_of); CHANGELOG.md 0.21.0; engineering-protocols 0.13.0, protocols/story-delivery/2.yaml; crates/takt/src/protocol_defaults.yaml; metaharness 0.23.0 (PINNED_VERSIONS); docs/runs/2026-10-29-self-run-story-delivery.md; a story run of takt 0.14.0 with Codex and --forge none on a scratch repository, 2026-10-26; the aep commands of step 1 run with aep 0.78.0 in a fresh repository, 2026-10-27; the local forge steps run with a build of takt 0.21.0 on the fixture of local_forge_cli.rs, 2026-10-29; docs/design/takt.md § 4
---

# Run a story

A story run takes a story from the repository's
[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
planning store and delivers it on the protocol `story.delivery/3`, takt's own revision of
`story.delivery/2`, which hands a change over only while its tests pass ([Protocol and case](../concepts/protocol-and-case.md#the-protocols-takt-runs)). takt does
not write or rank stories; AEP keeps them, and `aep` is the tool you use to add one.

The examples use the default harness, Claude Code. Codex and b10x-harness are set up in
[Configure a run](./configure-a-run.md#2-pick-the-harness).

## 1. Prepare the repository

| file | what it is for |
|---|---|
| `.engineering/project.yaml` and `.engineering/planning/` | the planning store: `aep plan reverse init` writes the first, `aep plan artifact new` the second |
| `.engineering/gates.yaml` | the gates document the gate step runs |
| `.engineering/assertions-project.yaml` | the commands the gates may start |
| `.engineering/commit-identity.conf` | the author of takt's commits |
| `Cargo.lock`, committed | so a test run writes no file |

Without a gates document, a story cannot get past the gate: the run stops `Unperformable` on
`gate.green`, exit 7. A gate's commands run with the environment cleared. A bare program name such
as `cargo` is found on `/usr/local/bin:/usr/bin:/bin`, never on your `PATH`; a command whose `env`
sets `PATH` finds its program on that `PATH` and runs with it. A program that lives elsewhere, such
as rustup's `cargo` in `~/.cargo/bin`, needs that directory in the command's `PATH`, or its absolute
path. A name found nowhere refuses the gate, naming the program and the path searched. This one
checks the formatting:

```yaml title=".engineering/gates.yaml"
format: engineering-gates/1
language: canon-expr/1
gates:
  - id: format
    text: The crate is formatted.
    assertions:
      - 'process.succeeded("format")'
```

```yaml title=".engineering/assertions-project.yaml"
format: engineering-assertions-project/1
timeout_seconds: 300
commands:
  format:
    executable: cargo
    args:
      - fmt
      - --check
```

The commit identity is git-config format:

```ini title=".engineering/commit-identity.conf"
[identity]
	name = Demo Bot
	email = demo-bot@example.com
```

Then make the repository an AEP project, add a story and make it ready. `aep plan reverse init`
writes `.engineering/project.yaml`; `--protocols` is a pinned source of the governing documents,
here the one takt's own store names. A story is ready when it is `proposed`, nothing blocks it
and its dependencies are done. Its body is a Markdown file of your own:

```md title="story-body.md"
## Acceptance

- `sub(5, 2)` returns 3, and a test says so.
```

```sh
aep plan reverse init --profile development.standard --planning-scope my-repo \
  --protocols 'git+https://github.com/beyond10x/aep#6d7a44d3607d2d9a6ffdf0a165993c546c43d0db'
aep plan artifact new story add-sub --title "Add a subtraction" --summary "sub(left, right) subtracts right from left"
aep plan artifact body story:add-sub --from story-body.md && rm story-body.md
aep plan artifact move story:add-sub --to proposed
git add -A
git commit -m "my-repo: a crate, its gates and its first story"
```

The tree must be clean when the run starts.

## 2. Run it

```sh
takt run --repo . --forge none --hours 1
```

With no `--harness`, Claude Code takes the judgement steps: `claude` on your `PATH`, logged in.
Before the first session takt asks metaharness whether that binary qualifies. One that does not
stops the run `HarnessUnqualified`, exit 1, and the stop names the versions that do; the
metaharness in takt 0.21.0 is pinned to Claude Code 2.1.304, 2.1.305 and 2.1.309. Name a
qualified binary with `--harness-program <absolute path>`.

With no `--intent` and no `--case`, takt resumes the open case of a story already being worked,
or else takes the next ready story. To pick the story yourself:

```sh
takt run --repo . --story story:add-sub --forge none --hours 1
```

A story the store does not list, or one that is not ready, is refused before any case opens, and
the tree stays on the branch it was on:

```console
$ takt run --repo . --story story:nope --harness none --forge none
takt: --story story:nope: the store lists no story:nope; the frontier would not take it, so no case was opened
```

## 3. Follow the steps

A run on `~/my-repo`, recorded on takt 0.14.0 with Codex and `story.delivery/1`, the `frontier:`
lines left out. Under Claude Code a harness step names `claude-code` where this one names
`codex`, and from 0.19.0 the console reads as in [The console](#the-console) below:

```console
frontier: took story:add-sub: Add a subtraction
case story-add-sub-20261026T231852Z on story.delivery/1 in ~/my-repo; inputs none; confinement none
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
step 2: gate.run {}
  effect: the gates document .engineering/gates.yaml from main at e4f3d09f, pinned at open: pass; every gate true: format; retained as gate-result-2.bundle.json
  artifact: .takt/cases/story-add-sub-20261026T231852Z/artifacts/gate-result-2.bundle.json
  evidence: gate_result pass
step 3: implementation.edit (codex)
  said: Added public sub(left, right) and a unit test in src/lib.rs asserting sub(5, 2) returns 3; tests left to takt. [metaharness hermetic on: H1a unk, H1b ok, H2 ok, H3 ok, H4 unk, H5 unk, H6 ok, H7 ok, H8 ok, H9 ok, H10 ok, H11 ok]
  effect: committed 1 path(s); implementation is now b09390e9
step 4: tests.run {}
  effect: the test command exited 0: pass; 2 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
step 5: gate.run {}
  effect: the gates document .engineering/gates.yaml from main at e4f3d09f, pinned at open: pass; every gate true: format; retained as gate-result-5.bundle.json
  artifact: .takt/cases/story-add-sub-20261026T231852Z/artifacts/gate-result-5.bundle.json
  evidence: gate_result pass
step 6: change.handover (codex)
  said: The public sub function and unit test satisfy the story; no further edits are needed. Hand over for pull request creation and CI. [metaharness hermetic on: H1a unk, H1b ok, H2 ok, H3 ok, H4 unk, H5 unk, H6 ok, H7 ok, H8 ok, H9 ok, H10 ok, H11 ok]
  effect: no file changed
  evidence: handover -
step 7: change.read {}
  effect: merging b09390e9 into main at e4f3d09f, its tip now, changes its tree outside .engineering/: ahead
  evidence: change_record ahead
candidate: d3f99cd08b31da20bbc26741779be713237909d2
stopped: Unperformable (pr.opened; nothing performs it without the forge); steps 7, tokens 75939, case ~/my-repo/.takt/cases/story-add-sub-20261026T231852Z
```

1. The frontier took the story and committed the take on the case's branch,
   `takt/<case>`. The store now lists the story `active`.
2. The tests and the gate ran on the base, as validators. Only a validator writes evidence.
3. The model edited the code, one action per step. takt committed the change. On the default
   `story.delivery/3` the model writes the story's tests first and the implementation after
   ([After a red gate](#after-a-red-gate)).
4. The tests and the gate ran again on the new revision.
5. The model handed the change over, and `change.read` found it ahead of `main`.
6. The next claim is `pr.opened`. With `--forge none` nothing opens a pull request, so the run
   stopped `Unperformable`, exit 7. Before it stopped, it printed the candidate: the code commits
   replayed on `main`'s tip under the commit identity, kept in the repository at
   `refs/takt/candidates/<case>`. Push it to a branch of your own and open the pull request
   yourself, or run again with a forge ([4. With a forge](#4-with-a-forge)):

   ```sh
   git push origin refs/takt/candidates/<case>:refs/heads/<branch>
   ```

### The console

From 0.19.0 a run's console opens with the story: `takt <version> · <story id>`, the story's
title, and up to three lines of the first paragraph of its `## Outcome`. A line names the
harness and the confinement. The frontier is said as what the run may do, `may:` once and then
`may now: +<added> -<removed>` when that changes; `--verbose` adds the full `frontier:` line at
each decision. Each step is one line: a validator's result, the failed tests or gates it names,
and its duration; a harness step's duration, tokens, the cached input tokens among them, and its
cost when the harness reports one. Before a harness step a `working` line names what it may do.
The harness's hermetic verdict is printed once with what the harness said, and again only when
it changes.

#### What a step lists

From 0.21.0 a harness step shows what the model did, so you see what it read, ran and committed
without opening the case. Under the step's `said:` line come its tool calls, in order: a shell
command as `$ <command>`, a file tool with its path or pattern (`Read src/lib.rs`, `Grep fn sub in
src`, `apply_patch src/lib.rs`). Before its `effect:` line comes the diff of its commit, outside
`.engineering/` and `.takt/`. A Claude Code step that ran two commands and added one function:

```text
step 3: implementation.edit (claude-code) · <duration> · <tokens> tok (<cached> cached)
  said: added sub
  $ cat src/lib.rs
  $ git status --short
    diff --git a/src/lib.rs b/src/lib.rs
    index <before>..<after> 100644
    --- a/src/lib.rs
    +++ b/src/lib.rs
    @@ -1 +1,2 @@
     pub fn add() {}
    +pub fn sub() {}
  effect: committed 1 path(s); implementation is now <revision>: src/lib.rs
```

The lines from `said:` to the diff are what `crates/takt/tests/console_reads_and_diffs.rs` and
the unit tests of `crates/takt/src/console.rs` check; the header and the `effect:` line are the
formats of `crates/takt/src/run.rs`, with the figures a real step prints written as `<…>`. The
listing stops after 20 tool calls with `… <n> more`, and the diff after 120 lines with
`… <n> more lines (git show <sha>)`. Each listed line is escaped and cut to the terminal width
([Console](../reference/configuration.md#console)).

| flag | what it does | example |
|---|---|---|
| `--color <WHEN>` | `auto`, `always` or `never`: colours the console, the action bold, a validator's pass green and its fail red, the labels and tool calls dim, the diff's added lines green and removed lines red. `auto`, the default, colours a terminal unless `NO_COLOR` is set and not empty; `always` colours a pipe too. No colour reaches the case's records | `takt run --repo . --forge none --color never` |
| `--quiet-steps` | lists neither the tool calls nor the diff; every other line stays as it was | `takt run --repo . --forge none --quiet-steps` |

#### A run on takt's own repository

The opening of a run on takt's own repository, with Codex, from a development build that preceded
0.19.0 and printed `takt 0.18.0` (the record is
[`docs/runs/2026-10-29-self-run-story-delivery.md`](https://github.com/beyond10x/takt/blob/main/docs/runs/2026-10-29-self-run-story-delivery.md),
run 15; the tree's path is written `<tree>` and the `case` line is cut at `…`):

```console
frontier: took story:duplicate-key-reader-limits: Three duplicate keys the YAML reader does not find at their line
settings: protocol story.delivery/3 (flag); tests_first=required (flag); gate_on=after_handover (flag)
takt 0.18.0 · story:duplicate-key-reader-limits
  Three duplicate keys the YAML reader does not find at their line
  Each of the three is refused at the duplicate's line, saying a key is held twice, and naming the
  key when it is a declared name.
case story-duplicate-key-reader-limits-20261029T013214Z on story.delivery/3 in <tree>; inputs tests_first=required(flag) gate_on=after_handover(flag); confinement none: …
  harness codex 0.165.0 · confinement none
harness: codex 0.165.0; advisory: H9 gap: the vendor is 0.165.0 and the adapter is pinned to ["0.153.4", "0.160.0"]; a verdict that changed because the reader changed must be visible as such
  may: change.handover repository.inspect tests.run tests.write
step 1: tests.run {} · pass · 14.8s
  effect: the test command exited 0: pass; 19 passed, 0 failed, 3 ignored over 3 result lines
  evidence: test_result pass
step 2: codex working (may: change.handover tests.write)
step 2: tests.write (codex) · 9.9s · 103.4k tok (88.3k cached)
  said: Enable three duplicate-key regression cases [metaharness hermetic on: H1a unk, H1b unk, H2 ok, H3 ok, H4 unk, H5 unk, H6 ok, H7 ok, H8 ok, H9 gap, H10 ok, H11 ok]
  effect: committed 1 path(s); implementation is now 28f82ddc: crates/takt/tests/w40_adv_dup_and_pin.rs
  evidence: tests_change -
```

The gate failed on format at step 8, the edit at step 9 fixed it, and the gate passed at step 13.
With `--forge none` the run then stopped at the pull request:

```console
step 13: gate.run {} · pass · 11.6s
  effect: the gates document .engineering/gates.yaml from wave/40 at 8b8ac758, pinned at open: pass; every gate true: specification, citations, changelog, format, lint; retained as gate-result-13.bundle.json
  artifact: .takt/cases/story-duplicate-key-reader-limits-20261029T013214Z/artifacts/gate-result-13.bundle.json
  evidence: gate_result pass
  may now: +pr.open
candidate: d8ffdee5e480c9c893ea4ab71ef3fd2b7bbffd28
stopped: Unperformable (pr.opened; nothing performs it without the forge); steps 13, tokens 1087139, wall 4m07s, commits 3 (3 paths), case <tree>/.takt/cases/story-duplicate-key-reader-limits-20261029T013214Z
```

### After a red gate

A story opens on `story.delivery/3` with `tests_first=required` and `gate_on=after_handover`
unless you set them ([Protocol defaults](../reference/protocol-defaults.md)). Under
`tests_first=required` the model may edit the implementation (`implementation.edit`) in two cases
only:

1. **The tests are red.** The story's tests were written, and they fail at the current revision.
2. **The tests are green and the gate or CI is red.** The story's tests were written, and at the
   current revision they pass and the gate or continuous integration failed. This is how a
   failure no test reads, such as formatting, lint or another platform, gets fixed. Under
   `story.delivery/2` the tests must also have been seen failing after their latest write
   ([When an edit is admitted](../concepts/protocol-and-case.md#when-an-edit-is-admitted)).

In any other state `implementation.edit` is not among what the step may do. Under
`gate_on=after_handover` the gate runs only on a revision whose tests pass and whose change was
handed over, so a red gate comes after a handover; the fix is a new commit, the model hands over
again, and the gate runs again. Three gate runs in a row that fail the same gates, each at a new
revision, stop the run `RepeatedGateFailure`, exit 3
([The gate stop](../concepts/how-a-run-ends.md#the-gate-stop)).

`--input tests_first=optional` lifts the rule for a case: the implementation may then be edited
at any time.

## 4. With a forge

With `--forge gitlab`, `--forge command`, `--forge connectors` or `--forge local`, the steps go on from `change.read`:

1. `pr.open` pushes the case's branch and opens the pull request.
2. `ci.read` reads its checks, waiting while they run. A run whose budget ends while it waits
   stops `Waiting`, exit 6, and the next run reads them again.
3. With CI green, `repository.merge` needs a grant: the run stops `ApprovalRequired`, exit 4.
4. After the grant, the next run commits the evidence and the story's move to `implemented` on
   the case's branch, then merges, so both reach the base with the merge. It stops `Completed`,
   exit 0.

### On this machine: the local forge

The local forge runs the whole path, pull request, checks and merge, on one machine, with no
account on a hosted forge. From 0.20.0 `--forge local` takes a bare repository on this machine
as the forge, the hub. The pull request is a request record in the hub, the checks are commands
you name, and the merge goes into a branch of the hub. Nothing leaves the machine. Every rule is
in [The local forge](../reference/configuration.md#the-local-forge).

No record yet shows a harness delivering through the local forge. The one record is an
end-to-end test with `--harness none`
([`crates/takt-cli/tests/local_forge_cli.rs`](https://github.com/beyond10x/takt/blob/main/crates/takt-cli/tests/local_forge_cli.rs),
`a_granted_case_merges_into_the_hub_and_records_pr_merged`). Try the steps below with
`--harness none` first, as the test does, and then with a model.

1. Make the hub a bare clone of the prepared repository, and the tree a clone of the hub with
   `--no-local`. A plain `git clone` of a local path shares object files with the hub, and takt
   refuses such a tree before the case opens:

   ```sh
   git clone --bare my-repo hub.git
   git clone --no-local hub.git tree
   ```

   The tree's `origin` is now the hub. The commit identity comes from
   `.engineering/commit-identity.conf`, committed in step 1, or from `--commit-identity`; the
   merge commit in the hub is made under it, and without one the run is refused.

2. Write the run file outside the tree, so the tree stays clean. `local_checks` is run as the test
   command is, without a shell:

   ```yaml title="run.yaml"
   forge: local
   local_checks: [cargo, test, --quiet]
   ```

   `local_checks` is required. A run file with `forge: local` and no `local_checks` is refused
   before any case opens, exit 1:

   ```text
   `forge: local` runs the checks a `local_checks` argument list in the run file names, and there is none: no check is never a pass; nothing was opened
   ```

3. Run the story. With a model:

   ```sh
   cd tree
   takt run --repo . --story story:add-sub --config ../run.yaml --hours 1
   ```

   With `--harness none`, as the test does, you make the change yourself. The first run opens
   the case, takes no step and stops `Iterations (0)`, exit 3; it leaves the tree on the case's branch, `takt/<case>`. Commit
   your change there, and the second run resumes the case:

   ```sh
   cd tree
   takt run --repo . --story story:add-sub --harness none --protocol story.delivery/1 \
     --config ../run.yaml --iterations 0
   git add -A && git commit -m "add sub"
   takt run --repo . --harness none --protocol story.delivery/1 --config ../run.yaml
   ```

   After `change.read`, `pr.open` pushes the case's branch to the hub and writes
   `<hub>/takt/requests/<n>.json`, `ci.read` runs `local_checks` on the head in a fresh clone,
   and with the checks passing the run stops `ApprovalRequired (repository.merge)`, exit 4.

   What you will see: the second run on the test's fixture, its story `story:ready-one`, with
   `true` as the test command and as `local_checks`, run with a build of takt 0.21.0. The
   scratch directory is written `<dir>`, and the lines between the `forge:` line and step 1
   (the story, the case, the harness and `may:`) are left out:

   ```console
   forge: local (<dir>/hub.git); pr.open, ci.read and repository.merge go through the hub on this machine, the checks are the run file's local_checks, run as the test command is
   step 1: tests.run {} · pass · 0.2s
     effect: the test command exited 0: pass; 
     evidence: test_result pass
   step 2: gate.run {} · pass · 1.7s
     effect: the gates document .engineering/gates.yaml from main at 734efe06, pinned at open: pass; every gate true: manifest; retained as gate-result-3.bundle.json
     artifact: .takt/cases/story-ready-one-20261029T111142Z/artifacts/gate-result-3.bundle.json
     evidence: gate_result pass
   step 3: none working (may: change.handover implementation.edit tests.write)
   step 3: change.handover (none)
     said: no harness: the work committed is taken as it stands
     effect: no file changed
     evidence: handover -
     may now: +change.read -change.handover
   step 4: change.read {} · 0.0s
     effect: merging 736e5c7c into main at 734efe06, its tip now, changes its tree outside .engineering/: ahead
     evidence: change_record ahead
     may now: +pr.open
   step 5: pr.open {} · 0.1s
     effect: pushed takt/story-ready-one-20261029T111142Z at 736e5c7c through local and opened pull request #1: open
     evidence: pull_request open
     may now: +ci.read
   step 6: ci.read {} · pass · 0.2s
     effect: checks on pull request #1 at 736e5c7c through local: pass
     evidence: ci_result pass
   stopped: ApprovalRequired (repository.merge); steps 6, tokens 0, wall 3.7s, commits 0, case <dir>/repo/.takt/cases/story-ready-one-20261029T111142Z
   ```

4. Grant the merge on the case the `case` line named, then resume it:

   ```sh
   takt approve --repo . <case> repository.merge --by human:<name>
   takt run --repo . --case <case> --config ../run.yaml --hours 1
   ```

   The run merges the case's head into the hub's `main`, a merge commit under the run's commit
   identity, and records `pr.merged`. It stops `Completed`, exit 0. `git -C ../hub.git log -1 main`
   shows the merge.

   What you will see on the same fixture, granted by `human:operator` (the resume's opening
   lines are left out, as above):

   ```console
   approved: repository.merge for case story-ready-one-20261029T111142Z by human:operator, written to story-ready-one-20261029T111142Z/authority.yaml
   recorded: approval on story:ready-one in the planning store
   ```

   ```console
   store: test_result recorded on story:ready-one at 2026-10-29T11:11:45Z from .takt/cases/story-ready-one-20261029T111142Z/evidence/0001-test-result-2.json#sha256=4578cf635c54ab2f5a915302fdab52f564d86c6a29c621c8455fadb2c591d17c&result=pass&revision=736e5c7cb68358e77a81320b2dd544344c8ef0e4&producer=takt.tests-run (committed 75b408a8 on takt/story-ready-one-20261029T111142Z)
   store: story:ready-one moved to implemented (committed ba54bd63 on takt/story-ready-one-20261029T111142Z)
   step 1: repository.merge {} · 0.7s
     effect: merged pull request #1 at 736e5c7c through local: merged; its head ba54bd63 carries takt's records above it
     artifact: .takt/cases/story-ready-one-20261029T111142Z/artifacts/merged-head-9.json
     evidence: merge_observation merged
   stopped: Completed (accepted); steps 1, tokens 0, wall 1.6s, commits 0, case <dir>/repo/.takt/cases/story-ready-one-20261029T111142Z
   $ git -C ../hub.git log -1 --format='%h %s (%an)' main
   dd30f6b Merge #1: Make the check pass (Test Bot)
   ```

## Approve and resume

```sh
takt approve --repo . <case> repository.merge --by human:<you>
takt run --repo .
```

`takt approve` writes the grant into the case's `authority.yaml` and records it on the story in
the planning store. That record is committed on the case's branch, so the tree must be on that
branch and clean. The next `takt run` resumes the case and merges under the grant.

A grant binds to the revision it was given at, so a commit after the grant needs a new one.
Above that revision only takt's own records may ride to the base: one approval per grant, one
`test_result` and the move to `implemented`. A case that is not yet stopped for the grant is
refused ([Approve a capability](./approve-a-capability.md)).

To deliver a known story on a GitHub repository, as takt's own trials do, see
[A trial on a GitHub repository](./scenario-run.md#a-trial-on-a-github-repository).

## Bounds and resuming after a stop

`--iterations` (steps), `--hours`, `--tokens` and `--active-seconds` bound the run. A run that
names no token budget, by `--tokens` or the run file's `tokens`, stops at 3,000,000 tokens,
exit 3.

| stop | exit | the next run |
|---|---|---|
| `ApprovalRequired` | 4 | merges once a grant is recorded |
| `Waiting` | 6 | reads the forge again |
| an error once the case was open ([Error](../concepts/how-a-run-ends.md#error-exit-5)) | 5 | resumes the case |
| `Unperformable` | 7 | resumes the case; give it what was missing (a forge, a gates document) |
| a budget, `StepRefused`, `NoProgress`, `NothingAdmissible` | 3 | resumes the case |
| `RedBase`, `RepeatedTestFailure` ([what counts as an attempt](/docs/concepts/how-a-run-ends#what-counts-as-an-attempt)) | 3 | runs the tests again first and stops again while they fail the same way |
| `RepeatedGateFailure` (0.19.0): three gate runs in a row, each at a new revision, failed the same gates | 3 | resumes the case; the row is read from the case's record, so the next gate run that fails the same gates stops it again |

`--case <id>` resumes a named case instead. Every stop is explained in
[How a run ends](../concepts/how-a-run-ends.md). Where a case's time and tokens went:
[Read a run report](./read-a-run-report.md).

## What is not proven yet

The design's proof for a story, one ready story merged with no manual step but the merge grant,
has not been recorded. The one recorded merge of a scenario, on 2026-10-18, took 4 invocations
where 2 were the target.

On 2026-10-29 takt delivered one of its own stories up to the pull request: five runs in a row on
`story.delivery/3`, with Codex and `--forge none`, each passed the story's tests and the
repository's five gates, handed the change over and stopped `Unperformable (pr.opened)`, exit 7.
No run opened a pull request, read its CI or merged it, so delivery with a forge on takt's own
repository is not proven
([run record](https://github.com/beyond10x/takt/blob/main/docs/runs/2026-10-29-self-run-story-delivery.md)).

For the local forge the one record is the end-to-end test
[above](#on-this-machine-the-local-forge): the binary opens the request, runs the checks, stops
for the grant and merges into the hub on a fixture, with no harness.
