---
format: aep.planning-md/3
id: review-result:findings-hunt-33ab42c
kind: review-result
status: active
title: 'Reviewer: findings hunt on main 33ab42c (CLI, forge path, case lifecycle)'
relations:
- reviews: story:records-reach-base
- reviews: story:cli-protocol-defaults
prose_only: a code review returning findings with predicates, not a plan critic verdict
revision: 1
---
I found six offline findings and ran every predicate the way `scripts/check-finding` does, with takt built from 3fe3f21. I reused that build because `git diff --stat 3fe3f21 33ab42c -- crates/ protocols/` is empty. Each predicate held as written, and the concurrency one held 3 times out of 3. None of them is in the three review results, the open story titles, story:wave11-review-follow-ups or confinement § Routes. Disk had 49G free.

| # | severity | title | file:line |
|---|---|---|---|
| 1 | low | Resuming a case whose branch was deleted ends in a raw git error | `crates/takt/src/case.rs:1528-1531` |
| 2 | low | `--gate` naming an action the protocol lacks is accepted and ignored | `crates/takt-cli/src/main.rs:187-189`, `:524`; `crates/takt/src/run.rs:3759` |
| 3 | medium | `--test-cmd` is not checked before the case opens | `crates/takt/src/validator.rs:206-215`, `:275-279`; `crates/takt-cli/src/main.rs:544` |
| 4 | medium | After `takt approve` on an intent case, the next plain `takt run` takes a story instead | `crates/takt-cli/src/main.rs:253`, `:38-40`; `crates/takt/src/run.rs:2712-2726` |
| 5 | medium | The scan policy path is resolved inside the case's tree, so a step can rewrite it | `crates/takt/src/forge.rs:850`, `:879-886` |
| 6 | high | Two `takt run` on one tree: the second run's case and branch are deleted, and HEAD is left on a branch that no longer exists | `crates/takt/src/case.rs:137-160`; `crates/takt/src/run.rs:2565`, `:3507` |

`source: REVIEW_RECORD` is a placeholder; it needs a real path before `file-finding` will parse the block.

---

**Finding 1: resuming a case whose branch was deleted ends in a raw git error**

Outcome: `takt run --case <id>` on a case whose branch `takt/<id>` is gone exits 1 with takt's own message. The message names the missing branch, the last head the case recorded, and the next step: recreate the branch at that head, or abandon the case.

Context: `Case::resume` runs `git checkout -q takt/<id>` and only adds context to the error (`case.rs:1528-1531`). Observed: exit 1, `takt: check out the branch of the case a: git checkout -q takt/a in <tree> exited exit status: 1: error: pathspec 'takt/a' did not match any file(s) known to git`. The case directory and its `branch.yaml` are still there, but nothing says how to recover.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent a --case-id a --harness none --forge none --iterations 3 --test-cmd ./check.sh >/dev/null 2>&1; git checkout -q trial/0 && git branch -q -D takt/a && "$TAKT" run --repo . --case a --harness none --forge none --iterations 3 --test-cmd ./check.sh
exit: 1
output: did not match any file(s) known to git
source: REVIEW_RECORD
```

Scope: `crates/takt/src/case.rs` (`Case::resume`).

Acceptance: In the predicate's run, stderr names `takt/a` as missing and says how to recreate it or abandon the case. No git pathspec text appears, and nothing is written.

---

**Finding 2: `--gate` naming no action of the protocol is accepted and ignored**

Outcome: A `--gate` that names no approval-required action of the protocol the case runs on is refused with exit 1 before the case opens.

Context: `--gate` is copied into `RunOptions.gate` (`main.rs:524`) and only used as a filter (`run.rs:3759`), so a typo means the gate is never offered. Observed with `--gate no.such.action`:
- the case opened on `software.change/1`;
- `repository.merge` became approval-required but was never offered;
- the run ended `stopped: error: … no harness`, exit 5.

A second flag is ignored the same way: `--case-id zzz` given with `--case a` resumed `a` and created no `zzz`, while `--protocol` and `--input` are refused on a resume (`main.rs:1217-1245`).

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent probe --harness none --forge none --iterations 3 --test-cmd ./check.sh --gate no.such.action
exit: 5
output: on software.change/1
source: REVIEW_RECORD
```

Scope: `crates/takt-cli/src/main.rs` (`LoopArgs::check`, `RunArgs` handling; refusing `--case-id` together with `--case`).

Acceptance: The predicate exits 1 and names the protocol's approval-required actions, and no case directory exists. `--case a --case-id zzz` exits 1.

---

**Finding 3: the test command is not checked before the case opens**

