---
format: aep.planning-md/3
id: approval-record:wave-38-trial-s7-defects
kind: approval-record
status: approved
title: 'Wave 38: the defects of the second kind-cluster trial, and release 0.14.0'
tags:
- loom-agent
relations:
- decides: story:repeated-identical-test-failures-stop-the-run
- decides: story:a-red-base-stops-at-open
- decides: story:edits-keep-ess-citations-resolved
- decides: story:fresh-home-first-test-run-is-not-refused
- decides: story:staging-tests-pass-under-a-shared-temp-directory
- decides: story:a-harness-step-without-an-action-is-retried
- decides: story:run-configuration-names-the-story-and-token-variable
- decides: story:run-state-directory-is-configurable
- decides: story:scenario-run-takes-config
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T20:53:18Z", actor: "human:timo", revision: 2}
---
# Approval: wave 38, the defects of the second kind-cluster trial

## Decision

The orchestrator, under the owner's grant (class C: wave, bot commits, pull request, merge,
release), 2026-10-26: "the defects the one-hour cluster run (S7) exposed ... Stories, already
drafted in the store on main (commits 368e3562 and c26afe32; accept them under the grant) ...
Order the wave by value: 1, 2, 8 first, then 3-7, 9. Each unit: failing test first, smallest
change, adversary pass, merge through the bot route. Then release the next minor". On
story:edits-keep-ess-citations-resolved: "decide the mechanism in the story ... a model that is
told "20 unresolved citations" repeatedly without the list cannot act on it."

The trial: 12 runs, 101 steps, 17,200,642 tokens, 0 accepted results; runs 7 and 8 spent
13,792,661 tokens editing against a test suite red for an environment reason while every edit
left 20 citations unresolved.

## Units

| unit | stories |
|---|---|
| U1 red tests stop the run | story:repeated-identical-test-failures-stop-the-run, story:a-red-base-stops-at-open |
| U2 the failed gate's lines reach the next step | story:edits-keep-ess-citations-resolved |
| U3 fresh HOME and shared temporary directory | story:fresh-home-first-test-run-is-not-refused, story:staging-tests-pass-under-a-shared-temp-directory |
| U4 a step without an action is retried | story:a-harness-step-without-an-action-is-retried |
| U5 run configuration | story:run-configuration-names-the-story-and-token-variable, story:run-state-directory-is-configurable, story:scenario-run-takes-config |

Commits this approval covers: one per unit and its review fixes, the store commits of the wave
(this record, the stories' moves, the close), the merge of `wave/38` into `main`, and release
0.14.0 through the repository's release process. Not covered: the container image (rebuild and
push are due, not done).

## Decisions taken under it

| fork | taken | why |
|---|---|---|
| the citations mechanism | a failed `gate.run` keeps the failing command's own output lines that name what failed (bounded in bytes), and the next judgement step's prompt shows them under the failed gate's id; takt re-points nothing itself and the handover is not refused | the gate commands are the repository's (`engineering-assertions-project/1` has no repair command), so a re-point in takt would hard-code one repository's script into a binary that runs on any; the handover's gating is protocol data, not takt code (`AGENTS.md` § Rules); the model can re-point once it sees each `<doc>:<line>: unresolved` line |
| repeated failures | 3 consecutive `tests.run` results failing the same set of tests at different revisions stop the run, exit 3, naming the tests and the count | the trial's runs 7 and 8 repeated one set 18 times |
| a red base | the case's first `tests.run`, before any effect, failing stops the run at open, exit 3, naming the failing tests; the protocol opt-in to continue is left out | the item: "a stop at open, with the failure recorded, not an edit loop" |
| harness retry bound | a judgement step whose session names no action is tried at most 3 times in one run | the item: "bounded retry; the harness-side cause is a separate harness item" |
| unit shape | one managed worktree, one branch `wave/38`, one `target/`; implementor sub-agents edit inside their scope and commit nothing; each unit committed by the dispatcher through the bot route; one adversary pass per unit | `AGENTS.md` § Rules and `.agents/dispatcher-agent.md`, and the item's "adversary pass" per unit |
| version | 0.14.0 | the item; new configuration keys and new stops are additions |
