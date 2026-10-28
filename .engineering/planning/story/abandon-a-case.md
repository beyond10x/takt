---
format: aep.planning-md/3
id: story:abandon-a-case
kind: story
status: draft
title: takt abandon ends a case that cannot finish
summary: takt abandon records explicitly_abandoned and deletes the case's branch; the story keeps the base's status, and the next run passes it over and takes the next story
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:unqualified-stop-records-no-layer
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/abandon_a_case.rs
revision: 11
---
# Story: abandon-a-case

## Outcome

An operator can abandon a case that cannot finish. After `takt abandon <case>` the case's outcome is `abandoned`, its local branch `takt/<case>` is gone, and the next argument-less `takt run` neither resumes that case nor takes its story, and takes the next ready story. The story is not moved: it keeps the status the base branch gives it, and the owner then archives it or files the work again under a new story. A case whose pull request was closed stopping its run is story:closed-pull-request-stops-the-run, split from this story in plan-critic round 1.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093; the lines below are re-read at 338082d.

- Every argument-less `takt run` resumes the first unfinished case before it takes a story (`Run::next_to_resume` and `Run::resumable_undeclined`, `crates/takt/src/run.rs:9720-9784`), and a story a case was opened for is never taken again (`run.rs:9852-9858`, "not taken: the case … was opened for it already"). A case that cannot finish blocks the frontier for good.
- `story.delivery/1` is not a file of this repository: it is embedded from the `b10x-canon-engineering` crate, engineering-protocols 0.12.0 (`Cargo.toml:13`, `crates/takt/src/protocol.rs:1044-1047`). Its `abandoned` outcome requires the decision `explicitly_abandoned` (engineering-protocols 0.12.0, `protocols/story-delivery/1.yaml:210-213`).
- Nothing in takt's sources supplies that decision. Only test fixtures name it (`crates/takt/tests/pr_opens_when_done.rs:810`, `:845`; `crates/takt/tests/no_inspect_for_stateless_harness.rs:290`), so the probe below reads the sources alone.

Where the story's status stands, at 338082d. The story lifecycle has no edge back to `proposed` (`aep plan artifact lifecycle story`: `active -> implemented, archived`), and this story needs none:

- A case's branch `takt/<id>` is created and checked out at the tip of its base (`crates/takt/src/case.rs:2876`), and the take, the store's `proposed -> active`, is committed on that branch (`run.rs:9887-9899`). The base's store never sees `active`; a tree on a case's branch is put on its base before the store is read (`to_base`, `run.rs:8416-8443`).
- So after `takt abandon` deletes the branch, the base gives the story the status it had before the run: `proposed` for a story `takt run` took. A story the base itself lists `active` (moved there outside takt) stays `active`.
- Either way the next run passes it over. Case files live under `.takt/`, outside version control (`case.rs:38`), so the abandoned case stays on disk after its branch is gone. A finished case is not resumed (`case::is_finished`, `case.rs:2063`; `resumable_undeclined`, `run.rs:9761-9767`). A story a case records is not taken (`cases_with_story`, `run.rs:8387`, checked at `:9852-9858`). An `active` story is no candidate at all (`Frontier::candidates` lists ready stories, `crates/takt/src/frontier.rs:641-644`).
- What this story adds on the frontier is the line: a story passed over because its case was abandoned is said as such, not as "opened for it already".

## Ordering

- `crates/takt/src/run.rs` is wave 39's file (story:unqualified-stop-records-no-layer). This story waits for wave 39 to merge: `depends_on` that story.
- `crates/takt/src/run.rs` is also scoped by story:closed-pull-request-stops-the-run and story:wall-time-test-without-sleep. story:closed-pull-request-stops-the-run goes after this story: its stop sends the operator to `takt abandon` (its `depends_on` this story). story:wall-time-test-without-sleep is tagged `rsi` and is scheduled first by that rule; its hunk is the step's cleanup path (`run.rs:12269-12502`), far from this story's (`:8374-8443`, `:9720-9858`), so no edge, and `aep plan artifact waves` keeps the two in different waves.
- `crates/takt-cli/src/main.rs` is also scoped by story:normalized-harness-trace (`TraceArgs`, `:437-452`) and story:promotion-records-its-toolchain (`PromoteArgs`, `:297-330`). This story adds a new subcommand and touches neither. No edge; the waves keep them apart. story:normalized-harness-trace is `priority:low` and goes after this one.

## Expected before

On takt at 338082d, holds; after the change, fails:

- `grep -rn explicitly_abandoned crates/takt/src crates/takt-cli/src` prints nothing and exits 1.
- `takt abandon --help` exits 2 (no such subcommand).

## Scope

- `crates/takt-cli/src/main.rs`: the `takt abandon <case>` verb (inferred)
- `crates/takt/src/case.rs`: recording the `explicitly_abandoned` decision (inferred)
- `crates/takt/src/run.rs`: the tree put on the case's base and the case's branch deleted; the frontier's line for a story whose case was abandoned (cited, `:8374-8443`, `:9720-9858`)
- `crates/takt/tests/abandon_a_case.rs`: the test (inferred)

## Acceptance

In a scratch repository with two `proposed` stories, A then B, and a case for A opened by `takt run --harness none --forge none` and left unfinished, the tree on the case's branch:

1. `takt abandon <case>` exits 0. The case's files record the decision `explicitly_abandoned`, and the case reads as finished with the outcome `abandoned`.
2. The tree is on the case's base, and `git branch --list takt/<case>` prints nothing.
3. `aep plan artifact show story:A` gives the status `proposed`, as the base gave it before the run, and `aep plan artifact history story:A` on the base shows no move.
4. The next argument-less `takt run` prints `frontier: story:A not taken: the case <case> was abandoned`, then that it took story:B, and writes no line to the abandoned case's `events.jsonl`.

Before the change `takt abandon` exits 2.
