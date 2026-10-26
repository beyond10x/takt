---
format: aep.planning-md/3
id: review-result:image-harness-0-20-0-adv1
kind: review-result
status: active
title: Adversarial review, story image-harness-0-20-0 and the wave review fixes, after their commits
tags:
- loom-agent
relations:
- reviews: story:image-harness-0-20-0
revision: 1
---
needs-fix
Adversarial review of story:image-harness-0-20-0 (35873d90) and of the wave review's fix commit (18fc7de6) on wave/37, read at 18fc7de6 with nothing run, because a `task check` held the shared target. Attacked without a break: one metaharness (0.19.0), one set of harness crates (0.20.0, the tag's commit) and loom 0.43.0 in both lockfiles; the Dockerfile SHA256 against the release's SHA256SUMS; the flags loom's session executor and metaharness-b10x 0.19.0 pass for each credential kind, all accepted by b10x-harness 0.20.0; no loom 0.42.0 or 0.43.0 breaking item on a type takt names; the signal handler's calls async-signal-safe, "killed by signal" kept when takt is not PID 1; no lossy YAML-to-JSON step besides `.inf` and `.nan`. Fixed in b8fa404c unless marked left.

```findings
[
{"file":"CHANGELOG.md","line":21,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the loom 0.43.0 line was not Breaking although --harness harness now admits only b10x-harness 0.20.0; fixed, marked Breaking with the requirement"},
{"file":"crates/takt-cli/tests/endpoint_token_env_adv1.rs","line":306,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a doc comment lost its first line; fixed"},
{"file":"crates/takt/src/harness.rs","line":2191,"category":"contract-drift","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the on_signal module doc omitted the PID 1 exit; fixed"},
{"file":"Dockerfile","line":54,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"takt was the image's PID 1 with a handler only while a token is staged, so docker stop otherwise ended in SIGKILL; fixed, the entrypoint runs takt under tini"},
{"file":"docs/design/takt.md","line":25,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"the design named b10x-canon-engineering 0.9.0; fixed, 0.10.0"}
]
```
