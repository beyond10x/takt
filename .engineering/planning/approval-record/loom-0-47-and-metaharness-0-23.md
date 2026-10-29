---
format: aep.planning-md/3
id: approval-record:loom-0-47-and-metaharness-0-23
kind: approval-record
status: approved
title: loom 0.47.0 and metaharness 0.23.0 are pinned in the next wave
summary: the session observer, partial runs and the session-request document; three blockers cleared
relations:
- decides: story:run-console-shows-the-session-live
- decides: story:harness-section-is-metaharness-session-request
- decides: story:harness-flags-spell-metaharness-names
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T23:17:11Z", actor: "human:timo", revision: 2}
---
# Approval: loom 0.47.0 and metaharness 0.23.0 are pinned in the next wave

## Decision

The orchestrator, under the owner's grant, 2026-10-29: loom 0.47.0 and metaharness 0.23.0 are
released; takt pins both in its next wave and clears the blockers they resolve.

| release | what it brings | clears |
|---|---|---|
| loom 0.47.0 | `SessionExecutor::with_observer(impl FnMut(&EventLine, &Path) + 'static)`: a session started from inside the observer is refused before any id, copy or child; a file the observer writes into the copy becomes part of the proposal, so the copy is read-only to it | dependency-blocker:loom-session-event-observer |
| loom 0.47.0 | partial runs: `SessionOverrides` over a kind's defaults | dependency-blocker:loom-session-executor-partial-runspec |
| loom 0.47.0 | a failed harness child is recorded with how it ended and its stderr tail, credential lines withheld | |
| metaharness 0.23.0 | `RunSpec::DOCUMENT_KEYS`, `RunSpec::from_document_str`, `RunSpec::from_document`, `RunSpecDocumentError` (malformed, not an object, duplicate key, unknown key, missing kind, bad value), `metaharness::check_spec`; the keys are `metaharness run`'s long flag names plus `kind` | dependency-blocker:metaharness-session-request-document |
| metaharness 0.23.0 | the `--version` probe waits 60 s under load; harness 0.22.0 admitted | |

`effort` is open vocabulary on the claude and codex kinds, so `effort: extreme` is admitted; a
check that a bad value is refused uses a value of the wrong type (a number). On the b10x kind any
`effort` is refused.
