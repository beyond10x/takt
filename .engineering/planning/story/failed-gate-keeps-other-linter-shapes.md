---
format: aep.planning-md/3
id: story:failed-gate-keeps-other-linter-shapes
kind: story
status: implemented
title: A failed gate drops failing lines of other linters' shapes, uncounted
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: failed-gate-keeps-other-linter-shapes
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: crates/takt/tests/failed_gate_lines.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T21:15:41Z", actor: "human:timo", revision: 3, executor: "agent:planner"}
- {from: "proposed", to: "active", at: "2026-10-28T21:15:42Z", actor: "human:timo", revision: 4, executor: "agent:developer"}
- {from: "active", to: "implemented", at: "2026-10-29T21:30:13Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# A failed gate drops failing lines of other linters' shapes, uncounted

## What is wrong

Beside a failed test run, `failing_lines` (`crates/takt/src/validator.rs:222` at 7c9e7129) keeps
another command's failing line only when `failure_line` (`crates/takt/src/validator.rs:518`)
reads it as a compiler's `error:` line or a located finding `<path>:<line>[:<col>]: <word>`,
`<path>` a file's (`file_path`, `:505`). Every other line is dropped and not counted, so the next
step's prompt says nothing was left out (omitted 0) while a check failed for a reason it does
not see. Wave 39's reviewer found these failing lines neither kept nor counted (probes f1c and
s6, at 7c9e7129):

- ruff 0.12's full format: `F401 [*] … imported but unused`, then ` --> src/app.py:1:8`
- black: `would reformat src/app.py`
- rustfmt `--check`: `Diff in /w/src/lib.rs at line 3:`
- tsc: `src/a.ts(3,5): error TS2322: …`
- a C compiler: `x.c:3:10: fatal error: y.h: No such file or directory`
- a path with no extension: `Dockerfile:3: error: …`
- an address: `127.0.0.1:8080: error: refused`

And one line that names no file is kept as if it were a finding: `example.com:443: error: refused`
(`file_path` reads `.com` as an extension).

## Check

Not runnable by `scripts/check-finding`: the command builds a probe test with cargo and does not
run `$TAKT`. Run it in a fresh checkout of the revision. The reviewer's probes f1c and s6 gave
these results at 7c9e7129 (f1c: kept 1, the test, omitted 0); this command was not run there,
because the wave's gate was building in that tree.

```expected_before
command: printf '%s\n' '#[test] fn probe() { let tests = "============ short test summary info ============\nFAILED tests/t.py::test_x - assert 1 == 2\n"; let mut wrong: Vec<String> = Vec::new(); for line in ["F401 [*] os imported but unused\n --> src/app.py:1:8", "would reformat src/app.py", "Diff in /w/src/lib.rs at line 3:", "src/a.ts(3,5): error TS2322: Type string is not assignable to type number.", "x.c:3:10: fatal error: y.h: No such file or directory", "Dockerfile:3: error: unknown instruction", "127.0.0.1:8080: error: refused"] { let (kept, omitted) = takt::validator::failing_lines(&format!("{line}\n{tests}"), 4096); let first = line.lines().next().unwrap(); if omitted == 0 && !kept.iter().any(|k| k.contains(first)) { wrong.push(first.to_owned()); } } let (kept, _) = takt::validator::failing_lines(&format!("example.com:443: error: refused\n{tests}"), 4096); if kept.iter().any(|k| k.contains("example.com")) { wrong.push("kept example.com".to_owned()); } assert!(wrong.is_empty(), "failing lines lost or a non-failure kept: {wrong:?}"); }' > crates/takt/tests/zz_finding_other_linter_shapes.rs && cargo test -p b10x-takt --test zz_finding_other_linter_shapes
exit: 101
healthy_exit: 0
output: failing lines lost or a non-failure kept
source: docs/waves/wave-39.md
```

## Outcome

Beside a failed test run, each failing line of another command in these shapes is kept or, when
it does not fit, counted as left out; a line that names a host, not a file, is not kept as a
finding.

## Acceptance

1. Each of the seven failing lines above, printed beside a failed pytest summary, is kept or
   counted in the omitted figure; the predicate above passes (exit 0).
2. ruff's full format keeps its code line with its ` --> <path>:<line>:<col>` line, or counts
   both.
3. `example.com:443: error: refused` is not kept as a failing line.
4. Every line the wave-39 tests keep, drop or withhold today (`crates/takt/tests/failed_gate_lines.rs`,
   `crates/takt/tests/failed_gate_lines_adv1.rs`) gives the same result; a warning, a note, a
   timestamp and a passing test's line are still not kept.

## Scope

- `crates/takt/src/validator.rs`: `failure_line` and `file_path` (cited)
- `crates/takt/tests/failed_gate_lines.rs`: the shapes above beside a failed test run (cited)
