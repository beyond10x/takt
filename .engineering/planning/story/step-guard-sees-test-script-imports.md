---
format: aep.planning-md/3
id: story:step-guard-sees-test-script-imports
kind: story
status: implemented
title: A step cannot shadow the test runner the test script loads
tags:
- finding
refs:
- provider: finding
  reference: step-guard-sees-test-script-imports
relations:
- informed_by: review-result:findings-hunt-3fe3f21
- informed_by: story:step-cannot-forge-its-validators
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T14:11:50Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T14:11:50Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T14:11:51Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A judgement step cannot earn `tests.run` or `gate.run` `pass` by adding a file that the test command's script loads in place of the real test runner.

## Context

Scenario trials use `--test-cmd ./check.sh`, which runs `PYTHONPATH=. python3 -m unittest …`. `scripts_of` and `JudgeSurface::new` (`run.rs:1297`, `:1420`) guard `check.sh` itself but never read its contents. So "the module `python -m` runs" (`JUDGE_SURFACE`, `:357`) is guarded only when `-m` is on the command line. Observed run (fake harness: step 3 `tests.write` adds a failing test, step 6 `implementation.edit` adds `unittest.py` that prints `OK`):
- step 4 `tests.run` fail (6 tests), step 5 `gate.run` fail;
- step 7 `tests.run` pass ("OK"), step 8 `gate.run` pass;
- the failing test is still in HEAD.

A `sitecustomize.py` containing `os._exit(0)` works too. I saw that with python 3.14 directly, not through takt.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCmQ9JChkaXJuYW1lICIkMCIpCm49JChjYXQgIiRkL24iIDI+L2Rldi9udWxsIHx8IGVjaG8gMCkKbj0kKChuICsgMSkpCmVjaG8gIiRuIiA+ICIkZC9uIgpjYXNlICRuIGluCjEpIHByaW50ZiAnaW1wb3J0IHVuaXR0ZXN0XG5jbGFzcyBGKHVuaXR0ZXN0LlRlc3RDYXNlKTpcbiAgICBkZWYgdGVzdF9mKHNlbGYpOlxuICAgICAgICBzZWxmLmZhaWwoIngiKVxuJyA+IHRlc3RzL3Rlc3RfZm9yZ2UucHk7IGE9dGVzdHMud3JpdGUgOzsKMikgcHJpbnRmICdwcmludCgiT0siKVxuJyA+IHVuaXR0ZXN0LnB5OyBhPWltcGxlbWVudGF0aW9uLmVkaXQgOzsKKikgYT1yZXBvc2l0b3J5Lmluc3BlY3QgOzsKZXNhYwpwcmludGYgJ3sicmVzdWx0Ijoie1xcImFjdGlvblxcIjpcXCIlc1xcIixcXCJzdW1tYXJ5XFwiOlxcInNcXCJ9IiwidXNhZ2UiOnsiaW5wdXRfdG9rZW5zIjoxLCJvdXRwdXRfdG9rZW5zIjoxfX1cbicgIiRhIgo= | base64 -d > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --forge none --test-cmd ./check.sh --iterations 10 >/dev/null 2>&1; grep '"kind":"test_result"' .engineering/cases/*/events.jsonl | tail -1 | grep -q '"result":"pass"' && git cat-file -e HEAD:unittest.py && echo forged-pass
exit: 0
output: forged-pass
source: .engineering/planning/review-result/findings-hunt-3fe3f21.md
```

## Scope

`crates/takt/src/run.rs` (`scripts_of`, `command_paths`, `JudgeSurface`, `PYTHON_FILES`, `JUDGE_SURFACE`).

## Acceptance

In the predicate's run, the step that adds `unittest.py` is refused and reverted. `sitecustomize.py` and `usercustomize.py` get the same treatment. The case records no `pass` while the failing test is present.