Outcome: A `--test-cmd` whose program cannot be started, or that holds shell quoting, is refused with exit 1 before any case is opened.

Context: `TestsRun::new` only splits on white space (`validator.rs:206-215`). The program is first started inside the `tests.run` step (`:275-279`). That contradicts `takt run --help` exit 1, "refused before the run (an argument …)".

- Missing file, observed: exit 5 with `run ./no-such-check.sh: run ./no-such-check.sh: No such file or directory (os error 2)`. The error repeats `run ./no-such-check.sh:` twice. The case stayed open and the tree was left on its branch.
- Quoted command, observed: `--test-cmd "sh -c 'PYTHONPATH=. python3 -m unittest discover -s tests'"` recorded `evidence: test_result fail`, with `unexpected EOF while looking for matching` in the summary. A quoting mistake became a test-failure record about the implementation.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent probe --harness none --forge none --iterations 2 --test-cmd ./no-such-check.sh
exit: 5
output: No such file or directory (os error 2)
source: REVIEW_RECORD
```

Scope: `crates/takt/src/validator.rs` (`TestsRun::new`), `crates/takt-cli/src/main.rs` (`validators_of`, the `--test-cmd` help).

Acceptance:
- The predicate exits 1 and names the program, and no case directory or `takt/` branch is created.
- A `--test-cmd` holding `'` or `"` exits 1 and says the command is split at white space without a shell.

---

**Finding 4: after `takt approve` on an intent case, the next plain `takt run` takes a story instead of resuming it**

Outcome: After `takt approve` on a case opened with `--intent`, the next step the operator is told to take resumes that case. Either a plain `takt run` resumes it, or the help, the approve help and the `ApprovalRequired` stop line name `takt run --case <id>`.

Context: Both `takt run --help` exit 4 (`main.rs:253`) and `takt approve --help` (`main.rs:38-40`) say "the next run resumes the case". But `next_to_resume` (`run.rs:2712-2726`) only finds story cases, and a tree on a case's branch gets `None`. So the plain run goes to `take_next`, which takes the next ready story. Observed:
- the intent case `probe` stopped `ApprovalRequired (repository.merge)`, exit 4;
- `takt approve` exited 0, granted it, and printed "recorded: nothing; the case was opened on an intent, not a story";
- the next `takt run` printed `frontier: took story:remove-a-todo`, opened a second case and exited 5;
- `probe` keeps its grant, unresumed.

