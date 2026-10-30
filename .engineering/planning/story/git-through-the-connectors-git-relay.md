---
format: aep.planning-md/3
id: story:git-through-the-connectors-git-relay
kind: story
status: implemented
title: Fetch and push go through a served Connectors instance's git relay
summary: connectors 0.62.0 git relay as origin behind a loopback boundary; refusals named
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:git-and-model-through-loopback-relays
scope:
- confidence: inferred
  path: .github/workflows/check.yml
- confidence: cited
  path: crates/takt-cli/tests/git_relay_refusals.rs
- confidence: inferred
  path: crates/takt-cli/tests/git_relay_served.rs
- confidence: cited
  path: crates/takt-cli/tests/loopback_relays_cli.rs
- confidence: cited
  path: crates/takt-cli/tests/support/smart_http.rs
- confidence: cited
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: ess/domains/takt.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T10:37:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T10:37:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-30T18:09:09Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":7}}}
---
## Outcome

A run's fetch and push, including `takt scenario run`'s `trial/<n>`, go through a real served
Connectors instance's git relay (`/git/<owner>/<repo>.git`, connectors 0.62.0), reached as
`origin` on a loopback host, with no forge credential in takt. A push the relay refuses ends the
run with the relay's reason named.

## Context

story:git-and-model-through-loopback-relays admits `http://` for git only on a loopback host, and
has been tested against a local smart-HTTP stand-in. Connectors 0.62.0's relay serves clone, fetch
and push for the repositories it lists. It admits a push only to its `ref_prefixes`, checks
`commit_identity` and an optional `pre_push_check`, and answers each refused command as
`! [remote rejected] <ref> (<reason>)`. In a pod the boundary adds the caller's bearer on
loopback, and the relay adds the forge's token upstream.

## Acceptance

1. A test runs connectors 0.62.0's served instance on loopback, from its release asset, pinned by
   sha256. Its git relay's upstream is a local bare repository served over smart-HTTP, and a
   loopback boundary stand-in adds the bearer. A run whose `origin` is the boundary fetches its
   base and pushes its branch, and the branch is in the upstream repository. CI installs the
   same asset and runs the test.
2. `takt scenario run` creates `trial/<n>` through the same relay when `ref_prefixes` admits
   it.
3. A push to a ref outside `ref_prefixes`, or a commit whose identity is not the relay's
   `commit_identity`, ends the run with the ref and the relay's reason in its stop line, and
   nothing reaches the upstream.
4. The git child's environment and arguments hold no token. Only the boundary stand-in holds the
   bearer.
