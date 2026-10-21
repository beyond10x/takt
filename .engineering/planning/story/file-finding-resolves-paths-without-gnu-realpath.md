---
format: aep.planning-md/3
id: story:file-finding-resolves-paths-without-gnu-realpath
kind: story
status: implemented
title: file-finding resolves its paths without GNU realpath -e
tags:
- finding
- macos
refs:
- provider: finding
  reference: file-finding-resolves-paths-without-gnu-realpath
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T14:44:43Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T14:44:43Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-21T22:46:01Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# file-finding resolves its paths without GNU realpath -e

## What is wrong

`scripts/file-finding` resolves `--from` and the predicate's `source` with `realpath -e`
(`scripts/file-finding:290`, `:309`). macOS's realpath has no `-e`: it prints
`realpath: illegal option -- e` and its usage, and file-finding stops before it reads the body.

The darwin job (`.github/workflows/darwin.yml`) at e10d0a6, run 39614054772 on macos-15: 44 tests
fail on it, 34 in `crates/takt-cli/tests/finding_scripts.rs`, 8 in `finding_scripts_adv1.rs` and
2 in `finding_scripts_adv2.rs`; the job log holds the line `realpath: illegal option -- e` 46
times. Before units A to C of wave 28 these tests stopped earlier, at bash 3.2's missing
`declare -A`.

## Change

Resolve both paths in a form bash 3.2 and both realpaths accept: the path must exist (`-e`),
then the physical path (`cd -P` on its directory and `pwd -P`, or `realpath` without `-e` after an
existence test). Same results on Linux for every path the tests give, including a link, a missing
file and a path outside the repository. `scripts/check-finding` uses no `realpath -e`. Its
reviewed digests (`CONFINED_CHECK_FINDING_SHA256S`, `crates/takt/src/improvement.rs:59`) do not
change, because only file-finding changes.

## Check

`scripts/check-finding` cannot run this predicate: it runs on Linux under bubblewrap, where
`realpath -e` works. The machine that checks it is the darwin job on macos-15.

```expected_before
host: macos-15, .github/workflows/darwin.yml job test-cli
command: cargo test -p b10x-takt-cli --test finding_scripts
exit: 101
healthy_exit: 0
output: realpath: illegal option -- e
source: .github/workflows/darwin.yml
```
