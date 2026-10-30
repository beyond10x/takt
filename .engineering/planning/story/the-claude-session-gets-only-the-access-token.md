---
format: aep.planning-md/3
id: story:the-claude-session-gets-only-the-access-token
kind: story
status: draft
title: The Claude session gets only the access token, and the copy ends with it
summary: no refresh token in the staged login; removed on every exit, SIGKILL included
tags:
- finding
- priority:high
- trial-0.24.0
relations:
- informed_by: story:harness-child-confined
revision: 1
---
## Outcome

A `--harness claude` session is given only what it needs from the operator's login (the access
token and its expiry, never the refresh token), in a directory only the operator can read, and
that copy is gone when the session ends, also when takt or the session is killed with SIGKILL.

## Context

From the 0.24.0 trial cycle's confinement review:
- Verified with a made-up login: the session's config home under its `TMPDIR`
  (`claude-home/.credentials.json`, file 0600, directories 0755) holds `accessToken`,
  `expiresAt`, `refreshToken`, `scopes` and `subscriptionType`.
- Verified: three such copies of a real login, each with a refresh token, were still on disk in
  the shared temporary directory after their sessions; the scratch is removed when a step ends
  normally, so the leftovers come from sessions that did not (inferred).
- Runs of under an hour worked on a home holding the access token alone.

Where: takt asks for the operator login whenever `~/.claude/.credentials.json` exists
(`crates/takt/src/harness.rs:1006-1020` at 0.24.0), and the session launcher copies the whole
file before every spawn. takt's own harness endpoint already reads only
`/claudeAiOauth/accessToken` (`harness.rs:4413-4416`). story:harness-child-confined confines the
child but does not name the copied login.

```expected_before
command: during a --harness claude step, read the keys of the .credentials.json under the session's TMPDIR
output: a claudeAiOauth.refreshToken key is present
```

## Acceptance

1. During a `--harness claude` step the staged login holds `accessToken` and `expiresAt` and no
   `refreshToken`, in a 0700 directory.
2. After the step ends normally, on a timeout, on an error, and after takt is killed with
   SIGKILL (removed at the latest by the next run's start or by a parent-death cleanup), no copy
   of the login remains under the session's scratch.
3. A session whose access token expires mid-run fails naming the expiry, not with a hidden
   refresh; the documentation states the limit.
