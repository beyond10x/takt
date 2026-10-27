---
format: aep.planning-md/3
id: story:a-run-touches-no-descriptor-it-inherited
kind: story
status: draft
title: A run touches no descriptor it inherited
summary: Inherited descriptors stay untouched by a run, its confined commands and promotion builds
revision: 2
---
# Story: a run touches no descriptor it inherited

## Outcome

A takt run, its confined test commands and its promotion builds leave every file descriptor they
inherited from their caller as it was: nothing reads from it, writes to it or closes it, and no
confined process receives it.

## Context

- On a hosted GitHub Actions runner the step shell inherits two descriptors of the runner's own
  worker channel (seen as 142, a pipe's read end, and 145, a pipe's write end). With the Substrate
  adapters serving there (`systemd` around `linux`), the test jobs lost their runner mid-run:
  "Found invalid data while decoding" in the worker's channel, and no log kept. Closing every
  inherited descriptor above 2 in the step before `task` ends that.
- Which of takt, its test doubles or Substrate uses the inherited descriptors is not known yet. A
  local run with an inherited FIFO holding known bytes kept all of them, so the path is one the
  hosted runner takes and a workstation does not.
- `check.yml` closes the descriptors as a workaround; this story removes the need for it.

## Acceptance

1. `inherited_fds::a_run_leaves_an_inherited_pipe_unread_and_unwritten`
2. `inherited_fds::a_confined_test_command_receives_no_inherited_descriptor`
3. `inherited_fds::a_promotion_build_receives_no_inherited_descriptor`
