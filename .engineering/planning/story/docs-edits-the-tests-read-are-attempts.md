---
format: aep.planning-md/3
id: story:docs-edits-the-tests-read-are-attempts
kind: story
status: implemented
title: An edit under docs/ that the story's tests read is never counted as an attempt
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: docs-edits-the-tests-read-are-attempts
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:a-revision-without-code-is-no-new-attempt
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T06:37:36Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-29T07:05:34Z", actor: "human:timo", revision: 5, executor: "agent:takt"}
- {from: "active", to: "implemented", at: "2026-10-29T07:49:35Z", actor: "agent:takt", revision: 9, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
# An edit under docs/ that the story's tests read is never counted as an attempt

## What is wrong

Since story:a-revision-without-code-is-no-new-attempt (wave 42 U9), a revision that changes
documentation only does not count toward `REPEATED_FAILURES`. `is_documentation_path`
(`crates/takt/src/run.rs`) calls everything under `docs/`, `README.md` and `CHANGELOG.md`
"documentation the tests cannot depend on". Tests do read it: in this repository
`crates/takt-cli/tests/cases_under_dot_takt.rs` reads a case under `docs/runs/`. When a story's
tests read a file under `docs/`, the edits that decide them are made there and none is counted, so
a run that keeps editing that file without turning its test green goes on to its iteration budget
instead of stopping `RepeatedTestFailure`.

Seen in wave 42's adversarial pass: the scripted case
`attempts_in_a_docs_file_the_tests_read_are_counted` (`crates/takt/tests/w42_adv_repeat_count.rs`,
ignored as this finding) makes 14 implementation attempts in `docs/fixture.txt`, each leaving the
test red, and the run stops `Iterations(30)`.

## Check

Not runnable by `scripts/check-finding`: it runs a library test with a scripted harness, not
`$TAKT`. Checked on the wave 42 tree at a2f650bf: exit 101.

```expected_before
command: cargo test -p b10x-takt --test w42_adv_repeat_count -- --exact attempts_in_a_docs_file_the_tests_read_are_counted --include-ignored
exit: 101
healthy_exit: 0
output: each leaving the test red
source: crates/takt/tests/w42_adv_repeat_count.rs
```

## Outcome

A revision counts as a new attempt when it changes a path the story's tests can read, documentation
or not. Either the documentation rule is narrowed to paths no test reads (for example the paths a
test file names are not documentation), or it is dropped for a story whose tests or scope name a
path under `docs/`. A revision that changes only documentation no test reads still does not count.

## Acceptance

1. The predicate above exits 0, and the case is no longer `#[ignore]`d.
2. `crates/takt/tests/w42_adv_repeat_count.rs`'s other case and the cases of
   story:a-revision-without-code-is-no-new-attempt still pass: a documentation-only revision that
   no test reads is not counted.

## Scope

- `crates/takt/src/run.rs`: `is_documentation_path`, `code_changed` (cited)
- `crates/takt/tests/w42_adv_repeat_count.rs`: the ignored case un-ignored (cited)
- `ess/domains/takt.yaml`: the repeated-failure rule's prose and its citations (inferred)

## Release 0.19.0

0.19.0 ships this story's cases `#[ignore]`d, accepted at its release check as a tracked finding
on the condition that each ignore's reason names this story.

## Decision (wave 44)

A first run declined this story (wave 44): in the scripted case nothing names `docs/fixture.txt`,
so takt holds no information that tells it from documentation no test reads. The planner's
decision, which settles that:

- The rule is the first of the two in § Outcome: a path under `docs/`, `README.md` or
  `CHANGELOG.md` is documentation unless a test file at the revision names it. A test file is a
  path the case's test-writing steps changed (the paths the tests' own commits touched). What
  "names it" means is the section "Decision (wave 44, review)" below, and only that section: the
  path or one of its parent directories, ending at a path boundary.
- The scripted case is in this story's scope and may be changed so its tests file names the
  fixture, as a real test does: the `Developer`'s first `tests.write` in
  `crates/takt/tests/w42_adv_repeat_count.rs` writes a tests file whose content includes
  `docs/fixture.txt`. The case's assertion (`RepeatedTestFailure`) does not change.
- The other case of that file and the cases of story:a-revision-without-code-is-no-new-attempt
  keep passing unchanged: their documentation edits are to files no tests file names.
- The review's cases in `crates/takt/tests/w44_adv_docs_named.rs` are the planner's: a step may
  reformat them, but never changes a case's input, name or assertion. A case that seems to
  contradict this story is a question for the planner, asked by declining with that case named.

## Decision (wave 44, review)

The adversarial pass on the first implementation (aa2b794e) found two cases red, now in
`crates/takt/tests/w44_adv_docs_named.rs`; the second pass (cda52fd0) four more, now in
`crates/takt/tests/w44_adv_docs_named_b.rs`. The planner's decision on "names it", which all of
them hold, and which replaces every other reading:

- A test file names a documentation path `P` when its content holds `P`, or a parent directory
  of `P` with or without its trailing `/`, as a match that ends at a path boundary: the end of
  the text, `/`, a quote (`"` or `'`), whitespace, `)`, `,` or `\`. Any other character after the
  match (a letter, a digit, `.`, `_`, `-`, `~`, `+`, `@`, `=`, `%`, `#`, `:`) continues the path,
  so `docs/guide.md.tpl` and `docs/guide.md~` do not name `docs/guide.md`.
- Nothing is required before the match, so `"/../../docs/runs/turn-1/case"` names the files
  under `docs/runs/turn-1/case`, as `crates/takt-cli/tests/cases_under_dot_takt.rs` reads them.
  The over-counts this allows (`website/docs/intro.md` names `docs/intro.md`; a test naming
  `docs` names every documentation file) stop a run earlier, never later, and stand.
- Windows separators (`docs\guide.md`) name nothing.
- The comparison that decides whether a revision changed code leaves `.engineering/` out. The doc
  comment of `code_changed` and the repeated-failure prose in `ess/domains/takt.yaml` state this
  rule, the boundary characters and the `.engineering/` exclusion.
