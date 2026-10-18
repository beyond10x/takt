---
format: aep.planning-md/3
id: review-result:adversary-revision-pass-1
kind: review-result
status: active
title: Adversary, binary-embeds-revision, pass 1
relations:
- reviews: story:binary-embeds-revision
revision: 1
---
# Adversary pass 1: story:binary-embeds-revision

Unit at a295a53 (impl/revision, from 8b0afc5). Verdict: NEEDS-CHANGE. Cases: 526 → 530, 4 red, in
`crates/takt-cli/tests/revision_adv1.rs` (unedited by the implementor).

Attacked without a break: rebuild after 13 git operations in files and reftable ref formats (26 of
26); `status.branch=true`; a pre-commit hook in a main worktree; staleness after a build with
`GIT_DIR` set; old `events.jsonl` lines; every CLI run path reaching `finish`; RunOpened citations;
`build.rs` in `cargo package --list`; sha256 repositories.

```findings
[
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "build.rs git inherits GIT_DIR exported by git hooks, rebase -x and aliases in linked worktrees: a clean commit built via a git alias or rebase -x in a linked worktree prints <sha>-dirty (unknown revision without a root Cargo.toml)",
    "line": 23
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "untracked source compiled into the binary is not reported as dirty: a commit naming mod extra with src/extra.rs never added prints the clean sha of a commit that does not build",
    "line": 60
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "unknown revision build watches nothing and stays unknown after a commit: build before git init or while git refuses the repo, then commit: the binary still prints unknown revision",
    "line": 82
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "build script git runs code from git configuration (core.fsmonitor): a configured core.fsmonitor command runs during cargo build, against the repository's no-configured-code rule",
    "line": 23
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "every tracked file in the repository is watched, so mtime-only changes recompile the binary: touching any tracked doc or planning file re-runs the build script and recompiles takt-cli with an identical revision",
    "line": 107
  }
]
```
