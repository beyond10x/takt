---
format: aep.planning-md/3
id: review-result:protocols-are-engineering-protocols-built-ins-adv1
kind: review-result
status: active
title: Adversarial review, story protocols-are-engineering-protocols-built-ins, before its commit
tags:
- loom-agent
relations:
- reviews: story:protocols-are-engineering-protocols-built-ins
revision: 1
---
holds
Adversarial review of story:protocols-are-engineering-protocols-built-ins on wave/37 before its commit (base 9e4e3e84): the engineering-protocols 0.10.0 pin, `LoadedProtocol::builtin` without `EMBEDDED`, the deleted `protocols/*/1.yaml`, the tests and the protocol viewer. Four cases written (`crates/takt/tests/protocols_builtin_adv1.rs`), three red on pre-existing `builtin` behaviour, green after the dispatcher's fix in f92b5239; one assertion widened by the dispatcher because the registry lists `eval.gated_change` as `eval-gated-change`. Attacked without a break: byte identity with 0.12.0, recorded protocol ids, the default protocol, every reader of the deleted paths, both `--locked` lockfiles, the two registries in the graph, the viewer.

```findings
[
{"file":"crates/takt/src/protocol.rs","line":37,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"the fallback only swapped '.' for '-', so the recorded id eval.gated_change/1 was refused as --protocol; fixed, '_' is read as '-' too"},
{"file":"crates/takt/src/protocol.rs","line":38,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"map_err(|_| first) hid UnknownMajor, so story.delivery/2 was called an unknown name; fixed, the registry spelling's refusal is reported"},
{"file":"crates/takt-cli/build.rs","line":23,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"COMPILED_IN still watches protocols/ and its comment says the binary embeds protocol documents; left, it only over-reports a dirty revision"},
{"file":"crates/takt/src/protocol.rs","line":310,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the unit's byte-identity tests compared the built-in with its own registry; fixed by a sha256 pin of the 0.12.0 documents"}
]
```
