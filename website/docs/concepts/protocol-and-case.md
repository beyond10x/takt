---
title: Protocol and case
sidebar_position: 2
description: A protocol says what must be true before work counts; a case is one unit of work held to one protocol, kept as files in the repository.
lede: takt holds every unit of work as a case on a Canon protocol, read from the engineering protocols' built-ins, and keeps the case as files in the repository so that any later run can resume it and any reviewer can replay it.
source: takt protocols and takt run --help at 0.14.0; engineering-protocols 0.10.0 protocols/software-change/1.yaml and protocols/story-delivery/1.yaml, 0.12.2 and 0.13.0 protocols/story-delivery/2.yaml; Cargo.lock after 0.20.0; CHANGELOG.md 0.21.0; checked against 0.21.0 source (Cargo.toml, crates/takt/src/protocol_defaults.yaml, run.rs case line) and engineering-protocols 0.13.0 protocols/story-delivery/2.yaml; protocols/story-delivery/3.yaml and protocols/bench-task/1.yaml; crates/takt/src/protocol.rs (OWN, builtins); crates/takt-cli/tests/bench_task_cli.rs; crates/takt/src/case.rs, frontier.rs, protocol.rs; CHANGELOG.md 0.13.0; a scratch run of takt 0.14.0
---

# Protocol and case

## Protocol

