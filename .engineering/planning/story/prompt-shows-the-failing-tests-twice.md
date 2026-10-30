---
format: aep.planning-md/3
id: story:prompt-shows-the-failing-tests-twice
kind: story
status: draft
title: A step after a red test run and a red gate shows the same failing tests twice
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: prompt-shows-the-failing-tests-twice
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# A step after a red test run and a red gate shows the same failing tests twice

When `tests.run` and the gate both fail on the same test command (scenario 1's gate runs
`check.sh`), the next judgement prompt leads with the failing tests and where each one's
assertion is, then repeats both under "The last gate run failed": the same fenced lines, the same
assertion block and the same paragraph after it, about 1,085 bytes in the call after them (about
690 tokens a test-first run of scenario 1 at the 0.335 tokens per prompt byte the trial of 0.23.0
measured; 510 at 4 bytes a token).

The gate's run is itself the duplicate. In run `scenario-1-trial-359`, `tests.run` ran
`--test-cmd ./check.sh` and the gate's one gate, `check`, ran `check.sh`
(`.engineering/assertions-project.yaml`), at the same revision `9c8e17af`, 1.2 seconds apart
(evidence `0001-test-result-1` and `0002-gate-result-2`). The owner's direction (2026-10-30): a
gate whose command the agent's test run already ran at the same revision is proven by that run
and is not run again.

The change (`crates/takt/src/validator.rs`, `GateRun`; `crates/takt/src/harness.rs`): a gate
whose process is the `tests.run` command (the same program, after resolving the path against
the tree, and the same arguments), with the command's script and every judge-surface path
unchanged since the case's base, is not run again; its `gate_result` record is written from the
`test_result` record at the same revision and names it. A gate that runs anything else still
runs, so a gate of several commands runs all but the one already run. The next prompt then
holds the failing tests once.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/fake" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a failing remove test\", \"commit\": {\"subject\": \"test: add a failing remove test\", \"body\": \"Adds one test for remove.\\nIt fails until remove exists.\\nNothing else changes.\"}}"}' > "$HOME/fake/w.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"decline\", \"summary\": \"prompt-shows-the-failing-tests-twice\"}"}' > "$HOME/fake/twice.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"decline\", \"summary\": \"the failing tests are shown once\"}"}' > "$HOME/fake/once.json" && printf '%s\n' '#!/bin/sh' 'f="$HOME/fake"' 'if [ "$1" = --version ]; then echo "2.1.309 (Claude Code)"; exit 0; fi' 'if [ ! -e "$f/wrote" ]; then touch "$f/wrote"; printf "%s\n" "" "class RemoveTest(unittest.TestCase):" "    def test_remove(self):" "        self.assertEqual(todo.main([\"remove\", \"1\"]), 0)" >> tests/test_todo.py; cat "$f/w.json"; exit 0; fi' 'case "$*" in *"SystemExit: 2"*"SystemExit: 2"*|*"assertion is, read from its output"*"assertion is, read from its output"*) cat "$f/twice.json" ;; *) cat "$f/once.json" ;; esac' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --protocol story.delivery/1 --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 6
exit: 9
healthy_exit: 9
output: Declined (prompt-shows-the-failing-tests-twice)
source: docs/runs/2026-10-30-trial-0.23.0.md
```
