---
format: aep.planning-md/3
id: story:a-harness-rate-limit-and-an-expired-token-stop-with-their-own-reason
kind: story
status: draft
title: A harness rate limit and an expired login stop with their own reason, not exit 5
summary: 'a rate-limited session and an expired token both end the run as stopped: error:, exit 5'
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-harness-rate-limit-and-an-expired-token-stop-with-their-own-reason

## What is wrong

A harness session that ends because the model provider answered with a rate limit, or because the harness's login
token expired, stops the run as `stopped: error:`, exit 5 (`crates/takt-cli/src/main.rs`, `EXIT_ERROR`), the code every
failed forge, validator or harness shares. An operator or an orchestrator driving takt cannot tell from the exit code or
the stop line whether to wait and resume, renew the login, or look for a defect; and the 3 retries of a judgement step
spend sessions against a limit that has not lifted.

Reported by a takt trial on 2026-11-06: a rate-limited session and an expired token both ended the run with
exit 5.

## Outcome

A rate limit and an expired or rejected login are distinct stops with their own stop names, exit codes documented in
`takt run --help`, and a stop line that names what to do (resume after the limit's reset when the provider names one;
renew the login). Neither is retried as a judgement step without an action, and neither is charged as a failed
attempt.

## Acceptance

- A fake harness session that reports a rate limit stops the run with its own stop name and exit code, the reset time
  in the line when reported, and no retry: a test.
- A fake harness session that reports an expired or rejected login stops with its own stop name and exit code, naming
  the login to renew and no credential value: a test.
- `takt run --help` lists both exit codes; the case stays open and resumes.
