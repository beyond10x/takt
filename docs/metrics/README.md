# Metrics over time

The figures takt is judged by, kept as a committed series so a later run can be compared with an
earlier one. The data files are JSON Lines, one object per line. The three record files are
appended to and never rewritten; `journeys.jsonl` is generated from them.

| file | one line per |
|---|---|
| `scenario-runs.jsonl` | scenario run of any arm |
| `improvement-turns.jsonl` | improvement turn: a finding checked before and after a fix |
| `waves.jsonl` | wave merged into `main` |
| `timeline.py` | (generator) draws `timeline.svg` from the three files above |
| `journeys.jsonl` | operation of a run, in one vocabulary (generated, rewritten by `journey.py`) |
| `journey.py` | (generator) writes `journeys.jsonl` and draws `journey.svg` from the raw records |
| `badges.py` | (generator) rewrites the badge block at the top of `README.md` from the planning store, these files and the citation check |

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
  the pull request opens before the tests are written (`story:pr-opens-when-change-is-done`).
- `I-modefail-1..3` are three attempts of the isolated arm that claude refused at start
  ("bypassPermissions not supported in restricted mode"), a runner configuration error. They are
  recorded with null figures and do not count towards the arm's medians.
- Codex is another vendor; its model (gpt-6-astra) is read from its configuration, not from the
  run, and it reports no cost.
- takt at 0a776d2 needed an https-origin workaround for a bug in its trial fetch; without it, 0/3
  runs got past the fetch (`T-fetchfail-1..3`; `story:trial-fetch-needs-no-user-ssh`).
- The earlier takt trials state only the stop, the steps and the tokens; they are not timed.
- n is small: first results, not a benchmark.

## The later runs and the first turn (2026-10-18)

- takt at 97623a6 (wave 09) and at 447423c (the turn-1 candidate), 3 runs each: median tokens
  142,454 and 151,626, median cost $0.31 and $0.32, median wall time 132 s and 159 s. Neither is
  cheaper than takt at 0a776d2 or than the isolated arm. All six passed the held-out check.
- Three of those six runs (T-5, T-7, T-9) needed a resume after a `repository.inspect` step ended
  in `NoProgress` (`story:no-inspect-for-stateless-harness`).
- `improvement-turns.jsonl` holds one turn, accepted by Canon. Its scorecard compares T-1..3 with
  T-7..9: no worse outcome, median tokens +23.8%. The run record,
  `docs/runs/2026-10-18-improvement-turn-1/README.md`, lists six deviations from the protocol as
  designed, the first being that the records were written by hand (phase R0).

## Round 3 (2026-10-18, 17:29 to 17:49 UTC)

Three more runs of every arm: T-11..13, I-4..6, C-4..6, X-4..6. takt ran at 56d1b70, main at
the time its binary was built; its crates are the same as at 97623a6.

- **The models changed, and nobody chose it.** No arm pinned a model. takt passed no `--model`:
  T-11 and T-13 ran every call on claude-fable-5-1, T-12 its first call on fable and the next two
  on claude-opus-5-5 (`modelUsage` per line of `steps.jsonl`). Claude Code with the full
  configuration ran on fable, the isolated arm on opus (`agent.json`). The cause is unknown;
  account rotation is an inference. takt's round 3 cost ($0.47 to $0.85) is therefore not
  comparable with rounds 1 and 2. From round 4 the runner pins `--model claude-opus-5-5` for every
  Claude arm.
- takt at 56d1b70: median 138,580 tokens and 142 s, no resume, 3/3 held-out pass. The isolated
  arm in the same round: 93,003 tokens and 53 s. takt took 1.5x the tokens and 2.7x the time, the
  same direction as round 1.
- Pooled over rounds 1 and 3 (n=6): the isolated arm 92,772 tokens, 55.5 s, $0.14; Codex 599,555
  tokens, 86 s. Claude Code with the full configuration is not pooled, because its model changed:
  round 3 took 112 s and $1.38 against 65 s and $0.48 in round 1. Why is not checked; the model
  and the operator's plugin set both changed between the rounds.
