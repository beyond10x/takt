---
format: aep.planning-md/3
id: story:adopted-orphan-group-leaders-are-reaped
kind: story
status: draft
title: An adopted orphan that leads its own process group is reaped while a child runs
summary: the reaper skips every group leader, so a setsid daemon's zombie outlives its killed group until the next sweep
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
- informed_by: story:adopted-orphans-are-reaped-while-a-child-runs
revision: 2
---
# Story: An adopted orphan that leads its own process group is reaped while a child runs

## Outcome

A repository whose test daemonizes a process (`setsid`) and later kills that daemon's group sees the group gone at once under takt, as it is under `init`, instead of its zombie answering signal 0 until takt's next sweep.

## Context

story:adopted-orphans-are-reaped-while-a-child-runs added a reaper thread (`leftovers::adopt`, `crates/takt/src/forge.rs`) that reaps zombie children of takt that do not lead a process group and are not in takt's group. It skips every group leader, because takt's own direct children (`run_child`, the Substrate local leader, the gate's provider commands) lead their groups and are waited on by their callers. An adopted orphan that leads its own group is therefore left until the next sweep.

The security pass on that story reproduced it (`orphaned_group_leader_is_reaped_while_the_child_runs`) and a second boundary: a direct child started through `takt::confinement::Command::process_group(<another child's group>)` loses its exit status to the reaper (`a_child_started_into_another_childs_group_keeps_its_exit_status`; nothing in takt or what it links starts one). Both cases are kept, not built, in `crates/takt/tests/pending/adopted_orphans_reaped_sec1.rs`.

Possible mechanisms: register every group leader takt starts and reap any other adopted leader, or move subreaping into a helper process that owns no waited-on children.

## Acceptance

- `orphaned_group_leader_is_reaped_while_the_child_runs` passes as a built test (`crates/takt/tests/adopted_orphans_reaped_sec1.rs`), and `adopted_orphans_reaped.rs` still passes.
- `a_child_started_into_another_childs_group_keeps_its_exit_status` passes, or the public `process_group` refuses a group other than 0 or the child's own.

## Out of Scope

- Orphans that join takt's own process group, which a sweep handles as before.
