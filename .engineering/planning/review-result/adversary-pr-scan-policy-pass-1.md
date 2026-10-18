---
format: aep.planning-md/3
id: review-result:adversary-pr-scan-policy-pass-1
kind: review-result
status: active
title: Adversary, pr-scan-policy, pass 1
relations:
- reviews: story:pr-scan-policy
revision: 1
---
unit: story:pr-scan-policy, worktree w7-pr-scan-policy at e9a25b0 plus one untracked test file
verdict: CONFIRMED
cases: executed 230→231, red 1
origin: introduced 1 / pre-existing 1 / undecided 0
wrote-outside-worktree: 4 paths (part 6)
needs-coordinator: yes. The live scenario-1 rerun only passes with `B10X_GATES_POLICY` set, and with it set 92b8cf3 passes too, so a green rerun is not evidence for this change.

The `scan_argv` / `api_argv` change holds. One case is red: the scan reads a JSON-escaped copy of the body, not the text the forge publishes. That gap was already on the base, so it is pre-existing.

**1. `git --no-pager diff --stat`**
The output is empty, because my only file is untracked. `git status --short` shows a single line: `?? crates/takt/tests/forge_scan_policy_adv1.rs`. I changed no implementation file.

**2. The case I added**
`crates/takt/tests/forge_scan_policy_adv1.rs::the_scan_reads_the_body_as_it_is_published` is red now.
- It calls `BotRoute::open_pull_request` with a recording runner and a multi-line body in the shape `body_of` builds, with a path at the start of a line.
- It asserts that the file `scan-text` reads has that line, and that it contains the body verbatim.

Red output from that case run alone, before the suite (the log's `EXIT=0` is the status of `tail`, not of cargo):
```
panicked at crates/takt/tests/forge_scan_policy_adv1.rs:88:5:
scan-text never sees the line the forge publishes as `/home/someone/notes/todo.txt`; it read:
{
  "base": "main",
  "body": "Remove the TODO\n\nThe list came from\n/home/someone/notes/todo.txt\n\n---\n\nOpened by takt ...",
  "head": "takt/c1",
  "title": "Remove the TODO"
}
test result: FAILED. 0 passed; 1 failed
```

Why it matters: I ran gates' own patterns from the gates repository @935e973 in a Python approximation (`homes_probe.py`). This is not a live b10x-gates run.

| Input | `homes` pattern (`scan.rs:266`) | wrapped literal (`scan.rs:215`) |
|---|---|---|
| decoded body | match | match |
| JSON request | no match | no match |

The JSON escape `\n` is two bytes, and the `n` is a character the `homes` boundary excludes. The line-break allowance for wrapped literals only accepts a real line break.

`b10x-gates api` scans the same raw JSON itself (gates `main.rs:252-262`), so nothing later catches it either.

**3. Suite run (after the case existed)**
`cargo test -p b10x-takt --locked --no-fail-fast` exited 101 with 230 passed and 1 failed. The only failure is `forge_scan_policy_adv1`. The 230 matches the implementor's count. `rustfmt --check` and clippy with `-D warnings` are clean on the new file.

**4. Findings (they cover e9a25b0 plus the untracked test)**

| file:line | verdict | origin | what was measured | what reaches it |
|---|---|---|---|---|
| `crates/takt/src/forge.rs:855-857` | CONFIRMED | pre-existing | The red case above. The base has the same pattern: `to_vec_pretty` then `scan_argv(&input)` at base `forge.rs:848-850`. | Every bot-route `pr.open`. `body_of` always writes several lines (`validator.rs:586-589`), and the intent is the story text. Fix: also write the title and body as plain text and pass that path to `scan-text`. |
| `crates/takt/tests/forge_scan_policy.rs:133-140` | CONFIRMED | introduced | The fake `Gates` refuses any line without `--policy`. Real b10x-gates reads `B10X_GATES_POLICY` from its environment (gates `main.rs:21`). | No CLI run reaches that state. In production only `for_tree` builds a `BotRoute`, and it takes the policy from that same variable (`forge.rs:655`). takt keeps `B10X_*` in the child's environment (`forge.rs:246-262`, no `env_clear`). So "trial reproduced on the base" is an artefact of the fixture, and the code change has no reachable effect on the CLI path. |

**5. What I attacked and could not break**
- **Argument agreement:** both lines come from one helper, `scope()` (`forge.rs:684`). They agree for a missing policy, a set policy, and paths with spaces. Leading dashes or an empty policy would put the same token on both lines, and `api` already behaved that way on the base. A scanned path starting with `-` cannot happen, because the path is always `<tree>/.engineering/cases/...`.
- **Push, merge, view, checks:** `b10x-gates bot` returns before it loads any policy (gates `main.rs:201`). Merge goes through `api_argv`, so it carries the scope. `gh pr view` and `gh pr checks` only read and need no policy.
- **New error text:** it is true on every path that reaches it. The only caller is `validator.rs:756`, after `push_revision` succeeded, and `BotRoute::push_revision` always pushes.
- **Mutants:** dropping `--repository` or `--policy`, or the branch name from the message, is caught by the exact-line assertions in `forge.rs:595` and `forge_scan_policy.rs`.
- **Response files:** they only hold what comes back. The merge request `{"sha"}` is scanned by `api` itself.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/pr-scan-policy/adv1/` (the directory)
- `<wave-scratch>/scratch/pr-scan-policy/adv1/red-case.log`
- `<wave-scratch>/scratch/pr-scan-policy/adv1/suite.log`
- `<wave-scratch>/scratch/pr-scan-policy/adv1/homes_probe.py`

The test's scratch directory under `CARGO_TARGET_TMPDIR`, inside the worktree, is removed by the case. I released the session lease `w7-pr-scan-policy-adv1`.

```findings
[
  {"file": "crates/takt/src/forge.rs", "line": 855, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "scan-text reads the JSON-escaped request, so gates' line-anchored home-path rule and wrapped-literal rule cannot match inside a multi-line pull-request body that the forge then publishes decoded"},
  {"file": "crates/takt/tests/forge_scan_policy.rs", "line": 133, "category": "acceptance", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the fake b10x-gates ignores B10X_GATES_POLICY, which the real binary reads and the only production constructor (for_tree) is fed from, so the base reproduction and the scenario-1 acceptance describe a state no CLI run reaches"}
]
```
