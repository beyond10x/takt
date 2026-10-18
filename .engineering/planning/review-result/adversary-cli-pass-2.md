---
format: aep.planning-md/3
id: review-result:adversary-cli-pass-2
kind: review-result
status: active
title: Adversary, cli-protocol-defaults, pass 2
relations:
- reviews: story:cli-protocol-defaults
revision: 1
---
unit: story:cli-protocol-defaults, correction 1 at 4358c28 (base ae94ccb), worktree w6-cli-protocol-defaults plus one untracked test file I added
verdict: CONFIRMED
cases: executed 177→179, red 2
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 3 paths (part 6)
needs-coordinator: none

**1. Diff stat**

`git --no-pager diff --stat` prints nothing: my only change is a new untracked file, and it is a test path. I changed no implementation file and committed nothing.
```
?? crates/takt-cli/tests/protocol_defaults_adv2.rs
```

**2. Cases added** (`crates/takt-cli/tests/protocol_defaults_adv2.rs`). Both are red now.

| case | asserts | red because |
|---|---|---|
| `adv2_a_store_that_changes_between_the_check_and_the_resume_lets_another_protocol_through` | Setup: an active story's case on `software.change/1`. A stub `aep` answers the first `list` as the store looked before the story was taken, then passes every later call to the real `aep`. A plain run with `--protocol story.delivery/1` should exit 1 with no `frontier: resumed`. | It exits 3: `open_next` resumes the case on `software.change/1`, and the mismatched `--protocol` goes through without a word |
| `adv2_run_help_says_an_input_for_a_resumed_case_is_refused` | The `--input` paragraph of `takt run --help` mentions a resumed case | That paragraph names only the undeclared-input refusal |

Red output of these cases run alone (`cargo test -p b10x-takt-cli --locked --test protocol_defaults_adv2`, EXIT=101):
```
panicked at crates/takt-cli/tests/protocol_defaults_adv2.rs:185:5:
the --input help says nothing of a resumed case: --input <NAME=VALUE>       A case input, `name=value`; repeatable. One the protocol a case opens on does not declare is refused before the case is opened
panicked at crates/takt-cli/tests/protocol_defaults_adv2.rs:162:5:
... stdout frontier: resumed story:ready-one: Make the check pass (case story-ready-one-20261018T101600Z)
case story-ready-one-20261018T101600Z on software.change/1 in ...
stopped: Iterations (0); steps 0, tokens 0, case ...
  left: (3, true)
 right: (1, false)
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

**3. Suite run, after the cases existed:** `cargo test -p b10x-takt -p b10x-takt-cli --locked --no-fail-fast`, EXIT=101.
- The library targets passed 127 cases (16, 35, 15, 16, 2, 30, 13).
- The CLI targets other than mine passed 50: main 17, forge_cli 3, frontier_cli 6, gate_cli 8, merge_cli 2, protocol_defaults_adv1 3, protocol_defaults_cli 11.
- `protocol_defaults_adv2`: `test result: FAILED. 0 passed; 2 failed`.
- 179 cases ran. The "before" figure of 177 is the same run with my 2 cases taken out.
- `cargo fmt --check` exits 0, and clippy on `b10x-takt-cli --all-targets` gives no warnings.

**4. Findings** (they cover 4358c28 plus my untracked test file)

| # | file:line | what was measured | what reaches it | verdict | origin |
|---|---|---|---|---|---|
| A1 | `crates/takt-cli/src/main.rs:278` (`case_to_resume`), `:489-490` (its own `active`/`resumable`) | The check lists the store once to choose a case, and `Run::open_next` (main.rs:403, run.rs:349-350) lists it again to choose what to resume. If the store changes between the two lists, the check passes on "nothing to resume" while `open_next` resumes a case on another protocol. Red at adv2.rs:162. | I found nothing that reaches it. It needs a story moved to active within milliseconds, while another run has opened its case but not yet taken its lock (otherwise the resume is refused as held). I built that state with a stub `aep`. | INFEASIBLE | introduced |
| A2 | `crates/takt-cli/src/main.rs:70-72` (`--input` doc) | The correction refuses every `--input` given for a resumed case (main.rs:283-287). `--help` documents the matching `--protocol` refusal but not this one; the `--input` paragraph speaks only of an opening protocol. Red at adv2.rs:185. | Any operator who reruns the command line that opened a case, with its `--input`, on the store path. | CONFIRMED | introduced |

Fixes, named but not applied:
- **A1:** in main.rs, when `case_to_resume` returned `Some((id, _))` on the store path, call `Run::resume(&repo, options, &id, …)` directly and print the `frontier: resumed` line there, instead of calling `open_next`. The store is then read once and the case checked is the case run. Only call `open_next` when nothing resumes.
- **A2:** one sentence in the `--input` doc: "Given for a resumed case, any input is refused; the case keeps its own."

**5. Attacked, could not break**
- Pass 1's F1 holds: all 3 adv1 cases are green, so the tree, HEAD and `case.yaml` are untouched on both the `--case` and the store path. Pass 1's F2 and F3 hold too (`frontier_cli.rs:264` reaches the value check again; inputs are checked against the resumed case).
- `case_to_resume` agrees with what actually resumes on every path asked about: `--case`, a missing case, a finished case, a held case, no active story, two active stories (same `resumable` order), and `--case` together with `--intent`.
- Every new refusal runs before `CaseDir::resume`'s checkout and revision write. Nothing between `case_to_resume` and `Run::resume`/`open_next` writes state.
- The extra `aep plan artifact list`:
  - It runs only on the store path and takes 0.015 s.
  - It writes no tree file: the `everything()` comparisons are green.
  - With no store, it raises the store error that `open_next` raised before.
  - The one change: that store error now appears before the harness, forge and gates refusals. Exit code 1 either way, and the message is still true.
  - The `frontier_cli` stub `aep` tests are green.
- Comparing protocol IR versus YAML, the `software-change/1` spelling, and an embedded protocol versus a file under the same id.

**6. Paths written outside the worktree**
- `<wave-scratch>/scratch/cli-protocol-defaults/adv2/` (directory)
- `<wave-scratch>/scratch/cli-protocol-defaults/adv2/red-alone.log`
- `<wave-scratch>/scratch/cli-protocol-defaults/adv2/suite.log`

The test repositories and the stub `aep` went under the worktree's `target/tmp/` and are removed on drop; none are left. I took the lease `takt-w6-cpd-adv2` and released it (`session-end` EXIT=0). No processes are left running.

```findings
- file: crates/takt-cli/src/main.rs
  line: 278
  category: concurrency
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: case_to_resume and Run::open_next each list the store, so a store change between the two reads lets the protocol check pass on no case while open_next resumes a case on another protocol
- file: crates/takt-cli/src/main.rs
  line: 70
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: takt run --help documents the --protocol refusal for a resumed case but not the new refusal of every --input given for one
```
