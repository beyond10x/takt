---
format: aep.planning-md/3
id: story:a-dispatched-child-is-guarded-and-resumable
kind: story
status: draft
title: A dispatched child case is guarded at the root and resumable from it
summary: 'the five gaps the Workspace seam left: root records guard, CLI resume, gate reads, orphan trees, lease renewal'
relations:
- decomposes: epic:continuous-orchestration
- serves: vision:safe-agentic-coding
- depends_on: story:the-workspace-seam-enters-and-leaves-managed-worktrees
revision: 2
---
# Story: a-dispatched-child-is-guarded-and-resumable

## Outcome

A child case that the Workspace seam dispatched is as safe and as resumable as a case run in place:

- **The root's records are guarded.** A step in the child's tree cannot write the root's cases
  directory. The root cases directory is snapshotted around each judgement step, as the judge
  surface is (`design:continuous-orchestration` § 6), and a change there refuses the step.
- **It resumes from the command line.** `takt run --case <child>` from the root resumes the child in
  its recorded tree, without checking the child's branch out in the root checkout.
- **Gates read its records.** The gate validator accepts the child's case directory at the root.
- **No orphan tree.** A failure between `worktree create` and the `worktree` line in `case.yaml`
  archives that tree and releases its lease; a run that ends on an error releases its lease too.
- **The lease stays live.** A long child run renews its lease, so another session's cleanup does
  not take its tree.

## Context

- story:the-workspace-seam-enters-and-leaves-managed-worktrees built the seam and `Run::dispatch` in
  the library, tested against worktree 0.19.0. It left these five out, and no CLI reaches dispatch yet.
- takt has no `abandoned` outcome, so today only `Completed` finishes a child's tree.

## Acceptance

1. `dispatch_guards::a_child_step_writing_the_root_cases_dir_is_refused`
2. `dispatch_guards::a_child_resumes_from_the_root_by_case_id`
3. `dispatch_guards::the_gate_validator_reads_a_childs_records_at_the_root`
4. `dispatch_guards::a_failure_before_the_tree_is_recorded_leaves_no_leased_tree`
5. `dispatch_guards::a_long_child_run_keeps_its_lease`
