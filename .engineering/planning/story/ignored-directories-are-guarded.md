---
format: aep.planning-md/3
id: story:ignored-directories-are-guarded
kind: story
status: draft
title: A step cannot change what an ignored directory already holds
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
revision: 2
---
## Outcome

A harness step's write inside a git-ignored directory that existed before the step (`__pycache__/`, `target/`, a vendored directory) is detected like any other write: refused when it is on the judge surface or can decide a later test run, and undone. The step guard compares the contents of pre-existing ignored directories, not only their names.

## Context

From the adversarial review of main at bd20f7b on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), finding A5.1, confirmed and raised to high. `git status --ignored=matching` lists an ignored directory as one entry (`crates/takt/src/git.rs:372@bd20f7b`), `Content::of` reports it as `Dir` (`:544-546`), and `Checkpoint::change` skips `Dir`→`Dir` (`:1333`); the judge surface reads root names only (`crates/takt/src/run.rs:1532-1560`); `tests.run` may create ignored paths and they are kept (`run.rs:3586-3596`) and runs in the same tree (`validator.rs:373-374`). Replay by the fact-checker: the first `tests.run` creates `pkg/__pycache__/`; a step plants a `.pyc` compiled from `return 2` with the source's mtime and size; status is unchanged, the step is not refused, and the next `tests.run` prints PASS with exit 0 on a source whose `f()` returns 1. The `target/` route (a test binary cargo considers fresh) is inferred. story:tree-hygiene left this gap; story:confinement-seam does not close it, the directory being inside the tree.

## Scope

- `crates/takt/src/git.rs`: `Content::of`, `Checkpoint::change` (cited)
- `crates/takt/src/run.rs`: the step guard's handling of ignored paths (cited)

## Acceptance

With the fact-checker's input (`.gitignore` `__pycache__/`, `pkg/mod.py` returning 1, a test asserting 2, one failing `tests.run`, then a harness step that writes a planted `pkg/__pycache__/mod.cpython-*.pyc`), the step is refused naming the planted path, the file is removed, and the next `tests.run` records fail.
