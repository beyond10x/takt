---
format: aep.planning-md/3
id: review-result:wave-48-adversary-a
kind: review-result
status: active
title: Adversary, wave 48 unit a
relations:
- reviews: story:git-and-model-through-loopback-relays
revision: 1
---
approve

Adversary pass on unit a (the Loom 0.52.0 pin, 1b2cf797): 8 cases in crates/takt-cli/tests/w48_adv_a.rs, all green, merged into wave/48 at b7c34698. Every loopback form takt admits starts a keyless session; off-loopback and disguised hosts are refused before a case opens; a token file reaches the harness as a path only.

```findings
[
{"file":"crates/takt-cli/tests/loopback_relays_cli.rs","line":749,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the Acceptance 2 header clause is asserted only by ignored real-harness cases, so CI never checks it"}
]
```
