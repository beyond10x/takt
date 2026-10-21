---
format: aep.planning-md/3
id: approval-record:takt-on-loom-sdk-closing-grant
kind: approval-record
status: approved
title: Wave 29's merge, the rest of epic:takt-on-loom-sdk and the release that closes it are granted
relations:
- decides: epic:takt-on-loom-sdk
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-21T22:46:20Z", actor: "human:timo", revision: 2}
---
## Decision

On 2026-10-22 the owner's side granted, through the orchestrator under the owner's standing
delegation:

1. The merge of wave 29 into `main` through the bot route, on a clean reviewer verdict at the
   merge sha (the whole diff `main..<merge sha>`, as decision-blocker:wave-29-merge-grant asks).
2. After that merge: story:harness-child-confined.
3. Once Loom 0.32.0 is tagged and the orchestrator says so: story:the-loop-is-run-until-blocked,
   story:takt-evaluation-code-retired and story:takt-uses-loom-session-executor.
4. story:design-names-loom-as-the-runtime.
5. Then the takt release that closes epic:takt-on-loom-sdk, through this repository's release
   process.

Not granted: the hardening drafts story:evidence-port-is-the-only-writer and
story:process-starts-refused-by-type wait.

Each merge after wave 29's still rests on a clean reviewer verdict at its own merge sha.
