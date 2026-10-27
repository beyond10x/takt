---
format: aep.planning-md/3
id: review-result:config-proxy-parallel-round-2
kind: review-result
status: active
title: 'Parallel-safety critic, round 2: config-proxy drafts'
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
approve

**What you read.** 6 artifacts, each in full with `aep plan artifact show <id>`. I also ran `aep plan artifact waves --kind story --status draft --format json`, `aep plan artifact relations`, and `show` on story:endpoint-credential-from-environment and story:one-config-contract-with-loom, which the bodies cite. The six land in waves 1, 9, 13, 15, 16 and 17 in the order llm-section, model-names, run-flags, one-run-file, harness-section, harness-flags. No two of them share a wave.

**Surfaces.** The surfaces of 6 of 6 items are cited. Two scope entries are inferred, `crates/takt/tests` on story:model-names-an-llm-route and `crates/takt-cli/tests` on two others, and the bodies cite `crates/takt-cli/tests/` in their Scope sections. None of the 6 is unplaceable. Every pair that shares a file is ordered by a `depends_on` chain, directly or transitively:

| Shared surface | Items | Ordering |
|---|---|---|
| `ess/ess-inputs.yaml` | all six | chain llm-section, model-names, run-flags, one-run-file, harness-section, harness-flags |
| `website/docs/reference/configuration.md` | five (not one-run-file) | same chain |
| `ess/domains/takt.yaml` | llm-section, model-names, harness-section | model-names depends on llm-section; harness-section depends on model-names |
| `crates/takt-cli/src/main.rs` | model-names, run-flags, harness-section, harness-flags | model-names, run-flags, one-run-file, harness-section, harness-flags |
| `Cargo.toml` and `Cargo.lock` | llm-section, one-run-file, harness-section | llm-section, model-names, run-flags, one-run-file, harness-section |
| `crates/takt/src/run_config.rs` | llm-section, harness-section | transitive through model-names |
| `crates/takt/src/harness.rs` | model-names, harness-section | direct edge |
| `crates/takt-cli/tests/run_config_cli.rs` | llm-section, one-run-file | direct edge |

The Order lines in the bodies also name the shared files as their reasons for the ordering.

**Readers beside producers.** I traced 8 acceptance reads to a producer in the set, and a `depends_on` chain records all 8. Seven have a direct edge and one runs through two edges:
- model-names reads the parsed llm section (direct edge to llm-section).
- run-flags reads `llm_digest` and the endpoint (transitive through model-names).
- one-run-file reads `llm_digest` and the endpoint (direct edges to llm-section, model-names and run-flags).
- harness-section's refused endpoint keys point at model-names (direct edge).
- harness-flags reads the stub harness binary and the mapping from harness-section (direct edge).

**What you could not establish.**
- Out of my lane: story:harness-section-is-metaharness-session-request waits on two upstream stories in metaharness and Loom that are not in the set, and I did not assess them.
- Other draft stories in the store (for example story:run-records-harness-model on `crates/takt/src/harness.rs`) are outside the set and I did not assess them.

```findings
[]
```
