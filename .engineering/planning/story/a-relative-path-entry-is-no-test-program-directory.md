---
format: aep.planning-md/3
id: story:a-relative-path-entry-is-no-test-program-directory
kind: story
status: active
title: A relative PATH entry does not make a test's case tree a program directory
summary: own_home keeps relative PATH entries, so the guard counts the case tree's files as programs
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/tests/support/own_home.rs
- confidence: inferred
  path: crates/takt/tests/tests_have_a_home_of_their_own.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T02:39:10Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T02:39:10Z", actor: "human:timo", revision: 3}
---
# A relative PATH entry does not make a test's case tree a program directory

## What is wrong

Found by the staged full gate of wave 50. `crates/takt/tests/support/own_home.rs` gives each test
binary program directories of its own, but it keeps every relative `PATH` entry as it was
(`if !dir.is_absolute() { path.push(dir) }`, `own_home.rs:154`). A relative entry such as `.`
resolves against the working directory of the command that reads it, and a case's test command
runs in the case tree. The step guard and the judge surface then count the tree's executable
entries as programs on `PATH`.

The gate ran from a shell whose `PATH` had `.` as its fourth entry. Two cases went red, and they
passed with the relative entries removed:

- `harness_proposes::a_scripted_proposal_of_a_refused_path_leaves_the_tree_unchanged`. The
  committed link `linked` was read as judge surface, so the step was refused before the harness
  rather than before the edit effect.
- `decline_adv1::a_decline_after_a_kept_edit_holds_on_resume`. An edit of `todo.py` was refused
  as a change of the judge surface.

This reaches any operator who runs the suite with a relative entry on `PATH`.

## Check

```expected_before
command: sh -c 'PATH=.:$PATH exec cargo test --locked -p b10x-takt --test harness_proposes a_scripted_proposal_of_a_refused_path_leaves_the_tree_unchanged'
exit: 101
healthy_exit: 0
output: before the edit effect
source: wave 50 staged gate at 82c9133ce
```

## Outcome

A test binary's `PATH` holds no relative entry, or the suite states that it needs none and says
so when one is found. The two cases above are green whatever relative entries the caller's
`PATH` holds.

## Scope

- `crates/takt/tests/support/own_home.rs`: the `PATH` rewrite (cited)
- `crates/takt/tests/tests_have_a_home_of_their_own.rs`: a check that no relative entry is left (inferred)
