---
format: aep.planning-md/3
id: review-result:endpoint-credential-from-environment-adv1
kind: review-result
status: active
title: Adversarial review, story endpoint-credential-from-environment, before its commit
tags:
- loom-agent
relations:
- reviews: story:endpoint-credential-from-environment
revision: 1
---
needs-fix
Verdict: NEEDS-CHANGE. Cases executed 35 -> 41, 3 red, all introduced by the unit. Tests left in
`crates/takt-cli/tests/endpoint_token_env_adv1.rs` and `crates/takt/tests/endpoint_token_env_adv1.rs`.

| # | sev | finding | decision |
|---|---|---|---|
| F1 | blocker | on the anthropic-messages wire the staged key reaches b10x-harness 0.18.0 as `--oauth-token-file`, sent as `authorization: Bearer` with the OAuth beta header, not `x-api-key` (observed against a local listener) | refuse an environment credential on anthropic-messages before any case opens until the session executor passes an API-key file; openai-responses stays admitted |
| F2 | warning | SIGTERM or SIGINT leaves the staged file; cleanup is in `Drop` only | a signal handler unlinks the staged paths and re-raises |
| F3 | warning | `remove_var` leaves the value in takt's initial environment block, readable from `/proc/<pid>/environ` by the harness session | scrub the bytes, or state the limit exactly |
| F4 | note | under `--confinement substrate` outside a delegated cgroup the variable is passed on to `systemd-run` and the re-run takt | stated in the flag help and the specification |

Held: the staged file passes the credential reader under `$XDG_RUNTIME_DIR` and `/dev/shm`; the test
command sees neither the variable nor the file; no value or path reaches case files or output;
empty, unset or misnamed variables and `env` beside `token_file` or `secret` are refused; staging
refuses symlinks and creates exclusively.

```findings
[
{"file":"crates/takt/src/harness.rs","line":308,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the staged API key reached b10x-harness as --oauth-token-file, sent on anthropic-messages as an OAuth Bearer instead of x-api-key; fixed, an environment credential is admitted on openai-responses only"},
{"file":"crates/takt/src/harness.rs","line":2112,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"SIGTERM or SIGINT left the staged key file; fixed, a sigaction handler unlinks it and re-raises"},
{"file":"crates/takt-cli/src/main.rs","line":1680,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"remove_var left the value in the startup environment readable from /proc/<pid>/environ; fixed on glibc by scrubbing the bytes, the limit on other C libraries stated"},
{"file":"crates/takt-cli/src/main.rs","line":1485,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"under --confinement substrate outside a delegated cgroup the variable passes to systemd-run and the re-run takt; stated in the help and the specification"}
]
```
