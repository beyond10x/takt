---
format: aep.planning-md/3
id: story:gate-compares-what-its-scripts-import
kind: story
status: implemented
title: gate.run compares at the base every module a gate script imports from its own directory
summary: a hand commit to a module a gate script imports turned the gate from fail to pass
tags:
- finding
- rsi
refs:
- provider: finding
  reference: gate-compares-what-its-scripts-import
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T00:48:39Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T00:48:39Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T01:41:20Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: gate-compares-what-its-scripts-import

## Outcome

`gate.run` compares at the base revision every file of the tree a gate's Python script imports
from its own directory, as it compares the script itself, so a commit on the case's branch that
changes such a module leaves the gate not run instead of deciding it.

## Context

Found by the wave 16 review at 05f70c4f (docs/waves/wave-16.md), filed for later; still there at
00a900ac. Before a gate runs, `GateRun::observe_by` compares at the case's pinned base what the
gate's commands run or read from the tree (`crates/takt/src/validator.rs:1166, 1199-1240` at
00a900ac): the executables, providers and catalogs the project file admits (`admitted_files`,
`validator.rs:822`) and every command argument that names a file (`argument_paths`,
`validator.rs:871`). A file one of those programs loads at run time is not on that list. Python
puts a script's own directory first on `sys.path`, so `python3 scripts/check.py` imports
`scripts/checklib.py` from the tree, and the gate compares `scripts/check.py` but not
`scripts/checklib.py`.

The review's probe: a `citations` gate runs `python3 scripts/check.py`, which imports
`checklib`; the gate fails at open. A commit on the case's branch that changes only
`scripts/checklib.py` makes the next `gate.run` pass ("every gate true: check, citations"). The
same commit to `scripts/check.py` is caught ("not run; what its commands run or read from the
tree differs from the base revision ...: scripts/check.py"). takt's own repository is not
exposed: `scripts/**` is on its judge surface. A repository whose judge surface does not list
the script's directory is.

The predicate builds that repository from scenario 1: the gate and its two files committed on
`trial/0`, a `story.delivery/1` case opened with no harness (the gate fails), then a hand commit
to `scripts/checklib.py` on the case's branch and a resume. Exit 9 says the resumed `gate.run`
passed. Exit 0 says it did not.

## Acceptance

1. A gate whose command runs a Python script is not run while a `.py` file, a package or a
   `.pth` file in the script's directory differs from the base revision, the refusal naming the
   file: a test with a script that imports a sibling module.
2. The same holds for a module found in the directory the command runs in and in each
   `PYTHONPATH` entry the gate's binding sets, as the step guard already treats those
   directories (`JUDGE_SURFACE`, `crates/takt/src/run.rs:1177`): a test, or a reason in the
   story why the gate needs less.
   Decided (dispatcher, wave 18): in the directory the command runs in and each `PYTHONPATH`
   entry, only the module `-m` names and the `*.pth` files are compared, not every module. Those
   directories are often the code the gate checks (`src`), and comparing all of it would stop
   every Python-testing gate once a step changed that code; the test
   `code_beside_a_module_the_gate_runs_lets_it_run_and_record` holds this.
3. A gate whose scripts import nothing from the tree is decided as before: the existing gate
   tests pass unchanged.
4. At the candidate, `scripts/check-finding story:gate-compares-what-its-scripts-import
   <candidate>` prints `not_reproduced`, and at the promoted revision `reproduced`.

```expected_before
scenario: 1
command: set -e; G="git -c user.name=probe -c user.email=probe@example.invalid"; mkdir -p scripts; printf 'import sys\nimport checklib\nsys.exit(checklib.run())\n' > scripts/check.py; printf 'def run():\n    print("citations: broken")\n    return 1\n' > scripts/checklib.py; printf '%s\n' '  - id: citations' '    text: Every citation matches.' '    assertions:' "      - 'process.succeeded(\"citations\")'" >> .engineering/gates.yaml; printf '%s\n' '  citations:' '    executable: /usr/bin/python3' '    args:' '      - scripts/check.py' >> .engineering/assertions-project.yaml; $G add -A; $G commit -q -m "a citations gate that fails"; set +e; "$TAKT" run --repo . --intent "Remove a todo by its number" --protocol story.delivery/1 --harness none --forge none --test-cmd ./check.sh --iterations 4 > "$HOME/open.out" 2>&1; c=$(ls .engineering/cases); git checkout -q "takt/$c" || exit 1; printf 'def run():\n    print("citations: fine")\n    return 0\n' > scripts/checklib.py; $G commit -q -m "the check passes" scripts/checklib.py; "$TAKT" run --repo . --case "$c" --harness none --forge none --test-cmd ./check.sh --iterations 8 > "$HOME/resume.out" 2>&1; grep -h 'effect: the gates document' "$HOME/open.out" "$HOME/resume.out" | sed "s|$HOME|~|g"; if grep -q 'effect: the gates document.*: pass;' "$HOME/resume.out"; then echo "the gate passed after a hand commit to scripts/checklib.py, which scripts/check.py imports" >&2; exit 9; fi
exit: 9
healthy_exit: 0
output: the gate passed after a hand commit to scripts/checklib.py
source: docs/waves/wave-16.md
```