`takt scenario run --help` already names `takt run --case <case>`. Not the filed NoProgress stop-line finding: this is the `ApprovalRequired` path, where the documented next step is wrong.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent probe --case-id probe --harness none --forge none --iterations 5 --test-cmd ./check.sh >/dev/null 2>&1; "$TAKT" approve --repo . --by human:reviewer probe repository.merge >/dev/null 2>&1 && "$TAKT" run --repo . --harness none --forge none --iterations 3 --test-cmd ./check.sh
exit: 5
output: frontier: took story:remove-a-todo
source: REVIEW_RECORD
```

Scope: `crates/takt/src/run.rs` (`next_to_resume`), or `crates/takt-cli/src/main.rs` (`EXIT_CODES`, the `Approve` doc, `finish`'s stop line).

Acceptance: In the predicate's run, the last command resumes `probe` and opens no story case. Or, if the fix is to the wording, `takt run --help`, `takt approve --help` and the stop line all name `takt run --case probe`.

---

**Finding 5: the bot route's scan policy is read from a path a judgement step can write**

Outcome: The `b10x-gates` policy `pr.open` scans with is a file no step can change. `B10X_GATES_POLICY` is resolved to an absolute path when the run starts, and a path under the repository's tree is refused with exit 1.

Context: `BotRoute::for_tree` takes `B10X_GATES_POLICY` as given (`forge.rs:850`) and passes it as `--policy` (`:879-886`) to children whose working directory is the tree. So a relative path resolves inside the case's tree, and the policy is on no judge surface. The same holds for an absolute path into the tree.

The run used a fake `claude`, `b10x-gates` and `gh` (the blob). Step 5 `implementation.edit` wrote `policy.yaml` and the commit was taken ("committed 2 path(s)"). Then `pr.open` ran `scan-text --repository o/r --policy policy.yaml …` in the tree, and the scanner read the step's text. The scan guards what leaves the machine, so this is the "nothing a model writes becomes a gate definition" rule (story:validator-confinement).

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCm1lPSR7MCMjKi99ClsgIiRtZSIgPSBjbGF1ZGUgXSAmJiBlY2hvICJjbGF1ZGUiID4+ICIkSE9NRS9mb3JnZS5sb2ciIHx8IGVjaG8gIiRtZXwkUFdEfCQqIiA+PiAiJEhPTUUvZm9yZ2UubG9nIgpjYXNlICRtZSBpbgpjbGF1ZGUpCiAgbj0kKGNhdCAiJEhPTUUvbiIgMj4vZGV2L251bGwgfHwgZWNobyAwKTsgbj0kKChuKzEpKTsgZWNobyAkbiA+ICIkSE9NRS9uIgogIGlmIFsgJG4gPSAxIF07IHRoZW4gcHJpbnRmICdcbmRlZiByZW1vdmUoaXRlbXMsIG4pOlxuICAgIHJldHVybiBpdGVtc1xuJyA+PiB0b2RvLnB5OyBbIC1uICIkRktfU1RFUCIgXSAmJiBzaCAtYyAiJEZLX1NURVAiOyBhPWltcGxlbWVudGF0aW9uLmVkaXQ7IGVsc2UgYT0ke0ZLX05FWFQ6LXJlcG9zaXRvcnkubWVyZ2V9OyBmaQogIHByaW50ZiAneyJyZXN1bHQiOiJ7XFwiYWN0aW9uXFwiOlxcIiVzXFwiLFxcInN1bW1hcnlcXCI6XFwic1xcIn0iLCJ1c2FnZSI6eyJpbnB1dF90b2tlbnMiOjEsIm91dHB1dF90b2tlbnMiOjF9fVxuJyAiJGEiOzsKYjEweC1nYXRlcykKICBjYXNlICQxIGluCiAgYm90KSBbICIkMyIgPSBwdXNoIF0gJiYgeyByPSR7NSUlOip9OyBnaXQgcmV2LXBhcnNlICIkciIgPiAiJEhPTUUvcHVzaGVkIjsgfTsgZXhpdCAwOzsKICBzY2FuLXRleHQpIHA9OyB3aGlsZSBbICQjIC1ndCAwIF07IGRvIFsgIiQxIiA9IC0tcG9saWN5IF0gJiYgcD0kMjsgc2hpZnQ7IGRvbmU7IFsgLW4gIiRwIiBdICYmIGVjaG8gInNjYW4tcmVhZDogJChjYXQgIiRwIikiID4+ICIkSE9NRS9mb3JnZS5sb2ciOyBleGl0IDA7OwogIGFwaSkgb3V0PTsgbT07IHdoaWxlIFsgJCMgLWd0IDAgXTsgZG8gY2FzZSAkMSBpbiAtLW91dHB1dCkgb3V0PSQyOzsgLS1tZXRob2QpIG09JDI7OyBlc2FjOyBzaGlmdDsgZG9uZQogICAgaWYgWyAiJG0iID0gUE9TVCBdOyB0aGVuIFsgLW4gIiRGS19QT1NUX0ZBSUwiIF0gJiYgeyBlY2hvICIkRktfUE9TVF9GQUlMIiA+JjI7IGV4aXQgMTsgfTsgZWNobyAneyJudW1iZXIiOjd9JyA+ICIkb3V0IjsgZWxzZSBbIC1uICIkRktfTUVSR0VfRkFJTCIgXSAmJiB7IGVjaG8gIiRGS19NRVJHRV9GQUlMIiA+JjI7IGV4aXQgMTsgfTsgZWNobyAneyJtZXJnZWQiOnRydWV9JyA+ICIkb3V0IjsgZmk7IGV4aXQgMDs7CiAgZXNhYzs7CmdoKQogIGNhc2UgJDIgaW4KICB2aWV3KSBwcmludGYgJ3sic3RhdGUiOiIlcyIsIm1lcmdlZEF0IjpudWxsLCJoZWFkUmVmTmFtZSI6IiVzIiwiaGVhZFJlZk9pZCI6IiVzIn1cbicgIiR7RktfU1RBVEU6LU9QRU59IiAiJChnaXQgc3ltYm9saWMtcmVmIC0tc2hvcnQgSEVBRCkiICIkKGNhdCAiJEhPTUUvcHVzaGVkIikiOzsKICBjaGVja3MpIFsgLW4gIiRGS19OT0NIRUNLUyIgXSAmJiB7IGVjaG8gIm5vIGNoZWNrcyByZXBvcnRlZCBvbiB0aGUgJ3gnIGJyYW5jaCIgPiYyOyBleGl0IDE7IH07IGVjaG8gJ1t7ImJ1Y2tldCI6InBhc3MifV0nOzsKICBlc2FjOzsKZXNhYwo= | base64 -d > "$HOME/bin/fk" && chmod +x "$HOME/bin/fk" && for n in claude b10x-gates gh; do ln -s fk "$HOME/bin/$n"; done && git remote add origin https://github.com/o/r.git && B10X_GATES_POLICY=policy.yaml FK_STEP='echo written-by-the-step > policy.yaml' PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --test-cmd ./check.sh --iterations 12 >/dev/null 2>&1; cat "$HOME/forge.log"
exit: 0
output: scan-read: written-by-the-step
source: REVIEW_RECORD
```

