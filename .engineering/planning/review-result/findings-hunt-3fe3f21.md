---
format: aep.planning-md/3
id: review-result:findings-hunt-3fe3f21
kind: review-result
status: active
title: 'Reviewer: findings hunt on main 3fe3f21 (waves 10-11 code)'
relations:
- reviews: story:records-reach-base
- reviews: story:binary-embeds-revision
- reviews: story:step-cannot-forge-its-validators
- reviews: story:scorecard-reader
- reviews: story:grant-binds-to-action-subject
prose_only: a code review returning findings with predicates, not a plan critic verdict
revision: 1
---
I found 5 defects, all checkable offline. I ran each predicate by hand the way `scripts/check-finding` does: a fresh clone of todo-app-example at 865eaa8 on `trial/0`, remote removed, user set, `env -i PATH HOME=<fresh> LANG TAKT SCENARIO`, and takt built from 3fe3f21. None of them is in the open stories, `story:wave11-review-follow-ups`, confinement § Routes or promotion § From the scorecard-reader review.

| # | severity | title | file:line |
|---|---|---|---|
| 1 | low | `takt scorecard` on a missing `--repo` exits 0 and saves an empty baseline | `crates/takt/src/scorecard.rs:421` |
| 2 | medium | `check-finding` builds report `unknown revision` | `scripts/check-finding:163` |
| 3 | medium | `--save --revision` names a scorecard without checking the runs' `binary` | `crates/takt/src/scorecard.rs:750-754` |
| 4 | medium | A failed harness step's reported tokens are dropped | `crates/takt/src/harness.rs:262-263`, `crates/takt/src/run.rs:3928-3943` |
| 5 | high | A step that adds `unittest.py` gets a forged `tests.run` and `gate.run` pass on scenario 1 | `crates/takt/src/run.rs:1297`, `:1420`, `:357` |

Every predicate needs a real `source:` path before it will parse. I left `source: REVIEW_RECORD` as a placeholder because I wrote no review record.

---

**Finding 1: `takt scorecard` treats a missing repository as one with no cases**

Outcome: A `--repo` that does not exist is refused with exit 1, and nothing is saved.

Context: `Scorecard::read` treats a missing cases directory as "0 cases" (`scorecard.rs:421`), so a mistyped `--repo` prints an empty table and exits 0. The help says exit 1 when "the repository … does not read" (`crates/takt-cli/src/main.rs:81`). With `--save`, the empty scorecard is written. It cannot be overwritten later, and comparing against it reports every scenario `no baseline`, so it is never worse. Observed: exit 0, `saved: …/scorecard-3fe3f21.json`, `"rows": []`.

```expected_before
scenario: 1
command: "$TAKT" scorecard --repo ./no-such-repo --save "$HOME/scorecards" --revision 3fe3f21
exit: 0
output: saved: 
source: REVIEW_RECORD
```

Scope: `crates/takt/src/scorecard.rs` (`read`, `report`).

Acceptance: A `--repo` that is not an existing directory exits 1, names the path and writes no file. An existing repository with no `.engineering/cases/` still prints 0 cases.

---

**Finding 2: verification binaries cannot say which revision they are**

Outcome: A takt built by `scripts/check-finding` prints its revision in `takt --version`, and records it as `binary` on `opened`.

Context: `check-finding` exports the tree with `git archive` (`scripts/check-finding:163`). With no `.git` there, `build.rs` reports `unknown revision`. So every promoted-versus-candidate verification run records `"binary":"unknown revision"`, and nothing ties its figures to a revision. Observed: my archive build of 3fe3f21 prints `takt 0.1.0 (unknown revision)`. The existing cached builds `~/.cache/takt/check-finding/builds/834205f…` and `8d49743…` print the same. A build from a checkout prints the sha. This predicate only reproduces under `check-finding`'s own build, which is where the defect is.

```expected_before
scenario: 1
command: "$TAKT" --version
exit: 0
output: (unknown revision)
source: REVIEW_RECORD
```

Scope: `scripts/check-finding` (the build step). Possibly `crates/takt-cli/build.rs`, if the fix is a revision the caller passes in.

Acceptance: `check-finding` at revision R builds a binary whose `--version` prints R (or `R-dirty`, never `unknown revision`), and the `opened` line records R.

---

**Finding 3: a saved scorecard's name is never checked against the runs it holds**

Outcome: `takt scorecard --save --revision R` is refused unless every run it saves has an `opened` line whose `binary` is R. Runs from other binaries are refused or listed separately.