- takt still runs CI twice in all three runs. None of the twelve runs stopped on `NoProgress`.
- T-10 is a runner race: a free-arm run had just pushed `trial/24`, so takt refused before its
  first step. Recorded with null figures; it does not count towards a median.
- Every arm streamed its events with arrival times (`agent.stream.tsv`, the shim's `started_at`),
  so all thirteen new lanes in `journey.svg` are timed.

## `improvement-turns.jsonl`

| field | meaning |
|---|---|
| `id`, `scenario` | the turn's name and the scenario it was checked on |
| `finding` | the story that names the defect |
| `predicate` | the command `scripts/check-finding` ran on the scenario |
| `before`, `after` | revision, `reproduced` or `not_reproduced`, exit code, date and source of each check |
| `merged`, `reached_main` | branch, commit and date the fix was merged at, and the merge into `main` that carried it |
| `scorecard_result`, `scorecard` | the comparison of the candidate with the baseline: the runs on each side, their median tokens, the change in per cent, notes and sources |
| `outcome` | Canon's outcome for the turn's case, such as `accepted` |
| `promoted_at` | the revision the fix was promoted at; `null` until one is promoted |
| `source`, `notes` | the run record and case the line was read from, and what a reader needs to read it right |

## `waves.jsonl`

One line per wave: its title and a short label from `docs/waves/wave-NN.md`, the stories it
delivered, the merge commit into `main` and that commit's time. `closing_store_commit` is the
planning-store commit that closed the wave, where one is named. `measured_at` is the first
measured revision that contains the wave, when the merge commit itself was not measured; the
chart marks the wave before that revision's column. A wave no measured revision contains yet gets
an empty column at its merge commit, and a measured revision that is an ancestor of a wave's merge
(`git merge-base --is-ancestor`) is drawn left of that wave's marker.

## The timeline

`python3 -I docs/metrics/timeline.py` writes `timeline.svg`: for each scenario-1 takt run, tokens
(left axis) and wall time where known (right axis) per takt revision, resumes as a label, the
waves and the improvement turns as markers, and the free-running medians as reference lines. If
`docs/pitch/index.html` carries the `timeline.svg begin`/`end` markers, the script replaces the
chart between them, so the deck needs no hand edit. Append a record and re-run it.

## The journeys

`python3 -I docs/metrics/journey.py` reads, for every record in `scenario-runs.jsonl`, the raw
files the record names and the run's `steps.jsonl` and `events.jsonl` beside them, and writes one
line per operation to `journeys.jsonl`. It draws `journey.svg`: one lane per run, grouped by arm and
then by takt revision. If `docs/pitch/index.html` carries the `journey.svg begin`/`end` markers,
the chart between them is replaced. A new record adds a lane with no hand edit.

**No token or cost figure comes from `journeys.jsonl`.** It holds order, kind and time only; token
and cost figures stay in `scenario-runs.jsonl`, read from raw usage.

| field | meaning |
|---|---|
| `run`, `arm`, `revision` | the record in `scenario-runs.jsonl` the operation belongs to |
| `seq` | position in the run, from 1 |
| `t_start`, `t_end` | seconds from `t0`; `null` where the source states no time |
| `t0` | the run's start: the runner's last exit note minus `wall_seconds` (to the second); `null` for lanes with no times |
| `kind` | one of the vocabulary below |
| `by` | `harness` (a model call, or a command the harness ran) or `deterministic` (a takt validator) |
| `action`, `effect`, `tool_class`, `label` | takt's action name; the effect class; the class of a free arm's command or tool; a pattern label |
| `source` | the raw file the operation was read from, `~` for the home directory |

| kind | takt (run log or `events.jsonl`) | Claude Code (free or isolated) | Codex running free |
|---|---|---|---|
| `model_call` | a harness step whose only effect is the approval request | `agent.json`: each of `num_turns`; stream: one `assistant` message | `agent.jsonl`: an `agent_message` item; stream: also the wait before each command or file change |
| `read` | `repository.inspect` by the harness | `Read`, `Glob`, `Grep`, or a `Bash` command that reads | a command that reads (`cat`, `rg`, `git status`, ...) |
| `edit` | `implementation.edit`, `tests.write` | `Edit`, `Write`, `MultiEdit`, or an editing `Bash` command | a `file_change` item, or an editing command |
| `test` | `tests.run` | `./check.sh` or a test runner through `Bash` | `./check.sh` or a test runner |
| `gate` | `gate.run` | | |
| `push` | `pr.open` that pushed to an open pull request | `git push` or a push through `b10x-gates bot` | the same |
| `pr_open` | `pr.open`: opened a pull request, or found nothing to open (hollow) | the pull-request `POST` through `b10x-gates api`, or `gh pr create` | the same |
| `ci_wait` | `ci.read` that saw pending checks | `gh pr checks --watch`, `gh run watch` | the same |
| `ci_pass` | `ci.read` that saw the checks pass | | |
| `approval_wait` | the stop `ApprovalRequired` | | |
| `refusal` | a refusal before or at a step (`takt: ...`, an error stop) | a refusal at start | |
| `no_progress` | the stop `NoProgress` | | |
| `resume` | the next invocation of the same case | | |
| `other` | `ci.read` with nothing to read, a merge, a grant, `Completed` | any other tool or command; the final `result` | any other command (`gh`, `git`, `b10x-gates`, tools); `turn.completed` |

A shell command takes the first class that matches, in this order: `pr_open`, `push`, `ci_wait`,
`test`, `other` for `gh`, `b10x-gates` and a `git` command that changes the repository, then
`edit`, then `read`. A compound command that edits a file and then runs `./check.sh` counts as
`test`. The command text is classified and not copied, since it carries local paths.

In takt's lanes every harness step is one model call; it is coloured by what it did and outlined.
Effect classes: `pass`, `fail`, `pending`, `opened`, `pushed`, `merged`, `approval`, `changed`,
`unchanged`, `nothing`, `refused`, `NoProgress`, `completed`, `granted`. An operation whose effect
is `nothing` or `unchanged` is drawn hollow. Labels mark the seed-time `pr.open` (before any edit),
the second CI pass, and an inspect step followed by `NoProgress`.

Where the times come from:

- takt with `events.jsonl` (97623a6 and later): every step's time at 1 s resolution; a step runs
  from the previous event to its own. Where the shim's `steps.jsonl` carries `started_at` and has
  one line per harness step, each harness step runs from its `started_at` to its `recorded_at`
  (millisecond resolution) instead.
- takt without `events.jsonl`: each harness step's model call from `steps.jsonl` (`started_at` to
  `recorded_at`, or `recorded_at` minus `duration_ms` where `started_at` is missing); stops from the
  runner's exit notes. The validator steps have no time; they are drawn dashed, spread in order
  between the timed operations either side ("model calls timed").
- The free and isolated arms with `agent.stream.tsv` (from 2026-10-18 17:29 UTC on): the runner
  stamps each line of the harness's JSON output with its arrival time in epoch milliseconds, and
  `t0` is the runner's `agent exit` note minus `wall_seconds`. Claude Code: a model call runs from
  the previous event to the last line of its `assistant` message, a tool from its `tool_use` to its
  `tool_result`; events inside a sub-agent fall inside the `Task` tool's span and are not drawn on
  their own. Codex: a command or file change runs from `item.started` to `item.completed`.
- Claude Code without the stream: `agent.json` holds `num_turns` and `duration_ms` only, no
  per-turn record, so its lane is "order only".
- Codex without the stream: `agent.jsonl` items in order without times ("order only").
- The earlier takt trials: run logs with no times ("order only").
- Refusals at start (`T-fetchfail-*`, `I-modefail-*`, and a takt run whose trial branch was already
  taken): one operation spanning `wall_seconds`.
