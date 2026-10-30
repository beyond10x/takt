---
title: Scenario run
sidebar_position: 8
description: Replay a declared scenario from a fresh trial branch, so runs of different takt revisions can be compared on the same work, and run a trial on a GitHub repository.
lede: A scenario is a story at a known seed commit, and replaying it from that seed on a fresh branch is how one takt revision is compared with another.
source: takt scenario run --help at 0.14.0; docs/metrics/README.md; the design record of recursive self-improvement, § Scenario corpus; the trial steps as run with takt 0.15.1 on todo-app-example; metaharness 0.23.0 (PINNED_VERSIONS)
---

# Scenario run

A scenario is a story of a scratch repository at a known seed commit. Replaying it from the same
seed is how takt's revisions are compared with each other, and with a harness running without
takt.

```sh
takt scenario run --repo <path> <id> --hours 1
```

`.engineering/scenarios.conf` on `main` names each scenario's story and seed commit.
`--declared-on` reads it from another revision. The run:

1. checks that the tree is clean;
2. fetches the remote's `trial/*` branches through the bot route;
3. cuts `trial/<n>` from the seed and checks it out, `<n>` one past the highest the checkout
   knows, local or remote;
4. refuses unless the store there shows the story ready;
5. pushes the branch with a lease that only lets it create it, and refuses unless the push
   reports it created;
6. opens the case `scenario-<id>-trial-<n>` on `story.delivery/1`, whose pull request targets
   `trial/<n>`.

Nothing of an earlier run is resumed. A refusal before the push removes the branch and puts the
tree back. Without the bot route nothing is fetched or pushed, and `<n>` is 22 digits instead:
the UTC time to the second and a random 8-digit nonce. A scenario run refuses `--forge gitlab`,
`command`, `connectors` and `local`.

A trial branch is deleted afterwards with
`b10x-gates bot --repository <owner>/<repo> -- delete-branch trial/<n>`, which the bot route
admits only while the branch's tip is merged into the default branch.

`takt scenario run` takes the same budget, harness, endpoint, forge and test flags as
`takt run`, `--continue-past-red-base` among them. Its token budget has the same default: with
neither `--tokens` nor the run file's `tokens`, it stops at 3,000,000 tokens, exit 3. (Since
0.23.0.)

`takt scenario run --config <FILE>` reads the same run configuration file as `takt run`, and
refuses a `story` in it, since the scenario names the story. (Not in 0.14.0.)

## A trial on a GitHub repository

A trial is one delivery of a known story on
[todo-app-example](https://github.com/beyond10x/todo-app-example), the scratch repository takt's
live trials run against, ending `Completed`, exit 0, with its pull request merged. Each trial is
one `takt scenario run` of scenario 1 (`story:remove-a-todo`, from the seed
`.engineering/scenarios.conf` on its `main` declares): the run cuts a fresh base branch
`trial/<n>` from the seed, opens the case `scenario-1-trial-<n>` on `story.delivery/1`, and its
pull request targets `trial/<n>`. A second trial is another `takt scenario run`, which takes the
next `<n>`.

What a trial needs that a plain run does not:

- **takt 0.15.1 or later.** 0.15.1 is the first release with `--harness-program` and the harness
  preflight; the steps below install a release from its archive ([Install](./install.md)).
- **The forge.** `--forge bot`. Under `--forge none` the run stops `Unperformable (pr.opened)`,
  exit 7, however good the change is. The bot route needs `b10x-gates` on `PATH`, acting as the
  bot's GitHub App, and `B10X_GATES_POLICY` naming its policy file: a regular file you own, not
  a symbolic link, at mode 0600 or stricter, in a directory you own that no other user can
  write. The run checks both before the case opens and is refused with exit 1 when either is
  missing; `b10x-gates`'s default policy path does not count here. takt reads the pull request and its checks through `b10x-gates api --method GET`, so no
  `gh` login is needed (0.22.1 and earlier read them with `gh`). todo-app-example need not be
  listed in the policy's repositories: the scan and the bot route serve it unlisted.
- **A commit identity.** The repository has no `.engineering/commit-identity.conf`, and the bot
  route refuses takt's default identity before the case opens: give `--commit-identity`.
- **The test command.** The repository's tests are `./check.sh`, not `cargo test`: give
  `--test-cmd ./check.sh`.
- **A qualified Claude Code binary.** The metaharness in takt 0.21.0 is pinned to Claude Code
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
qualified binary once, then clone for each trial (`trial-1`, `trial-2`, `trial-3`). `<version>` is
the takt release you install:

```sh
version=<version> target=x86_64-unknown-linux-gnu
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
`takt approve` commits the grant on that branch, and refuses a tree on another branch or with
uncommitted work:

```sh
takt approve --repo . scenario-1-trial-<n> repository.merge --by agent:takt-trial
takt run --repo . --case scenario-1-trial-<n> --harness claude \
  --harness-program ~/.local/opt/claude-2.1.309/claude --forge bot --test-cmd ./check.sh --hours 1
```

`--by` names who grants: `agent:takt-trial`, the role the seed delegates the merge to (below), for
the agent that runs the trial, or `human:<name>` for a person. The case keeps the commit identity
it was opened with, so the resume does not give it.

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

## A seed whose tests fail on purpose

A run stops `RedBase`, exit 3, when the case's first `tests.run` fails before any step changed
the tree. Some scenarios start from a seed that is red on purpose: the story is to make a failing
test pass. Scenario 3 of todo-app-example (`story:empty-or-broken-file`) is one: at its seed a
test for an empty `todo.json` fails. Run it as scenario 1 above, with `3` for `1`, from a clone of
its own. Such a scenario declares the red base in its section, with the key `red-base`:

```ini
[scenario "3"]
	story = story:empty-or-broken-file
	seed = <commit>
	red-base = expected
```

The run then goes on past a red base as `--continue-past-red-base` does, without the flag. A line
before the case opens says what the scenario declared, whether the seed's tests then fail or not:

```text
red base: expected by scenario 3 in .engineering/scenarios.conf at main: tests failing at the base do not stop the run
```

The case records the declaration as it opens (`red_base: expected` in its `case.yaml`), so a
resume of it with `takt run --case` needs no flag either, also after a stop before its first
`tests.run` such as `SetupFailed`. A `--config` file's
`continue_past_red_base = false` does not turn the declaration off. The key is `red-base`, not
`red_base`: git's configuration syntax admits no `_` in a key. `expected` is its one value; any
other is refused before anything is cut. A scenario without the key still stops `RedBase`; give
`takt scenario run` `--continue-past-red-base` to go on. A case that went on past its red base
resumes with `takt run --case` without the flag. A case that stopped `RedBase` resumes with
`--continue-past-red-base` added to that command. takt 0.16.0 and earlier do not read the key,
and refuse a section that gives it. (From 0.17.0.)

## Scenarios

Scenario 1 is a 31-line change to a toy CLI in the scratch repository. Scenarios 2 to 6 were
added on 2026-10-18 and each run once. The held-out check of each is a script no agent saw.
Results: [Runs](/docs/self-improvement/runs). `takt scorecard` reads a scenario run's outcome from
the held-out check beside its record ([Read a case](./read-a-case.md#every-run-of-the-repository-at-once)).
Where a run's time and tokens went: [Read a run report](./read-a-run-report.md).

To test takt on work it was not tuned for, a unit already merged in another public repository,
see [Variety run](./variety-run.md).
