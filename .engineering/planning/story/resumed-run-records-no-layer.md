---
format: aep.planning-md/3
id: story:resumed-run-records-no-layer
kind: story
status: draft
title: A resumed run that takes no harness step records no harness or model layer
tags:
- finding
- rsi
refs:
- provider: finding
  reference: resumed-run-records-no-layer
revision: 2
---
# A resumed run that takes no harness step records no harness or model layer

A run's `opened` line records only the `takt` layer (`crates/takt/src/run.rs`); the `harness`
and `model` layers are joined in by the scorecard from the run's harness steps, their `version`
and `model` (`crates/takt/src/scorecard.rs`, `Span::row`). A resumed run that takes only
validator steps, such as one that goes straight back to `pr.open`, has no harness step, so its
row records `harness` and `model` as none, even though it was started with the same
`--harness` as the run it resumes.

`takt scorecard` compares every run with the first and refuses, exit 1, when their layers
differ. One resume in a set is enough: a trial on metaharness 0.11.0, Codex, scenario 1, whose
case was resumed once, was refused with "layer `harness`: run <case>#1 records `0.153.4`, run
<case>#2 records none: the runs' layers differ; a scorecard scores runs of one stack, so nothing
is scored (pass --across-layers to compare across them)". The set had to be scored with
`--across-layers`, which also hides a real difference of stack.

