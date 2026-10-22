---
format: aep.planning-md/3
id: dependency-blocker:loom-handed-frontier
kind: dependency-blocker
status: cleared
title: Loom hands every performed action, so a grant-only or unperformable case ends on the host stop or the idle bound
relations:
- blocks: story:the-loop-is-run-until-blocked
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-22T13:02:27Z", actor: "human:timo", revision: 4}
---
# Blocker: Loom hands every performed action, so its stops come before takt's

## What is missing

Loom 0.34.0 (tag 8cfac2cc; paths under `crates/loom-commission/src/`) asks `host_stop` before
the executor whenever the handed frontier admits an action without a grant (`runtime.rs:1738-1746`,
`3645-3648`). The handed frontier keeps every admissible action the effect port performs
(`runtime.rs:3179-3197`), and `performs` is fixed per action, "at all, whatever its arguments"
(`ports/effect.rs:41-45`). `NoPerformableAction` fires only when no listed action is performed at
all (`runtime.rs:1593-1597`, `2466-2507`, `outcome.rs:184-197`).

takt decides its stops on obligations: `Shape::only_behind` and `Shape::unperformable`
(`crates/takt/src/run.rs:9984-10004`) ask whether an open obligation can still be moved by
anything but a grant, or by anything this run performs. A delivery case always lists
`tests.run`, `gate.run`, `change.read`, `implementation.edit`, `tests.write` and
`repository.inspect` as admissible, so Loom never sees a case that only a grant can move.

## Observed (2026-10-22)

A probe test against loom-sdk 0.34.0, deleted after one run: Loom's `CanonGovernor`, a port that
performs the 6 validator and 4 harness actions, an executor that proposes `repository.merge`,
`approval_gate: false`, an idle bound of 1, a step budget of 12.

| fixture | takt today | no-grant actions handed | Loom, disk fine | Loom, disk low |
|---|---|---|---|---|
| grant (`crates/takt/tests/free_disk.rs:512-532`) | ApprovalRequired, exit 4 | 8 | NeedsAuthority after 1 executor call | HostStop DiskLow, 0 calls, exit 8 |
| ahead (`free_disk.rs:535-548`) | Unperformable (pr.opened), exit 7 | 6 (`pr.open` admissible, no performer) | IdleBound 1, exit 3 | HostStop DiskLow, exit 8 |

## What clears it

A Loom release in which a run ends `NeedsAuthority` before the host stop when only a grant can
move an open obligation, and ends `NoPerformableAction` naming the claim when the only action that
decides an open claim has no performer, though other actions are performed. Workarounds rejected
as takt-side substitutes: a `performs()` that answers by case state; an executor that returns
takt's own stop.

dependency-blocker:loom-runtime-stops was cleared on a reading of 0.34.0; this probe shows the
reading missed the handed frontier.

## Decision (2026-10-22)

The orchestrator, under the owner's delegation: "A. Loom gets the change (NeedsAuthority before
host_stop when only a grant moves an open obligation; NoPerformableAction naming the claim when
the deciding action has no performer). It is approved to loom as a wave toward 0.35.0. ... The
exit-7 conditions from my last message still apply when the story resumes."

Option B, not taken: accept exit 8 where takt gives 4 and exit 3 where it gives 7.

## Worked around (2026-10-22)

Loom reported that both stops break its documented contract (loom `AGENTS.md:329-338` on where
the host stop is asked, and `:378`, "the frontier relates no action to a claim"), so the Loom
change of the decision above is withdrawn. The orchestrator approved option C instead: takt runs
`Shape::only_behind` and `Shape::unperformable` on its case before every Loom step and stops 4 or
7 itself (approval-record:loop-pre-checks-ahead-of-loom). The blocker is cleared as worked around;
a Loom contract change that would make the checks unnecessary is the owner's.
