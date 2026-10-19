---
format: aep.planning-md/3
id: review-result:main-ae680d5-adversarial
kind: review-result
status: active
title: 'Adversarial review of main at ae680d5: ten adversaries, fact-checked'
relations:
- reviews: story:git-reads-no-user-excludes
- reviews: story:ignored-directories-are-guarded
- reviews: story:promotion-refusal-is-not-final
- reviews: story:scorecard-outcome-from-held-out-check
- reviews: story:review-of-main-lows
revision: 1
---
needs-revision

Adversarial review of takt main at ae680d5, 2026-10-19: ten adversaries (a smaller model), one per attack surface, each told to break one promise; every finding then fact-checked against the code by a second reviewer. Verdicts: CONFIRMED, PARTLY, CONFIRMED-UNREACHABLE, COVERED (an existing story plans it), WRONG.

## Fact-checks

Fact-check of adversary 10 (7 findings): none high. A10.1 PARTLY low (records stay on the case branch: approval/test_result/move pushed only in the Open arm, validator.rs:1446; outside actor only); A10.2 PARTLY low (operator's --protocol choice; built-in requires ci.green; covered for takt cases by judge-guard); A10.3 CONFIRMED-UNREACHABLE (SIP D1); A10.4 CONFIRMED low (move after refused test_result; merge fails closed); A10.5 CONFIRMED low (test_result names no command; partly story:test-pass-needs-the-runners-report); A10.6 CONFIRMED low (has_evidence ignores result: flaky fail never re-run at X); A10.7 COVERED (judge-guard).

Fact-check of adversaries 3 and 2 (14 findings): 1 confirmed, 3 partly, 5 covered, 3 confirmed-unreachable, 2 wrong.
Correction: a0b8576 baseline is 5/5 ApprovalRequired (rank 3), not accepted.

| id | verdict | severity | covered |
|---|---|---|---|
| A3.1 | COVERED | medium | promote-binds-scorecard-and-merge; scorecard-save-checks-binary |
| A3.2 | CONFIRMED | medium | none — verdict inputs written by the candidate binary; held-out acceptance never read |
| A3.3 | CONFIRMED-UNREACHABLE | low | — |
| A3.4 | PARTLY | low (sticky liveness) | none — on takt's corpus ~2-8% false refusal (token 1.9-5.8%, outcome 1.9%); sticky via promotion.rs:1481-1501 |
| A3.5 | WRONG | — | decided rule |
| A3.6 | PARTLY | low | none — new: promoting a candidate that adds a scenario with <5 cases wedges the store (enough_trials :1672-1677; --baseline-only refuses :1504-1512) |
| A3.7 | PARTLY | low | T4/T5 — no_record never read by promotion; trials without a record drop out silently |
| A2.1 | COVERED | medium | promote-binds-scorecard-and-merge |
| A2.2 | COVERED | medium | promotion-records-its-toolchain § Also |
| A2.3 | CONFIRMED-UNREACHABLE | low | P1.3 |
| A2.4 | CONFIRMED-UNREACHABLE | low | half: P1.1 in review-of-main-lows — fd 9 readable in bwrap; agent shells have only 0-2 open |
| A2.5 | WRONG | — | by design (:562-564) |
| A2.6 | COVERED | low | git-filter-drivers-do-not-run |
| A2.7 | COVERED | low | confinement-seam C8 |

For the wave-12 promotion: judge scenario 1 only (A3.6 wedge); a false refusal is ~2-8% and sticky.

Fact-check of adversaries 7, 1, 8, 9 (25 findings): 13 hold (all low), 8 covered, 4 wrong. No high.
Cases-dir snapshot holds at ae680d5: run.rs:3797/3940/4002-4019 (harness), :3523/3611 (validator).

| id | verdict | severity | covered |
|---|---|---|---|
| A7.1 | COVERED | high | scan-before-push (fix 2e099b8 not on main) |
| A7.2 | WRONG (route); COVERED (no PR base check) | — | merge-checks-the-pull-request-base |
| A7.3 | WRONG | — | remote.*/url.* guarded (git.rs:745-753) |
| A7.4 | PARTLY | low | new: skipping = pass (forge.rs:1131); .github/** judge surface |
| A7.5 | CONFIRMED-UNREACHABLE | low | — |
| A7.6 | WRONG | — | — |
| A1.1 | COVERED | low | review-of-main-lows |
| A1.2 | CONFIRMED | low | report --trace error prints format raw |
| A1.3 | PARTLY | low | hand-made files only |
| A1.4 | CONFIRMED | low | --steps follows links (operator) |
| A1.5 | CONFIRMED | low | --steps/--trace unbounded (operator) |
| A1.6 | PARTLY | low | Cf part covered |
| A1.7 | CONFIRMED | low | report.rs:235 clamp (file corrected) |
| A1.8 | CONFIRMED | low | merge_steps O(n·m) |
| A8.1 | CONFIRMED | low | NUL / ≥128 KiB summary ends run on Err |
| A8.2 | CONFIRMED | low | refused merge labelled approval-only |
| A8.3 | CONFIRMED | low | newline path forges an out line |
| A8.4 | COVERED | low | failed-step-tokens-are-recorded |
| A9.1 | CONFIRMED-UNREACHABLE | low | needs judge-surface writes; cheap hardening (cargo JSON path) already sent to E1 |
| A9.2 | COVERED | low | S9.6 |
| A9.3 | CONFIRMED | low | MARKER misses _read/xread/homoglyph |
| A9.4 | PARTLY | low | symlink write-through, no gain over unconfined candidate |
| A9.5 | CONFIRMED | low | $TAKT not required (bogus filing only) |
| A9.6 | CONFIRMED | low | output min length |
| A9.7 | CONFIRMED | low | check-citations follows repo symlinks out |

## Adversary reports (condensed by the planner)

Adversary 1 (trace and report) at ae680d5, haiku. needs-revision. All inferred from code (installed takt has no trace/report).
Known already (story:review-of-main-lows): `|` in table cells; Cf bidi not escaped.

- A1.1 medium — report.rs:519-533 (:522), trace.rs:359-389 — `|` not escaped in table cells (KNOWN, filed).
- A1.2 medium — trace.rs:1254-1259 with main.rs:379 — `report --trace` prints the header `format` verbatim in the error; `eprintln!("takt: {error:#}")` unescaped → terminal escape on stderr. Input: trace header `"format":"\u001b]2;owned\u0007"`.
- A1.3 medium — trace.rs:720-722, report.rs:482-495, :595, trace.rs:1233-1279 — printed token total prefers stated `tokens` over components, so total ≠ sum of printed components; `--trace` trusts header totals; saturating adds print u64::MAX.
- A1.4 medium — trace.rs:1154-1157, :1126-1127 — `--steps` file read through symlinks (events.jsonl uses O_NOFOLLOW), so a symlink in the case reads outside it.
- A1.5 medium — events.rs:297-300, trace.rs:1127, :1235 — whole-file reads with no size bound; `--steps` symlink to /dev/zero grows memory; FIFO at --steps blocks; multi-GB events.jsonl exhausts memory (events FIFO and /dev/zero refused at trace.rs:1147 is_file).
- A1.6 low — trace.rs:810, report.rs:452, run.rs:3440-3451, :3636-3642, trace.rs:381 — refusal text printed unredacted; Cf bidi (KNOWN part).
- A1.7 low — trace.rs:231-236 — takt time clamped to 0 when measured exceeds wall (prints 0, against "unknown, never 0"); negative spans unclamped.
- A1.8 low — trace.rs:912-931 — merge_steps O(calls × lines) with parsing.

Adversary 2 (promotion) at ae680d5, haiku. needs-revision. Scratch: (the review scratch directory)

- A2.1 medium — promotion.rs:1016-1092, scorecard.rs:620-630 — scorecard accepted on content alone (KNOWN: P1.2, story:promote-binds-scorecard-and-merge).
- A2.2 medium — promotion.rs:1188-1189, :1209-1210, :1635-1636 — binary_sha256 written, never read on main; installed binary checked only by --version (KNOWN: baseline-sha c1 checks it for stated records; story:promotion-records-its-toolchain now covers every record).
- A2.3 medium — promotion.rs:1046-1047, :1473 — two reads of the scorecard (KNOWN: P1.3).
- A2.4 low NEW — promotion.rs:832-846 — bwrap passes inherited non-CLOEXEC fds into the confined build; no fd sweep, no --new-session. Input: `takt promote ... 9<secret.txt`; the build reads /proc/self/fd/9. VERIFIED with bwrap 0.12.0 (fdprobe).
- A2.5 low — promotion.rs:565-575 — installed binary run unconfined on the host during promote, against the module doc. Verified by reading.
- A2.6 low — promotion.rs:1273-1280, :1292 — host git checkout runs filter drivers (KNOWN: P1.4, story:git-filter-drivers-do-not-run).
- A2.7 low — promotion.rs:273-300, :396-432 — a refusal at the chain tail is removable undetected (KNOWN: C8, story:confinement-seam).

Adversary 3 (scorecard verdict) at ae680d5, haiku. needs-revision. Replay: (the review scratch directory)

- A3.1 high — scorecard.rs:444-488, :120-158 — reader never reads opened.binary (events.rs:76; main.rs:969); runs of different builds merge into one case (best completed wins); repo trials of an old binary count in a candidate's n; --revision only names the file. Replayed R1, R2a/b. (Overlaps P1.2 / promote-binds-scorecard-and-merge.)
- A3.2 high — scorecard.rs:309-318, :371-373, events.rs:163-167 — judge inputs (stop string, tokens) are written by the build under test; no independent check. Inferred. (Same as fc-23 extra note.)
- A3.3 medium — scorecard.rs:364-368, :131-137, :737-739 — saturated sum makes baseline max u64::MAX, disabling the token rule; verdict ignores `saturated`. Replayed; reachability needs ≥2^63 tokens.
- A3.4 medium — scorecard.rs:737-739, :773-774 — the rule marks a GOOD candidate worse often: with n=5 vs 5 i.i.d. tokens, P(candidate median > baseline max) = 21/252 = 8.3% per scenario, 35.3% that at least one of five scenarios is worse; the outcome rule fires 11.1% at p=0.8 (17.2% at p=0.5). Exact enumeration (R4).
- A3.5 medium — scorecard.rs:783-792 — verdict ignores refusals, errors, NoProgress, steps (compared only when a definition version changed, 748-759); doc 15-17, 638-646 states it. By reading.
- A3.6 medium — scorecard.rs:689-692 — a candidate scenario absent from the baseline is "no baseline", never counted; enough_trials (promotion.rs:1658-1690) checks baseline scenarios only. Replayed R7.
- A3.7 medium — scorecard.rs:444-488 (:451, :481-485), promotion.rs:1658-1690 — n counts case dirs with a readable record, not trials run; no-record dirs drop out; symlinked dirs vanish silently; copied events.jsonl counted as separate cases (opened.case not checked). Replayed R5a/b, R6.

Adversary 4 (grants and authority) at ae680d5, haiku. needs-revision, 3 low; no merge without a valid grant found.

- A4.1 low NEW — case.rs:1115 — approvals_only returns true at the revision without checking every grant has its approval record on the branch (`unrecorded` ignored); the approval record need not reach base. Verified by reading.
- A4.2 low NEW — case.rs:646, frontier.rs:401-405 — approve commits the record, then writes the grant; a failed grant write leaves an orphan approval commit; resume (case.rs:1546-1548) moves off the granted revision; fail-closed. Inferred.
- A4.3 low — git.rs:1035-1038 — refs/replace unguarded (KNOWN, A6.1/GIT-8).
Checked, no bypass: merge gate validator.rs:1407-1410, sha-pinned merge forge.rs:1155, Held::take create_new, authority.yaml snapshot coverage, applies_at/grant_subject binding, exact capability match.

Adversary 5 (step guard escape) at ae680d5, haiku. needs-revision. Scratch: (the review scratch directory) (h1, h2).

- A5.1 medium NEW — git.rs:545, :1333; run.rs:1596 — Content::of reports directories as Dir and change() skips Dir→Dir, so writes inside a pre-existing git-ignored directory are not compared, undone, or judged (JudgeSurface::roots looks at root names only); vendored/build.rs survives. Verified code + replay (git status collapses ignored dirs); reach inferred.
- A5.2 medium — run.rs:1743-1765 — Outside omits ~/.cargo/bin/* and ~/.rustup/** (being covered by turn 4 c3 on its branch, not main).
- A5.3 medium NEW — git.rs:26, :388, :369-377 — global git excludes (core.excludesFile, ~/.config/git/ignore) not overridden or restored; an untracked file they match drops out of `uncommitted` (run.rs:3463) and changed_paths. Verified replay; record consequence inferred.
- A5.4 low — run.rs:1588-1591, :1495 — tracked out-of-tree link on the surface compared by target only. Inferred; needs a committed link.
- A5.5 low — run.rs:324, :337 — restore unlinks and rewrites, so a hard link to an outside file keeps the step's bytes. Verified replay; no new capability.
- A5.6 low — run.rs:1368 — judge_list_at fail-open (KNOWN R5.5).
- A5.7 low — git.rs:26-44, :1363 — global ~/.gitconfig filter driver in takt's reset/status (KNOWN, story:git-filter-drivers-do-not-run). Inferred; replay did not fire.

Adversary 6 (git runs repo code) at ae680d5, haiku. needs-revision. Scratch: (the review scratch directory)

- A6.1 high — validator.rs:524-527, :578, :590; git.rs:1035-1038 — refs/replace unguarded; no --no-replace-objects on main; gates policy read at base through a step-written replace ref (`policy: none` vs `strict`). Read path verified with takt's flags (exp2). KNOWN: GIT-8 high (fc-456), story:replace-refs-do-not-steer-takts-reads (draft); loop-scorecard's branch adds GIT_NO_REPLACE_OBJECTS but has not merged.
- A6.2 low NEW — git.rs:690 — .git/shallow not guarded; changes rev-list count and is-ancestor answers (validator.rs:984, :1440). Verified exp3; fail-closed only.
- A6.3 low — promotion.rs:532-550 — promotion's git() bypasses git::command hardening (no REDIRECTING_ENV, no --no-replace-objects); operator GIT_* env reaches it. Inferred. (Related: story:git-filter-drivers-do-not-run covers promotion git().)
Non-findings: uploadpack.packObjectsHook in repo config not executed on 2.55 (verified); diff drivers guarded; alternates content-addressed; clone copies no refs/replace.

Adversary 7 (forge and publish) at ae680d5, haiku. needs-revision.

- A7.1 high — validator.rs:1007-1008 with forge.rs:1066 (from validator.rs:1045) — pr.open pushes the case branch before scan-text runs; scan covers PR title and body only. Input: a step commits a file with a private-rule token, pr.open runs. Verified code path + error text forge.rs:1051-1082; gates push-side scan inferred absent. (Overlaps scan-before-push, not on main.)
- A7.2 high — validator.rs:973, :883-899 (request base), case.rs:198 (Branch.base), run.rs:362 (judge surface excludes .engineering/cases/**) — PR base from harness-writable branch.yaml, unvalidated; PrMerged never compares the PR base. Input: step writes `base: <other branch>` into branch.yaml. Verified in code. (CHECK: the cases-dir snapshot restores around steps.)
- A7.3 medium — forge.rs:812, :860-862, :870-873; URL checked once in for_tree (:834-846) — the push resolves `origin` at push time; a repointed remote or pushurl sends elsewhere while the PR POST targets the startup repository. Input: test command `git remote set-url origin <other>` before pr.open. takt side verified; gates re-check inferred.
- A7.4 medium — forge.rs:1130-1131 — bucket `skipping` counts as pass; ci_result pass for a check that never ran. Input: `[{"bucket":"skipping"}]`. Verified in code.
- A7.5 low — validator.rs:1045-1063 — open recorded from the POST response number without read-back. Inferred.
- A7.6 low — validator.rs:973, :1389-1395 — PrMerged takes pull_request from harness-writable branch.yaml. Inferred.
Non-findings: no JSON misparse in forge.rs:1020-1181; merged requires a post-merge read; trial push checks porcelain and lease.

Adversary 8 (hostile harness output) at ae680d5, haiku. needs-revision, all low. Scratch: (the review scratch directory)

- A8.1 low — run.rs:4286-4290, git.rs:437-439, harness.rs:192 — summary with NUL, or ≥131072 bytes, makes the commit spawn fail; change discarded, run ends on Err, no refusal recorded. execve limit replayed; NUL rejection inferred.
- A8.2 low — trace.rs:785, :451, :455-460, :847-848 — refused step keeps its classified outcome: refused repository.merge labelled "approval-only call". Replayed.
- A8.3 low — run.rs:4249-4250, :4256-4260, :4267 — model-named path with a newline forges a takt line on `out`; events JSON-escaped. Inferred.
- A8.4 low — harness.rs:292-293, run.rs:3957-3958, :2622, :2679 — extends N2: parse failure spends tokens uncounted; --tokens restarts at 0 per invocation. Inferred.
Non-findings: offered actions enforced (run.rs:4249); judge-surface checks before the action is used; stdout capped at 8 MiB (harness.rs:104, 265).

Adversary 9 (fool the verification) at ae680d5, haiku. needs-revision. Scratch: (the review scratch directory)

- A9.1 high NEW — scripts/check-finding:225-237 — binary copied from target/debug/takt; with the tree's .cargo/config.toml `[build] target = <host triple>` cargo writes target/<triple>/debug/takt, so a committed target/debug/takt from the candidate is copied and cached per sha. Inferred (cargo layout documented). Not closed by build confinement.
- A9.2 high — scripts/check-citations:347-398, :383, Taskfile.yml:12 — accept has no gate; the digest ledger is in the candidate tree; change cited text then accept, check passes. Verified r1. (Overlaps S9.6, judged CONFIRMED-UNREACHABLE because ess/ is a judge surface.)
- A9.3 medium NEW — scripts/check-citations:69 — MARKER `\bread\s+from` misses `_read from:`, `xread from:`, homoglyphs: silently unchecked and uncounted. Verified.
- A9.4 medium NEW — scripts/check-finding:248-251 — `.cache/aep/protocol-sources` in the fresh HOME is a symlink into the caller's real cache; the predicate command writes through it. Verified with a fake HOME.
- A9.5 medium — scripts/file-finding:119, :85 — predicate accepted without $TAKT in the command; `command: exit 3`, `exit: 3` reproduces for every revision. Verified. (Overlaps check-finding-positive-control, reproduced side.)
- A9.6 low — scripts/file-finding:184 — no minimum for `output` (one character accepted). Verified.
- A9.7 low — scripts/check-citations:171, :329-333, :314, :515 — repo symlink followed outside the repo, and its lines printed. Verified.

Adversary 10 (protocol order) at ae680d5, haiku. needs-revision. Canon read at ~/beyond10x/canon tag 0.6.0.

- A10.1 medium — validator.rs:1410, :1517, :1531 — merged-state branch records merged without grant/CI/test check (earlier H3.1 judged WRONG because the validator runs only when admissible with a grant; this variant: PR merged outside takt BEFORE the approve, then approve, then run → recorded merged although takt never merged). Verified by reading.
- A10.2 medium — story-delivery/1.yaml:141-144, :150-154, validator.rs:1473 — CI read by the merge validator only when head ≠ revision; `accepted` needs only pr.merged; a --protocol file dropping ci.green merges with no CI read (extends X10.7).
- A10.3 medium (latent) — case.rs:1365-1366, run.rs:2788, :4301, case.rs:1548 — non-implementation artifacts stay at r0 (KNOWN: C4.4, CONFIRMED-UNREACHABLE; SIP D1 sets baseline/candidate at open).
- A10.4 low — run.rs:3032-3084, frontier.rs:662 — move to implemented proceeds even when the revision-bound test_result write was refused (relates to R5.2).
- A10.5 low — validator.rs:330-392, main.rs:284-286, evidence.rs — store test_result names no command; `--test-cmd true` indistinguishable.
- A10.6 low — case.rs:1733-1743, run.rs:136-146 — has_evidence ignores result; a flaky fail at revision X is never re-run at X (liveness).
- A10.7 low — case.rs:1523, :1639 — resume recompiles protocol.yaml without comparing protocol.ir.json; out-of-band edit applies. Inferred.

```findings
[
 {
  "file": "crates/takt/src/git.rs",
  "line": 1333,
  "category": "security",
  "severity": "blocker",
  "message": "A5.1: writes inside a pre-existing ignored directory are not guarded; a planted pyc fakes a test pass (replayed). Filed as story:ignored-directories-are-guarded."
 },
 {
  "file": "crates/takt/src/scorecard.rs",
  "line": 309,
  "category": "correctness",
  "severity": "warning",
  "message": "A3.2: the verdict's inputs are written by the candidate binary; the held-out check is not read. Filed as story:scorecard-outcome-from-held-out-check."
 },
 {
  "file": "crates/takt/src/git.rs",
  "line": 26,
  "category": "security",
  "severity": "warning",
  "message": "A5.3: global excludes hide files from takt's git. Filed as story:git-reads-no-user-excludes."
 },
 {
  "file": "crates/takt/src/promotion.rs",
  "line": 1481,
  "category": "reliability",
  "severity": "note",
  "message": "A3.4, A3.6: a refusal is final; a new scenario under trials wedges the store. Filed as story:promotion-refusal-is-not-final."
 },
 {
  "file": "various",
  "category": "correctness",
  "severity": "note",
  "message": "Confirmed lows appended to story:review-of-main-lows."
 }
]
```
