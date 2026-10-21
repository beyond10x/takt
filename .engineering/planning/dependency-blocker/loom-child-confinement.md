---
format: aep.planning-md/3
id: dependency-blocker:loom-child-confinement
kind: dependency-blocker
status: open
title: Loom 0.32.0 confines a child only to its end (gaps G1-G5)
summary: live child, standard input, whole output, read-only roots, host paths
tags:
- loom
relations:
- blocks: story:harness-child-confined
revision: 1
---
# Blocker: Loom 0.32.0 confines a child only to its end

story:harness-child-confined runs the harness session, git, the bot-route forge and `aep` under a Substrate profile through loom-sdk. The two gaps its body names are closed in Loom 0.32.0 (tag `0.32.0`, a238395b): loom-sdk exports `ChildCommand` (`crates/loom-sdk/src/lib.rs:84`), `SubstrateRunner::run_child` runs one (`crates/loom-intake-slice/src/confinement.rs:424`), and a child can reach one named egress aperture (`with_egress`, `:763`; one aperture per run, `:659`). `run_child` runs a child to its end and returns its result, which leaves five gaps. Paths below are in `crates/loom-intake-slice/src/` at 0.32.0 unless named otherwise.

| gap | Loom 0.32.0 | takt needs | blocks |
|---|---|---|---|
| G1, a live child | `run_child` waits for the end (`wait: true`, `confinement.rs:892`) and returns only the result | metaharness 0.11.0 reads lines from, writes to, kills and waits on a live child (`process.rs:93`, `107-111`, `215-260`); the session executor wraps `SpawnRunner` (`crates/session-executor/src/lib.rs:776-790`); git's long-lived `cat-file --batch` reader (`crates/takt/src/git.rs:626-650`) | the harness, git |
| G2, standard input | `ChildCommand` has no standard input (`confinement.rs:663-674`) | git is written to on standard input (`crates/takt/src/git.rs:90-101`, `184`, `633`, `2908`, `3071`) | git |
| G3, output | the last 8192 bytes of standard output, then of standard error, merged into one lossily decoded string (`confinement.rs:557-561`, `1370-1372`) | both streams whole and apart: `crates/takt/src/git.rs:113-160`, the forge (`crates/takt/src/forge.rs:1268-1274`), the transcript metaharness keeps (`process.rs:65-71`) | the harness, git, the forge, `aep` |
| G4, read-only roots | no read-only roots or files for a child (`read_only_roots: Vec::new()` at `confinement.rs:898`, `read_only_files` at `:899`, no setter among `:719-823`); the sandbox sees `/usr`, `/bin`, `/lib`, `/lib64`, `/proc`, `/dev`, a tmpfs `/tmp` and the workspace | the harness CLIs, `aep` and `b10x-gates` are installed under the user's home, the gates policy file too, and a linked worktree's `.git` points outside the workspace | the harness, `aep`, the forge |
| G5, host paths | the workspace is mounted at `/workspace` | host paths do not resolve inside: metaharness's working directory, decision channel, transcript and settings paths; takt's `--git-dir=<host path>` (`crates/takt/src/git.rs:282-287`) | the harness, git |

Not known: whether one bot-route push reaches both `api.github.com` and `github.com`, which matters while a run has one aperture.

Clearing this needs a Loom release that confines a live child (G1) with standard input (G2), both output streams whole (G3), read-only roots (G4) and host paths kept or mapped (G5). takt builds no substitute (`epic:takt-on-loom-sdk` § Out of scope).
