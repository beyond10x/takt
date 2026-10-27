---
title: Run a story
sidebar_position: 3
description: Prepare a repository for story delivery, let takt take the next ready story or the one you name, follow its steps to a pull request, and grant the merge.
lede: takt delivers one story of the planning store per case, through tests and gates it runs itself, and the merge waits for your grant.
source: takt run --help and takt approve --help at 0.14.0; crates/takt/src/frontier.rs; crates/takt/src/case.rs (approvals_only); a story run of takt 0.14.0 with Codex and --forge none on a scratch repository, 2026-10-26; the aep commands of step 1 run with aep 0.78.0 in a fresh repository, 2026-10-27; docs/design/takt.md § 4
---

# Run a story

A story run takes a story from the repository's
[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
planning store and delivers it on the protocol `story.delivery/1`. takt does not write or rank
stories; AEP keeps them, and `aep` is the tool you use to add one.

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
takt run --repo . --harness codex --forge none --hours 1
```

With no `--intent` and no `--case`, takt resumes the open case of a story already being worked,
or else takes the next ready story. To pick the story yourself:

```sh
takt run --repo . --story story:add-sub --harness codex --forge none --hours 1
```

A story the store does not list, or one that is not ready, is refused before any case opens, and
the tree stays on the branch it was on:

```console
$ takt run --repo . --story story:nope --harness none --forge none
takt: --story story:nope: the store lists no story:nope; the frontier would not take it, so no case was opened
```

## 3. Follow the steps

The run on `~/my-repo`, with the `frontier:` lines left out:

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
3. The model edited the code, one action per step. takt committed the change.
4. The tests and the gate ran again on the new revision.
5. The model handed the change over, and `change.read` found it ahead of `main`.
6. The next claim is `pr.opened`. With `--forge none` nothing opens a pull request, so the run
   stopped `Unperformable`, exit 7. Before it stopped, it printed the candidate: the code commits
   replayed on `main`'s tip under the commit identity, kept at `refs/takt/candidates/<case>`.
   Merge or push it yourself, or run again with a forge.

## 4. With a forge

With `--forge bot` or `--forge gitlab`, the steps go on from `change.read`:

1. `pr.open` pushes the case's branch and opens the pull request.
2. `ci.read` reads its checks, waiting while they run. A run whose budget ends while it waits
   stops `Waiting`, exit 6, and the next run reads them again.
3. With CI green, `repository.merge` needs a grant: the run stops `ApprovalRequired`, exit 4.
4. After the grant, the next run commits the evidence and the story's move to `implemented` on
   the case's branch, then merges, so both reach the base with the merge. It stops `Completed`,
   exit 0.

### A trial on a GitHub repository

A trial that should end `Completed`, exit 0, with a merged pull request needs two things a plain
`--forge none` run does not give it:

- **The forge.** A repository on github.com takes `--forge bot`. Under `--forge none` the run
  stops `Unperformable (pr.opened)`, exit 7, however good the change is.
- **A qualified Claude Code binary.** The metaharness takt builds on qualifies Claude Code
  2.1.304, 2.1.305 and 2.1.309. A newer `claude` on `PATH` has no memory canary, so a hermetic
  run holds its first proposal after the session has been paid for. Name a qualified binary with
  `--harness-program <absolute path>`; `opened` records the path, its sha256 and its
  `--version` line.

  Claude Code's updater keeps its versions under `~/.local/share/claude/versions/` and prunes old
  ones, so do not point a trial there. Copy the qualified version once to a stable path of your
  own, outside the updater's directory, and note its sha256:

```sh
install -D -m 755 ~/.local/share/claude/versions/2.1.309 ~/.local/opt/claude-2.1.309/claude
sha256sum ~/.local/opt/claude-2.1.309/claude
takt run --repo . --story story:add-sub --harness claude \
  --harness-program ~/.local/opt/claude-2.1.309/claude --forge bot --hours 1
```

After the run, compare the sha256 the case's `opened` record names with the one you noted: a
different value means the run did not start the binary you qualified.

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

## Bounds and resuming after a stop

`--iterations` (steps), `--hours`, `--tokens` and `--active-seconds` bound the run.

| stop | exit | the next run |
|---|---|---|
| `ApprovalRequired` | 4 | merges once a grant is recorded |
| `Waiting` | 6 | reads the forge again |
| an error once the case was open; a judgement step whose session ends without the line naming its action is tried 3 times first, each try a step that `--iterations` counts (a budget a try spent is exit 3) | 5 | resumes the case |
| `Unperformable` | 7 | resumes the case; give it what was missing (a forge, a gates document) |
| a budget, `StepRefused`, `NoProgress`, `NothingAdmissible` | 3 | resumes the case |
| `RedBase`, `RepeatedTestFailure` | 3 | runs the tests again first and stops again while they fail the same way |

`--case <id>` resumes a named case instead. Every stop is explained in
[How a run ends](../concepts/how-a-run-ends.md).

## What is not proven yet

The design's proof for a story, one ready story merged with no manual step but the merge grant,
has not been recorded. The one recorded merge of a scenario, on 2026-10-18, took 4 invocations
where 2 were the target.
