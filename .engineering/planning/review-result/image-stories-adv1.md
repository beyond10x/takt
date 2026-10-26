---
format: aep.planning-md/3
id: review-result:image-stories-adv1
kind: review-result
status: active
title: Adversarial review, the four image stories, after their commit
tags:
- loom-agent
relations:
- reviews: story:image-runs-the-toolchain-it-tests-with
- reviews: story:gate-exec-finds-the-toolchain-in-the-image
- reviews: story:image-ships-b10x-gates
- reviews: story:image-cargo-home-is-writable
revision: 1
---
needs-fix
Adversarial review of the image stories on wave/37 at 93f3c8da (Dockerfile from dfd4b0d2, README § Container): an image built from `git archive 93f3c8da` and the probe image, run as uid 10001 with a read-only root and a tmpfs HOME, as Loom's Command variant runs it. No cargo case fits (every finding is a container run). Attacked without a break: `cargo`, `rustc`, `cargo fmt` and `cargo clippy` through gate-exec under `env -i`, the openssl-sys build, `cargo test`; plain binaries, not rustup proxies; nothing written to `.rustup` or `/tmp`; `b10x-gates 0.1.23` static, `scan-text` passes the clean file and refuses the planted one. After cda0d230 the probe image has `/opt/rust` 755 root and `touch /opt/rust/bin/x` is refused as uid 10001.

```findings
[
{"file":"Dockerfile","line":14,"category":"boundary","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the rust image leaves /opt/rust mode 777, so a step running as uid 10001 could replace cargo on gate-exec's trusted path (reachable under docker run without --read-only, not under Loom 0.42.0); fixed, chmod -R go-w /opt/rust in the toolchain stage"},
{"file":"README.md","line":150,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the image has no rustup, so a target repository's rust-toolchain.toml is ignored without a word; fixed, the README says so"},
{"file":"AGENTS.md","line":62,"category":"contract-drift","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the pin rule named aep, b10x-harness and ess but not b10x-gates; fixed, GATES_VERSION moves to the newest gates release"},
{"file":"Dockerfile","line":1,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"RUST_IMAGE and BASE_IMAGE are tags, not digests, and rustup component add downloads unpinned; left, every release download is checksum-pinned and the base tags follow Debian and Rust security patches"},
{"file":"README.md","line":147,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"b10x-gates refuses a policy in a directory another uid owns; fixed, the README states the ownership and mode it requires"}
]
```
