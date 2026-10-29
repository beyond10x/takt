---
title: The step loop and the frontier
sidebar_label: The step loop and the frontier
sidebar_position: 3
description: The loop takt runs, what the frontier is, which step comes next, and what a judgement step may do.
lede: A run is a loop of evaluate, pick one step, perform it, observe, and evaluate again; the frontier Canon computes decides what may be done, and a harness is offered only what the frontier admits and no validator performs.
source: crates/takt/src/lib.rs, run.rs (the loop in Run::run_held, Shape::for_harness, Shape::for_stateless), harness.rs (SessionHarness, ClaudeCode::ISOLATION, ClaudeCode::TOOLS, prompt_with, red_tests_block, red_tests_block_scanned, scanned_per_line, ChangeSoFar, tests_path_block, failed_block, earlier_gate_line, CHECKS_RULE), obligations.rs (block, commands_at), console.rs (may, may_now), frontier.rs; docs/design/takt.md § 3; takt run --help at 0.14.0; a scratch run of takt 0.14.0
---

# The step loop and the frontier

A run is a loop:

```text
evaluate → frontier → one step → effect → observation → validator → evidence → evaluate
```

It goes round until an outcome is legitimate or the run reaches a named stop
([How a run ends](./how-a-run-ends.md)).

## Evaluate

Before every step, Loom's governor evaluates the case with
[Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)). The
evaluation is a pure function from the protocol, the case, its evidence records, its authority
and the instant to:

- each **claim**: `true`, `false` or `unknown`;
- the open **obligations**;
- each **action**'s status;
- the **legitimate outcomes**.