The predicate opens a case on a fake Claude Code that states its version and model, lets
`pr.open` fail at the fake forge (exit 1, 502, every time), resumes the case once, which stops at
`pr.open` again without a harness step, and scores the case.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCm1lPSR7MCMjKi99Cmg9QEhACmNhc2UgJG1lIGluCmNsYXVkZSkKICBjYXNlICIkKiIgaW4gKi0tdmVyc2lvbiopIGVjaG8gIjIuMS4yNTkgKENsYXVkZSBDb2RlKSI7IGV4aXQgMDs7IGVzYWMKICBuPSQoY2F0ICIkaC9uIiAyPi9kZXYvbnVsbCB8fCBlY2hvIDApOyBuPSQoKG4rMSkpOyBlY2hvICRuID4gIiRoL24iCiAgWyAtbiAiJEZLX0lOSVQiIF0gJiYgZWNobyAneyJ0eXBlIjoic3lzdGVtIiwic3VidHlwZSI6ImluaXQiLCJjbGF1ZGVfY29kZV92ZXJzaW9uIjoiMi4xLjI1OSIsIm1vZGVsIjoiZmFrZS1tb2RlbCJ9JwogIGNhc2UgJG4gaW4KICAxKSBwcmludGYgJ1xuZGVmIHJlbW92ZShpdGVtcywgbik6XG4gICAgcmV0dXJuIGl0ZW1zXG4nID4+IHRvZG8ucHk7IGE9aW1wbGVtZW50YXRpb24uZWRpdDs7CiAgMikgYT1jaGFuZ2UuaGFuZG92ZXI7OwogICopIGE9cmVwb3NpdG9yeS5tZXJnZTs7CiAgZXNhYwogIHByaW50ZiAneyJ0eXBlIjoicmVzdWx0Iiwic3VidHlwZSI6InN1Y2Nlc3MiLCJyZXN1bHQiOiJ7XFwiYWN0aW9uXFwiOiBcXCIlc1xcIiwgXFwic3VtbWFyeVxcIjogXFwic1xcIn0iLCJ1c2FnZSI6eyJpbnB1dF90b2tlbnMiOjEsIm91dHB1dF90b2tlbnMiOjF9fVxuJyAiJGEiOzsKYjEweC1nYXRlcykKICBjYXNlICQxIGluCiAgYm90KSBbICIkMyIgPSBwdXNoIF0gJiYgeyBnaXQgcmV2LXBhcnNlICIkezUlJToqfSIgPiAiJGgvcHVzaGVkIjsgZWNobyAiJHs1Iyo6cmVmcy9oZWFkcy99IiA+ICIkaC9icmFuY2giOyB9OyBleGl0IDA7OwogIGFwaSkgb3V0PTsgbT1HRVQ7IHA9OyB3aGlsZSBbICQjIC1ndCAwIF07IGRvIGNhc2UgJDEgaW4gLS1vdXRwdXQpIG91dD0kMjs7IC0tbWV0aG9kKSBtPSQyOzsgLS1wYXRoKSBwPSQyOzsgZXNhYzsgc2hpZnQ7IGRvbmUKICAgIGI9JChjYXQgIiRoL2JyYW5jaCIgMj4vZGV2L251bGwpOyBzPSQoY2F0ICIkaC9wdXNoZWQiIDI+L2Rldi9udWxsKQogICAgaWYgWyAiJG0iID0gUE9TVCBdOyB0aGVuCiAgICAgIGlmIFsgLWUgIiRoL3ByIiBdOyB0aGVuIGVjaG8gImIxMHgtZ2F0ZXM6IEdpdEh1YiBvcGVyYXRpb24gZmFpbGVkICg0MjIgVW5wcm9jZXNzYWJsZSBFbnRpdHkpOiBWYWxpZGF0aW9uIEZhaWxlZDsgUHVsbFJlcXVlc3QgY3VzdG9tOiBBIHB1bGwgcmVxdWVzdCBhbHJlYWR5IGV4aXN0cyBmb3IgbzokYi4iID4mMjsgZXhpdCAxOyBmaQogICAgICBbIC16ICIkRktfTE9TVCIgXSAmJiB0b3VjaCAiJGgvcHIiCiAgICAgIGVjaG8gImIxMHgtZ2F0ZXM6IEdpdEh1YiBvcGVyYXRpb24gZmFpbGVkICg1MDIgQmFkIEdhdGV3YXkpOiB0aGUgcmVzcG9uc2Ugd2FzIGxvc3QiID4mMjsgZXhpdCAxCiAgICBmaQogICAgcHI9JChwcmludGYgJ3sibnVtYmVyIjo3LCJzdGF0ZSI6Im9wZW4iLCJtZXJnZWQiOmZhbHNlLCJoZWFkIjp7InJlZiI6IiVzIiwic2hhIjoiJXMifX0nICIkYiIgIiRzIikKICAgIGNhc2UgJG06JHAgaW4KICAgIEdFVDoqL3B1bGxzLzcqKSBlY2hvICIkcHIiID4gIiRvdXQiOzsKICAgIEdFVDoqKSBpZiBbIC1lICIkaC9wciIgXTsgdGhlbiBlY2hvICJbJHByXSIgPiAiJG91dCI7IGVsc2UgZWNobyAnW10nID4gIiRvdXQiOyBmaTs7CiAgICAqKSBlY2hvICd7Im1lcmdlZCI6dHJ1ZX0nID4gIiRvdXQiOzsKICAgIGVzYWM7IGV4aXQgMDs7CiAgKikgZXhpdCAwOzsKICBlc2FjOzsKZ2gpCiAgYj0kKGNhdCAiJGgvYnJhbmNoIiAyPi9kZXYvbnVsbCk7IHM9JChjYXQgIiRoL3B1c2hlZCIgMj4vZGV2L251bGwpCiAgY2FzZSAkMiBpbgogIHZpZXcpIHByaW50ZiAneyJudW1iZXIiOjcsInN0YXRlIjoiT1BFTiIsIm1lcmdlZEF0IjpudWxsLCJoZWFkUmVmTmFtZSI6IiVzIiwiaGVhZFJlZk9pZCI6IiVzIn1cbicgIiRiIiAiJHMiOzsKICBsaXN0KSBpZiBbIC1lICIkaC9wciIgXTsgdGhlbiBwcmludGYgJ1t7Im51bWJlciI6Nywic3RhdGUiOiJPUEVOIiwiaGVhZFJlZk5hbWUiOiIlcyIsImhlYWRSZWZPaWQiOiIlcyJ9XVxuJyAiJGIiICIkcyI7IGVsc2UgZWNobyAnW10nOyBmaTs7CiAgY2hlY2tzKSBlY2hvICdbeyJidWNrZXQiOiJwYXNzIn1dJzs7CiAgZXNhYzs7CmVzYWMK | base64 -d | sed "s|@H@|$HOME|g" > "$HOME/bin/fk" && chmod +x "$HOME/bin/fk" && for n in claude b10x-gates gh; do ln -s fk "$HOME/bin/$n"; done && git config user.name t && git config user.email t@example.invalid && git remote add origin https://github.com/o/r.git && export XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" FK_INIT=1 FK_LOST=1 && "$TAKT" run --repo . --intent "Remove a todo by its number" --test-cmd ./check.sh --min-free-gib 0 --iterations 12 > "$HOME/run1" 2>&1; c=$(ls .engineering/cases | head -n 1); "$TAKT" run --repo . --case "$c" --test-cmd ./check.sh --min-free-gib 0 --iterations 12 > "$HOME/run2" 2>&1; grep -q "(502 Bad Gateway)" "$HOME/run2" || { cat "$HOME/run1" "$HOME/run2"; echo "the resumed run did not stop at pr.open" >&2; exit 2; }; "$TAKT" scorecard --repo .
exit: 1
healthy_exit: 0
output: records none: the runs' layers differ
source: .engineering/planning/approval-record/trial-2026-10-21-bb4072a3-hand-over.md
```
