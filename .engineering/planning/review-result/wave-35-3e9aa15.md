---
format: aep.planning-md/3
id: review-result:wave-35-3e9aa15
kind: review-result
status: active
title: Grant review, wave 35 takt on macOS and loom 0.37.0, at 3e9aa15
relations:
- reviews: story:takt-runs-its-tests-on-macos
- reviews: story:takt-confines-on-macos
- reviews: story:wave-35-review-fixes
revision: 1
---
approve
Grant review of wave/35 at 3e9aa15, the diff since needs-revision at 9adc328 (U4, U4b, U4c, loom 0.37.0, and the CHANGELOG resolution of the merge of main at f4c377a).

| check | result |
|---|---|
| H1, M2, M3, M4, M5, L6, L8, L9 at 9adc328 | fixed: `seatbelt.rs:64-69,73,104,234,296-340,385,391-392,406`, `release.yml:70-75,134`, `darwin.yml:7-13,46`, `forge.rs:711-758`, `promotion.rs:1874-1918`, `marked.rs:153`, `improvement.rs:60-69`, `scripts/check-finding`; each with a test |
| L7 | accepted, unchanged, as story:wave-35-review-fixes records |
| `CONFINED_CHECK_FINDING_SHA256S` | two entries: `6c39e63b…` (reviewed at 9adc328) and `59080409…`, equal to `sha256sum scripts/check-finding` at 3e9aa15 |
| release gate | `publish` needs `darwin` with no `if:`; a tag push runs darwin |
| U4b and U4c tests | none weakened: Linux assertions unchanged, macOS asserts the denial; the fresh-home exception admits one entry equal to `$TMPDIR` |
| U4c zombie skip (`marked.rs:90-92`) | leaves no live child unkilled; a live child whose `getsid` fails is still caught by the process-group test (`:95-97`) |
| loom 0.37.0 (c9319f9) | `Cargo.toml` tag, two README lines, 13 loom entries in each lockfile |
| CHANGELOG merge | `[0.9.0]` equals main's; only Unreleased lines added |
| tests | Linux, the changed suites at 3e9aa15: 8 results, 221 passed, 0 failed; darwin run 39936935309 at 3e9aa15: test-library and test-cli success |
| authority | narrowed only: Seatbelt denies added, write allowances dropped, a gate before publish |

Findings N1 to N4 are pre-existing and are filed in story:sibling-git-dirs-are-sealed.

```findings
- file: crates/takt/src/git.rs
  line: 1493
  category: security
  severity: warning
  verdict: approve
  origin: pre-existing
  message: "a sibling worktree's commondir and gitdir are neither guarded nor sealed; filed as finding:sibling-commondir-is-writable"
- file: crates/takt/src/seatbelt.rs
  line: 327
  category: security
  severity: note
  verdict: approve
  origin: pre-existing
  message: "in a linked worktree the common dir is not sealed as a literal; filed as finding:linked-common-dir-can-be-moved"
- file: crates/takt/src/seatbelt.rs
  line: 391
  category: security
  severity: note
  verdict: approve
  origin: pre-existing
  message: "a hard link in the tree to a sealed git config may pass the literal seal; the content guard restores the file"
- file: .github/workflows/darwin.yml
  line: 15
  category: reliability
  severity: note
  verdict: approve
  origin: pre-existing
  message: "the concurrency group of a release backfill on main collides with a manual darwin dispatch on main; the release fails closed"
- file: crates/takt/src/marked.rs
  line: 90
  category: reliability
  severity: note
  verdict: approve
  origin: introduced
  message: "zombies of takt's unwaited children are left to their waiter"
```
