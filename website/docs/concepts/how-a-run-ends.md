---
title: How a run ends
sidebar_position: 6
description: Every stop a takt run can end on, grouped, with its exit code and what the next run does; the two test stops added in 0.14.0.
lede: Every run ends on one named stop, printed as one stopped line and recorded in the case, with an exit code a script or a Loom Job can read; every stop but Completed leaves the case open, and the next run resumes it.
source: takt run --help (Exit codes) at 0.14.0; crates/takt/src/run.rs (Stop, STOP_KINDS, REPEATED_FAILURES, Run::tests_stop, Run::before_any_effect), crates/takt-cli/src/main.rs (EXIT_*); crates/takt-cli/tests/test_failure_stops_cli.rs; CHANGELOG.md 0.14.0; Loom 0.44.0 CHANGELOG (status.run); scratch runs of takt 0.14.0; one case record of a cluster run of takt 0.13.1
---

# How a run ends

A run ends on exactly one stop. takt prints it as the last line, records it in the case's
`events.jsonl` as a `stop` line, and exits with the stop's code:

```text
stopped: <stop>; steps <n>, tokens <n>, case <case directory>
```

Every stop but `Completed` leaves the case open. The next `takt run` on the repository resumes
it: a story's open case is found on its own, and an intent's case is resumed with
`--case <case>`. The generated table of codes is [Exit codes](/docs/reference/exit-codes).

## Every stop, grouped

| exit | stop | what the next run does |
|---|---|---|
| 0 | `Completed (<outcome>)`, or `frontier: nothing ready` | nothing to resume: the case has its outcome, or no story was ready |
| 1 | refused before the run | nothing was opened; fix what stderr names |
| 2 | the arguments do not parse | nothing was opened |
| 3 | a budget, a loop stop or a test stop (below) | resumes the case |
| 4 | `ApprovalRequired` | resumes once `takt approve` granted it |
| 5 | `stopped: error: …` | resumes the case |
| 6 | `Waiting` | resumes and observes again |
| 7 | `Unperformable` | resumes, with the performer it lacked |
| 8 | `DiskLow` | resumes once there is space |

### Completed: exit 0

`Completed (<outcome>)`: an outcome of the protocol is legitimate, such as `accepted` for a
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
| `Tokens (<n>)` | `--tokens`: tokens the harness reported |
| `ActiveTime (…)` | `--active-seconds`: wall time less the time spent waiting on the forge |

A budget is read before each step, so a step under way runs to its end. The next run starts with
a fresh budget.

### Loop stops: exit 3

| stop | why |
|---|---|
| `NoProgress` | a pass changed no file and left the decision as it was; another would not either |
| `NothingAdmissible` | no action is left to take and none needs an approval |
| `StepRefused (<why>)` | a step did what the frontier did not admit; its change was discarded |

A refused step is described in [Capabilities and approvals](./capabilities-and-approvals.md#a-step-that-does-more-is-refused).

### The two test stops

0.14.0 adds two stops on `tests.run` results, both exit 3. They stop a run whose tests are red
before it spends model calls on them.

- **`RedBase`**: the case's first `tests.run` fails before any step changed its base. The run
  stops at once, before any model call, naming the failed tests and the runner's rerun hint.
  The failure is recorded as the case's test result like any other. `--continue-past-red-base`
  (or `continue_past_red_base: true` in a run's configuration) goes on past it, for a base whose
  tests fail on purpose.
- **`RepeatedTestFailure`**: two `tests.run` results in a row, each at a new revision, fail the
  same tests. The edits between them moved nothing the tests decide; the run stops before
  another judgement step, naming the tests. A failing set that changes, or a failure whose output
  names no test, does not stop the run.

A resume after either stop runs the tests again before any model call, and stops again while
they fail the same way. Once they pass, or fail differently, the run goes on.

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
refused, a validator or a harness that failed. Whatever the failed step changed is discarded, the
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

## Reading the code from outside

A run's outcome lives in its files and its exit code, never only in a process, so a script, a CI
job or an orchestrator can drive takt without parsing its output. Under Loom, a takt container
deployed as a `LoomAgent` of the `Command` variant runs once as a Job, and Loom 0.44.0 writes the
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
