---
format: aep.planning-md/3
id: review-result:adversary-finding-intake-pass-2
kind: review-result
status: active
title: Adversary, finding-intake, pass 2
relations:
- reviews: story:finding-intake
revision: 1
---
unit: story:finding-intake, worktree w8-finding-intake at 68aaec7 (base 4cbc096) plus one untracked test file
verdict: INFEASIBLE
cases: executed 341→343, red 2
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, the unit's scratch `adv2/`
needs-coordinator: none; one exploratory probe fetched public aep protocol sources over the network (a scratch HOME without the cache link)

**1. Diff.** `git --no-pager diff --stat` empty; `git status --short` shows `?? crates/takt-cli/tests/finding_scripts_adv2.rs`.

**2. Cases added** (alone: `FAILED. 0 passed; 2 failed`). Each first runs `takt scenario run` on the declaration and checks takt refuses it (passes), then runs `check-finding`.

| test | declaration | takt | expected | actual |
|---|---|---|---|---|
| `a_scenario_giving_its_seed_twice_gives_no_result` | `seed` twice | refuses, "gives `seed` twice" (`crates/takt-cli/src/main.rs:736-740`) | non-zero, empty stdout | `exit Some(0)`, `reproduced` |
| `a_scenario_giving_a_key_takt_does_not_read_gives_no_result` | adds `base = main` | refuses, "which takt does not read" (`main.rs:724-729`) | non-zero, empty stdout | `exit Some(0)`, `reproduced` |

`scripts/check-finding:129-132` reads only `seed` and `story` with `git config --get` (the last value); its header says the declaration is read as `takt scenario run` reads it.

**3. Gate.** fmt 0; clippy 0; `cargo test --workspace --locked --no-fail-fast` exit 101, 343 executed, 341 passed, 2 failed, all in `finding_scripts_adv2` (`finding_scripts` 9, `finding_scripts_adv1` 9 pass); `ess specify validate --path ess` 0.

**4. Attacked, not broken.**
- Acceptance end to end: real takt at b71cb85 on a read-only scenario-1 checkout, no network, fresh HOME: `reproduced` (takt exit 3, "on software.change/1"); at 4cbc096 `not_reproduced`; the checkout's HEAD and branches unchanged.
- The protocol-sources link: writable only on purpose (snapshot dirs 0555, files 0444); the predicate already runs as the user, so the link gives nothing new; a copy costs 2.4G and a read-only bind needs bwrap or a user namespace. Judged: the link stays.
- flock: a stale lock file is harmless; a killed holder lets the filing go ahead; a filing killed while waiting writes nothing and prints nothing; `-w 120` expiry exits 1 with nothing written (read from code); linked worktrees share the common dir.
- Store read: kind and name regexes reject `/` and `.`; a blocker lands where the script reads it.
- `--no-includes --blob` reads the declared ref; the id regex rules out `.` and quotes.
- `checkout -B trial/0` acts only in the clone; source refs unchanged.
- Output contract and exit codes 64, 2, 3, 1 hold. Pass-1 cases 9 of 9 green; the clippy `else` only reaps. All 9 `read from: scripts/...` citations resolve.

```findings
[
  {"file": "scripts/check-finding", "line": 129, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the scenario declaration is read with git config --get for seed and story only, so a declaration takt scenario run refuses (seed given twice, a key takt does not read) still yields a result, contrary to the header's as takt scenario run reads it"}
]
```
