---
format: aep.planning-md/3
id: story:judge-reads-protocol-and-store-from-base
kind: story
status: draft
title: A case is judged by the protocol and store of the promoted base, pinned by digest
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: story:judge-guard
revision: 1
---
## Outcome

A case's protocol and the planning store it reads come from the promoted base revision, outside the worktree a step can write, and the protocol is pinned by digest, so a candidate branch cannot change the rules it is judged by.

## Context

story:judge-guard (grant request d150152, 2026-10-19) built the merge guard, typed grants and the allow-list and delegation records read at the target's tip. It did not build two parts of its Outcome, because the story gave no mechanism for them:

- the store and protocol read "from the promoted base outside the worktree";
- a digest pin of the protocol a case runs under.

Today the case keeps a copy of `protocol.yaml` and `protocol.ir.json` (`crates/takt/src/case.rs:1849-1850`), and a resume refuses a different IR (`crates/takt-cli/src/main.rs:1526-1545`). Nothing ties that copy to the promoted base.

## Scope

- `crates/takt/src/case.rs`: the protocol copy at open, a digest beside it (inferred)
- `crates/takt-cli/src/main.rs`: the run's protocol load and the resume check (inferred)
- `crates/takt/src/frontier.rs`: the store read (inferred)

## Acceptance

- A case opened while the worktree's `protocols/` differs from the promoted base runs under the base's protocol, and its record names the digest.
- A case whose kept protocol copy is edited between steps refuses to resume, naming the digest it expected.
- The store read for the next story ignores a store edit in the worktree that the base does not hold.
