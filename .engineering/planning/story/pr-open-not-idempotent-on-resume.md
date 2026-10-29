---
format: aep.planning-md/3
id: story:pr-open-not-idempotent-on-resume
kind: story
status: active
title: pr.open is not idempotent on resume
tags:
- finding
- rsi
refs:
- provider: finding
  reference: pr-open-not-idempotent-on-resume
relations:
- depends_on: story:bot-forge-reads-through-the-app
- decomposes: epic:takt-as-a-loom-agent
- depends_on: story:forge-preflight
- serves: vision:safe-agentic-coding
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:36:07Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-29T21:36:07Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
---
# pr.open is not idempotent on resume

`pr.open` records the pull request's number in the case only after the forge's answer has been
read (`crates/takt/src/validator.rs`, `PrOpen`: `branch.write` follows a successful POST). When
the step ends after the forge created the pull request but before takt recorded it, the case
holds no number, and the resumed run POSTs again. GitHub answers 422, "A pull request already
exists", and the resumed run stops `error` with the case still open, although the pull request
it needs is there and could be read.

Seen in a trial run on metaharness 0.11.0, Codex, scenario 1: `pr.open` was refused after the
forge had created the pull request (the step guard saw files written by another process under
the toolchain's home), and the resume stopped `stopped: error: pr.open (takt.pr-open): open the
pull request for takt/<case> through the forge bot: b10x-gates api --method POST ... exited 1:
b10x-gates: GitHub operation failed (422 Unprocessable Entity): Validation Failed; PullRequest
custom: A pull request already exists for <owner>:takt/<case>.`, exit 5. Every later resume
ends the same way, so the case can neither be merged nor closed through takt, and the trial's
acceptance has to be read from the forge by hand.

The predicate reaches the same state with a fake forge: the first POST creates the pull
request and its answer is lost (exit 1, 502); the resume's POST gets the 422. A takt that reads
the existing pull request on resume does not stop on it.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCm1lPSR7MCMjKi99Cmg9QEhACmNhc2UgJG1lIGluCmNsYXVkZSkKICBjYXNlICIkKiIgaW4gKi0tdmVyc2lvbiopIGVjaG8gIjIuMS4yNTkgKENsYXVkZSBDb2RlKSI7IGV4aXQgMDs7IGVzYWMKICBuPSQoY2F0ICIkaC9uIiAyPi9kZXYvbnVsbCB8fCBlY2hvIDApOyBuPSQoKG4rMSkpOyBlY2hvICRuID4gIiRoL24iCiAgWyAtbiAiJEZLX0lOSVQiIF0gJiYgZWNobyAneyJ0eXBlIjoic3lzdGVtIiwic3VidHlwZSI6ImluaXQiLCJjbGF1ZGVfY29kZV92ZXJzaW9uIjoiMi4xLjI1OSIsIm1vZGVsIjoiZmFrZS1tb2RlbCJ9JwogIGNhc2UgJG4gaW4KICAxKSBwcmludGYgJ1xuZGVmIHJlbW92ZShpdGVtcywgbik6XG4gICAgcmV0dXJuIGl0ZW1zXG4nID4+IHRvZG8ucHk7IGE9aW1wbGVtZW50YXRpb24uZWRpdDs7CiAgMikgYT1jaGFuZ2UuaGFuZG92ZXI7OwogICopIGE9cmVwb3NpdG9yeS5tZXJnZTs7CiAgZXNhYwogIHByaW50ZiAneyJ0eXBlIjoicmVzdWx0Iiwic3VidHlwZSI6InN1Y2Nlc3MiLCJyZXN1bHQiOiJ7XFwiYWN0aW9uXFwiOiBcXCIlc1xcIiwgXFwic3VtbWFyeVxcIjogXFwic1xcIn0iLCJ1c2FnZSI6eyJpbnB1dF90b2tlbnMiOjEsIm91dHB1dF90b2tlbnMiOjF9fVxuJyAiJGEiOzsKYjEweC1nYXRlcykKICBjYXNlICQxIGluCiAgYm90KSBbICIkMyIgPSBwdXNoIF0gJiYgeyBnaXQgcmV2LXBhcnNlICIkezUlJToqfSIgPiAiJGgvcHVzaGVkIjsgZWNobyAiJHs1Iyo6cmVmcy9oZWFkcy99IiA+ICIkaC9icmFuY2giOyB9OyBleGl0IDA7OwogIGFwaSkgb3V0PTsgbT1HRVQ7IHA9OyB3aGlsZSBbICQjIC1ndCAwIF07IGRvIGNhc2UgJDEgaW4gLS1vdXRwdXQpIG91dD0kMjs7IC0tbWV0aG9kKSBtPSQyOzsgLS1wYXRoKSBwPSQyOzsgZXNhYzsgc2hpZnQ7IGRvbmUKICAgIGI9JChjYXQgIiRoL2JyYW5jaCIgMj4vZGV2L251bGwpOyBzPSQoY2F0ICIkaC9wdXNoZWQiIDI+L2Rldi9udWxsKQogICAgaWYgWyAiJG0iID0gUE9TVCBdOyB0aGVuCiAgICAgIGlmIFsgLWUgIiRoL3ByIiBdOyB0aGVuIGVjaG8gImIxMHgtZ2F0ZXM6IEdpdEh1YiBvcGVyYXRpb24gZmFpbGVkICg0MjIgVW5wcm9jZXNzYWJsZSBFbnRpdHkpOiBWYWxpZGF0aW9uIEZhaWxlZDsgUHVsbFJlcXVlc3QgY3VzdG9tOiBBIHB1bGwgcmVxdWVzdCBhbHJlYWR5IGV4aXN0cyBmb3IgbzokYi4iID4mMjsgZXhpdCAxOyBmaQogICAgICBbIC16ICIkRktfTE9TVCIgXSAmJiB0b3VjaCAiJGgvcHIiCiAgICAgIGVjaG8gImIxMHgtZ2F0ZXM6IEdpdEh1YiBvcGVyYXRpb24gZmFpbGVkICg1MDIgQmFkIEdhdGV3YXkpOiB0aGUgcmVzcG9uc2Ugd2FzIGxvc3QiID4mMjsgZXhpdCAxCiAgICBmaQogICAgcHI9JChwcmludGYgJ3sibnVtYmVyIjo3LCJzdGF0ZSI6Im9wZW4iLCJtZXJnZWQiOmZhbHNlLCJoZWFkIjp7InJlZiI6IiVzIiwic2hhIjoiJXMifX0nICIkYiIgIiRzIikKICAgIGNhc2UgJG06JHAgaW4KICAgIEdFVDoqL3B1bGxzLzcqKSBlY2hvICIkcHIiID4gIiRvdXQiOzsKICAgIEdFVDoqKSBpZiBbIC1lICIkaC9wciIgXTsgdGhlbiBlY2hvICJbJHByXSIgPiAiJG91dCI7IGVsc2UgZWNobyAnW10nID4gIiRvdXQiOyBmaTs7CiAgICAqKSBlY2hvICd7Im1lcmdlZCI6dHJ1ZX0nID4gIiRvdXQiOzsKICAgIGVzYWM7IGV4aXQgMDs7CiAgKikgZXhpdCAwOzsKICBlc2FjOzsKZ2gpCiAgYj0kKGNhdCAiJGgvYnJhbmNoIiAyPi9kZXYvbnVsbCk7IHM9JChjYXQgIiRoL3B1c2hlZCIgMj4vZGV2L251bGwpCiAgY2FzZSAkMiBpbgogIHZpZXcpIHByaW50ZiAneyJudW1iZXIiOjcsInN0YXRlIjoiT1BFTiIsIm1lcmdlZEF0IjpudWxsLCJoZWFkUmVmTmFtZSI6IiVzIiwiaGVhZFJlZk9pZCI6IiVzIn1cbicgIiRiIiAiJHMiOzsKICBsaXN0KSBpZiBbIC1lICIkaC9wciIgXTsgdGhlbiBwcmludGYgJ1t7Im51bWJlciI6Nywic3RhdGUiOiJPUEVOIiwiaGVhZFJlZk5hbWUiOiIlcyIsImhlYWRSZWZPaWQiOiIlcyJ9XVxuJyAiJGIiICIkcyI7IGVsc2UgZWNobyAnW10nOyBmaTs7CiAgY2hlY2tzKSBlY2hvICdbeyJidWNrZXQiOiJwYXNzIn1dJzs7CiAgZXNhYzs7CmVzYWMK | base64 -d | sed "s|@H@|$HOME|g" > "$HOME/bin/fk" && chmod +x "$HOME/bin/fk" && for n in claude b10x-gates gh; do ln -s fk "$HOME/bin/$n"; done && git config user.name t && git config user.email t@example.invalid && git remote add origin https://github.com/o/r.git && export XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" && "$TAKT" run --repo . --intent "Remove a todo by its number" --test-cmd ./check.sh --min-free-gib 0 --iterations 12 > "$HOME/run1" 2>&1; test -e "$HOME/pr" || { cat "$HOME/run1"; echo "the first run created no pull request" >&2; exit 2; }; c=$(ls .engineering/cases | head -n 1); "$TAKT" run --repo . --case "$c" --test-cmd ./check.sh --min-free-gib 0 --iterations 12 > "$HOME/run2" 2>&1; rc=$?; cat "$HOME/run2"; grep -q "A pull request already exists" "$HOME/run2" && exit 9; [ "$rc" -ne 5 ] && exit 0; exit 2
exit: 9
healthy_exit: 0
output: A pull request already exists
source: .engineering/planning/approval-record/trial-2026-10-21-bb4072a3-hand-over.md
```

## Scope

- `crates/takt/src/validator.rs`: `PrOpen` (`:3718`): before a POST, and after a POST that failed, read the open pull requests whose head is the case's branch and take an existing one's number (cited)
- `crates/takt/src/forge/bot.rs`: a read of the open pull requests for a head branch, through `b10x-gates api` `GET` (inferred)
- `crates/takt/tests/`: the predicate's lost-answer world with a forge double (inferred)

The read of open pull requests is shared with story:fenced-forge-dispatch, which records the
outcome class of the same write: one unit delivers both.

## Acceptance

The predicate in the body exits 0 on takt with the fix: with a forge double whose first `POST`
creates the pull request and then fails (502), the resumed run reads the pull request the first
POST created, records its number in the case, sends no second `POST`, and does not stop on
`A pull request already exists`. The double's call log shows exactly one `POST` to `/pulls`
across both runs.

## Why a resume

A cluster run resumes a case: to run once more, the operator changes the `LoomAgent`, the new
generation's Job starts takt again, and takt resumes the case that is open for the story
(`examples/cluster-delivery/README.md`, § One run; `website/docs/guides/deploy-on-kubernetes.md`,
§ Watch it). A resume after a lost `pr.open` answer then stops on GitHub's 422 for good.

This story owns `pr.open`'s lost answer: before a `POST`, and after a `POST` that failed, it reads
the open pull requests for the case's branch and takes an existing one. story:fenced-forge-dispatch
keeps the outcome record of the forge writes and the merge's `unknown`, and depends on this story.

## The predicate after the fix

The `expected_before` predicate above predates story:bot-forge-reads-through-the-app and
story:forge-preflight, and exits non-zero on any takt that carries them: its `b10x-gates` double
answers pull requests with no `base` and no `head.repo.full_name`, and the run sets no
`B10X_GATES_POLICY`, so the preflight refuses it before the case opens. The Rust test
`crates/takt/tests/pr_open_after_a_lost_answer.rs`,
`a_post_that_fails_after_creating_the_pull_request_takes_its_number`, builds the same world (a
POST that creates the pull request and answers 502) and checks one POST across both runs.
