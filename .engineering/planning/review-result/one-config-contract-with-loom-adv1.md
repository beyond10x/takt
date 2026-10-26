---
format: aep.planning-md/3
id: review-result:one-config-contract-with-loom-adv1
kind: review-result
status: active
title: Adversarial review, story one-config-contract-with-loom, after its commit
tags:
- loom-agent
relations:
- reviews: story:one-config-contract-with-loom
revision: 1
---
needs-fix
Adversarial review of story:one-config-contract-with-loom on wave/37 at fee81cba: `RunConfig::read` for TOML, YAML and `loom.agent/1`, the entrypoint, README § Container. Five red cases written (`crates/takt/tests/run_config_adv1.rs`); four green after 7d774c0d, one kept ignored with its reason. Attacked without a break: no value echoed for unknown keys, token-like key names, `!!binary`, syntax errors, multi-document streams, another `format`, a list or scalar `takt`; merge keys refused; alias bombs refused; quoted numbers refused; directories and upper-case extensions refused; line numbers for type errors and unknown keys; relative `endpoint_file` beside the file; flags over the file.

```findings
[
{"file":"crates/takt/src/run_config.rs","line":20,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a loom.agent/1 member other than takt was not checked against Loom's thirteen, so run fields beside format or a misspelt Takt were dropped; fixed, such a member is refused"},
{"file":"crates/takt/src/run_config.rs","line":100,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"YAML string fields accepted a number or boolean; fixed, YAML is typed as strictly as TOML"},
{"file":"crates/takt/src/run_config.rs","line":79,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a YAML tag was dropped and the value read; fixed, a tag is refused as Loom refuses one"},
{"file":"crates/takt/src/run_config.rs","line":77,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a duplicate YAML key is refused at the line its mapping starts, not the duplicate's; left, the case is kept ignored with the reason"},
{"file":"crates/takt/src/run_config.rs","line":86,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a loom.agent/1 document with a byte order mark and a takt member was refused; Loom writes none; fixed anyway, a leading mark is read past"},
{"file":"AGENTS.md","line":59,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"AGENTS.md named /config/run.toml after the entrypoint moved to /config/agent.yaml; fixed"},
{"file":"README.md","line":135,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"undecided","message":"Loom 0.42.0 reserves /config for its ConfigMap, so /config/endpoint.toml exists only under docker run; the README now gives the --endpoint-* flags with --endpoint-token-env for Loom"}
]
```
