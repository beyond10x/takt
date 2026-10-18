---
format: aep.planning-md/3
id: review-result:adversary-revision-pass-2
kind: review-result
status: active
title: Adversary, binary-embeds-revision, pass 2
relations:
- reviews: story:binary-embeds-revision
revision: 1
---
# Adversary pass 2: story:binary-embeds-revision

Unit at dd9bf05 (impl/revision; unit a295a53, correction 1 dd9bf05). Verdict: CONFIRMED, 5 red cases
in `crates/takt-cli/tests/revision_adv2.rs`; 533 → 538 executed. None is a blocker.

Attacked without a break: the env scrub (GIT_CONFIG_GLOBAL/SYSTEM, HOME, XDG_CONFIG_HOME,
includeIf against `-c`, GIT_CONFIG_KEY without COUNT, GIT_EXEC_PATH, GIT_OPTIONAL_LOCKS, GIT_TRACE);
textconv and diff.external; safe.directory; pathspecs with spaces, globs and magic-looking names;
Cargo.lock; a normal checkout stays Fresh.

```findings
[
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "an untracked file compiled in from outside crates/ leaves the revision clean: protocol.rs EMBEDDED gains include_str! of protocols/x/1.yaml never added: the binary prints the clean sha of a commit that does not build",
    "line": 110
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a directory under crates/ holding an ignored path is not watched itself, so a new file beside it does not rerun the script: with an ignored file in crates/x/src, a new src/extra.rs after a clean build leaves the binary clean while the script says -dirty",
    "line": 253
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "info/exclude is not watched: removing an info/exclude line that hid an untracked file under crates/ leaves the binary clean while the script says -dirty",
    "line": 187
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "an unknown-revision build is not rerun after git init and a first commit at the workspace root: build a workspace before git init, then init and commit at its root: the binary still prints unknown revision",
    "line": 183
  },
  {
    "file": "crates/takt-cli/build.rs",
    "category": "correctness",
    "severity": "note",
    "message": "git status in the build script runs a configured clean filter, against the no-configured-code contract: a repository whose .gitattributes names a filter (git-lfs) runs filter.<driver>.clean during cargo build on any stat-dirty tracked file",
    "line": 27
  }
]
```
