---
format: aep.planning-md/3
id: review-result:adversary-followups-pass-1
kind: review-result
status: active
title: Adversary, wave11-review-follow-ups, pass 1
relations:
- reviews: story:wave11-review-follow-ups
revision: 1
---
# Adversary pass 1: story:wave11-review-follow-ups

Unit at 7f1eeb0 (impl/followups). Verdict: NEEDS-CHANGE, 4 red cases in
`crates/takt/tests/pr_opens_when_done_adv1.rs` and `crates/takt-cli/tests/revision_build_adv1.rs`;
suite 722 passed, 4 failed.

Attacked without a break: item 3 (info/attributes filters, required process filters, dotted and
capitalised driver names); item 4 (untracked .cargo symlink); item 6 (tag named like the base, base
gone); item 9 (every story too long); item 12 (every strip of takt/ and every join to the cases
directory runs check_id); item 2 (changes() refuses a non-UTF-8 path).

```findings
[
  {
    "file": "crates/takt/src/validator.rs",
    "category": "correctness",
    "severity": "blocker",
    "message": "change.read reads none for a conflicted merge whose written tree keeps the base side: case deletes a file the base modified, or both change a binary file: merge-tree exit 1, merged tree equals tip, result none, pr.open never admitted",
    "line": 1768
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a root .cargo/config.toml made after a build is compiled in under a clean revision: clean build, then add .cargo/config.toml with [env], build again: cargo recompiles with the env, the script does not rerun, the binary prints the clean sha",
    "line": 22
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "a submodule's own filter driver runs during the build: submodule with its own filter clean and info/attributes, file stat changed: the clean command runs; this repository has no .gitmodules",
    "line": 243
  },
  {
    "file": "crates/takt/src/validator.rs",
    "category": "correctness",
    "severity": "note",
    "message": "judged() decodes git::bytes output lossily: git::bytes says a non-UTF-8 path is the caller's to refuse; judged() decodes ls-tree and diff-tree output with from_utf8_lossy",
    "line": 142
  },
  {
    "file": "crates/takt-cli/tests/version_cli.rs",
    "category": "correctness",
    "severity": "note",
    "message": "version_cli cannot fail on an unknown revision in this checkout: binary output compared with env!(TAKT_REVISION) from the same build script; the sha checks are skipped when it is unknown revision",
    "line": 63
  }
]
```
