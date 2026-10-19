# Wave 16: a delivery's candidate reaches a turn and merges only after its grant

Stage: **closed**. Shape: one checkout, one wave branch `wave/16` cut from `main`, units merged or
committed on it by the dispatcher, one adversarial review and one full gate before the close (the
owner's direction of 2026-10-20). Every unit is a judge surface; the merge into `main` needs the
planner's grant of the wave's exact commit, on a reviewer verdict.

## Units

| lane | unit | acceptance (condensed) |
|---|---|---|
| 1 | `story:takt-commits-with-the-repositorys-identity` | takt commits with `--commit-identity` or `.engineering/commit-identity.conf`, recorded in the case; with none, as `takt <takt@localhost>`, and a run that pushes is refused at open |
| 2 | `story:a-judge-surface-candidate-merges-after-its-grant` | the grant records the mainline's tip; `candidate.merge_judge_surface` records nothing for a candidate that tip already holds, or for a grant with no tip, an unfetchable one or one off the mainline |
| 3 | `story:a-delivery-hands-its-candidate-to-a-turn` | a forge-none delivery stopped at `pr.opened` replays its code commits on the base under the configured identity, keeps them under `refs/takt/candidates/<case-id>` and prints the candidate a turn opens on |
| 4 | `story:the-gate-compares-its-scripts-and-tool-config-at-the-base` | `gate.run` compares at the base every file a gate command takes as an argument, the tool configuration by file name, the lint tables and `rust-toolchain*` |

`story:a-turn-refuses-a-candidate-main-already-holds` was planned as lane 5 and moves to wave 17:
the four finished units do not wait on an unstarted one.

Lanes 1 to 4 were implemented and reviewed on their own branches before this wave opened, each
from wave 15's lane B tip; each carries one correction round for its review's findings.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at e9fa23f8; wave 15 (tip 0fae6305) merged in first |

## Log

- 2026-10-20: wave opened from `main` at e9fa23f8; wave 15 merged in (ff76126f); lanes 1 to 4
  merged (089c055a, 205d5dd1, a3f86d7d, cc9fd51e). Their conflicts were in
  `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv` and `CHANGELOG.md` only.
- 2026-10-20: `cargo build --workspace` exit 0; `cargo test --workspace --no-fail-fast` at cc9fd51e: 1372 passed, 5 failed, the five wave 15 tests whose fixtures pushed with no commit identity, fixed by 1258fded (dispatcher commit). Citations re-pointed (8304f628). `main` merged in after 0.3.0 (05f70c4f).
- 2026-10-20: adversarial review at 05f70c4f: needs-revision; the twelve findings of the lanes' own reviews fixed. Fix round 1: ebe93c9a (a turn resumed through another forge is refused), db3fd637 (a failed replay removes the earlier candidate's ref; the conflict refusal's wording; an exact exit assertion), 38d32d62 (changelog), 622e8a0d (ESS). Re-check at 622e8a0d: approve.
- 2026-10-20: `task check` at 622e8a0d: exit 0, 1381 passed, 0 failed. Granted by the planner on the approve at 622e8a0d; the four stories moved to implemented.
