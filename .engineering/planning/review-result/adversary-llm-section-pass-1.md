---
format: aep.planning-md/3
id: review-result:adversary-llm-section-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on the llm section unit: NEEDS-CHANGE, findings dispositioned'
tags:
- adversary-pass
relations:
- decides: story:llm-section-is-an-llm-routes-document
revision: 1
---
unit: story:llm-section-is-an-llm-routes-document
verdict: NEEDS-CHANGE
cases: executed 0→22, red 4 (before the correction round)
origin: introduced 5, pre-existing 0, undecided 2
wrote-outside-worktree: scratch directories and harness outputs only
needs-coordinator: yes

Disposition, taken by the coordinator after this pass:
- F2 (empty llm member read as absent): fixed in the unit before merge; a present llm member that is empty or not a document is refused.
- F3 (path escape with `..`): fixed in the unit before merge; an absolute or `..` path is refused before reading.
- F1 (YAML refusal names no field): the case is rewritten to assert the current state (line and column) with a message naming the story; llm 0.16.0 gives no field name for YAML. Recorded as an upstream limit on the story.
- F4 (refusal position in re-serialised text): recorded as a known limit on the story; no change.
- F5 (declared ESS members not yet read by code): by design; used by stories 2 and 5.
- F6 (harness union not declared): belongs to story:harness-section-is-metaharness-session-request.
- F7 (forge-wait timing case failed once under load, passed on reruns): not this unit's code; the gate is rerun.

```findings
- file: crates/takt/src/run_config.rs
  line: 162
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: acceptance item 3 requires llm's refusal to name the unknown field, but for a YAML llm document (inline, or a .yaml path) llm 0.16.0 names only the line and column, and the unit's test at crates/takt-cli/tests/run_config_llm.rs:255 asserts only the YAML wording, not the field
- file: crates/takt/src/run_config.rs
  line: 268
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: an llm member with no body is read as absent, so the run opens a case with no routes and no llm_digest, where llm refuses the empty section
- file: crates/takt/src/run_config.rs
  line: 142
  category: acceptance
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: a path such as ../llm-outside.yaml is read outside the run file's directory and opens a case; the story says read beside the run file, and intent is undecided
- file: crates/takt/src/run_config.rs
  line: 161
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: an inline YAML llm refusal reports a line and column in takt's re-serialised text, not in the run file
- file: ess/domains/takt.yaml
  line: 604
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: takt.work.RouteName, takt.work.HarnessSettingOwner (line 611) and takt.work.HarnessSettingRefused (line 617) are declared but no code under crates/ writes or reads them
- file: ess/domains/takt.yaml
  line: 572
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: undecided
  message: the brief asks harness to become a union with takt.work.HarnessSessionRequest, but harness stays Optional<takt.work.HarnessName> and HarnessSessionRequest appears nowhere in ess/; the story's Scope does not list harness
- file: crates/takt/tests/active_time_budget.rs
  line: 361
  category: concurrency
  severity: warning
  verdict: INFEASIBLE
  origin: undecided
  message: a_forge_wait_longer_than_the_active_budget_does_not_stop_the_run failed once under load (active 2.005 against a budget of 2) and passed on two solo reruns; the unit's diff changes no active-time code
```
