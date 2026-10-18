---
format: aep.planning-md/3
id: story:scan-policy-outside-the-tree
kind: story
status: draft
title: The bot route's scan policy cannot be written by a step
tags:
- finding
refs:
- provider: finding
  reference: scan-policy-outside-the-tree
relations:
- informed_by: review-result:findings-hunt-33ab42c
- informed_by: story:validator-confinement
- depends_on: story:forge-preflight
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

The `b10x-gates` policy `pr.open` scans with is a file no step can change. `B10X_GATES_POLICY` is resolved to an absolute path when the run starts, and a path under the repository's tree is refused with exit 1.

## Context

`BotRoute::for_tree` takes `B10X_GATES_POLICY` as given (`forge.rs:850`) and passes it as `--policy` (`:879-886`) to children whose working directory is the tree. So a relative path resolves inside the case's tree, and the policy is on no judge surface. The same holds for an absolute path into the tree.

The run used a fake `claude`, `b10x-gates` and `gh` (the blob). Step 5 `implementation.edit` wrote `policy.yaml` and the commit was taken ("committed 2 path(s)"). Then `pr.open` ran `scan-text --repository o/r --policy policy.yaml …` in the tree, and the scanner read the step's text. The scan guards what leaves the machine, so this is the "nothing a model writes becomes a gate definition" rule (story:validator-confinement).

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCm1lPSR7MCMjKi99ClsgIiRtZSIgPSBjbGF1ZGUgXSAmJiBlY2hvICJjbGF1ZGUiID4+ICIkSE9NRS9mb3JnZS5sb2ciIHx8IGVjaG8gIiRtZXwkUFdEfCQqIiA+PiAiJEhPTUUvZm9yZ2UubG9nIgpjYXNlICRtZSBpbgpjbGF1ZGUpCiAgbj0kKGNhdCAiJEhPTUUvbiIgMj4vZGV2L251bGwgfHwgZWNobyAwKTsgbj0kKChuKzEpKTsgZWNobyAkbiA+ICIkSE9NRS9uIgogIGlmIFsgJG4gPSAxIF07IHRoZW4gcHJpbnRmICdcbmRlZiByZW1vdmUoaXRlbXMsIG4pOlxuICAgIHJldHVybiBpdGVtc1xuJyA+PiB0b2RvLnB5OyBbIC1uICIkRktfU1RFUCIgXSAmJiBzaCAtYyAiJEZLX1NURVAiOyBhPWltcGxlbWVudGF0aW9uLmVkaXQ7IGVsc2UgYT0ke0ZLX05FWFQ6LXJlcG9zaXRvcnkubWVyZ2V9OyBmaQogIHByaW50ZiAneyJyZXN1bHQiOiJ7XFwiYWN0aW9uXFwiOlxcIiVzXFwiLFxcInN1bW1hcnlcXCI6XFwic1xcIn0iLCJ1c2FnZSI6eyJpbnB1dF90b2tlbnMiOjEsIm91dHB1dF90b2tlbnMiOjF9fVxuJyAiJGEiOzsKYjEweC1nYXRlcykKICBjYXNlICQxIGluCiAgYm90KSBbICIkMyIgPSBwdXNoIF0gJiYgeyByPSR7NSUlOip9OyBnaXQgcmV2LXBhcnNlICIkciIgPiAiJEhPTUUvcHVzaGVkIjsgfTsgZXhpdCAwOzsKICBzY2FuLXRleHQpIHA9OyB3aGlsZSBbICQjIC1ndCAwIF07IGRvIFsgIiQxIiA9IC0tcG9saWN5IF0gJiYgcD0kMjsgc2hpZnQ7IGRvbmU7IFsgLW4gIiRwIiBdICYmIGVjaG8gInNjYW4tcmVhZDogJChjYXQgIiRwIikiID4+ICIkSE9NRS9mb3JnZS5sb2ciOyBleGl0IDA7OwogIGFwaSkgb3V0PTsgbT07IHdoaWxlIFsgJCMgLWd0IDAgXTsgZG8gY2FzZSAkMSBpbiAtLW91dHB1dCkgb3V0PSQyOzsgLS1tZXRob2QpIG09JDI7OyBlc2FjOyBzaGlmdDsgZG9uZQogICAgaWYgWyAiJG0iID0gUE9TVCBdOyB0aGVuIFsgLW4gIiRGS19QT1NUX0ZBSUwiIF0gJiYgeyBlY2hvICIkRktfUE9TVF9GQUlMIiA+JjI7IGV4aXQgMTsgfTsgZWNobyAneyJudW1iZXIiOjd9JyA+ICIkb3V0IjsgZWxzZSBbIC1uICIkRktfTUVSR0VfRkFJTCIgXSAmJiB7IGVjaG8gIiRGS19NRVJHRV9GQUlMIiA+JjI7IGV4aXQgMTsgfTsgZWNobyAneyJtZXJnZWQiOnRydWV9JyA+ICIkb3V0IjsgZmk7IGV4aXQgMDs7CiAgZXNhYzs7CmdoKQogIGNhc2UgJDIgaW4KICB2aWV3KSBwcmludGYgJ3sic3RhdGUiOiIlcyIsIm1lcmdlZEF0IjpudWxsLCJoZWFkUmVmTmFtZSI6IiVzIiwiaGVhZFJlZk9pZCI6IiVzIn1cbicgIiR7RktfU1RBVEU6LU9QRU59IiAiJChnaXQgc3ltYm9saWMtcmVmIC0tc2hvcnQgSEVBRCkiICIkKGNhdCAiJEhPTUUvcHVzaGVkIikiOzsKICBjaGVja3MpIFsgLW4gIiRGS19OT0NIRUNLUyIgXSAmJiB7IGVjaG8gIm5vIGNoZWNrcyByZXBvcnRlZCBvbiB0aGUgJ3gnIGJyYW5jaCIgPiYyOyBleGl0IDE7IH07IGVjaG8gJ1t7ImJ1Y2tldCI6InBhc3MifV0nOzsKICBlc2FjOzsKZXNhYwo= | base64 -d > "$HOME/bin/fk" && chmod +x "$HOME/bin/fk" && for n in claude b10x-gates gh; do ln -s fk "$HOME/bin/$n"; done && git remote add origin https://github.com/o/r.git && B10X_GATES_POLICY=policy.yaml FK_STEP='echo written-by-the-step > policy.yaml' PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --test-cmd ./check.sh --iterations 12 >/dev/null 2>&1; cat "$HOME/forge.log"
exit: 0
output: scan-read: written-by-the-step
source: .engineering/planning/review-result/findings-hunt-33ab42c.md
```

## Scope

`crates/takt/src/forge.rs` (`BotRoute::for_tree`, `scope`); possibly `crates/takt-cli/src/main.rs` (`forge_of`).

## Acceptance

In the predicate's run, the run exits 1 before the case opens and names the policy path as being under the tree. A `B10X_GATES_POLICY` that is absolute and outside the tree is passed on unchanged.
