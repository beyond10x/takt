---
format: aep.planning-md/3
id: approval-record:trial-harness-arm-endpoint
kind: approval-record
status: approved
title: The trial agent's harness arm runs on OpenCode Go, its key in the OS keychain
relations:
- decides: story:harness-endpoint-is-configured
revision: 4
transitions:
- {from: "draft", to: "approved", at: "2026-10-21T23:18:16Z", actor: "human:timo", revision: 2}
---
## Decision

The owner, 2026-10-22, on which endpoint the trial agent's `--harness harness` arm uses: "A - we
have "Go" luna + haiku is fine".

- Option A: OpenCode Go, with two endpoints:
  - wire `anthropic-messages`, base `https://opencode.ai/inference/go/anthropic/v1`, model
    `claude-haiku-5-5`;
  - wire `openai-responses`, base `https://opencode.ai/inference/go/openai/v1`, model `gpt-6-luna`.

The owner, later on 2026-10-22, on where its API key lives: "i dont want to store it in file - it
should go to os keychain".

- The key is a secret in the owner's secrets store, backed by the OS keychain: namespace `llm`,
  name `opencode-go-api-key` (corrected the same day from an earlier service and entry pair).
- takt passes the secret's name down and the harness resolves it; takt never reads the key.

## Consequence

- story:harness-endpoint-is-configured is approved and scheduled after epic:takt-on-loom-sdk
  closes; its acceptance names both endpoints and the secret's name.
- Until it lands, `--harness harness` reaches only the endpoint fixed in
  `crates/takt-cli/src/main.rs`, so the trial arm waits on that story and on the harness
  resolving a secret's name.
- Recorded by the dispatcher: the trial agent writes to no planning store (`AGENTS.md` § Roles).

## Scheduling

The orchestrator, on the owner's behalf, 2026-10-23: "Accept story:harness-endpoint-is-configured
now (approval-record:trial-harness-arm-endpoint, approved 2026-10-21). Deliver it in your next
wave, then release. Its depends_on epic:takt-on-loom-sdk is a scheduling note, not a code
dependency. If AEP refuses the move, replace that edge with depends_on
story:takt-uses-loom-session-executor (implemented) and record why. The credential is in the OS
keychain, service b10x-secrets, label keyring:default/llm/default/opencode-go-api-key@b10x-secrets.
Reference it by name only and never print it. harness resolves a secret name since
story:api-key-from-the-secrets-store, and metaharness since story:b10x-api-key-from-keychain."
Then: "story:harness-endpoint-is-configured goes first, because it is on the goal path (ADR 0101).
The GitLab stories follow it."

The dispatcher takes it into wave 36 as unit U3 beside the GitLab units already in flight; the
wave's merge carries all three, and a release follows the merge.
