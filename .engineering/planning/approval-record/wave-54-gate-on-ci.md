---
format: aep.planning-md/3
id: approval-record:wave-54-gate-on-ci
kind: approval-record
status: approved
title: 'Wave 54: the gate without a local test-library stage'
summary: test-cli counts green with the host's tunl0 gap; test-library runs on the pull request's CI
relations:
- decides: story:test-cmd-checked-before-open
- decides: story:gate-flag-names-a-protocol-action
- decides: story:resume-names-a-missing-branch
- decides: story:a-git-read-error-leaves-no-process
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-05T22:39:26Z", actor: "human:timo", revision: 2}
---
# Approval: wave 54's gate without a local test-library stage

## Decision

The owner's standing delegation, 2026-11-05, through the orchestrator:

- `task test-cli` counts as green. It ran at dc64f9833 with 938 passed and 1 failed. The one failure,
  `finding_scripts::the_predicate_reaches_no_network_ipc_or_socket_of_the_host`, comes from the host, as in wave 53:
  the machine has the `ipip` module loaded, so every new network namespace holds `tunl0` beside `lo`. No commit of the
  wave touches that case or `scripts/`, and it passes on CI.
- `task test-library` does not run locally. Its named peak of 44G and its start floor of 84G free do not fit the host
  while other sessions build there. The repository's delivery rule allows this: where the full suite does not fit the
  disk budget, the touched crates and the projection checks run locally, and CI's full gate runs on the pull request.
  `task check-static` ran green, the units ran their targeted tests in both crates, and the pull request's CI runs
  the whole suite, the library's included.
