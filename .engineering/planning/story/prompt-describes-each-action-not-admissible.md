---
format: aep.planning-md/3
id: story:prompt-describes-each-action-not-admissible
kind: story
status: draft
title: Every step's prompt repeats the protocol's description of each action not admissible
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: prompt-describes-each-action-not-admissible
relations:
- serves: vision:safe-agentic-coding
revision: 2
---
# Every step's prompt repeats the protocol's description of each action not admissible

Since the "Not admissible now" list, each blocked write action's line ends with " The protocol: "
and the protocol's whole description of the action. On scenario 1 that is 618 bytes, for `pr.open`
and `repository.merge`, in every judgement prompt, five model calls a run: about 1,035 tokens a run
at the 0.335 tokens per prompt byte the trial of 0.23.0 measured (770 at 4 bytes a token). What a
step needs in order to decline is the action's id and what its precondition lacks, which the line
already says. The change: `blocked_block` in `crates/takt/src/harness.rs` stops appending the
description; each line stays `- <id>: <what its precondition lacks>.`, and the sentence on
declining stays.

Rejected on its re-trial (trial of 0.23.0, 2026-10-30). The candidate removes the text (no
re-trial transcript holds " The protocol: "), but both re-trial runs stopped `Tokens (49030)`
before `pr.open`, 53,809 and 53,545 tokens, against 54,415 and 52,065 at the release. Two cases
ranked below the baseline's `ApprovalRequired` with a passing held-out check decide "worse" at 5
trials and a margin of 1, so the set stopped after run 2. The saving does not show in the totals: change.handover fell from 10.3k to 10.1k a run, the other steps are within run-to-run spread. The
release's runs ended 5,385 and 3,035 tokens over the promoted baseline's saved maximum (49,030), more
than this change can remove. Keep the finding open: it is still red at `f6fb9c85`, and it belongs
in a wave with the other prompt-size findings of this trial, or after a re-baseline.

The comparison as `takt scorecard --against` printed it:

```text
against f4f8d8d21fb4552799e0a17f394816259a133173 (the revision of takt under test)
rule  trials 5 a scenario  margin 1 case  tokens worse when the median exceeds the saved maximum
scenario-1-trial-361  scenario 1  failed
scenario-1-trial-362  scenario 1  failed
scenario 1  too few cases (2 of 5)
takt: too few cases for a verdict: scenario 1 (2 of 5); the rule needs 5 cases a scenario on each side, so run the short side again before comparing
```

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/fake" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"decline\", \"summary\": \"prompt-describes-each-action-not-admissible\"}"}' > "$HOME/fake/described.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"decline\", \"summary\": \"the actions not admissible are named with what they lack\"}"}' > "$HOME/fake/named.json" && printf '%s\n' '#!/bin/sh' 'f="$HOME/fake"' 'if [ "$1" = --version ]; then echo "2.1.309 (Claude Code)"; exit 0; fi' 'case "$*" in *"Not admissible now"*"open its pull request through the"*|*"Not admissible now"*"never only a connection to the repository host"*) cat "$f/described.json" ;; *) cat "$f/named.json" ;; esac' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --protocol story.delivery/1 --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 3
exit: 9
healthy_exit: 9
output: Declined (prompt-describes-each-action-not-admissible)
source: docs/runs/2026-10-30-trial-0.23.0.md
```
