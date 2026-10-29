---
format: aep.planning-md/3
id: dependency-blocker:metaharness-session-request-document
kind: dependency-blocker
status: cleared
title: metaharness publishes no session-request document with unknown keys refused and a per-kind check
relations:
- blocks: story:harness-section-is-metaharness-session-request
- blocks: story:harness-flags-spell-metaharness-names
revision: 5
transitions:
- {from: "open", to: "cleared", at: "2026-10-29T03:53:02Z", actor: "human:timo", revision: 4}
---
# Dependency blocker: metaharness-session-request-document

## What blocks

story:harness-section-is-metaharness-session-request (acceptance items 1, 4 and 5) and
story:harness-flags-spell-metaharness-names (acceptance items 1 and 3) need metaharness to publish its session request
as a document: `RunSpec` with unknown keys refused, a public parse, and `check_spec` over it, so that a bad value and a
key the harness kind does not take are refused in metaharness's own words
(epic:run-config-proxies-llm-and-metaharness § Out of Scope).

metaharness's store at `origin/main` 75a44bcc (tag 0.22.0) holds no story for it: no story under
`.engineering/planning/story/` names `RunSpec` or a session-request document (`git grep -il`, 2026-10-28).

## What clears it

A metaharness release whose public API parses a session request with unknown keys refused and checks it per harness
kind, reached by takt through loom-sdk. takt's tests `a_flag_not_carried_yet_is_refused_before_any_case_opens` and
`a_key_not_carried_yet_is_refused_not_dropped` then change from refusals to carried keys.

## Cleared by loom 0.48.0

loom 0.48.0 (tag `0.48.0`, `82e64a4a`) carries metaharness 0.23.0 (`dfee56ee`) and Harness 0.22.0
(`3eca074a`); takt's lock holds one `metaharness-protocol`, 0.23.0, and takt names metaharness only
through `loom_sdk::session::metaharness`. Wave 43 pins loom 0.48.0.

metaharness 0.23.0's session-request document is what this blocker asked for, reached through
loom-sdk, but for one example: an unknown `effort` value (the blocked story's acceptance 4 named
`effort: extreme`) is passed through to whoever serves the model and refused by neither reader
(metaharness `spec.rs:357-365`); that acceptance item now names a value of the wrong type.

- `loom_sdk::session::metaharness::protocol::RunSpec::from_document_str` and `RunSpec::from_document`
  read a run-spec document whose keys are `metaharness run`'s long flags plus `kind` and
  `auditor-args` (`RunSpec::DOCUMENT_KEYS`); `RunSpecDocumentError` refuses text that is not JSON,
  a document that is not an object, a key written twice, an unknown key, a missing `kind` and a
  value of the wrong type.
- `loom_sdk::session::metaharness::check_spec` refuses, per kind and before anything starts, a
  setting the kind's launch would drop: Codex's `--max-turns` and `--max-budget-usd`, `--hooks` and
  `--driver` on Claude and Codex, `--max-budget-usd` and `--effort` on b10x-harness.

Observed in takt's tree on loom 0.48.0 with a throwaway test target, deleted after the run:
`RunSpec::from_document_str("{\"kind\":\"claude\",\"no-such-key\":1}")` returned
`Err(UnknownKey { key: "no-such-key" })`, and `check_spec` over `{"kind":"codex","max-turns":3}`
returned `Err(Launch { .. })` naming that codex exec has no turn ceiling.

Breaking for takt with this pin:

- The `metaharness` types loom-sdk re-exports, including the `EventLine` that
  `SessionExecutor::with_observer` hands over, are metaharness 0.23.0's.
- Loom's b10x-harness session with `strict_version` admits Harness 0.22.0 only. takt sets no
  `strict_version` (`--strict-version` is refused as not carried yet): an older b10x-harness reads
  as an H9 gap in the preflight, advisory. The container image moves to b10x-harness 0.22.0.
- Every session Loom starts passes `check_spec` first.

takt compiled on loom 0.48.0 with no source change.

Both stories this blocks have no open blocker left: dependency-blocker:loom-session-executor-partial-runspec
is cleared, and this one clears with the pin. story:harness-flags-spell-metaharness-names still
depends on story:harness-section-is-metaharness-session-request.