Scope: `crates/takt/src/forge.rs` (`BotRoute::for_tree`, `scope`); possibly `crates/takt-cli/src/main.rs` (`forge_of`).

Acceptance: In the predicate's run, the run exits 1 before the case opens and names the policy path as being under the tree. A `B10X_GATES_POLICY` that is absolute and outside the tree is passed on unchanged.

---

**Finding 6: two `takt run` on one tree destroy each other's state**

Outcome: A second `takt run` on a tree where a run is in progress is refused with exit 1 before it opens or checks out anything. The first run's case, branch and HEAD stay as they were.

Context: The only lock is per case, in `run.lock` (`case.rs:137-160`). The clean-tree check at open passes while the first run's step is still running. So run b opened case `b` and checked out `takt/b` underneath run a, which was in its `tests.run` step. Run a's step guard (`run.rs:3507` and the cases snapshot) then treated b's writes as its own validator's:
- it put back the cases directory and the refs, deleting `.engineering/cases/b/` and `refs/heads/takt/b`;
- it left `.git/HEAD` pointing at `refs/heads/takt/b`;
- run a stopped `StepRefused (tests.run wrote what its validator does not: .engineering/cases/b/…, .git/HEAD, refs/heads/takt/b from nothing to 865eaa8b)`, exit 3;
- run b exited 5 with `git rev-parse HEAD … unknown revision`.

Afterwards `git status` showed every tracked file as a staged addition on a branch with no commits. 3 out of 3 runs gave this. This is not story:case-scoped-step-guard, which is about a `takt approve` during a step. Here two runs open cases in one worktree and nothing serializes them.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent a --case-id a --harness none --forge none --iterations 1 --test-cmd "sleep 3" >/dev/null 2>&1 & sleep 1; "$TAKT" run --repo . --intent b --case-id b --harness none --forge none --iterations 1 --test-cmd "sleep 3" >/dev/null 2>&1; wait; git rev-parse -q --verify HEAD >/dev/null || echo "HEAD names no commit: $(git symbolic-ref HEAD)"
exit: 0
output: HEAD names no commit: refs/heads/takt/b
source: REVIEW_RECORD
```

Scope: `crates/takt/src/case.rs` (a tree-level hold taken before open, resume or take), `crates/takt/src/run.rs` (`open_unit`, `resume`, `take_next`), `crates/takt-cli/src/main.rs` (`scenario_run`, which checks out a trial branch before any hold).

Acceptance: In the predicate's run, the second `takt run` exits 1 and says another run holds the tree. No case `b` and no branch `takt/b` are created. Run a's step is judged only on its own writes, and HEAD stays on `takt/a`.

---

**Forge path (bot forge faked offline with `claude`, `b10x-gates` and `gh` on `PATH`; no predicates; I don't know how the real forge behaves):**

- **Merge refused (405 "not mergeable"):** exit 5 every time. Each resume sends the PUT again (2 PUTs over 2 runs), and nothing offers a way past it, such as a rebase step or a decline. Before the PUT, the rider commits `takt: test_result` and `takt: implemented story:…` already sit on the case's branch. Not resumable; closest filed is story:abandon-a-case.
- **Open refused (422 "already exists"):** exit 5, and the next run sends the POST again with no lookup of the existing pull request by head. Covered by story:fenced-forge-dispatch's "resolve by a read".
- **CI never reports:** `no checks reported` counts as pending for good (`forge.rs:1111-1121`). Every run ends `Waiting (ci.read; Iterations (n))`, exit 6, with 10 `gh pr checks` calls over 18 steps. A repository with no workflow can never finish, and no stop line says so.
- **Wasted steps at the seed:** every story run spends steps 1-4 on `tests.run`, `gate.run`, `pr.open` ("no commit beyond trial/0") and `ci.read` ("no pull request") before any change exists. This is close to story:no-merge-approval-without-a-change, but about validators rather than the approval.
- **`takt approve --by`:** the help says only "Who grants it". A story case refuses `--by r` (it wants `human:<name>` or `agent:<role>`), while an intent case accepts `--by reviewer`. `takt approve` and `takt protocols` list no exit codes.

Nothing was written to the repository or the store, and nothing is left running. The runs, the fake forge (`fk.sh`), the predicate files (`preds/p1` … `preds/p6`) and their outputs are in the reviewer's scratch directory.