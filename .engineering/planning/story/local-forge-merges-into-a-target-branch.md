---
format: aep.planning-md/3
id: story:local-forge-merges-into-a-target-branch
kind: story
status: draft
title: A run delivers a story into a local target branch, --forge local
summary: 'A local hub repository is the forge: takt pushes, opens a request record, runs local checks and merges the case into the source branch in the hub with a compare-and-swap.'
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# A run delivers a story into a local target branch, `--forge local`

## What is wrong

A story run without a forge (`--forge none`) stops `Unperformable` at `pr.opened`, exit 7: the
change is delivered, tested, gated and handed over, and nothing merges it. An orchestrator that
runs several stories at once, each in its own tree, and reviews their sum on one integration
branch before a single pull request has no way to let takt finish a story into that branch. It
merges each candidate by hand, outside the case, and the case never records `pr.merged`.

## Shape

`--forge local` makes a local repository the forge: the tree's `origin` remote names a path on
the machine (a bare repository, the hub), outside the tree, so no step can move its refs
(D3 of story:judge-guard). Each case's tree is a `git clone --no-local` of the hub, on the source
branch, sharing no object file with it; a tree whose object files are the hub's (hard links or an
alternates file) is refused before a case opens. The case's base is that branch, as for every
forge, and the merge target is the same branch in the hub.

| action | the local forge |
|---|---|
| push | `git push` of the case's branch to the hub, no scan: nothing leaves the machine |
| `pr.open` | a request record in the hub (`<hub>/takt/requests/<n>.json`, `n` taken with an exclusive create, so runs at once never share a number): branch, base, title, body |
| read | the record, the head as the hub's `refs/heads/<branch>`, merged once the record says so |
| checks | the argument list the run file's `local_checks` names, run without a shell through the run's confinement in a fresh `--no-local` clone of the hub at the head; exit 0 pass, else fail; a verdict is kept per head and argument list. No `local_checks`, or one the confinement cannot run, and the run is refused before a case opens: no check is never a pass. Under the free-space floor (`--min-free-gib`) the checks are not run |
| merge | in the hub: a head the base already holds moves no ref and is recorded merged by the commit that holds it, a record believed when its merge is on the base and holds the head in its history; else `git merge-tree --write-tree` of base and head; a conflict is refused, naming the paths; else a merge commit under the run's commit identity, the message takt's merge message, and `update-ref refs/heads/<base> <new> <old>`, so a base another run moved meanwhile is refused, not overwritten |
| target tip | the hub's `refs/heads/<base>` |

A refused merge (conflict, base moved) ends the step as a forge refusal does: the case stays open
and a later run resumes it on the moved base.

## Check

```expected_before
command: grep -q '"local" =>' crates/takt-cli/src/main.rs && test -f crates/takt/src/forge/local.rs
exit: 1
healthy_exit: 0
output: ""
```

## Acceptance

1. The predicate above exits 0.
2. A test: a hub with branch `integration`, two clones each delivering a change through the local
   forge; both merge into `integration` in order, the second on the first's merge, and the case
   records `pr.merged` for each.
3. A test: two changes to the same lines; the second merge is refused naming the path, the hub's
   `integration` stays at the first merge.
4. A test: a base moved between the read and the merge is refused by the compare-and-swap.
5. `--forge local` without `local_checks`, or with an `origin` that is not a local path outside
   the tree, is refused before a case opens.
6. The ESS domain declares the local forge; `takt run --help` names it.
