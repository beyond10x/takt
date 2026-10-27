---
format: aep.planning-md/3
id: story:the-command-forge-takes-a-request-number-only-from-the-request
kind: story
status: draft
title: The command forge takes a request number only from the request path
summary: no number from a URL that names no request
tags:
- priority:medium
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: the-command-forge-takes-a-request-number-only-from-the-request

## Outcome

Without `id_pattern`, the command forge takes a request number only from the request path segment its docs name; a URL that names no request (a "new request" page, a host with a port) yields no number and `pr.open` refuses instead of recording one.

## Context

- The B2 adversary review (2026-10-27): `crates/takt/src/forge/command.rs:442-451` takes the last number in any URL (2 from `project-2/-/merge_requests/new`, 8443 from a port), and `pr.open` records it without reading it back (`validator.rs:3213`).
- Off the exit-0 path of the trials, which use `--forge bot`; deferred to after the release (D105).
- Red test: `adv_b2_a_url_that_names_no_request_gives_no_number`, parked on branch `review/tr-b2-followup` in `crates/takt/tests/adv_b2_command_forge_followup.rs`.

## Acceptance

1. `adv_b2_command_forge_followup::adv_b2_a_url_that_names_no_request_gives_no_number` passes.
2. A recorded request number is read back through the `read` entry before `pr.opened` holds.
