---
format: aep.planning-md/3
id: approval-record:wave-fresh-repository-defects
kind: approval-record
status: approved
title: A wave of the defects a plain run meets on a fresh repository
summary: 'four units: the untracked Cargo.lock remedy, gate programs on a system path, a colon in a gate script, harness none after a grant'
relations:
- decides: story:untracked-cargo-lock-remedy
- decides: story:gate-commands-find-programs-on-a-system-path
- decides: story:a-gate-script-with-a-colon-is-run
- decides: story:harness-none-takes-a-granted-action
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-27T02:32:04Z", actor: "human:timo", revision: 2}
---
# Approval: a wave of the defects a plain run meets on a fresh repository

## Decision

The orchestrator, under the owner's grant (class C: a wave; commits, pushes and merges through the
bot route), 2026-10-27: "Implement the active wave-38 stories the carry did not cover, as one wave
of at most 4 units (haiku implementors, subagent_type aep:implementor model haiku; opus
aep:adversary per unit), each merged into integration/takt and pushed." and, on the defects the
site guides met: "Sequencing is yours inside the ≤4-unit cap: defects that stop a plain `takt run`
on a fresh repository (2, 3, 6) go before cosmetic ones."

The orchestrator, 2026-10-27 (its decision D65): "Remaining wave-38 stories: start each as soon as
its depends_on is merged, in parallel, implementors commit at green, merge+push at green, reviews
afterwards. My 01:46Z "one cargo tree at a time" order is withdrawn: builds run in parallel inside 40G
for takt; disk-guard covers the floor. [...] Still no PR, no merge to main." The units therefore run
in parallel as disk allows, each merges at green, and its adversary pass follows on
`integration/takt`.

The carry of wave 38 implements every active wave-38 story, defect (2) included
(story:fresh-home-first-test-run-is-not-refused), so the wave takes the defects next in that order.

## Units

Each in its own worktree, merged into `integration/takt` and pushed at green; the units share no
function, so none waits on another.

| unit | story | why in this wave |
|---|---|---|
| U1 | story:untracked-cargo-lock-remedy | defect (3): a repository without a committed `Cargo.lock` stops at step 1 |
| U2 | story:gate-commands-find-programs-on-a-system-path | defect (6): a gates document written by program name fails its gate |
| U3 | story:a-gate-script-with-a-colon-is-run | a gate script holding `:` ends `gate.run` in a git pathspec error |
| U4 | story:harness-none-takes-a-granted-action | defect (5): `--harness none` cannot finish a case after a grant |

Commits this approval covers: one per unit and its review fixes, the merge of each unit into
`integration/takt`, the store commits of the wave (this record, the stories' moves). Not covered: a
pull request, a merge into `main`, a tag or a release.

## Left out, and why

| story | reason |
|---|---|
| story:forge-none-merges-locally | the forge-adapter work lands on the same surface at the same time |
| story:a-granted-merge-is-taken-before-another-edit | open design question: the preference may belong in the protocol document, not in the frontier |
| story:cargo-home-sources-are-step-changes, story:a-refused-step-leaves-the-cargo-home-it-made | needs a way to tell cargo's own unpacking from a planted file before code |
| story:exit-4-reads-the-same-in-run-and-scenario-help, story:the-merge-driver-test-holds-under-load, story:the-active-budget-test-holds-under-load, story:cli-reference-is-generated-by-a-rust-crate and the site stories | cosmetic, test-load or documentation; after the defects that stop a run |