takt writes each evaluation as `decisions/NNNN.json` in the case. A case keeps each run's latest
alone by default; `--records trial` keeps every one ([Records
profiles](../reference/configuration.md#records-profiles)).

From 0.19.0 the console says the frontier as what the run may do: `may: <action> …` once, at the
first decision, then `may now: +<added> -<removed>` only when the admissible set changes.
`takt run --verbose` also prints the full `frontier:` line at every decision, as takt printed it
before 0.19.0 and as the example below shows. From 0.21.0 a harness step also lists its tool
calls and its commit's diff ([The console](../guides/run-a-story.md#the-console)).

## The frontier

The frontier is the actions of the protocol with their status at this evaluation:

| status | meaning |
|---|---|
| `admissible` | may be taken now |
| `approval-required` | its precondition holds, but it requires a capability the case holds no grant of |
| `blocked` | its precondition does not hold, or the capability is denied |

Here is a real run of takt 0.14.0 on a repository whose one test passes. Before the tests ran,
the merge was
`blocked`; once a `test_result` pass is recorded at this revision, `implementation.verified` is
true and the merge becomes `approval-required`:

```console
$ takt run --repo ~/demo --intent "make the failing test pass" --harness none --forge none --iterations 1
case takt-20261026T231123Z on software.change/1 in ~/demo; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
stopped: Iterations (1); steps 1, tokens 0, case ~/demo/.takt/cases/takt-20261026T231123Z
```

The word frontier has a second use in takt: the `Frontier` seam is the planning store, which
answers which stories are ready. It is a list of units of work, not of actions, and no model takes
part in it.

## Which step comes next

Each turn of the loop takes at most one step. takt picks it in this order:

1. **A due validator.** A deterministic step whose action is admissible and whose evidence the
   case lacks at this revision: `tests.run`, `gate.run`, `change.read`, `pr.open`, `ci.read`,
   `repository.merge`. After a stop on the tests, `tests.run` is observed again first.
2. **A wait.** When a validator waits on the forge (checks still running, a pull request's head
   not yet shown), the run pauses, observes again, and offers no judgement step meanwhile.
3. **Two pre-checks.** When only a grant stands between the case and its outcome, the run stops
   `ApprovalRequired` without asking anyone. When the only action that decides a claim left has
   no performer in this run (`--forge none`, no gates document), it stops `Unperformable`.
4. **A judgement step.** The harness is offered the admissible actions no validator owns, such
   as `implementation.edit`, `tests.write` or `change.handover`. An action that may produce
   evidence only a validator writes is never offered to a harness. A harness that keeps no state
   between steps is offered a read-only action only when taking it records a choice.

The step itself is one call of Commission's runtime (`run_until_blocked`) with a budget of one
step. The budgets and the free disk are checked before it; the governor admits the proposal;
an action that requires a capability is asked of the case's authority; then the effect performs
it.

## A judgement step

- The harness works in a scratch copy of the tree: tracked and untracked files, not ignored
  ones, and the cases directory. It never edits the tree itself.
- It names one offered action and proposes a change. takt's edit effect applies the change to
  the tree under the step guard and commits it on the case's branch, so the implementation
  moves to a new revision and every claim about it is unknown again.
- A proposal that names an action not offered, or changes what that action may not change, is
  refused and nothing of it is kept ([Capabilities and approvals](./capabilities-and-approvals.md)).
- What the harness says is a trace in the record, never evidence.

## A judgement step's prompt

The prompt is the same for every harness but for how it names the scope files and its rules. In
order, it holds:

1. **The failed tests, first.** From 0.19.0, when the last `tests.run` failed, the prompt opens
   with `The last test run failed`: each failing test and its last error line, fenced as data
   the command printed, and the ask to act on them first. For a step that offers
   `implementation.edit`, a list follows of where each failing test's assertion is,
   `- <test> at <file>:<line>[:<col>]`, read from the runner's output, with the ask to follow the
   calls from each assertion to the behaviour the intent names and change every function on that
   path. From 0.20.0 paths under the tree are written relative to it, and a line the secret scan
   refuses is withheld on its own, counted in the block's heading; before, one refused line
   withheld the whole block.
2. **The intent**, and the files of the story's `## Scope`: attached as `@<path>` for Claude
   Code, or named as given in full for Codex. From 0.20.0 a Codex step is given each file once.
   It is not given the `AGENTS.md` Codex loads itself from the session's copy, and a step whose
   session shows Codex did not load it stops `StepRefused`. Beside an `AGENTS.md` it is not given
   a `CLAUDE.md` that only points there or whose text `AGENTS.md` holds; any other `CLAUDE.md` is
   given. A file the change so far shows whole is not given again.
3. **The protocol, the step, the claims and the open obligations.**
4. **The repository obligations**: each gate of the gates document at the case's base, with its
   status at this revision. From 0.19.0 each line also names the gate's commands,
   ``Runs `<command>`.``, read from `.engineering/assertions-project.yaml` at the case's base:
   a command belongs to a gate when one of the gate's assertions names it. The commands are left out when
   one has a credential's shape or the block does not pass the secret scan.
5. **The failed gates.** After a red gate, `The last gate run failed`: for each failed gate, the
   lines its command printed that name what failed, fenced as data, and that a file the failed
   gates' output names is part of this change even outside the story's Scope. From 0.19.0 paths
   under the tree are written relative to it, and a line the secret scan refuses is withheld on
   its own; the gate's heading says how many lines were withheld.
6. **Where the gate stands.** From 0.19.0, when the gate failed at an earlier revision and has not
   run at this one, one line says so: `The gate last failed at <revision> on <gates>; it has not
   run at this revision; it runs after <actions>.`
7. **The change so far, the offered actions** with their meaning, the write actions that are not
   admissible now with what their precondition lacks, the actions that need an approval, and
   `decline`. From 0.20.0 the change so far shows every file whole within its 8 KiB bound, test
   files in the intent's Scope too; a test file outside the Scope keeps only its changed lines.
   A binary file, a mode-only change and a file with a carriage return are not counted as shown
   whole.
8. **The rules.** For a harness that runs commands (Codex) and a step that may change files,
   when the obligations name commands, from 0.19.0 the rules ask it to run each named command that
   compiles nothing and changes no file (a format, specification or citation check, never a
   build, test or lint) before it ends the step, and to fix what it reports at its cause.

`takt replay` rebuilds a recorded step's prompt from the case and checks it byte for byte.

The harness session is hermetic. metaharness builds the child's environment from its own
allowlist, so no variable takt withholds reaches it, and every child takt starts gets a timeout
and a bounded tail of its output. Claude Code runs with `--restricted`, `--strict-mcp-config`
and hooks turned off, so it loads no project or user settings, hooks, plugins or MCP servers,
and it is offered only the file tools `Read,Edit,Write,Glob,Grep`.

:::caution[Planned]
No operating-system sandbox holds the harness session yet. `--confinement substrate` holds the
test command only.
:::

## A deterministic step

A validator runs a program takt starts itself: the test command (`--test-cmd`, default
`cargo test --quiet`, split at white space and run without a shell), the repository's gates,
the forge's pull request, CI read and merge. It observes under the same guard as a judgement
step, and a write it makes outside what its validator is allowed is undone and refused.
What it observed becomes evidence: [Evidence and the judge](./evidence-and-the-judge.md).

## No progress

When a pass of the loop changes nothing (a judgement step changed no file and the decision is
the same as before), another pass would not either, and the run stops `NoProgress`.
