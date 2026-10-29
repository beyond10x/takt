---
format: aep.planning-md/3
id: task:ess-0-77-0-and-loom-0-50-1
kind: task
status: active
title: ESS moves to 0.77.0 and Loom to 0.50.1
summary: the specification, CI and the image use ESS 0.77.0; loom-sdk at 0.50.1
tags:
- dependency
relations:
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:22:44Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T21:22:44Z", actor: "human:timo", revision: 3}
---
## What

ESS 0.77.0 and Loom 0.50.1 are released. takt pins ESS only as a command-line tool:
`ess/ess-inputs.yaml` requires ess 0.77, and `check.yml`, `darwin.yml` and the `Dockerfile` set
`ESS_VERSION` 0.77.0 with the SHA256 of the linux x86_64 and macOS aarch64 archives from the
release's `SHA256SUMS`. `b10x-loom-sdk` moves from tag 0.50.0 to 0.50.1 (libz-sys 1.1.31 and
ESS 0.77.0 on Loom's side), with `Cargo.lock` updated by `cargo update -p b10x-loom-sdk`.

## Check

```expected_before
command: grep -qx 'requires: ess 0.77' ess/ess-inputs.yaml && grep -q 'ESS_VERSION: "0.77.0"' .github/workflows/check.yml && grep -q 'tag = "0.50.1"' Cargo.toml && { echo "ess 0.77, loom 0.50.1"; exit 0; } || { echo "older pins"; exit 9; }
exit: 9
healthy_exit: 0
output: older pins
```

## Acceptance

1. The predicate above exits 0.
2. `ess specify validate --path ess` with ess 0.77.0 says the specification is valid.
