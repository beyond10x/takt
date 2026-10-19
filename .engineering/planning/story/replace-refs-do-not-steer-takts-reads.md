---
format: aep.planning-md/3
id: story:replace-refs-do-not-steer-takts-reads
kind: story
status: draft
title: A step's refs/replace/ entries do not change what takt's git reads see
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:scan-before-push
revision: 1
---
## Outcome

A step that writes under `refs/replace/` (or sets `GIT_REPLACE_REF_BASE` or `core.useReplaceRefs` anywhere a step can write) cannot change what any git read takt runs as the judge returns. Either every git call takt makes runs with `GIT_NO_REPLACE_OBJECTS=1`, or a step that creates or changes a ref under `refs/replace/` is refused and undone.

## Context

From the developer's adversary passes on story:scan-before-push (2026-10-19). The scan before push ignores replacements since correction 2 (c888209), but takt's other git reads still follow them, for example the check that the case records are on top before a merge. story:loop-scorecard sets `GIT_NO_REPLACE_OBJECTS` in `git::output` on its own branch, which may cover the reads once both units are in wave 12; whether it covers every call site (not only `git::output`) has not been checked.

## Scope

- `crates/takt/src/git.rs`: the environment of every git call (inferred)
- `crates/takt/src/run.rs`: the step guard over refs a step writes (inferred)

## Acceptance

On scenario 1 with a fake harness whose step runs `git replace <the base tip> <a commit that drops the case records>`, every later takt read (records on top, change.read, the merge check) reports what it reports without the replacement, or the step is refused and the ref removed.