Context: `report` and `save` (`scorecard.rs:750-754`, `:550`) take the operator's `--revision` on trust. `runs()` never reads `binary` (`:313-347`). In the observed run, the `opened` line recorded `"binary":"3fe3f219…"`, the save as `0000000` was accepted, and the output said `named by the revision of takt under test`. This is not the filed `--against` provenance finding: that one is about reading a saved file, this one is about writing it.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --harness none --forge none --test-cmd true --iterations 1 >/dev/null 2>&1; "$TAKT" scorecard --repo . --save "$HOME/scorecards" --revision 0000000
exit: 0
output: named by the revision of takt under test
source: REVIEW_RECORD
```

Scope: `crates/takt/src/scorecard.rs` (`runs`, `Row`, `save`, `report`).

Acceptance: Saving under a revision that differs from a run's `binary` (an `unknown revision` binary included) exits 1 and names the case. Saving under the matching sha succeeds.

---

**Finding 4: tokens a failed harness step reported never reach the record**

Outcome: Tokens a harness reports are recorded and charged to `--tokens` even when its step fails.

Context: `ClaudeCode::step` bails (no action line, non-zero exit, cut output) before `self.reported.set(usage)` (`harness.rs:262-263`). The error path in `run_held` (`run.rs:3928-3943`) writes an `error` line with no tokens. The fake harness reported 700+300 tokens; the stop line read `tokens 0` and the scorecard row read `tokens 0`. I don't know whether real `claude -p` exits 0 or non-zero when it hits `--max-turns`; both paths drop the usage. The blob is a fake `claude` that prints that usage and no action line.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCnByaW50ZiAneyJyZXN1bHQiOiJzdG9wcGVkIGJlZm9yZSB0aGUgYWN0aW9uIGxpbmUiLCJ1c2FnZSI6eyJpbnB1dF90b2tlbnMiOjcwMCwib3V0cHV0X3Rva2VucyI6MzAwfX1cbicK | base64 -d > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --forge none --test-cmd ./check.sh --iterations 5
exit: 5
output: the harness ended without the JSON line naming its action; steps 3, tokens 0,
source: REVIEW_RECORD
```

Scope: `crates/takt/src/harness.rs` (`ClaudeCode::step`), `crates/takt/src/run.rs` (harness error path), `crates/takt/src/events.rs` (tokens on `error`).

Acceptance: In the predicate's run, the stop line and the scorecard show 1000 tokens, and a `--tokens` budget counts them.

---

**Finding 5: the step guard does not see what the test command's script runs**

Outcome: A judgement step cannot earn `tests.run` or `gate.run` `pass` by adding a file that the test command's script loads in place of the real test runner.

Context: Scenario trials use `--test-cmd ./check.sh`, which runs `PYTHONPATH=. python3 -m unittest …`. `scripts_of` and `JudgeSurface::new` (`run.rs:1297`, `:1420`) guard `check.sh` itself but never read its contents. So "the module `python -m` runs" (`JUDGE_SURFACE`, `:357`) is guarded only when `-m` is on the command line. Observed run (fake harness: step 3 `tests.write` adds a failing test, step 6 `implementation.edit` adds `unittest.py` that prints `OK`):
- step 4 `tests.run` fail (6 tests), step 5 `gate.run` fail;
- step 7 `tests.run` pass ("OK"), step 8 `gate.run` pass;
- the failing test is still in HEAD.

A `sitecustomize.py` containing `os._exit(0)` works too. I saw that with python 3.14 directly, not through takt.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCmQ9JChkaXJuYW1lICIkMCIpCm49JChjYXQgIiRkL24iIDI+L2Rldi9udWxsIHx8IGVjaG8gMCkKbj0kKChuICsgMSkpCmVjaG8gIiRuIiA+ICIkZC9uIgpjYXNlICRuIGluCjEpIHByaW50ZiAnaW1wb3J0IHVuaXR0ZXN0XG5jbGFzcyBGKHVuaXR0ZXN0LlRlc3RDYXNlKTpcbiAgICBkZWYgdGVzdF9mKHNlbGYpOlxuICAgICAgICBzZWxmLmZhaWwoIngiKVxuJyA+IHRlc3RzL3Rlc3RfZm9yZ2UucHk7IGE9dGVzdHMud3JpdGUgOzsKMikgcHJpbnRmICdwcmludCgiT0siKVxuJyA+IHVuaXR0ZXN0LnB5OyBhPWltcGxlbWVudGF0aW9uLmVkaXQgOzsKKikgYT1yZXBvc2l0b3J5Lmluc3BlY3QgOzsKZXNhYwpwcmludGYgJ3sicmVzdWx0Ijoie1xcImFjdGlvblxcIjpcXCIlc1xcIixcXCJzdW1tYXJ5XFwiOlxcInNcXCJ9IiwidXNhZ2UiOnsiaW5wdXRfdG9rZW5zIjoxLCJvdXRwdXRfdG9rZW5zIjoxfX1cbicgIiRhIgo= | base64 -d > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --forge none --test-cmd ./check.sh --iterations 10 >/dev/null 2>&1; grep '"kind":"test_result"' .engineering/cases/*/events.jsonl | tail -1 | grep -q '"result":"pass"' && git cat-file -e HEAD:unittest.py && echo forged-pass
exit: 0
output: forged-pass
source: REVIEW_RECORD
```

Scope: `crates/takt/src/run.rs` (`scripts_of`, `command_paths`, `JudgeSurface`, `PYTHON_FILES`, `JUDGE_SURFACE`).

Acceptance: In the predicate's run, the step that adds `unittest.py` is refused and reverted. `sitecustomize.py` and `usercustomize.py` get the same treatment. The case records no `pass` while the failing test is present.

---

**Outside takt:** in engineering-protocols 0.9.0, `canon-engineering-assertions` `snapshot::identity` hashes ignored files. When the gate's command first creates `__pycache__/`, `gate.run` ends with "source changed during collection" and exit 5. I saw this on scenario 1 with the default `--test-cmd`. That belongs in that repository, not this store.


The runs, the fake harnesses and the archive build were kept in the reviewer's scratch directory outside the repository.
