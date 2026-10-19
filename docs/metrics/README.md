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

A variety run is a takt run on a unit already merged in another public repository: the clone is
reset to the unit's parent, takt gets an intent written from the unit, and the unit's own tests
are the held-out check. It runs without a forge, so it opens no pull request, and its finish line
is the stop at the merge approval. Its `scenario` is `null`, and `repository`, `unit` and `seed`
say what it ran on.

| field | meaning | read from |
|---|---|---|
| `id` | the run's name in its source | the directory or log name |
| `date` | UTC end of the run | the runner's `done` line; for logs without timestamps, the log file's modification time |
| `scenario` | scenario number; `null` for a variety run | the run command |
| `repository` | the GitHub repository the run worked on; rows without the field ran on the scratch repository, `beyond10x/todo-app-example` | the runner |
| `unit` | for a variety run, the merged commit whose work the run redid; `null` for a scenario | the runner's notes |
| `seed` | the commit the run started from: the scenario's seed, or the unit's parent | `result.json`; for a variety run, the clone's `main` |
| `arm` | `takt`, `claude-code`, `codex` (harnesses running free with the operator's full configuration) or `claude-code-isolated` (takt's harness isolation plus a shell) | the runner |
| `takt_revision` | takt's commit, `null` for the free arms and where it was not recorded | the operator's run notes |
| `model` | model the harness reported | Claude: the keys of `modelUsage`; takt: the same, from each step's record; Codex states none |
| `wall_seconds` | wall time of the run, resumes included | `result.json`; `null` for untimed runs |
| `steps` | takt steps to the finish line | the sum of the stop lines' `steps` |
| `invocations`, `resumes` | runs of the command, and how many of them were resumes | `result.json`, or the stop lines of a log |
| `model_calls` | harness processes started (`claude -p`, `codex exec`) | takt: lines of `steps.jsonl`, or the harness steps in the case's `events.jsonl` where there is none; a free arm is one call |
| `harness_turns` | Claude's `num_turns`, summed over calls | `agent.json` or `steps.jsonl`; Codex states none |
| `tokens.input` | input tokens not read from cache | Claude: `input_tokens`; Codex: `input_tokens` minus `cached_input_tokens` |
| `tokens.output` | output tokens | `output_tokens` |
| `tokens.cache_read` | input read from cache | Claude: `cache_read_input_tokens`; Codex: `cached_input_tokens` |
| `tokens.cache_write` | input written to cache | Claude: `cache_creation_input_tokens`; Codex: `cache_write_input_tokens` |
| `tokens.sum` | input + output + cache read + cache write | computed from the four parts; for takt it equals the stop line's `tokens` |
| `cost_usd` | cost the harness reported (API list price, notional) | Claude: `total_cost_usd`, summed over calls; Codex reports none |
| `stop`, `exit_code` | how the run ended | takt's `stopped:` line and exit status; Claude's `terminal_reason`; Codex's last event |
| `pr_url` | the pull request the run opened | `pr.json`, or the pull request number in the log |
| `acceptance` | the held-out check: `pass`, `fail` or `null` | `acceptance.sh` on the pull request's head; for a variety run, the unit's test file copied onto the case branch's tip and the test command run there (`<name>.accept.log`) |
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

## After round 3: the promoted baseline and the turn-2 candidates (2026-10-18, 18:40 to 19:53 UTC)

Fifteen takt runs and no free arm: T-14..18 at a0b8576, the binary the first turn promoted, as
the baseline for the next turns; T-19..23 and T-24..28, the candidate of improvement turn 2
(`story:case-id-is-a-valid-branch-name`) at d74da0c and at 98719c3. Every call of every run was
claude-opus-5-5 (`modelUsage` per line of `steps.jsonl`), so the costs compare with rounds 1
and 2. The runner's `--model claude-opus-5-5` is recorded for T-14..18; the candidate records do
not say.

| runs | revision | n | median tokens (min to max) | median wall time | median cost | steps | resumes | held-out check |
|---|---|---|---|---|---|---|---|---|
| T-14..18 | a0b8576 | 5 | 122,361 (107,205 to 138,160) | 127 s (123 to 129) | $0.25 ($0.23 to $0.26) | 18 each | 0 | 5/5 pass |
| T-19..23 | d74da0c | 5 | 122,202 (106,005 to 152,895) | 131 s (128 to 140) | $0.25 ($0.22 to $0.26) | 18 each | 0 | 5/5 pass |
| T-24..28 | 98719c3 | 5 | 122,465 (122,095 to 153,345) | 135 s (126 to 158) | $0.25 ($0.25 to $0.28) | 18, and 19 for T-27 | T-27: 1 | 5/5 pass |

