---
title: How a run ends
sidebar_position: 6
description: Every takt run ends on one named stop with its own exit code. This page lists each stop, what caused it and what the next run on the case does.
lede: Every run ends on one named stop, printed as one stopped line and recorded in the case, with an exit code a script or a Loom Job can read; every stop but Completed leaves the case open, and the next run resumes it.
source: takt run --help (Exit codes) at 0.14.0; crates/takt/src/run.rs (Stop, STOP_KINDS, REPEATED_FAILURES, REPEATED_GATE_FAILURES, is_documentation_path, code_changed, names_doc_path, Run::tests_stop, Run::before_any_effect), crates/takt/src/console.rs (stop_figures), crates/takt-cli/src/main.rs (EXIT_*); crates/takt-cli/tests/test_failure_stops_cli.rs; crates/takt/tests/w42_adv_repeat_count.rs, w44_adv_docs_named.rs, w44_adv_docs_named_b.rs; CHANGELOG.md 0.14.0; Loom 0.44.0 CHANGELOG (status.run); scratch runs of takt 0.14.0; one case record of a cluster run of takt 0.13.1; the test stops checked against 0.21.0 source (run.rs code_changed, names_doc_path, Failing; CHANGELOG.md 0.21.0)
---

# How a run ends

A run ends on exactly one stop. takt prints it as the last line, records it in the [case](/docs/concepts/glossary#case)'s
`events.jsonl` as a `stop` line, and exits with the stop's code:

```text
stopped: <stop>; steps <n>, tokens <n>, wall <duration>[, cost $<dollars>], commits <n>[ (<p> paths)], case <case directory>
```

From 0.19.0 the line names the run's wall time, its cost when a step reported one, and the
commits its steps made with how many paths they committed. An error's `stopped: error:` line
carries the same figures.

The examples on this page are from takt 0.14.0. Their stopped lines end at `tokens`, and they
print a `frontier:` line at each decision. A 0.21.0 run prints `may:` once and then `may now:`
only when the set of admissible actions changes; `--verbose` adds the `frontier:` lines back.

Every stop but `Completed` leaves the case open. The next `takt run` on the repository resumes
it: a [story](/docs/concepts/glossary#story)'s open case is found on its own, and an intent's case is resumed with
`--case <case>`. The generated table of codes is [Exit codes](/docs/reference/exit-codes).

## Every stop, grouped

| exit | stop | what the next run does |
|---|---|---|
| 0 | `Completed (<outcome>)`, or `frontier: nothing ready` | nothing to resume: the case has its outcome, or no story was ready |
| 1 | refused before the run | nothing was opened; fix what stderr names |
| 2 | the arguments do not parse | nothing was opened |
| 3 | a budget, a loop stop, a test stop or the gate stop (below) | resumes the case |
| 4 | `ApprovalRequired` | resumes once `takt approve` [granted](/docs/concepts/glossary#grant) it |
| 5 | `stopped: error: …` | resumes the case |
| 6 | `Waiting` | resumes and observes again |
| 7 | `Unperformable` | resumes, with the performer it lacked |
| 8 | `DiskLow` | resumes once there is space |
| 9 | `Declined (<reason>)` | stops `Declined` again until a commit moves the case's revision |

### Completed: exit 0

`Completed (<outcome>)`: an outcome of the [protocol](/docs/concepts/glossary#protocol) is legitimate, such as `accepted` for a
story whose pull request merged. A story's case that ends `accepted` records its test result on
the story and moves it to `implemented`. `frontier: nothing ready` also exits 0: no story of the
planning store was ready, and no case was opened.

### Refused: exit 1 and 2

Exit 1 is a refusal before anything ran: an argument, the repository, the protocol or the case.
The reason is on stderr and no step was taken. Exit 2 is a command line that does not parse.

### Budgets: exit 3

| stop | budget |
|---|---|
| `Iterations (<n>)` | `--iterations`: steps taken |
| `Hours (<h>)` | `--hours`: wall time |
| `Tokens (<n>)` | `--tokens`: tokens the [harness](/docs/concepts/glossary#harness) reported |
| `ActiveTime (…)` | `--active-seconds`: wall time less the time spent waiting on the [forge](/docs/concepts/glossary#forge) |

A budget is read before each step, so a step under way runs to its end. The next run starts with
a fresh budget.

### Loop stops: exit 3

| stop | why |
|---|---|
| `NoProgress` | a pass changed no file and left the decision as it was; another would not either |
| `NothingAdmissible` | no action is left to take and none needs an approval |
| `StepRefused (<why>)` | a step did what the [frontier](/docs/concepts/glossary#frontier) did not admit; its change was discarded |

A refused step is described in [Capabilities and approvals](./capabilities-and-approvals.md#a-step-that-does-more-is-refused).

### The two test stops

0.14.0 adds two stops on `tests.run` results, both exit 3. They stop a run whose tests are red
(the last test run failed) before it spends model calls on them.

- **`RedBase`**: the case's first `tests.run` fails before any step changed its base. The run
  stops at once, before any model call, naming the failed tests and the runner's rerun hint.
  The failure is recorded as the case's test result like any other. `--continue-past-red-base`
  (or `continue_past_red_base: true` in a run's configuration) goes on past it, for a base whose
  tests fail on purpose; a scenario declares that with `red-base = expected` in its section of
  `.engineering/scenarios.conf` ([Scenario run](../guides/scenario-run.md)).
- **`RepeatedTestFailure`**: two test runs in a row fail the same tests, each at a new revision.
  The edits between them changed nothing the tests decide. The run stops before another
  judgement step and names the tests. A set of failing tests that changes does not stop the
  run, and neither does a failure whose output names no test. Not every test run counts toward
  the two: [What counts as an attempt](#what-counts-as-an-attempt).

A resume after either stop runs the tests again before any model call, and stops again while
they fail the same way. Once they pass, or fail differently, the run goes on.

#### What counts as an attempt

A failing test run counts toward `RepeatedTestFailure` only when its revision is a new attempt:
since the last run that counted, the case changed something the tests decide. Two kinds of run
never count (from 0.19.0):

- The red run at the revision a `tests.write` step committed. The protocol asks for the story's
  new tests to fail there, so a first edit that leaves them red does not end the run.
- A run at a revision that changed only documentation. Documentation is everything under
  `docs/`, plus `README.md` and `CHANGELOG.md`.

From 0.21.0 a documentation file counts as code when a test file names it. A test file is any
file that a `tests.write` commit of the case changed.

| the revision changed | is it an attempt? |
|---|---|
| a code file, such as `src/lib.rs` | yes |
| `docs/fixtures/case.txt`, and a test file holds `docs/fixtures/case.txt` or `docs/fixtures` | yes, from 0.21.0 |
| `README.md`, and a test file holds `README.md` | yes, from 0.21.0 |
| `docs/guide.md`, and no test file names it | no |
| anything under `.engineering/` | never: those paths are left out before the check |

A test file names a path when it holds the path, or one of its parent directories with or
without a trailing `/`, and the character right after it cannot go on with a path. For example,
`docs/runs-old/a.md` does not name `docs/runs/a.md`, because `-` goes on with a path. The
characters that go on with a path are letters, digits, non-ASCII characters and `_ - . / ~`.
The top directory alone names nothing: a test file that holds only `docs` names no file under
it, while `docs/runs` names `docs/runs/a.md`. The test file is read at the revision of the run.
When git cannot compare the two revisions, the run counts.

### The gate stop

0.19.0 adds one stop on `gate.run` results, exit 3:

- **`RepeatedGateFailure`**: three `gate.run` results in a row, each at a new revision, fail the
  same gates. The run stops before another judgement step, naming the gates; the case stays open:

  ```text
  stopped: RepeatedGateFailure (3 gate.run results in a row, each at a new revision, failed the same gates: <gates>); steps <n>, …
  ```

A pass, or a failure on another set of gates, ends the row; a result at the revision of the one
before it counts once. The row is read from the case's record, not from the run, so after a
resume the next gate run that fails the same gates at a new revision stops the run again.

Before it, a red gate reaches the next judgement step's prompt with what each failed gate
printed ([A judgement step's prompt](./one-step-at-a-time.md#a-judgement-steps-prompt)).

A red base, on a one-crate repository whose only test fails:

```console
$ takt run --repo ~/red --intent "make the failing test pass" --harness none --forge none --iterations 1
case takt-20261026T231110Z on software.change/1 in ~/red; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 1: tests.run {}
  effect: the test command exited 101: fail; 0 passed, 1 failed, 0 ignored over 1 result line; to rerun pass `--lib`; failed: tests::adds_two_numbers
  evidence: test_result fail
stopped: RedBase (the tests fail at the case's base 4663159f, before any step changed it: tests::adds_two_numbers; to rerun pass `--lib`); steps 1, tokens 0, case ~/red/.takt/cases/takt-20261026T231110Z
$ echo $?
3
```

Resumed with `--case takt-20261026T231110Z`, the run ran the tests again first and stopped on
the same `RedBase`, exit 3, still with 0 tokens. The same command on a copy of the repository
whose test passes goes on past the test run and stops on its step budget instead:

```console
$ takt run --repo ~/green --intent "make the failing test pass" --harness none --forge none --iterations 1
case takt-20261026T231123Z on software.change/1 in ~/green; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
stopped: Iterations (1); steps 1, tokens 0, case ~/green/.takt/cases/takt-20261026T231123Z
$ echo $?
3
```

### Approval: exit 4

`ApprovalRequired (<actions>)`: only actions that need a grant are left, or the harness asked
for the approval gate. `takt approve` records the grant, and the next run resumes the case
([Capabilities and approvals](./capabilities-and-approvals.md)).

### Error: exit 5

`stopped: error: <what>`: the run ended on an error once its case was open, such as a forge that
refused, a [validator](/docs/concepts/glossary#validator) or a harness that failed. Whatever the failed step changed is discarded, the
error is recorded as an `error` line, and the case stays open. A judgement step whose session
ends without the line naming its action is tried again first, 3 tries at most, each try a step
that `--iterations` counts; a try that spent the budget stops the run on the budget, exit 3. With `--harness none`, a run past
a red base ends here as soon as a judgement step is the only way on:

```console
$ takt run --repo ~/red --case takt-20261026T231110Z --harness none --forge none --iterations 3 --continue-past-red-base
case takt-20261026T231110Z on software.change/1 in ~/red; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
stopped: error: step 1: the harness failed; everything the step changed was discarded: no harness: a judgement step among implementation.edit, tests.write is left; steps 1, tokens 0, case ~/red/.takt/cases/takt-20261026T231110Z
```

### Waiting: exit 6

`Waiting (<actions>; …)`: the run waited on the forge (checks still running, a pull request's
head behind the push, a merge held for a required check) and its `--iterations` or `--hours`
budget ended first, or it had neither to wait in; or it waits on something local, such as a
saved scorecard or a promoted binary, and stopped at once. Nothing is recorded for what it
awaits. The next run observes again.

### Unperformable: exit 7

`Unperformable (<claim>; nothing performs it without <the forge, the gates document>)`: no step
left moves an open obligation, and the only action that decides the claim has no performer in
this run. No judgement step is taken. With `--forge none`, a delivery whose change is ready ends
here at `pr.opened` and first prints `candidate: <full commit id>`: its code commits, replayed on
the base branch's tip and named by `refs/takt/candidates/<case>`. Run again with a forge, or a
gates document, to go on.

### DiskLow: exit 8

`DiskLow (<path>: <free> free, under <threshold>)`: before a step, the file system of the tree,
or of `CARGO_TARGET_DIR`, had less free than `--min-free-gib` (default 10; `0` turns the check
off). The case stays open. A scenario trial with a `DiskLow` run gets no scorecard verdict, so a
trial runner discards it rather than resume it.

### Declined: exit 9

`Declined (<reason>)`: a judgement step named `decline` instead of an action, changed no file,
and gave its reason, for a story the rules it was given forbid. takt writes a `decline` line
with the reason and the case revision to `events.jsonl`; it is a record, not [evidence](/docs/concepts/glossary#evidence), and the
protocol's outcome stays open. A step that names `decline` and changed a file is refused instead
(`StepRefused`, exit 3). A run on the case at the same case revision stops `Declined` again
before any model call, and an argument-less `takt run` passes the case over, until a commit
moves its revision. A stuck run still ends `NoProgress`, exit 3.

## Reading the code from outside

A run's outcome lives in its files and its exit code, never only in a process, so a script, a CI
job or an orchestrator can drive takt without parsing its output. Under [Loom](/docs/concepts/glossary#loom), a takt container
deployed as a `LoomAgent` of the `Command` variant runs once as a Job, and Loom 0.45.0 writes the
program's exit code, end time and reason into the resource's `status.run` and its `Completed`
condition. A delivery run is built to end on 0 (accepted, or nothing ready), 3 (a budget, or
tests that need a person), 4 (the merge waits for a grant) or 7 (`forge: none`, with the
candidate commit printed). An exit 5 or 8 says the environment needs attention.

One case from a cluster run of takt 0.13.1 shows why each stop leaves the case open: it was
resumed by 12 runs. The first five stopped `StepRefused`, exit 3, because the test command wrote
under the container's home directory; the last seven ended `stopped: error:`, exit 5, each time
the harness session ended without naming an action. Every run's lines are in the one
`events.jsonl`.

How to deploy it: [Deploy on Kubernetes](../guides/deploy-on-kubernetes.md).
