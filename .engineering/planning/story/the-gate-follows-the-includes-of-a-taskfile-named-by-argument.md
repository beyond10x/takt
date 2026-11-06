---
format: aep.planning-md/3
id: story:the-gate-follows-the-includes-of-a-taskfile-named-by-argument
kind: story
status: draft
title: The gate follows the includes of a Taskfile a command names by argument
summary: an edit to a file ci/build.yml includes, named by task -t, ran and recorded pass
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: the-gate-follows-the-includes-of-a-taskfile-named-by-argument

## Outcome

A Taskfile a gate command names by argument (`task -t ci/build.yml`, `--taskfile`) has its `includes:` followed and
compared at the base, as a Taskfile found by its name is.

## Context

Found by the wave 55 adversary pass at d0e8fba54 (`crates/takt/src/validator.rs:3608`): the include walk starts only
from files named like a Taskfile. `ci/build.yml` is compared as an argument, the `ci/lib.yml` it includes is not, and an
edit there ran and recorded pass. The case, not committed with the wave (outside its acceptance):

```rust
#[test]
fn a_file_a_taskfile_named_by_argument_includes_is_compared_or_refused() {
    let tree = repository(
        "task-t",
        &project("/bin/sh", &["scripts/task", "-t", "ci/build.yml", "check"]),
        &[
            ("scripts/task", "grep -q 'exit 0' ci/lib.yml\n"),
            ("ci/build.yml", "version: '3'\nincludes:\n  lib: ./lib.yml\ntasks:\n  check:\n    cmds:\n      - task: lib:check\n"),
            ("ci/lib.yml", "version: '3'\ntasks:\n  check:\n    cmds:\n      - exit 1\n"),
        ],
    );
    let outcome = observed(&tree, "task-t", &[("ci/lib.yml", "version: '3'\ntasks:\n  check:\n    cmds:\n      - exit 0\n")]);
    refused(&outcome, &["ci/lib.yml"]);
}
```

The helpers are those of `crates/takt/tests/w55_adversary.rs`.

## Acceptance

- The case above is green: the edit to `ci/lib.yml` is refused before the gate runs, naming it.