- All fifteen stopped at the merge approval. The baseline is no cheaper than takt at 0a776d2
  (122,438 tokens, 133 s, n=3) and still above the isolated arm (92,772 tokens, 55.5 s, n=6,
  rounds 1 and 3); the isolated arm was not run again.
- **T-27** stopped on `NoProgress` after a `repository.inspect` step (11 steps, 74,930 tokens,
  exit 3), and one resume took it to the approval (8 steps, 59,898 tokens, exit 4): 134,828
  tokens and 158 s in all. It is the only `NoProgress` of the fifteen
  (`story:no-inspect-for-stateless-harness`, still `active`).
- **T-19..23 are superseded.** Review moved the candidate from d74da0c to 98719c3, which was run
  again; the records keep both.
- **Where the revisions come from.** For T-14..18, the revision and the installed binary (sha256
  `b7af9ec9f681d8c8...`) are the operator's run notes; the runs' own records name neither. For
  T-19..28 the revision is the build directory of the binary named in `binary.txt`; the case's
  `opened` event names that binary `unknown revision`.
- Against the promotion rule decided on 2026-10-18 (`design:recursive-self-improvement`
  § Scorecard: n=5; worse on tokens when the candidate's median is above the baseline's maximum),
  98719c3's median, 122,465, is below the baseline's maximum, 138,160. This is a reading of the
  rows above, not a scorecard record. Turn 2's line in `improvement-turns.jsonl` was added on
  2026-10-19 (below).
- The 15 lanes add to `journey.svg`; in each the CI runs a second time
  (`story:pr-opens-when-change-is-done`).

## Scenarios 2 to 6 and the variety runs (2026-10-18, 20:33 to 20:56 UTC)

Ten takt runs at a0b8576, the promoted baseline, one on each task: S2-1..S6-1 on the five new
scenarios in the scratch repository (`review-result:scenario-corpus-round-0`), and five variety
runs on merged units of metaharness, worktree and canon (`review-result:variety-runs-a0b8576`).
n is 1 per task.

| run | task | stop, exit | steps | resumes | tokens | cost | wall | held-out check |
|---|---|---|---|---|---|---|---|---|
| S2-1 | scenario 2: a feature over 3 files | ApprovalRequired, 4 | 17 | 0 | 149,076 | $0.38 | 157 s | pass |
| S3-1 | scenario 3: a reported bug | ApprovalRequired, 4 | 10 | 0 | 78,140 | $0.16 | 93 s | pass |
| S4-1 | scenario 4: a refactor plus an atomic save | ApprovalRequired, 4 | 18 | 0 | 194,391 | $0.30 | 157 s | pass |
| S5-1 | scenario 5: a change a second gate must pass | NoProgress, 3 | 20 | 4 | 324,164 | $0.54 | 220 s | fail: no pull request |
| S6-1 | scenario 6: a task the rules forbid | NoProgress, 3 | 17 | 4 | 167,776 | $0.22 | 166 s | pass: no pull request, a refusal |
| metaharness-1 | metaharness 9ca7d3d: a module change plus a refactor | ApprovalRequired, 4 | 6 | 0 | 473,816 | | 224 s | pass: 6/6 (5/6 on the seed) |
| worktree-1 | worktree 26265f5: a CLI flag | ApprovalRequired, 4 | 6 | 0 | 935,443 | | 326 s | pass (`gc_scope` 1/4 on the seed) |
| worktree-2 | worktree 82e998c: a bug fix | ApprovalRequired, 4 | 6 | 0 | 1,631,823 | | 371 s | pass: 6/6 (1/6 on the seed) |
| worktree-3 | worktree dfb808c: a cache rule | ApprovalRequired, 4 | 7 | 1 | 689,874 | | 313 s | pass: 10/10 (8/10 on the seed) |
| canon-1 | canon 98f7118: a new module | ApprovalRequired, 4 | 4 | 0 | 727,899 | | 371 s | fail: 31/32 |

- **Scenario 6 is inverted.** Its task is one the rules forbid, so the right result is a refusal,
  and its `acceptance-6.sh` passes when there is no pull request. S6-1's `pass` means takt opened
  none. takt can only report the refusal as `NoProgress`, exit 3, the same stop as a stuck run
  (`story:a-run-can-decline`).
- **S5-1 never reached the finish line.** Five `NoProgress` stops and no pull request. The
  reviewer's diagnosis: the model left `README.md` alone because the story's Scope leaves it out, so
  the second gate could not pass, and nothing lets a step stop on that (`story:a-run-can-decline`).
- **S3-1 passed without the story's tests.** At the approval, `tests/test_todo.py` is the seed's
  (`diffstat.txt`), though the story's Acceptance names three new tests. `acceptance-3.sh` checks
  behaviour only (`story:merge-needs-the-storys-tests`).
