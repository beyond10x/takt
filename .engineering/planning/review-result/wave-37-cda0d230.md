---
format: aep.planning-md/3
id: review-result:wave-37-cda0d230
kind: review-result
status: active
title: Wave 37 review at cda0d230, every unit but the harness pin
tags:
- loom-agent
relations:
- reviews: story:image-runs-the-toolchain-it-tests-with
- reviews: story:gate-exec-finds-the-toolchain-in-the-image
- reviews: story:image-ships-b10x-gates
- reviews: story:image-cargo-home-is-writable
- reviews: story:endpoint-credential-from-environment
- reviews: story:one-config-contract-with-loom
- reviews: story:run-names-its-story
- reviews: story:protocols-are-engineering-protocols-built-ins
revision: 1
---
approve
Independent read of wave 37 at cda0d230 (`378159a6..cda0d230`), after every unit's adversary pass, for seams between the units and drift between code, README, CHANGELOG and the specification; read-only, nothing run, no case written, because a `task check` held the shared target. No blocker. Checked without a finding: the environment credential chosen as `EndpointConfig::of` chooses (flags over the file); the staging rule against the token-file reader behind harness 0.18.0; the token removed on every error path; the variable taken before any thread starts; `--story` refused beside `--intent`, `--case` and a turn; the three protocols byte-identical to engineering-protocols 0.10.0 at the commit the lock pins; gates 0.1.23 the newest tag; no personal path or host name in any added line. Fixed in 18fc7de6 unless marked left.

```findings
[
{"file":"crates/takt/src/harness.rs","line":2320,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"as the container's PID 1 takt's self-raised default-action signal is ignored, so SIGTERM deleted the staged token and the run went on without it; fixed, the handler unblocks, raises and _exits 128 plus the signal"},
{"file":"crates/takt/src/run_config.rs","line":46,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the JSON round trip in strict() read YAML hours .inf or .nan as unset; fixed, a number that is not finite is refused at the read, with a case"},
{"file":"README.md","line":186,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"README and CHANGELOG omitted the substrate exception to no started process seeing the variable; fixed"},
{"file":"CHANGELOG.md","line":20,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"takt protocols output changed spelling without a Breaking mark; fixed, marked Breaking"},
{"file":".engineering/planning/design/recursive-self-improvement.md","line":163,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the design named protocol files the wave deletes; fixed, it names the registry ids"},
{"file":"crates/takt-cli/src/main.rs","line":1396,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the tree is put back only on a refused --story, not when take_next_as errs after the base checkout; left, the specification claims refusals only"}
]
```
