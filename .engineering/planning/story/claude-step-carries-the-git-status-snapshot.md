---
format: aep.planning-md/3
id: story:claude-step-carries-the-git-status-snapshot
kind: story
status: draft
title: Each Claude step carries Claude Code's git status snapshot, which the step cannot use
tags:
- finding
- rsi
refs:
- provider: finding
  reference: claude-step-carries-the-git-status-snapshot
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Each Claude step carries Claude Code's git status snapshot, which the step cannot use

## What is wrong

takt starts the `claude` child of a judgement step with `--settings {"disableAllHooks":true}`
(`ClaudeCode::ISOLATION`, `crates/takt/src/harness.rs:187-192`), and it passes nothing that turns
off Claude Code's git status snapshot. Claude Code 2.1.304 builds that snapshot whenever its
git-instructions preference is on. The preference is the `includeGitInstructions` setting
(default true), and the `CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS` environment variable overrides it.
takt removes every `CLAUDE_CODE_*` variable from the child (`harness.rs:746-750`). The snapshot
holds a fixed paragraph, the current branch, "Main branch (you will usually use this for PRs)",
`git config user.name`, `git status` and `git log --oneline -n 5`. The template and the gate are
read from the 2.1.304 bundle (inferred: not run). `--restricted`'s help text says `--settings`
still applies. The step gets no git tool, its prompt says "Do not commit", and takt itself states
the claims, so the step cannot act on any of the snapshot. Claude Code still sends it in every API
request of the step.

The record is takt 0.4.0 (8eb9b3a5) on scenario 1: 5 runs, each with 13 steps and 8 API requests
(3, 3 and 2 in steps 3, 6 and 9). It comes from the stream-json transcripts, each request's `usage`.

| figure | step 3 | step 6 | step 9 |
|---|---|---|---|
| prompt takt sent, bytes | 1,934 | 1,969 | 1,934 |
| first request's cache write, tokens, 5 runs | 1,461–1,463 | 1,528–1,546 | 1,570–1,592 |
| commits on the branch at the step | seed, take | + `tests.write` | + `implementation.edit` |

- Steps 3 and 9 send prompts of the same length, yet step 9's first request writes 109–130 tokens
  more (per run: 127, 109, 127, 116, 130). Between those two steps takt commits two steps, and
  each commit's subject is the model's own summary. The log lines added are 296–363 bytes, at
  0.35–0.39 tokens a byte in every run.
- Step 9 minus step 6 follows the length of the `implementation.edit` log line across runs. A
  least-squares fit over 5 points (157–184 bytes, 42–53 tokens) gives 0.374 tokens a byte.
- At step 3 every run has the same log, and the first request's write varies by 2 tokens across
  runs. At steps 6 and 9 it varies by 18 and 22. So what one step writes as its summary reaches
  the next step's context through a channel outside takt's prompt.
- The snapshot also puts the tree's configured git user name into every step's context.

Tokens, median run (66,949; cache read 50,424, cache write 13,343, output 3,166, input 16). A
snapshot of `n` tokens is written once in a step's first request and read in each request after
it, so it costs `n` × the step's requests:

