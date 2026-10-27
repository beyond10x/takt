---
format: aep.planning-md/3
id: approval-record:cfgproxy-end-wave-review-and-conformance
kind: approval-record
status: accepted
title: Records the end-of-wave reviews of the run-config epic and the open conformance acceptance
tags:
- wave-approval
relations:
- decides: epic:run-config-proxies-llm-and-metaharness
revision: 3
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T11:56:46Z", actor: "human:timo", revision: 3}
---
Decider: the wave's coordinator, under D84 (end-of-wave reviews, fixes, push, closing report). Recorded here because the
review-result artifacts are immutable and carry only their titles.

Reviews on origin/integration/takt 9886a6ca (read-only; findings in the scratch files, not the repository):

- Security (review-result:security-end-wave-9886a6ca): APPROVE-WITH-FIXES, no blocker, 1 major (SEC-1, no test measured an
  env: route credential reaching no child), 5 minor (SEC-2 non-UTF-8 names, SEC-3 other endpoints' env: variables reaching
  the test command, SEC-4 an unknown model name on stderr, SEC-5 an unbounded llm file read after the check, SEC-6 an
  unknown harness name echoed, pre-existing). Fixed at 6e4ff569; SEC-1 has a measuring test.
- Adversary (review-result:adversary-end-wave-9886a6ca): APPROVE-WITH-FIXES, no blocker, 1 major (ADV-1), 7 minor
  (ADV-2 to ADV-8). ADV-2, ADV-5, ADV-6, ADV-8 fixed at 6e4ff569; ADV-3 and ADV-4 are docs and ESS prose (the docs pass);
  ADV-7 (a named pipe hangs the endpoint and run file reads, pre-existing) is a follow-up, not fixed in this wave.

Conformance acceptance, open (decided here): story:llm-section-is-an-llm-routes-document, story:model-names-an-llm-route,
story:run-flags-spell-metaharness-and-llm and story:one-run-file-for-host-and-loom-agent are implemented on cargo-test
evidence. Their acceptance names conformance scenarios run by task check; ess/ess-inputs.yaml has no scenarios and no
runner exists. The lifecycle does not allow implemented back to active (only archived), so the four stories stay
implemented, each with a Conformance acceptance section saying the item is open. The runner is a later wave. This is the
decision; it is recorded, not hidden.