A protocol is a [Canon](/docs/concepts/glossary#canon) `protocol/1` document, data rather than code. It declares:

- the **artifacts** a unit of work has (in `software.change/1`: `intent`, `plan`,
  `implementation`, `release`, …), each at a revision;
- the **[evidence](/docs/concepts/glossary#evidence) kinds** that can be recorded about them (`test_result`, `gate_result`, …);
- the **claims** that evidence makes true or false (`tests.pass`, `ci.green`, `pr.merged`);
- the **obligations** those claims discharge;
- the **actions** that can be taken, with their preconditions and the capability an action
  `requires`, if any;
- the **outcomes** a case can end in, and what each requires (`accepted` needs `pr.merged`).

takt has no Rust model of a protocol. Canon reads the document and computes everything from it.

### The protocols takt runs

Most protocols takt runs are built-ins of
[engineering protocols](https://beyond10x.github.io/engineering-protocols/)
([GitHub](https://github.com/beyond10x/engineering-protocols)); takt 0.21.0 pins its release
0.13.0. One is takt's own: `story.delivery/3`. `takt protocols` lists them all, takt's own after
the registry's `story-delivery/2`:

```text
eval-gated-change/1
incident-response/1
loop-improvement/1
ml-experiment/1
self-improvement/1
software-change/1
story-delivery/1
story-delivery/2
story-delivery/3
support-triage/1
```

The ones takt's defaults and [validators](/docs/concepts/glossary#validator) are written for:

| protocol | a case is | opened by |
|---|---|---|
| `software.change/1` | one change for an intent | `--intent` (the default protocol) |
| `story.delivery/2` | one [story](/docs/concepts/glossary#story) of the planning store, delivered to a merged pull request, with the case inputs `tests_first` and `gate_on` | a story the store gives (the default; [defaults](../reference/protocol-defaults.md)) |
| `story.delivery/3` | the same story delivery, takt's own revision of `/2` (below) | `--protocol story.delivery/3` |
| `story.delivery/1` | the same story delivery with no case input | `--protocol story.delivery/1` |
| `self.improvement/1` | one improvement turn of takt itself | `--protocol self.improvement/1` |
| `loop.improvement/1` | one change to the improvement loop | `--protocol loop.improvement/1` |

### The default story protocol

A story opens on `story.delivery/2` unless something names another protocol. Its case has two
inputs. takt gives each its [default](../reference/protocol-defaults.md) unless `--input`, a
story tag or the run file sets it:

| input | values | default | what it decides |
|---|---|---|---|
| `tests_first` | `required`, `optional` | `required` | whether the story's tests must fail before the implementation is edited |
| `gate_on` | `after_handover`, `each_revision` | `after_handover` | when the repository's gate runs: once, at the revision handed over, or at every revision |

A test run, a gate or CI is **red** when its last result at the current revision failed, and
**green** when it passed.

#### When an edit is admitted

Under `tests_first=required`, `story.delivery/2` admits `implementation.edit` in two cases:

1. The story's tests are written and red at the current revision.
2. The tests are green at the current revision, the gate or CI is red there, and some test run
   failed after the tests were first written.

The second case lets a run fix a failure no test reads, such as formatting or lint. Its last
condition is new in engineering protocols 0.13.0, the release takt pins from 0.21.0: tests that
never failed cannot reach the edit through a red gate. Under `tests_first=optional` the edit is
always admitted.

### takt's own story protocol

`story.delivery/3` is the file `protocols/story-delivery/3.yaml` of this repository, embedded in
the binary from 0.19.0. Its actions, inputs, artifacts and input defaults are those of `/2`. It
differs in two rules:

- `change.handover` is not admissible while the tests are red at the current revision.
- After a red gate or CI it admits the edit once the tests are written and green; they need not
  have failed first. That is the rule `/2` had in engineering protocols 0.12.2.

Its descriptions of the two actions also say what a run does after a red gate: the gate runs at
the handed-over revision, a red gate admits `implementation.edit` again, and a failure no test
reads (formatting, lint, citations, another platform) is fixed where it is.

### Other protocols

In `story.delivery/1` every claim is about the implementation at its current revision, so a
new commit returns each of them to unknown, and the tests, the gate and CI are observed again.
It has no case input.

`bench.task/1` (`protocols/bench-task/1.yaml`, from 0.19.0) is a file, not a built-in: one
benchmark task, worked under `--protocol protocols/bench-task/1.yaml --forge none`. It has no
pull request and no merge; a case is `completed` once the task's visible tests pass at the
revision the [harness](/docs/concepts/glossary#harness) handed over. The benchmark's own grading tests stay outside the case.

### Naming a protocol

`takt protocols` prints names with a hyphen (`story-delivery/1`); the cases, the help and these
pages write them with a dot (`story.delivery/1`). `--protocol` takes either spelling, and the
case records the dotted one:

```console
$ takt run --repo ~/demo --intent "check" --protocol software-change/1 --harness none --forge none --test-cmd true --iterations 1
case takt-20261026T231245Z on software.change/1 in ~/demo; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
```

That line is from takt 0.14.0. From 0.15.0 it goes on after `confinement none` to describe the
[Substrate](/docs/concepts/glossary#substrate) adapter the run starts its commands through.

`--protocol` also takes the path of a `protocol/1` file. [Protocols](../reference/protocols.md)
describes each built-in; the [protocol viewer](pathname:///takt/protocols/) draws them.

## Case

A case is one unit of work held to one protocol. It opens on an intent (`--intent`), on a story
you name (`--story`), or on the next ready story of the planning store, and only on a tree with
no uncommitted work outside the cases directory. A story is ready when it is `proposed`,
nothing blocks it and everything it depends on is `implemented` or `archived`; taking it moves
it to `active`, so no story gets two cases.

Opening a case does three things:

1. It forks a branch for the case, `takt/<case>`, from the branch the tree is on, and records
   the base commit in `branch.yaml`. Every commit of the case lands there.
2. It writes the case's directory, `.takt/cases/<case>/`, with the case itself
   (`case.yaml`, a `canon-case/1` document of the artifacts and their revisions, and the inputs)
   and the protocol it runs on (`protocol.yaml`).
3. It appends the run's `opened` line to `events.jsonl`, naming the case, the protocol, the
   inputs, the base, the binary and the [confinement](/docs/concepts/glossary#confinement).

The case directory of an intent run after three runs and one [grant](/docs/concepts/glossary#grant):

```console
$ ls .takt/cases/takt-20261026T231123Z
authority.yaml
branch.yaml
case.yaml
decisions
events.jsonl
evidence
intent.md
observations.json
protocol.ir.json
protocol.yaml
```

| file | holds |
|---|---|
| `case.yaml` | the case: protocol, artifacts at their revisions, inputs |
| `decisions/NNNN.json` | each evaluation the [governor](/docs/concepts/glossary#governor) returned, numbered; each run's latest alone under the default records profile, every one under `--records trial` |
| `evidence/` | each `canon-evidence/1` record a validator wrote |
| `authority.yaml` | the grants, once `takt approve` gave one |
| `events.jsonl` | the run record, one line per event ([reference](../reference/events-jsonl.md)) |

A story's case also holds `story.yaml`; every file is described in
[Case files](../reference/case-files.md).

### Resuming and replaying

A case outlives the run that opened it. `takt run --case <case>` resumes it by id, and a run with
neither `--intent` nor `--case` resumes the open case of a story being worked before it takes a
new one. A resumed case keeps its protocol and the inputs it was opened with: another protocol or
any `--input` is refused before the run. A case an earlier takt opened under `.engineering/cases`
is resumed where it is ([Case files](../reference/case-files.md#where-cases-live-and-git)).

Neither a judgement step nor a validator's command may write into a case's directory, so every
file in it was written by takt. From the same files `takt replay` rebuilds the prompt one
judgement step was given, checks it byte for byte against the recorded one, and runs that step
again in a scratch worktree at the revision it started from, writing nothing into the case. A step
whose decision a `default` run dropped is refused, naming `--records trial`.

Next: [The step loop and the frontier](./one-step-at-a-time.md).
