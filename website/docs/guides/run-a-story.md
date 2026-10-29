---
title: Run a story
sidebar_position: 3
description: Prepare a repository for story delivery, let takt take the next ready story or the one you name, follow its steps to a pull request, and grant the merge.
lede: takt delivers one story of the planning store per case, through tests and gates it runs itself, and the merge waits for your grant.
source: takt run --help and takt approve --help at 0.14.0; crates/takt/src/frontier.rs; crates/takt/src/console.rs, run.rs (said_line, gate_stop); docs/runs/2026-10-29-self-run-story-delivery.md; crates/takt/src/case.rs (approvals_only); a story run of takt 0.14.0 with Codex and --forge none on a scratch repository, 2026-10-26; the aep commands of step 1 run with aep 0.78.0 in a fresh repository, 2026-10-27; docs/design/takt.md § 4
---

# Run a story

A story run takes a story from the repository's
[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
planning store and delivers it on the protocol `story.delivery/2`, or, with `--protocol
story.delivery/3`, on takt's own revision of it, which hands a change over only while its tests
pass ([Protocol and case](../concepts/protocol-and-case.md#the-protocols-takt-runs)). takt does
not write or rank stories; AEP keeps them, and `aep` is the tool you use to add one.

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

The run on `~/my-repo`, on takt 0.14.0 and `story.delivery/1`, with the `frontier:` lines left
out (from 0.19.0 the console reads as in [The console](#the-console) below):

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

The opening of a run on takt's own repository, from a development build that preceded 0.19.0 and
printed `takt 0.18.0` (the record is
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

A trial is one delivery of a known story on
[todo-app-example](https://github.com/beyond10x/todo-app-example), the scratch repository takt's
live trials run against, ending `Completed`, exit 0, with its pull request merged. Each trial is
one `takt scenario run` of scenario 1 (`story:remove-a-todo`, from the seed
`.engineering/scenarios.conf` on its `main` declares): the run cuts a fresh base branch
`trial/<n>` from the seed, opens the case `scenario-1-trial-<n>` on `story.delivery/1`, and its
pull request targets `trial/<n>` ([Scenario run](./scenario-run.md)). A second trial is another
`takt scenario run`, which takes the next `<n>`.

What a trial needs that a plain run does not:

- **takt 0.15.1.** The first release with `--harness-program` and the harness preflight; the steps
  below install it from the release archive ([Install](./install.md)).
- **The forge.** `--forge bot`. Under `--forge none` the run stops `Unperformable (pr.opened)`,
  exit 7, however good the change is. The bot route needs `b10x-gates` on `PATH`, acting as the
  bot's GitHub App; its policy file, mode 600, at `~/.config/b10x/gates/policy.json` or where
  `B10X_GATES_POLICY` names it (with neither, `b10x-gates` refuses: `protected file ... unavailable`);
  and `gh` logged in to read the pull request. todo-app-example need not be listed in the policy's
  repositories: the scan and the bot route serve it unlisted.
- **A commit identity.** The repository has no `.engineering/commit-identity.conf`, and the bot
  route refuses takt's default identity before the case opens: give `--commit-identity`.
- **The test command.** The repository's tests are `./check.sh`, not `cargo test`: give
  `--test-cmd ./check.sh`.
- **A qualified Claude Code binary.** The metaharness takt builds on qualifies Claude Code
  2.1.304, 2.1.305 and 2.1.309. Before the first session takt asks metaharness about the binary
  it would start; one that does not qualify stops the run `HarnessUnqualified`, exit 1, naming
  the versions that do. Name a qualified binary with `--harness-program <absolute path>`;
  `opened` records the path, its sha256 and its `--version` line.

  Claude Code's updater keeps its versions under `~/.local/share/claude/versions/` and prunes old
  ones, so do not point a trial there. Copy the qualified version once to a stable path of your
  own, outside the updater's directory, and note its sha256.

Each trial runs in a fresh clone of its own, trials 2 and 3 included: `takt scenario run` needs a
clean tree, and a trial leaves its clone on the case's branch with the case directory in it. The
clone's `origin` is the repository on github.com, which the bot route serves. Install takt and the
qualified binary once, then clone for each trial (`trial-1`, `trial-2`, `trial-3`):

```sh
version=0.15.1 target=x86_64-unknown-linux-gnu
release=https://github.com/beyond10x/takt/releases/download/$version
curl -fsSLO "$release/takt-$version-$target.tar.gz"
curl -fsSLO "$release/SHA256SUMS"
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf "takt-$version-$target.tar.gz"
install -D -m 755 "takt-$version-$target/takt" ~/.local/bin/takt
takt --version
install -D -m 755 ~/.local/share/claude/versions/2.1.309 ~/.local/opt/claude-2.1.309/claude
sha256sum ~/.local/opt/claude-2.1.309/claude
git clone https://github.com/beyond10x/todo-app-example trial-1 && cd trial-1
takt scenario run --repo . 1 --harness claude \
  --harness-program ~/.local/opt/claude-2.1.309/claude --forge bot \
  --commit-identity 'takt-trial <takt-trial@example.com>' --test-cmd ./check.sh --hours 1
```

The first line names the case, `scenario-1-trial-<n>`. The run goes on until it needs something:

| stop | exit | what to do |
|---|---|---|
| `Waiting` | 6 | the pull request's checks are still running: run the resume command below again |
| `ApprovalRequired (repository.merge)` | 4 | grant the merge, then resume |
| `NoProgress` | 3 | the merge was refused: read the last `repository.merge` evidence; a missing delegation reads as below |
| `HarnessUnqualified` | 1 | the binary is not a qualified version: name one and run again |
| `Completed` | 0 | the pull request is merged into `trial/<n>`: the trial is done |

Grant and resume in the same clone. The run that stopped `ApprovalRequired` left the clone on the
case's branch, `takt/scenario-1-trial-<n>` (`git branch --show-current` prints it), and clean;
`takt approve` commits the grant on that branch, and refuses a tree on another branch or with uncommitted work:

```sh
takt approve --repo . scenario-1-trial-<n> repository.merge --by agent:takt-trial
takt run --repo . --case scenario-1-trial-<n> --harness claude \
  --harness-program ~/.local/opt/claude-2.1.309/claude --forge bot --test-cmd ./check.sh --hours 1
```

`--by` names who grants: `agent:takt-trial`, the role the seed delegates the merge to (below), for
the agent that runs the trial, or `human:<name>` for a person. The case keeps the commit identity it was opened with, so the resume does not give it.

**An agent's grant needs a delegation at the base.** Before it merges, `repository.merge` judges
every path the change touches against `.engineering/judge-allow.yaml` at the tip of the merge
target, `trial/<n>`. A path that file does not admit is a judge surface. With no such file, every
path is one. A change to a judge surface merges under the owner's grant (`human:<id>`). An agent
role's grant (`agent:<role>`) counts only when `.engineering/delegations.yaml` at that same tip
records an unrevoked delegation to exactly that role. todo-app-example's scenario-1 seed (`896382c`,
as `.engineering/scenarios.conf` on its `main` names it) holds one:

```yaml
delegations:
  - owner: human:operator
    role: agent:takt-trial
    capability: repository.merge.judge_surface
    given_at: "2026-10-27T17:44:48Z"
```

`role` must be the `--by` value word for word. Both files are read only at the merge target's tip,
never from the case's tree, so they belong in the seed commit the trial branch is cut from; a
case's own edit of either counts for nothing. A change to either file merges under the owner's
grant alone.

When the delegation is missing, or names another role, the `ApprovalRequired` stop does not say so.
`takt approve` succeeds, and the resumed run merges nothing and stops `NoProgress`, exit 3. With no
file at all, its `repository.merge` evidence reads:

```text
pull request #<number> at <revision> is open and granted by agent:<role>, and merging <revision>
into trial/<n> at <tip> changes judge surfaces, ...; agent:<role> is not the owner, and no
delegation of repository.merge.judge_surface to it is on record at trial/<n> <tip>
(.engineering/delegations.yaml is not there); nothing merged, the claim stays unknown
```

On another repository, put the delegation on the commit the merge target is at, then cut a new
trial and run it again.

The record is the clone's `.takt/cases/scenario-1-trial-<n>/` ([Case files](../reference/case-files.md)):
`events.jsonl` holds the `opened` line, with `harness_program` and `preflight`, every step, and the
last line's stop; each harness step's transcript is under `~/.local/state/takt/transcripts`. Copy
the case directory out of the clone before deleting it.

A trial counts when both hold:

- the last `takt run` printed `stopped: Completed` and exited 0;
- `gh pr list -R beyond10x/todo-app-example --base trial/<n> --state merged` lists the case's
  pull request.

Compare the sha256 that `opened`'s `harness_program` names with the one you noted: a different
value means the run did not start the binary you qualified, and the trial does not count.

#### A scenario whose seed is red

Scenario 3 (`story:empty-or-broken-file`) starts red on purpose: at its seed a test for an empty
`todo.json` fails, and the story is to make it pass. Run it as scenario 1, with `3` for `1`, from
a clone of its own. Its first `tests.run` fails before any step, so the run stops `RedBase`,
exit 3, before any model call, unless the scenario declares the red base expected.

The declaration is one line in the scenario's section of `.engineering/scenarios.conf`,
`red-base = expected` ([Scenario run](./scenario-run.md#a-seed-whose-tests-fail-on-purpose)).
With it, `takt scenario run` goes on past the red base without a flag, and prints before the case
opens:

```text
red base: expected by scenario 3 in .engineering/scenarios.conf at main: tests failing at the base do not stop the run
```

Without it, give `takt scenario run` `--continue-past-red-base`. takt 0.16.0 and earlier do not
read the key, and refuse a section that gives it. A case that went on past its red base resumes
with `takt run --case` as above, without the flag, and so does a case whose scenario declared it,
even one that stopped before its first `tests.run`: the case records the declaration as it opens.
A case that stopped `RedBase` resumes with `--continue-past-red-base` added to that command.

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
| `RepeatedGateFailure` (0.19.0): three gate runs in a row, each at a new revision, failed the same gates | 3 | resumes the case; the row is read from the case's record, so the next gate run that fails the same gates stops it again |

`--case <id>` resumes a named case instead. Every stop is explained in
[How a run ends](../concepts/how-a-run-ends.md).

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
