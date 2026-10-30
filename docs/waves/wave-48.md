# Wave 48: Loom 0.52.0, the connectors forge at a served instance, git through its relay

Stage: **unit (a) committed; (b) and (c) wait for connectors 0.62.0**. Shape: one coordinator
and implementor sub-agents in one tree, on `wave/48` from 1c0e692c (0.24.0's merge).

## Units

| unit | story | stage |
|---|---|---|
| a | story:git-and-model-through-loopback-relays (Acceptance 2 and 4) | committed (1b2cf797) |
| b | story:connectors-forge-at-a-served-instance | active; waits for connectors 0.62.0 |
| c | story:git-through-the-connectors-git-relay | active; waits for connectors 0.62.0 |

Ride-along: story:implementation-prompt-shows-the-tests-path implemented on its recorded self-run
(docs/runs/2026-10-30-self-run-tests-path.md).

## Log

- (a) Loom 0.52.0's session executor starts a b10x-harness session whose endpoint is on a
  loopback host and names no credential; 0.51.0 refused it. The pin needed no takt change. The
  two tests with a double were red on 0.51.0 and are green on 0.52.0; the wire test needs a real
  b10x-harness, which CI does not install, and is ignored there.
- (b) and (c): connectors 0.62.0 ships a served instance's keyed writes under a standing grant and
  its git smart-HTTP relay in one release. Both stories test against that release's served
  instance, installed from its asset in the test and in CI.
