# Wave 19: the gate reads Python through a shell, check-finding runs one at a time, the trial agent reports

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/19` cut from
`main`, implementor sub-agents in the shared tree, one adversarial review and one full gate before
the close. Every unit is a judge surface.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:gate-compares-python-run-through-a-shell` | a gate running Python through `sh -c` or a shell script compares the script and its imports at the base; 26 of the review's 34 probed shell forms closed, 5 filed as story:the-gate-reads-every-shell-form | rsi |
| B | `story:check-finding-runs-one-at-a-time` | one check-finding build at a time per machine; a refusal under 20 GiB free; a killed run leaves no lock | rsi |
| C | `story:trial-agent-reports-to-the-monitor`, `story:forge-none-help-names-the-unperformable-stop`, `story:commit-identity-refusal-names-the-identity-file` | the trial agent reports to the owner's monitor and hands it proposals for other repositories (ADR 0101); the `--forge none` help names the Unperformable stop; the identity refusal names the identity file | rsi |

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 8eb9b3a5 (wave 18 merged, 0.4.0) |

## Log

- 2026-10-20: wave opened from `main` at 8eb9b3a5.
- 2026-10-20: units committed: C 3024b92d, B 3a7fc1fd (with the new check-finding digest beside the old one; check.yml sets `CHECK_FINDING_MIN_FREE_GIB=0`), A 1f9a2945; ESS 8e3b2862. `cargo test --workspace --no-fail-fast` at 1f9a2945: 1425 passed, 0 failed.
- 2026-10-20: adversarial review at 8e3b2862: needs-revision; of its 34 shell-form probes, 11 still let a change decide the gate. Fix round 1: db2f5e5f closes 3 (`exec` options, a shell reading a tree file on standard input, glob and brace words); 3 more run the code under work at the root by wave 18's decision; 5 remain, each an ignored test, filed as story:the-gate-reads-every-shell-form. ESS e28ac930.
- 2026-10-20: `task check` at e28ac930: exit 0, 1429 passed, 0 failed, 11 ignored. The five stories moved to implemented. A delivery on takt that changes a `.py`, `.sh` or program file stops takt's own gates until story:gate-exec-is-readable-by-the-gate, wave 20's first unit.
- 2026-10-20: the confirm at 5159fe81: needs-revision; a descriptor before a redirection (`sh < FILE 2>&1`, `sh 0< FILE`) let a change decide the gate. Fix round 2: 126498f7 reads it as part of the redirection; of 22 further probed forms 20 are closed, 2 (`cd` forms) filed. `task check` at 126498f7: exit 0, 1437 passed, 0 failed, 11 ignored.
- 2026-10-20: the final confirm at 2984e771: the redirect forms hold; `exec 0< FILE; sh` (and two siblings) and `env -C dir python3 -m x` slip, at `main` too, while the changelog claimed the class closed. Corrected by documentation, not code: the four forms and the two `cd` forms are ignored tests naming story:the-gate-reads-every-shell-form (eleven in all), and the changelog states the counts.
