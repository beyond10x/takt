# Why takt takes 2.4x the wall time and 1.3x the tokens of a free isolated run (scenario 1)

Read-only analysis, 2026-10-18. Records: the baseline runner's run directories T-14..18, T-24..28 and I-4..6.
Code cited at **a0b8576**, the revision T-14..18 ran (read from an export of that revision;
HEAD 7f28c08 differs: run.rs +2829 lines, so HEAD lines are given where they moved). Representative runs:
**T-16** (127 s, 107,205 tokens, 18 steps, 3 claude processes) and **I-4** (53 s, 93,003 tokens, 1 process, 8 turns).
Scratch scripts kept outside the repository. GitHub Actions timings read with `gh run list` (read-only).

## Verdict

| | T-16 | I-4 | gap |
|---|---|---|---|
| wall | 127 s | 53 s | +74 s |
| waiting for CI: fixed 30 s sleeps vs one `gh pr checks --watch` | 60 s (2 x 30) | 11.4 s | +48.6 s |
| forge steps: 4 pr.open + 5 ci.read (3 pushes, 6 views, 4 checks) vs push + PR POST | 15 s | 4.0 s | +11.0 s |
| model API time (`duration_api_ms`); takt's includes 6.9 s for a call that only requests the approval | 40.7 s | 33.1 s | +7.6 s |
| claude process start/stop, 3 processes vs 1 | 5.3 s | 0.5 s | +4.8 s |
| tests, gate, commit as validator steps vs in-turn `./check.sh` + `git commit` | ~3 s | 0.1 s | +2.9 s |
| trial fetch + push before the agent starts (both runners) | 3 s | ~4 s | -1 s |
| tokens | 107,205 | 93,003 | +14,202 (median gap 122,438 - 92,772 = 29,666; one approval call is 29.7k-30.1k) |

The whole wall gap is protocol sequencing (sleep, second CI cycle, approval call, serialised forge steps), not model
work: the two working model calls of takt (46.2k + 47.1k) cost as many tokens as the free run's entire run (93.0k).
All 10 takt runs have the same shape: 2 waits of 30 s, 3 calls of 7/4/1-3 turns, 4 pr.open, 5 ci.read, 2 CI passes
(`sweep.py` over events.jsonl and steps.jsonl).

## 1. Wall-time attribution, T-16 (runner start 18:44:36Z, end 18:46:43Z, 127 s)

Start of run = exit-note time (run.log:80, 18:46:43Z) minus `wall_seconds` 127 (result.json): +-1 s. Instants are
events.jsonl `at` (1 s granularity) and steps.jsonl `started_at`/`recorded_at` (ms, written by the shim `shim/claude`).

