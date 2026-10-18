---
format: aep.planning-md/3
id: review-result:adversary-step-forge-pass-2
kind: review-result
status: active
title: Adversary, step-cannot-forge-its-validators, pass 2
relations:
- reviews: story:step-cannot-forge-its-validators
revision: 1
---
unit: story:step-cannot-forge-its-validators, worktree w10-step-forge at 7fb9313 (correction 1) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 489→503, red 12
origin: introduced 2 / pre-existing 9 / undecided 1
wrote-outside-worktree: the unit's scratch `adv2/`
needs-coordinator: build-tool config (.npmrc) on the surface (decided: yes); fail closed on unknown programs (decided: yes)

**Cases** (`crates/takt/tests/step_cannot_forge_adv2.rs`; alone: 2 passed, 12 failed)

| case | expected | head | base |
|---|---|---|---|
| `[package] # ]`, then `build = "gen.rs"` | refused | not refused | red |
| `"build" = "gen.rs"` | refused | not refused | red |
| `description = """x""""`, then `build`, then `# "` | refused | not refused | red |
| a dependency under `[target.'cfg(unix)'.dependencies]` | kept, pass | refused | green |
| `.github/copilot-instructions.md -> ../AGENTS.md` at base, step fixes code | kept, pass | refused before the harness | green |
| `bash -c -- ./check.sh` | no pass | pass | red |
| `bash -c './check.sh'` | no pass | pass | red |
| `perl -I lib t/check.pl` | no pass | pass | red |
| `taskset -c 0 ./check.sh` | no pass | pass | red |
| `sh ../repo/check.sh` | no pass | pass | red |
| `npm test`, `.npmrc` gets `script-shell=/usr/bin/true` | no pass | pass | red |
| `scripts_of(deno run --allow-read tests/check.ts)` | names the script | `["--allow-read","deno"]` | — |

`cargo metadata --offline --no-deps` lists `gen.rs` as the `custom-build` target for all three manifests.

**Gate.** `task check` exit 201 at `step_cannot_forge_adv2`; without fail-fast 491 passed, 12 failed, 2 ignored. Confinement tests 17, 8, 2 pass; adv1 7 pass, 2 ignored.

**Attacked, not broken.** Quoted keys, whitespace in keys, dotted `package.build`, CRLF, a comment hiding `]` in an array, a root inline `profile`, `build.workspace = true`; nested wrappers, `-eo`, `--` after a wrapper, `env -S sh`, `npm --prefix`/`run-script`, `make -C -f`, absolute paths; a `docs` link off the surface; hard links; a link created and removed in one step.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 1349, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "a header comment holding a bracket renames the table for the scanner, so a build key under it is not refused while cargo runs gen.rs as the build script"},
  {"file": "crates/takt/src/run.rs", "line": 1243, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "key_parts does not decode TOML escapes, so a unicode-escaped build key names a build script the guard and tests.run do not see"},
  {"file": "crates/takt/src/run.rs", "line": 1280, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "a multi-line string ending in four quotes desyncs the scanner, which swallows the following build line as string content"},
  {"file": "crates/takt/src/run.rs", "line": 1405, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "any edit under [target.*] is refused, so adding a unix-only dependency is a false refusal"},
  {"file": "crates/takt/src/run.rs", "line": 595, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "npm's .npmrc is not on the surface, and script-shell=/usr/bin/true turns a failing npm test into tests.pass"},
  {"file": "crates/takt/src/run.rs", "line": 1014, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "a program outside the wrapper table (taskset -c 0 ./check.sh) still hides the script; the table does not fail closed"},
  {"file": "crates/takt/src/run.rs", "line": 789, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "after sh -c, a -- separator is taken as the program"},
  {"file": "crates/takt/src/run.rs", "line": 836, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "the -c command word is not shell-unquoted"},
  {"file": "crates/takt/src/run.rs", "line": 408, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "perl's -I takes the next word, and the guard takes that directory for the script"},
  {"file": "crates/takt/src/run.rs", "line": 634, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "normalise drops a path that climbs above the tree and comes back"},
  {"file": "crates/takt/src/run.rs", "line": 979, "category": "boundary", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "after deno run, the first option is taken for the script"},
  {"file": "crates/takt/src/run.rs", "line": 2678, "category": "acceptance", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "any committed link under .github/ or .engineering/ refuses every judgement step, locking out a repository whose link decides nothing"}
]
```
