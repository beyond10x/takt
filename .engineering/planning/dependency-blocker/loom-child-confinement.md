---
format: aep.planning-md/3
id: dependency-blocker:loom-child-confinement
kind: dependency-blocker
status: open
title: Loom 0.46.0 confines a child but writes only inside the workspace
summary: several apertures and a working directory delivered; a writable root outside /workspace is not (Loom story:writable-host-root)
tags:
- loom
relations:
- blocks: story:harness-child-confined
revision: 8
---
# Blocker: Loom confines a child only partly as takt needs

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

## Against Loom 0.33.0 (2026-10-22)

Read at tag `0.33.0` (3d56bca9); C is `crates/loom-intake-slice/src/confinement.rs`, CH is
`crates/loom-intake-slice/src/confinement/child.rs`. Read, not probed.

| gap | 0.33.0 | what is left |
|---|---|---|
| G1 | closed: `spawn_child` (C:490); `write_stdin`, `read`, `signal`, `wait` (CH:195, 211, 220, 241) | a child is ended when 16 queued output frames go unread for 1 s (CH:113-115); a driver for another aperture or directory is refused while a launch runs (C:98-101), so every driver opens before the harness child starts |
| G2 | closed with a cap: `with_stdin` for `run_child` (C:1246); a live child takes 16 MiB of input over its life, in 64 KiB frames (CH:191-194, C:1011) | git's `cat-file --batch` reader (`crates/takt/src/git.rs:629-650`) is reopened before 16 MiB |
| G3 | mostly closed: `run_child` keeps each stream apart up to 1 MiB, flagged when cut (C:63-67, 2431); `spawn_child` frames are tagged by stream (CH:22-27) | output over 1 MiB is read live |
| G4 | closed: `with_read_only_root` (C:1074), `with_read_only_file` (C:1095), `at_host_path` (C:1113) | none |
| G5 | reads only: a host path resolves read-only (`at_host_path`, `sandbox_path`, C:1113, 1336); a child starts in `/workspace` and writes only there (C:974-976) | the harness session's working directory, its decision channel and config home, and a linked worktree's common git directory cannot be written |
| aperture | open: one aperture per run (C:966) | the bot route mints a token at `api.github.com` and pushes to `github.com` (gates at `main` e5fe0e5, `src/delivery.rs:461`, `:907-913`), so one push needs two hosts |

On 0.33.0: `aep` can be confined; git can be confined where `.git` is inside the workspace; the
forge and the harness session cannot.

Clearing this for 0.33.0: no. It needs a writable mount or working directory outside
`/workspace` (G5 writes) and more than one egress aperture per run, or the owner's decision to
confine only `aep` and git first.

## Decision on 0.33.0 (2026-10-22)

The orchestrator, under the owner's delegation: "A, taken now. Reason: B changes takt's exit
contract (4 -> 8, crates/takt/tests/free_disk.rs:509-528) and leaves confinement blocked. ...
Loom 0.34.0 carries the host_stop ordering; a substrate session now builds the writable
mount/workdir and several apertures".

Option A: this blocker stays open until a Loom release gives a child a writable mount or working
directory outside `/workspace` and more than one egress aperture per run.

## Against Loom 0.46.0 and Substrate 0.14.0 (2026-10-28)

Read at Loom tag `0.46.0`, which takt pins (`Cargo.toml`, `b10x-loom-sdk`), and Substrate's latest release 0.14.0. C is `crates/loom-intake-slice/src/confinement.rs` at 0.46.0. Read, not probed.

| what 0.33.0 left | 0.46.0 | left |
|---|---|---|
| one egress aperture per run | closed in Loom 0.37.0 (b75bd6c5): `ChildCommand::with_egress` takes up to `MAX_EGRESS_APERTURES` (4), Substrate's own limit (C:36-37, 1249) | none: a bot-route push reaches `api.github.com` and `github.com` |
| G5, a working directory outside `/workspace` | partly closed in Loom 0.37.0 (2b16c642): `with_workdir` (C:1428) starts a child at or beneath `/workspace` or a read-only root's mount | the directory is read-only unless it is in the workspace |
| G5, writes outside `/workspace` | open: `with_writable` (C:1200) names only directories of the workspace, and Loom asks Substrate for no writable root (`writable_roots: Vec::new()`, C:1572; C:1034 says Substrate 0.10.0 offers them) | Loom's story:writable-host-root, still `draft` in Loom's store |

The Substrate side is delivered: writable roots since 0.10.0, several apertures since 0.9.0. What is missing is Loom's: a `ChildCommand` that writes an operator-declared root outside the workspace, for the harness session's working directory, decision channel and config home, and a linked worktree's common git directory.

Clearing this for 0.46.0: no. It needs a Loom release that delivers story:writable-host-root. Until then the 0.33.0 reading stands for everything else: `aep` can be confined, git can be confined where `.git` is inside the workspace, and now the forge too (several apertures). The harness session cannot.