| # | phase | start | end | s | source |
|---|---|---|---|---|---|
| a | runner start -> case opened: `b10x-gates bot -- fetch` of trial/*, next_trial, branch, aep list/show, `b10x-gates bot -- push` trial/38 | 18:44:36 | 18:44:39 | 3 | main.rs a0b8576:886 fetch, :948 create; events.jsonl `opened` |
| b | decision 1; step 1 tests.run (Ran 5 tests in 0.008s); step 2 gate.run; step 3 pr.open (rev-list: no commit beyond base, no forge call); step 4 ci.read (no PR) | 18:44:39 | 18:44:40 | 1 | events.jsonl steps 1-4; validator.rs:770 |
| c | **claude process 1**, implementation.edit, 7 turns; `duration_ms` 16,542, api 16,028, `time_to_request` 470 ms, ttft 3,707 ms; 0.9 s outside claude's clock | 18:44:40.236 | 18:44:57.675 | 17.4 | steps.jsonl line 1 |
| d | takt commits d72df2cf; step 6 tests.run; step 7 gate.run | 18:44:57 | 18:44:57 | <1 | events.jsonl |
| e | step 8 pr.open: `b10x-gates bot -- push`, `scan-text`, `api POST` -> PR #37 | 18:44:57 | 18:45:02 | 5 | events.jsonl; validator.rs:782, :819 |
| f | step 9 ci.read: `gh pr view` + `gh pr checks` -> pending (Actions run created 18:45:03) | 18:45:02 | 18:45:03 | 1 | events.jsonl; forge.rs:909, :925, :1072 |
| g | **sleep 30 s** (CI finished 18:45:10: 23 s of this after CI was done) | 18:45:03 | 18:45:33 | 30 | events.jsonl `waiting`; run.rs:485, :1437; `gh run list` updatedAt |
| h | decision 11; step 10 ci.read -> pass | 18:45:33 | 18:45:35 | 2 | events.jsonl |
| i | **claude process 2**, tests.write, 4 turns; `duration_ms` 18,990, api 17,771, ttft 6,164 ms; 1.2 s outside | 18:45:35.895 | 18:45:56.074 | 20.2 | steps.jsonl line 2 |
| j | takt commits 1df6d41b; step 12 tests.run (Ran 7 tests in 0.148s); step 13 gate.run | 18:45:56 | 18:45:57 | 1 | events.jsonl |
| k | step 14 pr.open: push 1df6d41b, `gh pr view` -> PR head still d72df2cf, waits | 18:45:57 | 18:46:00 | 3 | events.jsonl; validator.rs:808 |
| l | step 15 ci.read: view + checks -> pending (run created 18:46:03) | 18:46:00 | 18:46:01 | 1 | events.jsonl |
| m | **sleep 30 s** (CI finished 18:46:10: 21 s of this after CI was done) | 18:46:01 | 18:46:31 | 30 | events.jsonl; `gh run list` |
| n | step 16 pr.open: pushes 1df6d41b **again** (branch already at it), view -> open | 18:46:31 | 18:46:33 | 2 | validator.rs:782 -> forge.rs:1042-1049 |
| o | step 17 ci.read -> pass | 18:46:33 | 18:46:34 | 1 | events.jsonl |
| p | **claude process 3**, repository.merge approval request, 1 turn, no file changed; `duration_ms` 7,212, api 6,864; 1.1 s outside | 18:46:34.920 | 18:46:43.258 | 8.3 | steps.jsonl line 3 |
| q | stop ApprovalRequired; runner note | 18:46:43 | 18:46:43 | <1 | run.log:79-80 |
| | **sum** | | | **126** | 1 s unattributed (second-granularity rounding of a, b, d, j) |

Model API time 40.7 s (c+i+p `duration_api_ms`); process overhead 2.1 s inside claude's clock + 3.2 s outside = 5.3 s;
sleeps 60 s; forge steps 15 s; tests/gate/commit ~3 s; pre-case 3 s.

## 2. Wall-time attribution, I-4 (runner start 17:45:38Z, end 17:46:31Z, 53 s)

Relative seconds from the stream's `system init` event (agent.stream.tsv arrival times). Pre-claude: 53 s wall minus
`duration_ms` 48,952 = ~4 s for the runner's own trial fetch + push (run-arm.sh:50-56), by subtraction.

| phase | start | end | s | note |
|---|---|---|---|---|
| init -> first tool call (`cat story && git ls-files && cat check.sh gates.yaml`) | 0.000 | 2.584 | 2.6 | ttft 2,641 ms; first API iteration cold: cache_read 0, cache_creation 7,448 |
| second read (`cat todo.py tests/test_todo.py CLAUDE.md AGENTS.md README.md`) | 2.644 | 4.820 | 2.2 | |
| model writes one python heredoc that edits todo.py + tests + README **and runs ./check.sh** | 4.840 | 13.078 | 8.2 | thinking + 1,928 cache_creation |
| tests run in-turn | 13.078 | 13.200 | 0.1 | 7 tests, output in the same tool result |
| model -> `git commit` + `b10x-gates bot -- push` in one call | 13.200 | 19.391 | 6.2 | tool part 17.378-19.391 = 2.0 s |
| model writes PR JSON; `scan-text` ok, `api POST` fails (output dir) | 19.391 | 24.855 | 5.5 | tool 0.1 s |
| model retries `api POST` -> PR #32 | 24.855 | 30.251 | 5.4 | tool 28.357-30.251 = 1.9 s |
| model -> `gh pr checks 32 --watch` | 30.251 | 43.896 | 13.6 | watch 32.481-43.896 = **11.4 s**; Actions run created 17:46:14, done 17:46:22 (8 s) |
| final summary (455 output tokens) | 43.896 | 48.888 | 5.0 | |
| **claude total** | | | **48.9** | `duration_api_ms` 33,061 + tools 15.7 s (CI watch 11.4 of it) |

I-5: 46.1 s, `sleep 10; gh pr checks --watch` 10.9 s; I-6: 54.5 s, watch 16.3 s (a failed POST retried first). Same shape.

## 3. Causes, ranked by seconds per run (T-16 unless stated)

| # | cause | mechanism | cost per run | status | story |
|---|---|---|---|---|---|
| 1 | Fixed 30 s sleep while CI is pending, taken twice | run.rs a0b8576:485 `POLL = 30 s`, :806 `pause()`, :1426-1437 `Waiting` + `thread::sleep` (HEAD 2549, 3740); validator.rs:970-1000 pending -> `waits`; forge.rs:1072 "no checks reported" -> Pending | 60 s slept; CI itself took 7 s each time, so 44 s idle after CI was done (23 + 21). Free run: 11.4 s watch | verified (events.jsonl, `gh run list`) | **ci-pending-waits** (implemented) added the wait; **no story** on the interval or on watching instead of sleeping |
| 2 | Pull request opened after implementation.edit alone, so push + head-lag wait + CI run again after tests.write | protocols/story-delivery/1.yaml:124-127 precondition only `tests.pass`, `gate.green`; run.rs:1206-1215 a due validator runs before any judgement step; validator.rs:808 head lag -> waits | steps 14-17: 18:45:57 -> 18:46:34 = 37 s (30 s of it is one of cause 1's sleeps; 7 s forge steps) + 1 extra Actions run | verified (events.jsonl evidence 0006 and 0010: two `ci_result pass`) | **pr-opens-when-change-is-done** (active) |
| 3 | A claude process spent only on asking for the merge approval | run.rs:1444-1456 `offered` non-empty -> judgement step; approvals passed when `options.gate` names them; harness.rs:130-137 prompt "These need an approval..." | T-16: 8.3 s, 13,884 tokens, 1 turn. 8 of 10 runs: 3 turns (it re-reads the files), 8.6-14.6 s, 29,712-30,125 tokens, $0.066-0.072 | verified (steps.jsonl line 3 of each run) | **no-model-step-for-approval** (active) |
| 4 | Forge steps serialised and repeated: 3 pushes (steps 8, 14, 16; step 16 re-pushes a revision the branch is already at), 6 `gh pr view`, 4 `gh pr checks`, 1 scan, 1 POST | validator.rs:782 `push_revision` -> forge.rs:1042-1049 `push` when branch == revision; CiRead reads view then checks (validator.rs:950-965) | 15 s (e 5, f 1, h 2, k 3, l 1, n 2, o 1). Free run: 4.0 s for push + POST | verified (events.jsonl, 1 s granularity) | cause 2's story removes 7 s; the re-push after head lag and the view+checks pair: **no story** (`fenced-forge-dispatch`, draft, is about outcome classes) |
| 5 | Three cold `claude -p` processes instead of one conversation | harness.rs:189 `--no-session-persistence`, one process per step (run.rs:1503 `self.harness.step`) | 3.2 s outside claude's clock + 1.2 s `time_to_request` = 4.4 s; tokens: process 2 spends 47,112 (4 turns) to add two tests, process 3 re-reads in 8/10 runs | timing verified; "re-reads" inferred (per-turn tool calls are not in steps.jsonl) | **no-inspect-for-stateless-harness** (active) covers the inspect -> NoProgress symptom only; the cost has **no story** |
| 6 | Larger prefix per process in the T arm | harness.rs:183-186 `--allowedTools ... --disallowedTools Bash,...` and no `--tools`; the I arm passes `--tools Bash Read Edit Write Glob Grep MultiEdit` (run-arm.sh:70) | 1-turn process 3: 13,362 input (9,424 cached + 3,938 created) vs I-arm first iteration 7,448 (I-4 cold; I-5 6,002 + 1,446): +5.9k per process start | numbers verified; cause (tool schemas in the system prompt) inferred; composition of the 3,938: I don't know | **no story** |
| 7 | Trial fetch + push before the case | main.rs:886, :948 | 3 s (3-5 s over 10 runs) | verified | not a cause of the gap: the I-arm runner does the same (~4 s) |
| 8 | tests.run, gate.run, commit, snapshot guards after each step | run.rs:1206-1215, :1503 and the checkpoint/snapshot code around it | <1 s per cycle, 3 cycles (tests take 0.148 s) | verified | negligible on this scenario |

By category (verdict table): sleeps vs watch +48.6, forge steps +11.0, model API +7.6 (6.9 of it the approval
call), process start/stop +4.8, validator tests/gate/commit +2.9, pre-case -1 = +73.9 s against the 74 s gap. Causes
1-5 overlap (cause 2 contains one of cause 1's sleeps and 7 s of cause 4; cause 3 is inside the API and process rows).

## 4. Token attribution

| call | action | turns | input | output | cache read | cache write | sum | $ |
|---|---|---|---|---|---|---|---|---|
| T-16 / 1 | implementation.edit | 7 | 6 | 1,522 | 37,746 | 6,935 | 46,209 | 0.093 |
| T-16 / 2 | tests.write | 4 | 6 | 1,632 | 38,423 | 7,051 | 47,112 | 0.097 |
| T-16 / 3 | repository.merge (approval) | 1 | 2 | 520 | 9,424 | 3,938 | 13,884 | 0.044 |
| T-16 total | | 12 | 14 | 3,674 | 85,593 | 17,924 | 107,205 | 0.234 |
| I-4 | whole run | 8 | 16 | 3,150 | 76,145 | 13,692 | 93,003 | 0.188 |
| T-14 / 3 (typical) | approval, 3 turns | 3 | | 678 | 22,803 | 6,604 | 30,089 | 0.071 |

Across the 10 runs call 1 is 46,114-46,382 in 9 (T-15: 62,463) with cache read 37,742-37,756: the same sequence
every time. Call 2 is 45,985-47,112 in 8 (T-25: 77,226, 6 turns; T-27 split by a resume). The median token gap
(29,666) equals one typical approval call (29,712-30,125).

**What the per-step prompt repeats** (harness.rs a0b8576:108-148; reconstructed from intent.md and decisions/
0006, 0013, 0020 by `prompt.py`): 1,708 chars / 281 words / 46 lines at steps 5 and 11, 1,828 / 306 at step 18.
Identical each call: the intent (578 chars, 106 words: the whole story text) and the fixed header, action
descriptions, rules and JSON instruction (1,130 chars, 175 words). Varying: `Step N`, five claim values, the open
obligations, the approval paragraph (119 chars). No file listing and no protocol text are in the prompt. Its token
count: I don't know (no tokenizer run); it lies inside the 3,938 `cache_creation` of the 1-turn call. The large
repeated part is Claude Code's own system prompt and tool schemas, 9,424 tokens read from cache at that call.

**Cache hits across takt's separate processes: yes, verified.** Process 3 is a single API iteration in a fresh
process and reads 9,424 from cache (steps.jsonl line 3 `usage.iterations[0]`). In the I arm, I-4's first iteration
read 0 (cold) and I-5's read 6,002 (`agent.stream.tsv` first assistant usage). For processes 1 and 2 only the last
iteration is in the record (`iterations` holds one entry), so their first-iteration hit is not visible; the sum
37,746 over 7 turns bounds process 1's first read at <= 5,392, less than the 9,424 of process 3: what the extra
cached prefix at process 3 is (the same run's processes 1-2 having written the dynamic system part), I don't know.

## 5. What the free run does that takt forbids or serialises

| free run (I-4) | takt | where |
|---|---|---|
| edits implementation, tests and README in one Bash heredoc and runs `./check.sh` in the same call (13.078 s, tests 0.12 s) | Bash disallowed; "Do not run commands"; one action per process: implementation.edit, then tests.write, each followed by tests.run + gate.run validators | harness.rs:140 prompt, :185-186 flags; 1.yaml:102-107; run.rs:1206 |
| commits and pushes in one call (2.0 s) | takt commits after the step; pr.open validator pushes: 3 pushes | run.rs:1769 `commit_paths` after the step; validator.rs:782 |
| opens the PR once, after the complete change | PR opened after implementation.edit alone (precondition tests.pass + gate.green), then head-lag wait, re-push, second CI | 1.yaml:124-127; validator.rs:808 |
| one `gh pr checks --watch` (10 s refresh) returns 11.4 s after start, CI done after 8 s | two reads of view + checks, each followed by a fixed 30 s sleep; CI done after 7 s each | run.rs:485, :1437 |
| decides it is done and stops (5.0 s, 455 output tokens) | a third process to request the approval (8.3-14.6 s, 13.9k-30.1k tokens) | run.rs:1444-1456 |
| one conversation: files read once, prefix cached within the process | 3 processes with `--no-session-persistence`, each re-reads and re-caches | harness.rs:189 |

## Not known

- The per-turn tool calls inside takt's processes (steps.jsonl holds only the final `--output-format json` result).
- Token count of the takt prompt and the composition of the 3,938 created tokens at a 1-turn process.
- Whether the T arm's larger prefix (+5.9k) comes from more tool schemas or from something else in the system prompt.
- Sub-second timings of validator steps (events.jsonl has 1 s granularity at a0b8576; T-24..28 at 98719c3 have microseconds).
