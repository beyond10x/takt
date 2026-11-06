---
format: aep.planning-md/3
id: story:test-cmd-checked-before-open
kind: story
status: implemented
title: takt run checks --test-cmd before it opens a case
tags:
- finding
refs:
- provider: finding
  reference: test-cmd-checked-before-open
relations:
- informed_by: review-result:findings-hunt-33ab42c
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:14:03Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T02:14:03Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-06T03:10:31Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

A `--test-cmd` whose program cannot be started, or that holds shell quoting, is refused with exit 1 before any case is opened.

## Context

`TestsRun::new` only splits on white space (`validator.rs:206-215`). The program is first started inside the `tests.run` step (`:275-279`). That contradicts `takt run --help` exit 1, "refused before the run (an argument …)".

- Missing file, observed: exit 5 with `run ./no-such-check.sh: run ./no-such-check.sh: No such file or directory (os error 2)`. The error repeats `run ./no-such-check.sh:` twice. The case stayed open and the tree was left on its branch.
- Quoted command, observed: `--test-cmd "sh -c 'PYTHONPATH=. python3 -m unittest discover -s tests'"` recorded `evidence: test_result fail`, with `unexpected EOF while looking for matching` in the summary. A quoting mistake became a test-failure record about the implementation.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent probe --harness none --forge none --iterations 2 --test-cmd ./no-such-check.sh
exit: 5
output: No such file or directory (os error 2)
source: .engineering/planning/review-result/findings-hunt-33ab42c.md
```

## Scope

`crates/takt/src/validator.rs` (`TestsRun::new`), `crates/takt-cli/src/main.rs` (`validators_of`, the `--test-cmd` help).

Acceptance:
- The predicate exits 1 and names the program, and no case directory or `takt/` branch is created.
- A `--test-cmd` holding `'` or `"` exits 1 and says the command is split at white space without a shell.

## Status

Built in wave 54 (unit b, 872bae7b4, corrected in 7501e6380), released in 0.32.0 from 9e7a3904e, the merge of pull
request 61.

- `crates/takt/src/validator.rs`: `TestsRun::new` refuses a command holding `'` or `"`; `check_program_at` finds the
  program as the step's `execvp` does and requires `access` with `X_OK`, reading a path in the tree from the case's
  branch on a resume from another branch.
- `crates/takt-cli/src/main.rs`: `validators_of` checks both before any case opens or resumes, exit 1; the
  `--test-cmd` help and the configuration pages say so.
- `crates/takt-cli/tests/test_cmd_checked_cli.rs` (red, then green) and the adversary's cases in `w54_adversary.rs`
  and `w54_adversary_cli.rs`.

Review: review-result:w54-adv-1, B1, B2 and B4 fixed.
