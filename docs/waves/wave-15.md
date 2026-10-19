# Wave 15: the first self-improvement round: a judge on the whole corpus, runs that stop by name, unique trials, tests that hold under load, kept transcripts

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed**. Approval: the planner role granted this lane plan with eight rulings, after the owner's word to keep going and self-improve; the units are the stories filed from the self-improvement trials of 2026-10-19. Every unit is a judge surface; each merge needs the planner's grant of its exact commit, on a reviewer verdict. Mode: interactive, up to five implementors in parallel.

## Units

| lane | unit | acceptance (condensed) |
|---|---|---|
| A1 | `story:the-judge-requires-the-whole-corpus` | the judge compares both sides' whole scenario sets: empty against empty and a scenario either side lacks are refused, naming it; a new scenario joins only through `--rebaseline` with a superset |
| A2 | `story:promotion-judged-against-the-series-baseline` | a promotion is also judged against the series' first record (the latest baseline or rebaseline), so one-case-worse steps cannot add up |
| B1 | `story:forge-none-stops-when-only-forge-actions-remain` with `story:delivery-refuses-an-obligation-nobody-performs` | a run stops by name, exit 7, when no admissible action can move an open claim and that claim's only producer is not registered; an unknown stop never scores as completed |
| B2 | `story:a-local-wait-exits-waiting` | a wait on a local file exits waiting at once instead of polling, and says nothing of a forge under `--forge none` |
| B3 | `story:runs-check-free-disk` | a run stops before a step when the file system it builds on has less free space than the threshold (10 GiB by default), naming it |
| C | `story:trial-numbers-are-unique-across-checkouts`, and the install root accepted on resume (a follow-up of `story:self-improvement-protocol`) | trial numbers are unique, not consecutive, across fresh checkouts; an equal `--install-root` is accepted on resume |
| D1 | `story:timing-tests-hold-under-load`, with `story:wall-time-test-without-sleep` and `story:gate-tests-are-deterministic` | the timing tests hold at a load of twice the CPU count, each bound still below the time its mutant takes |
| D2 | `story:takt-has-a-gates-document` | takt's own delivery reaches `pr.open`: a gates document binding the Taskfile's five commands directly |
| E | `story:step-transcripts-are-kept` | each harness step's transcript is kept in the case (stream-json, capped at 16 MiB, mode 0600), also for a step that errors |

## Order

Lane A: A1, then A2 (the same five files). Lane B: B1, B2, B3 (the run loop, the `Stop` enum, the exit codes). Lane C in parallel. Lane D: D1, then D2, after B1. Lane E merges after B3.

Between B1 and D2, a `story.delivery/1` run on takt itself stops at its seed with exit 7, naming `gate.green` and the missing gates document, as B1 intends: takt has no gates document until D2 lands. A trial in that window that stops so is not a regression.

## Left out

`story:run-child-sweep-survives-setsid` (a real race, not a timing bound) stays out of D1.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at c4613a74 (0.2.2 released at f0728964; the ten stories active) |
| model budget | not stated; N = 5 |
| real store | a real self-improvement run uses the installed takt and its store; nothing in this wave writes there |

## The commits this wave makes

One opening commit (this page), each unit's commits on its own branch, the merges of the units into this branch on their grants, a merge of main when it moves, the closing commit, and the merge into local main. The planner pushes the wave tip and waits for its check before the wave's grant. No push, tag or release by this branch's maker.

## Log

- 2026-10-19: wave opened from main at c4613a74; A1, B1, C, D1 and E dispatched.
- 2026-10-20: wave tip 0fae6305. Close gate there: `task check`, `cargo test --workspace --locked --no-fail-fast` and `task fuzz` each exited 0. The wave-close adversarial review approved at 0fae6305 (the nine integration merges lost nothing; five later findings filed, four as stories with predicates, one closed by wave 16). Merged into `main` as b17a8290 and released as 0.3.0 at bee77822 (check.yml and release.yml green).
