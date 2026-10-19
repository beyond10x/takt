# Wave 14: toward a self-improvement turn the installed takt accepts: the judge guard, the self-improvement protocol, a scorecard bound to its binary, an anchored promotion chain, a confined check run

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed**. Approval: the planner role named these units and lanes for release 0.2.0, which ships when one `self.improvement/1` case reaches `accepted` with every piece of evidence recorded by validators, promoted by the installed takt; each judge-surface merge needs the planner's grant of its exact commit, on a reviewer verdict. Mode: interactive, up to five implementors in parallel.

## Units

| lane | unit | serves | acceptance (condensed) | judge surface |
|---|---|---|---|---|
| A1 | `story:judge-guard` | `vision:safe-agentic-coding` | a judge-surface merge granted by `agent:planner` is refused unless a delegation is on record at the base, and refused again once revoked | yes |
| A2 | `story:self-improvement-protocol` | `vision:safe-agentic-coding` | a `self.improvement/1` case reaches its outcomes with every claim from a validator; part 1 (declined, unreproducible) in parallel with A1, part 2 (the merge validators, accepted) after A1's grant | yes |
| B1 | `story:scorecard-save-checks-binary` | `vision:safe-agentic-coding` | a save under a revision other than its runs' binary exits 1 naming the case | yes |
| B2 | `story:promotion-chain-head-anchored` | `vision:safe-agentic-coding` | a deleted or replaced latest promotion record is refused, naming it | yes |
| C | `story:check-finding-run-step-confined` | `vision:safe-agentic-coding` | the predicate writes nothing on the host, reads nothing of the caller's home, and no process left by one check changes the next | yes |
| carry-over | improvement turn 4: `story:step-guard-sees-test-script-imports` (granted at 1e2fdb3 in wave 13's time, off the critical path) | `vision:safe-agentic-coding` | a step cannot shadow the test runner the test script loads | yes |
| R1 | `story:cases-root-link-is-undone` (a high review finding the owner moved forward) | `vision:safe-agentic-coding` | a step that replaces the cases directory with a link is undone, and nothing is written through a link on the way to a case | yes |
| R2 | `story:ignored-directories-are-guarded` (likewise) | `vision:safe-agentic-coding` | a step cannot change what an ignored directory already holds; a rewritten file the operator owned is set aside, never deleted | yes |
| R3 | `story:test-command-reaches-no-forge-login` (likewise) | `vision:safe-agentic-coding` | the test command runs with no forge login, keyring or session bus in reach | yes |
| D | `story:promotion-without-daily-cap` (the owner's decision, for trials one after another) | `vision:safe-agentic-coding` | a second revision is promoted the same UTC day; one promotion at a time under the store's lock | yes |

## Selection path

The planner's wave 14 after the owner's word to bring the next release forward with self-improvement: lane A on the critical path (judge-guard, then the self-improvement protocol, its first part started beside it on files judge-guard does not hold), lane B (the scorecard save the protocol depends on, then the anchored chain), lane C (the confined check run that guards the protocol's evidence). The lanes branched from wave 13's tip 4e1aa08 while its close was reviewed; this branch opens from main at 5df99e9, after wave 13 and the CI fix for bare test repositories, and each lane merges it before its grant.

## Left out

Until after release 0.2.0, on the planner's resequencing: `story:every-yaml-read-refuses-aliases`, `story:rejudge-is-bounded`, `story:scorecard-binds-binary-digest`, `story:story-delivery-tests-first`, and the lows of `story:review-of-main-lows`.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 52b63f0 (wave 13): `task check` exit 0, 975 passed, citations 658 of 658; 5df99e9 adds the bare-repository CI fix, a CI-like run at its branch 975 passed, 0 failed |
| model budget | not stated; N = 5 |

## The commits this wave makes

One opening commit (this page), each unit's commits on its own branch, the merges of the units into this branch on their grants, a merge of main when it moves, the closing planning-store commit, and the merge into local main. No push, tag or release.

## Log

- 2026-10-19: wave opened from main at 5df99e9; lanes A1, A2, B1, B2 and C already running from 4e1aa08.
- 2026-10-19: merged on their grants, each noted on its merge: turn 4 at b767fb5 (1e2fdb3), B1 at 81c78e0 (96871f8), C at 85dfae7 (1c905c7) and 0b86f7c (f2a20e9), B2 at e75ba57 (941ec85), A1 judge-guard at aef1925 (5d9989b, after four corrections), R3 at 08efb2c (94405a8), R1 at 24140d8 (799bff1), A2 RSI part 1 at e27e437 (b8881bd), R2 at 85a2d2a (93127d5), D at 12dfd63 (9f3818a), A2's follow-up at bf10429 (920487f). Integration fixes on this branch, each granted: 79a11a3 (ff7a0ee, cases_root_link commits its test scripts, as the step guard now reads what the test script runs) and bdbd57f (f519248, the test command's registry login hidden whatever home takt has; the tree read once more after files are set aside). Merge resolutions: test-forge's login hiding applied on turn 4's fresh home (08efb2c); cases-root-link's single undo carrying turn 4's outside directories (24140d8); ignored-dirs' set-aside folded into that undo, moved last and only when every part put its files back, a run with no set-aside root opening and refusing its first harness step (85a2d2a). Main merged in at 86ad10c, 8579d74, 5a463e1 and 94b8f30.
- 2026-10-19: closed. At 94b8f30: `task check` exit 0 (load 19.6); `cargo test --workspace --no-fail-fast` 1197 passed, 0 failed, 6 ignored (load 23.6); `task fuzz` exit 0, four targets, no crash; citations 727 of 727; the store valid. Not in this wave: A2 part 2 (the merge validators and binary.promote, so a turn reaches `accepted`), under review on impl/rsi-part-2 for 0.2.1; the planner ran trials 2 and 3 on it in scratch state, both accepted and promoted. Left as findings: the delegation record (the owner's B, after judge-guard), the judge-guard test for an init without a template (covered by review evidence), and the reviews' queued lows.
