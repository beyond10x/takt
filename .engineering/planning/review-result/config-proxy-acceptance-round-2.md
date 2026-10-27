---
format: aep.planning-md/3
id: review-result:config-proxy-acceptance-round-2
kind: review-result
status: active
title: 'Acceptance critic, round 2: config-proxy drafts'
tags:
- config-proxy
relations:
- reviews: story:llm-section-is-an-llm-routes-document
- reviews: story:model-names-an-llm-route
- reviews: story:harness-section-is-metaharness-session-request
- reviews: story:run-flags-spell-metaharness-and-llm
- reviews: story:one-run-file-for-host-and-loom-agent
- reviews: story:harness-flags-spell-metaharness-names
revision: 1
---
needs-revision
story:llm-section-is-an-llm-routes-document — "Each item is a conformance scenario named in `ess/ess-inputs.yaml` (empty today), run by `task check`" cannot be checked: `task check` runs `ess specify validate`, clippy and `cargo test` and no `ess verify conform` step, `scenarios` is `[]`, and no story scope names `Taskfile.yml`, so all six sets' scenarios have no runner; the body must add the runner to scope and Decision — .engineering/planning/story/llm-section-is-an-llm-routes-document.md:64
story:harness-section-is-metaharness-session-request — scenario 1 asks the stub's argv file to show "the effort argument metaharness gives Codex for `high`", but metaharness 0.19.0 writes Codex's effort into the generated config as `model_reasoning_effort`, not into argv, so the argv file cannot show it; name the observable (the generated config content) — .engineering/planning/story/harness-section-is-metaharness-session-request.md:81 (metaharness 0.19.0 `crates/metaharness-codex/src/launch.rs:1264-1265`)
story:harness-section-is-metaharness-session-request — scenario 3 expects an unknown key refused "with metaharness's message", while the Decision refuses every key outside the admitted six in takt, naming the key, so an unknown key never reaches metaharness and the two disagree on whose message is the check — .engineering/planning/story/harness-section-is-metaharness-session-request.md:85 (Decision, line 61)
story:harness-flags-spell-metaharness-names — scenario 1 compares only argv between a flag run and a file-key run, but `--strict-version` and `--context` (and `--effort` on Codex) never reach the harness's argv in metaharness 0.19.0, so a run with the flag ignored passes; the scenario needs an observable per flag that differs from a run without it — .engineering/planning/story/harness-flags-spell-metaharness-names.md:53 (metaharness 0.19.0 `crates/metaharness/src/builder.rs:628`, `crates/metaharness-claude/src/launch.rs:945`)

**What I read:** 6 of 6 ids, all in full (`aep plan artifact show <id>`, `aep plan artifact kinds`, `aep plan artifact lifecycle story`). I checked the takt `Taskfile.yml`, `ess/ess-inputs.yaml` and `run_config.rs`, llm 0.16.0 `routes.rs`, loom 0.44.0 `CHANGELOG.md` and `admission.rs`, and metaharness 0.19.0 launch and spec code.

**What I could not establish:**
- Whether loom 0.44.0's walk catches a literal in `llm.yaml`'s `auth.secret` (story:one-run-file-for-host-and-loom-agent scenario 3). `SecretLiteral` exists for configFiles content, but I did not run it.
- How scenario 2 of that story runs loom's admission from a takt test.
- Whether `--strict-version` and `--context` have any argv or stub-visible effect in the b10x-harness path.
- Whether the first finding is out of my lane (the test runner is a shared surface). I raised it because the stated check cannot run today.

```findings
- file: .engineering/planning/story/llm-section-is-an-llm-routes-document.md
  line: 64
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance says each item is a conformance scenario in ess/ess-inputs.yaml run by `task check`, but `task check` runs no ess conformance step, ess-inputs.yaml has `scenarios: []`, and no story scope names Taskfile.yml, so the check has no runner; the body must add the runner to scope and Decision"
- file: .engineering/planning/story/harness-section-is-metaharness-session-request.md
  line: 81
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenario 1 asks the stub's argv file to show the effort argument metaharness gives Codex for high, but metaharness 0.19.0 writes Codex effort into the generated config as model_reasoning_effort, not argv, so the argv file cannot show it; name the observable"
- file: .engineering/planning/story/harness-section-is-metaharness-session-request.md
  line: 85
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "scenario 3 expects an unknown key refused with metaharness's message, while the Decision refuses every key outside the admitted six in takt, naming the key, so an unknown key never reaches metaharness and the two disagree on whose message is the check"
- file: .engineering/planning/story/harness-flags-spell-metaharness-names.md
  line: 53
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scenario 1 compares only the stub's argv between a flag run and a file-key run, but --strict-version, --context and --effort on Codex never reach argv in metaharness 0.19.0, so a run that ignores the flag passes; the scenario needs a per-flag observable that differs from a run without it"
```