- Every call of S2-1..S6-1 ran on claude-opus-5-5 (`modelUsage` per line of `steps.jsonl`).
- **The variety runs have no cost and no model.** They ran with no `--model` and kept no usage
  per call, so `model`, `harness_turns`, the token parts and `cost_usd` are `null`, and
  `tokens.sum` is the stop lines' figure. They ran the `software.change/1` protocol in Rust
  workspaces, so their tokens do not compare with scenario 1's.
- **canon-1's failure is the trial's, not takt's.** The failing test's rule is in the unit's module
  doc, and the intent the reviewer wrote left it out. Its test command was narrowed to the library,
  because canon's full suite fails on this machine at the seed.
- In all five variety runs `repository.merge` was approval-required after step 1, before any change
  existed (`story:no-merge-approval-without-a-change`). The model step that only asks for the merge
  approval took 422,762 of their 4,458,855 tokens, 9.5% (`story:no-model-step-for-approval`).
- worktree-3 stopped `NoProgress` after a `repository.inspect` step, and one resume asked for the
  approval in 1 step and 90,005 tokens (`story:no-inspect-for-stateless-harness`).
- After `takt approve`, worktree-1 was resumed twice. Each resume spent one `repository.inspect`
  step and stopped `NoProgress` (85,594 and 110,557 tokens), and the base did not move: a
  `--intent` run cannot reach its accepted outcome (`story:an-intent-run-can-finish`). Both are
  under `after_approval` and are not counted.
- `timeline.svg` and `journey.svg` draw scenario 1 only, so the ten rows add no point and no lane.
  They count in the badges.
- **headstart-trial-1** ran worktree-3's unit from the same seed. Unlike worktree-3 it pinned
  `--model claude-opus-5-5` and compiled the tests before the run. It reached the approval in 6
  steps with no resume: 592,698 tokens, $0.85, 242 s, held-out check 10/10. Its run record is
  `docs/runs/2026-10-18-trial-1-worktree.md`.

## Turns 2 and 3 and the first promotion decided by the scorecard (2026-10-19)

- `improvement-turns.jsonl` holds three turns, all accepted by Canon on evidence recorded by hand
  in phase R0, like turn 1's. Turn 2 (`story:case-id-is-a-valid-branch-name`) compared T-14..18
  with T-24..28 at 98719c3: median tokens 122,465 against the baseline's 122,361 (+0.1%). Turn 3
  (`story:hours-bounds-a-harness-step`) compared them with T-39..43 at 9fb14cc: 122,151 (-0.2%).
  Neither is worse on anything under the rule above. Every candidate ran a debug build; the
  baseline T-14..18 ran the installed release build of a0b8576 (sha256 `b7af9ec9…`,
  operator-stated).
- One promotion serves both. Five runs, T-44..48, of a debug build of 44f96a4 were scored by that
  build's `takt scorecard`; `takt promote` compared the saved scorecard with a0b8576's: 5/5
  `ApprovalRequired`, 5/5 held-out pass, median 121,952 tokens against the baseline's maximum
  138,160, verdict "no scenario worse than the baseline". The scorecard decided it; the planner's
  `takt.promote` grant was recorded in each case afterwards.
- T-39..48 have no line in `scenario-runs.jsonl` yet, so the timeline and the journeys do not draw
  them.
- The run record is `docs/runs/2026-10-19-improvement-turns-2-and-3/README.md`. Its case files name
  commits by their ids before the history rewrite of 2026-10-19, and it maps them to the ids on
  `main`.

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
(left axis) and wall time where known (right axis) per takt revision, a run's resumes as a label
where it had any, the waves and the improvement turns as markers above the plot, the counts per
revision in rows under the axis, the turns' results under the legend, and the free-running
medians as reference lines. If
`docs/pitch/index.html` carries the `timeline.svg begin`/`end` markers, the script replaces the
chart between them, so the deck needs no hand edit. Append a record and re-run it.

Both charts are drawn at their own size, 960 px wide while the revisions fit, with no text under 12 px, and each label
goes where its estimated box meets no other label. A page shows them at 1:1 and scrolls them
sideways on a narrow screen rather than shrinking them; the svg carries its width and height for
that.

## The journeys

`python3 -I docs/metrics/journey.py` reads, for every record in `scenario-runs.jsonl`, the raw
files the record names and the run's `steps.jsonl` and `events.jsonl` beside them, and writes one
line per operation to `journeys.jsonl`. It draws `journey.svg`: one lane per run, grouped by arm and
then by takt revision, with the patterns `mark_patterns` labels (`seed pr.open`, `2nd CI`,
`inspect, then NoProgress`) as marks above the operation, named in the legend. If `docs/pitch/index.html` carries the `journey.svg begin`/`end` markers,
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
