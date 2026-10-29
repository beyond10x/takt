---
format: aep.planning-md/3
id: story:a-relative-state-dir-is-refused-as-inside-the-tree
kind: story
status: draft
title: A relative --state-dir outside the tree is refused as inside it
summary: --state-dir ../state from inside the repository is refused as inside the tree because .. is not resolved before the comparison
tags:
- finding
- priority:low
- rsi
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A relative --state-dir outside the tree is refused as inside it

## What is wrong

Run from inside a repository, `takt run --repo . --state-dir ../state …` is refused, exit 1, with
"the set-aside directory …/repo/../state/set-aside is inside the tree …/repo, where a later
restore would remove what was moved" (`crates/takt/src/git.rs`). `../state` is outside the tree;
the check compares the paths as written, without resolving `..`. An absolute path to the same
directory is accepted.

Seen while recording the local-forge sample for the public guides (wave 44 follow-up), with the
0.21.0 tree's debug build.

## Check

Runnable by `scripts/check-finding` with `$TAKT` set; not yet run through it.

```expected_before
command: sh -c 'd=$(mktemp -d -p "$PWD/target") && git init -q "$d/repo" && git -C "$d/repo" -c user.name=t -c user.email=t@localhost commit -q --allow-empty -m x && cd "$d/repo" && "$TAKT" run --repo . --state-dir ../state --harness none --forge none --intent x --iterations 0'
exit: 1
healthy_exit: 0
output: is inside the tree
source: crates/takt/src/git.rs
```

## Outcome

A relative `--state-dir` is resolved (`..` included) before it is compared with the tree, so a
directory outside the tree is accepted and one inside it is still refused.
