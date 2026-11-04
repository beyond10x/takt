---
format: aep.planning-md/3
id: story:program-rewritten-through-a-link-refuses-the-step
kind: story
status: active
title: A program rewritten through its link in a program directory refuses the step
summary: the guard reads a linked program as the link only; a rewrite behind it is not refused
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/support/own_home.rs
- confidence: cited
  path: crates/takt/tests/w50_sec_unattributed.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T06:19:36Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T06:19:36Z", actor: "human:timo", revision: 3}
---
# A program rewritten through its link in a program directory refuses the step

## What is wrong

Found by the security review of wave 50's unit c (review-result:w50-sec-c-1, finding 1). The step
guard reads an entry of a program directory that is a symbolic link as the link only
(`Entry::Link`, `crates/takt/src/run.rs:7610` at 9385bf5b1). A step that rewrites the program
behind such a link changes a program a command finds on `PATH` without being refused: in the
review's case the run went on and stopped `NoProgress`. Had the step also edited a file,
`tests.run` would have run the rewritten program.

What reaches it, as the review reports it: a linked program in a program directory (pipx,
`~/.local/bin`, dotfile managers) together with a step or test command that can write outside the
tree, as under `--confinement none`. Confined runs were not checked.

A related hazard in the tests: `crates/takt/tests/support/own_home.rs` builds a test binary's
private program directory as links to the real programs, so a test step that writes through an
existing name writes into the real home, and the guard does not see it.

## Check

```expected_before
command: cargo test --locked -p b10x-takt --test w50_sec_unattributed a_program_rewritten_through_its_link_on_path_during_a_step
exit: 0
healthy_exit: 0
output: story:program-rewritten-through-a-link-refuses-the-step
source: review-result:w50-sec-c-1
```

The case asserts today's state (the run does not stop `StepRefused`) with a message naming this
story; when the guard sees the rewrite, the case goes red and is rewritten to assert the refusal.

## Outcome

A step that changes the program a link in a program directory resolves to is refused, as a change
of the link itself is.

## Scope

- `crates/takt/src/run.rs`: the program directory snapshot (`Entry::Link`) (cited)
- `crates/takt/tests/support/own_home.rs`: private program directories of copies or of links the
  guard follows (inferred)

## Decision

Decided 2026-11-04 under the owner's standing delegation, through the orchestrator: a test binary's
private program directories keep links to the real programs, not copies. Copies would put several
gigabytes of programs under every test binary and leave a copied `rustc` without its sysroot. What
the links leave, another process reinstalling a program at an existing name while a test step runs,
is filed as story:a-program-reinstalled-at-its-name-during-a-test-step-refuses-it, a draft outside
this wave. Until it is delivered, a gate refusal that names a program-directory link counts as that
known cause, and its stage is rerun once.
