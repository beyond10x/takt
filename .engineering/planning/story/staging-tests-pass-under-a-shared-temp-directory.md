---
format: aep.planning-md/3
id: story:staging-tests-pass-under-a-shared-temp-directory
kind: story
status: draft
title: The token staging tests pass under a temporary directory without the sticky bit
summary: three endpoint_tests fail when /tmp is mode 2777, as a container volume is (second kind-cluster trial)
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 2
---
## Outcome

The endpoint token staging tests pass when the system temporary directory is writable by others
without the sticky bit, as a container's `/tmp` volume can be, so a repository checked out in
such a container is not red before any change.

## Context

- The second kind-cluster trial, 2026-10-26 (takt 0.13.1 as a `LoomAgent` on a local kind cluster, `/tmp` an empty-directory
  volume of mode 2777): every `tests.run` of runs 6 to 8, at d6516a7a (the base e5499f76 and the take) and at each later revision,
  ended `126 passed, 4 failed` (`cargo test --quiet --locked -p b10x-takt --lib`), the four in
  `harness::endpoint_tests`.
- In the same pod, outside takt's test namespace, at the case's head 8f5128f6:
  `staging_writes_a_protected_file_under_the_first_passing_candidate_and_drop_removes_it`,
  `staging_under_a_linked_ancestor_uses_the_resolved_path` and
  `a_staged_token_reaches_the_session_as_its_token_file_and_is_recorded_by_name` fail with
  `TMPDIR` unset (127 passed, 3 failed); with `TMPDIR` a 0700 directory under `/dev/shm` (1777)
  all 130 pass.
- The tests' scratch directories sit under `std::env::temp_dir()`, whose ancestor the staging
  rule refuses (writable by group or others, no sticky bit).

## Acceptance

1. With `TMPDIR` unset and `/tmp` of mode 2777, `cargo test -p b10x-takt --lib
   harness::endpoint_tests` passes.
2. The staging rule itself is unchanged: a candidate under a directory writable by others without
   the sticky bit is still refused.
