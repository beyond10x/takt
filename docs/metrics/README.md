# Metrics over time

The figures takt is judged by, kept as a committed series so a later run can be compared with an
earlier one. Each file is JSON Lines: one object per line, appended to, never rewritten.

| file | one line per |
|---|---|
| `scenario-runs.jsonl` | scenario run of any arm |
| `improvement-turns.jsonl` | improvement turn: a finding checked before and after a fix |
| `waves.jsonl` | wave merged into `main` |
| `timeline.py` | (generator) draws `timeline.svg` from the three files above |

## The rule

Token and cost figures come only from raw usage: the harness's own usage record (Claude Code's
`--output-format json` result, Codex's `turn.completed` event) or takt's stop line, which sums the
same records. No figure is estimated, rounded into a record or carried over from a slide. A figure
the source does not state is `null`, never 0. A 0 in a record is a 0 the source stated.

## `scenario-runs.jsonl`

A scenario run is one attempt at a scenario, up to its finish line: a pull request open against
its own trial branch with CI green, which for takt is the stop at the merge approval. Invocations
after the approval are listed under `after_approval` and are not counted in the totals.

| field | meaning | read from |
|---|---|---|
| `id` | the run's name in its source | the directory or log name |
| `date` | UTC end of the run | the runner's `done` line; for logs without timestamps, the log file's modification time |
| `scenario` | scenario number | the run command |
| `arm` | `takt`, `claude-code`, `codex` (harnesses running free with the operator's full configuration) or `claude-code-isolated` (takt's harness isolation plus a shell) | the runner |
| `takt_revision` | takt's commit, `null` for the free arms and where it was not recorded | the operator's run notes |
| `model` | model the harness reported | Claude: the keys of `modelUsage`; takt: the same, from each step's record; Codex states none |
| `wall_seconds` | wall time of the run, resumes included | `result.json`; `null` for untimed runs |
| `steps` | takt steps to the finish line | the sum of the stop lines' `steps` |
| `invocations`, `resumes` | runs of the command, and how many of them were resumes | `result.json`, or the stop lines of a log |
| `model_calls` | harness processes started (`claude -p`, `codex exec`) | takt: lines of `steps.jsonl`; a free arm is one call |
| `harness_turns` | Claude's `num_turns`, summed over calls | `agent.json` or `steps.jsonl`; Codex states none |
| `tokens.input` | input tokens not read from cache | Claude: `input_tokens`; Codex: `input_tokens` minus `cached_input_tokens` |
| `tokens.output` | output tokens | `output_tokens` |
| `tokens.cache_read` | input read from cache | Claude: `cache_read_input_tokens`; Codex: `cached_input_tokens` |
| `tokens.cache_write` | input written to cache | Claude: `cache_creation_input_tokens`; Codex: `cache_write_input_tokens` |
| `tokens.sum` | input + output + cache read + cache write | computed from the four parts; for takt it equals the stop line's `tokens` |
| `cost_usd` | cost the harness reported (API list price, notional) | Claude: `total_cost_usd`, summed over calls; Codex reports none |
| `stop`, `exit_code` | how the run ended | takt's `stopped:` line and exit status; Claude's `terminal_reason`; Codex's last event |
| `pr_url` | the pull request the run opened | `pr.json`, or the pull request number in the log |
| `acceptance` | the held-out check: `pass`, `fail` or `null` | `acceptance.sh` on the pull request's head |
| `invocation_stops`, `after_approval` | per invocation: stop, steps, tokens, exit code | the stop lines of a log |
| `notes` | what a reader needs to read the record right | |
| `source` | the files the record was read from, `~` for the home directory | |

Token sums include cache reads, which are billed at about 1/20 of input; compare cost beside
tokens, not tokens alone.

## Reading the first runs (2026-10-18)

- One scenario: a 31-line change to a toy CLI, 3 runs per arm. All arms passed 3/3, so the
  held-out check does not tell them apart.
- takt at 0a776d2 against `claude-code-isolated` (Claude Code with takt's harness isolation,
  plus a shell and the file tools): about 1.7x the cost ($0.25 against $0.14), 1.3x the tokens
  (122,438 against 92,541) and 2.1x the wall time (133 s against 64 s). H2, "cheaper per
  accepted unit", is not supported on this scenario.
- takt against Claude Code with the operator's full configuration: 4.7x fewer tokens, 1.9x lower
  cost. Nearly all of that gap is the isolation, not the protocol. takt's own harness has the
  file tools only.
- Two known overheads are filed: a model step spent only on asking for the merge approval
  (`story:no-model-step-for-approval`, about 30k tokens per run), and CI running twice because
  the pull request opens before the tests are written.
- `I-modefail-1..3` are three attempts of the isolated arm that claude refused at start
  ("bypassPermissions not supported in restricted mode"), a runner configuration error. They are
  recorded with null figures and do not count towards the arm's medians.
- Codex is another vendor; its model (gpt-6-astra) is read from its configuration, not from the
  run, and it reports no cost.
- takt at 0a776d2 needed an https-origin workaround for a bug in its trial fetch; without it, 0/3
  runs got past the fetch (`T-fetchfail-1..3`).
- The earlier takt trials state only the stop, the steps and the tokens; they are not timed.
- n is small: first results, not a benchmark.

## `improvement-turns.jsonl`

| field | meaning |
|---|---|
| `finding` | the story that names the defect |
| `predicate` | the command `scripts/check-finding` ran on the scenario |
| `before`, `after` | revision, `reproduced` or `not_reproduced`, exit code, date and source of each check |
| `merged` | branch, commit and date the fix was merged at |
| `promoted_at` | the release the fix was promoted in; `null` until a scorecard promotes one |

## `waves.jsonl`

One line per wave: its title and a short label from `docs/waves/wave-NN.md`, the stories it
delivered, the merge commit into `main` and that commit's time.

## The timeline

`python3 -I docs/metrics/timeline.py` writes `timeline.svg`: for each scenario-1 takt run, tokens
(left axis) and wall time where known (right axis) per takt revision, resumes as a label, the
waves and the improvement turns as markers, and the free-running medians as reference lines. If
`docs/pitch/index.html` carries the `timeline.svg begin`/`end` markers, the script replaces the
chart between them, so the deck needs no hand edit. Append a record and re-run it.