- measured growth alone: 3 × ~68 (step 6, net of its prompt's 35 more bytes) + 2 × 127 (step 9)
  ≈ 458;
- the snapshot at step 3 cannot be separated in the record. Rebuilt from the bundle's template
  and the tree, it is 417 bytes, about 104–156 tokens at 0.25–0.374 a byte, × 8 requests ≈
  830–1,250 (inferred);
- total ≈ 1,290–1,710 a run (1.9–2.6 %) from the bundle's template; the probe below measured the
  snapshot at 44 to 125 tokens a request, which puts the total at about 810–1,460 a run. The
  median would be about 65,490–66,140, under the promoted baseline's median of 66,421 (inferred).

## Change

`ClaudeCode::ISOLATION`'s settings document becomes
`{"disableAllHooks":true,"includeGitInstructions":false}`, and its doc comment says why: the step
has no git tool and gets its state from takt's claims.
`the_harness_child_loads_no_project_or_user_settings` asserts the new value. The changed line is
cited from `ess/domains/takt.yaml:258` by its needle (`r#"{"disableAllHooks":true}"#,`) with a
digest (`ess/domains/takt.citations.tsv:50`): the needle moves to the new text, and the digest
waits for a person's `scripts/check-citations accept`. No other Claude Code
behaviour changes for a step offered `Read,Edit,Write,Glob,Grep`. The same preference also gates
the Bash tool's commit and pull-request text (Bash is not offered) and a remote-session hint
(inferred from the bundle).

Probe, after the record: one `claude -p` with takt 0.4.0's argv (`--tools`, `--restricted
--strict-mcp-config --settings`, `--model claude-opus-5-5`) in a clone of the seed, the
environment cleared of credentials, in the order without, with, without, with. The request's
input-side tokens (input, cache read and cache write) were 5,515 and 5,596 without the setting
and 5,471 and 5,471 with it: 44 and 125 fewer a request. The two calls without it differ by 81,
because the snapshot itself changed between them (the first ran in a fresh clone). At 44 to 125
a request and 8 requests a run, the snapshot alone is about 350 to 1,000 tokens a run.

## What shows it worked

- The probe above: 44 to 125 fewer input-side tokens a request with the setting.
- In a re-trial, each step's first request, by its whole prompt size (input, cache read and
  cache write together, not the cache write alone, which moves with what earlier steps left in
  the cache), is about the same at steps 3 and 9, where today step 9's is 107–129 larger
  (6,279–6,281 at step 3, 6,388–6,410 at step 9). The scenario's median tokens fall by about 800
  to 1,500. Outcome stays 5 of 5 `ApprovalRequired` and steps stay 13.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" && printf '%s\n' '#!/usr/bin/env python3' 'import json, os, pathlib, sys' 'a = sys.argv[1:]' 'v = next((x.split("=", 1)[1] for x in a if x.startswith("--settings=")), None)' 'if v is None and "--settings" in a[:-1]: v = a[a.index("--settings") + 1]' 's = {}' 'if v is not None:' '    try: s = json.loads(v)' '    except ValueError:' '        try: s = json.loads(pathlib.Path(v).read_text())' '        except (OSError, ValueError): s = {}' 'e = os.environ.get("CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS")' 'off = e.strip().lower() not in ("", "0", "false", "no") if e is not None else isinstance(s, dict) and s.get("includeGitInstructions") is False' 'if not off: sys.exit("claude-step-carries-the-git-status-snapshot")' 'r = {"action": "repository.merge", "summary": "the step carries no git status snapshot"}' 'print(json.dumps({"type": "result", "subtype": "success", "result": json.dumps(r)}))' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 6
exit: 5
healthy_exit: 4
output: claude exited 1: claude-step-carries-the-git-status-snapshot
source: crates/takt/src/harness.rs
```

The fake `claude` applies Claude Code's own rule: the variable, if set, decides; otherwise the
`--settings` document's `includeGitInstructions` decides, whether that document is inline JSON or
a file. Any spelling of the fix that turns the snapshot off answers `repository.merge`, and the
run stops at `ApprovalRequired`, exit 4. While the snapshot is on, the fake exits 1 and takt stops
with exit 5.

## Trial result: candidate rejected after re-trial 1

Candidate branch `trial-cand/2026-10-20-8eb9b3a5-1`, cut from 8eb9b3a5 (0.4.0): `2d28be88`, the
change above. `cargo test -p b10x-takt`: 983 passed, 0 failed; the predicate against its build,
outside the confinement: exit 4, its healthy exit. Not checked by `scripts/check-finding` at the
candidate.

The re-trial stopped after 3 of 5 runs, under the trial's stop rules (two runs with a worse
outcome decide "worse"; each run capped at 76,449 tokens, the trigger set's maximum):

| run | outcome | tokens | step 6 |
|---|---|---|---|
| 1 | stopped at the token cap before `pr.open` | 78,730 | `Glob **/*`, 4 reads (two of them the case's evidence files), 4 edits: 39,247 |
| 2 | `ApprovalRequired`, held-out check passed | 65,155 | 1 read, 4 edits: 25,795 |
| 3 | stopped at the token cap before `pr.open` | 90,506 | `Glob **/*`, 4 reads (two of them the case's evidence files), 4 edits: 51,252 |

`takt scorecard --against` gives no verdict on 3 cases (`too few cases (3 of 5)`, exit 1); the
stop rule decided "worse".

Reason: the premise does not hold. The step does use the snapshot. At 8eb9b3a5 no step 6 of 5
runs ran a `Glob` or read under `.engineering/`; with the snapshot off, 2 of 3 did. Inferred from
that pattern: the snapshot's `git status` and commit subjects tell the `implementation.edit` step
what the case has changed, and without them it looks for it. So the change does not stand alone.
It belongs together with story:step-prompt-omits-the-change-so-far, which puts the change in the
prompt itself; the snapshot can go once the prompt carries what it gave.

The trial's figures and rollouts are in the trial agent's cycle record, outside this repository.
