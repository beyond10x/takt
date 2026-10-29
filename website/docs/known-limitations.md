---
title: Known limitations
sidebar_position: 4
description: What takt 0.21.0 does not do yet, most important first, what to do today, and which open story addresses each.
lede: The largest gap first; each takt gap is an open story in the planning store.
source: docs/runs/2026-10-29-self-run-story-delivery.md; crates/takt-cli/tests/local_forge_cli.rs; takt run --help of 0.21.0; metaharness 0.23.0 (PINNED_VERSIONS of metaharness-claude); runs of a build of 0.21.0's source, 2026-10-29 (HarnessUnqualified with Claude Code 2.1.312; an intent case resumed without --case); .engineering/planning/story (story:duplicate-key-reader-limits and its decisions of wave 44); crates/takt/src/run_config.rs (duplicate_yaml_key); Loom CHANGELOG 0.44.0; CHANGELOG.md
---

# Known limitations

What takt 0.21.0 does not do yet, the largest gap first. Each takt gap is a story in the
[planning store](https://github.com/beyond10x/takt/tree/main/.engineering/planning/story); the
[status page](/docs/status) lists the open wave. What each release fixed is in the
[changelog](https://github.com/beyond10x/takt/blob/main/CHANGELOG.md).

## Delivery through a hosted forge is not proven

No recorded run has yet taken a story through a hosted [forge](./concepts/glossary.md#forge) to a merge:
open the pull request, read its CI and merge it under a grant.

| limitation | what to do today |
|---|---|
| On 2026-10-29 takt delivered one of its own stories, `story:duplicate-key-reader-limits`, up to the pull request: five runs in a row on `story.delivery/3`, with Codex and `--forge none`. Each enabled the story's three ignored tests, edited the code, passed the tests and the repository's five gates, handed the change over and stopped `Unperformable`, exit 7. They used development builds that lacked part of what 0.19.0 released. Record: [`docs/runs/2026-10-29-self-run-story-delivery.md`](https://github.com/beyond10x/takt/blob/main/docs/runs/2026-10-29-self-run-story-delivery.md). No record under [`docs/runs/`](https://github.com/beyond10x/takt/tree/main/docs/runs) shows a merge through `--forge bot` or `--forge gitlab`. | Take the printed `candidate: <commit>` (`refs/takt/candidates/<case>`) and open its pull request yourself. |
| `--forge local`, a bare repository on this machine as the forge, has one record: an end-to-end test of the binary, [`local_forge_cli.rs`](https://github.com/beyond10x/takt/blob/main/crates/takt-cli/tests/local_forge_cli.rs). With `--harness none` and the work committed by the test, the run opens the request, runs `local_checks`, stops `ApprovalRequired`, exit 4, and after `takt approve` merges into the hub's `main` and records `pr.merged`, exit 0. No record shows a run with a harness delivering a story through it. | Try it with `--harness none` first: [Run a story](./guides/run-a-story.md#on-this-machine-the-local-forge). |
| In four of those five runs, the Codex session said it could not run the format and specification checks: `ess` was unavailable and rustup's home could not be written. Only the gate ran them, so a format failure cost one more edit and one more gate run. | Expect the first gate to find formatting under Codex; the gate's output reaches the next step's prompt. |

## What runs unconfined

| limitation | what to do today |
|---|---|
| The test command runs on the host as you, unconfined, by default (`--confinement none`). It can read whatever you can. The run's `case` line says so. | Use `--confinement substrate`: no network, read-only toolchains, or a refusal by name ([Confinement](./guides/install.md#confinement)). |
| No operating-system sandbox holds the [harness](./concepts/glossary.md#harness) session, under either confinement. It works in a scratch copy of the tree, with an allowlisted environment, and Claude Code is offered only file tools. | Read what each step did: the console lists its tool calls and the diff it committed ([Getting started](./getting-started.md#2-a-run-with-claude-code)). |

## Claude Code versions

| limitation | what to do today |
|---|---|
| takt 0.21.0 runs Claude Code 2.1.304, 2.1.305 and 2.1.309 only. Any other version, newer ones included, stops the run `HarnessUnqualified`, exit 1, before any model call. | Keep a qualified binary at a path of your own and name it with `--harness-program` ([Claude Code's version](./guides/install.md#claude-codes-version)). |

## takt

| limitation | what to do today | planned fix |
|---|---|---|
| Resuming an intent's case without `--case`, in a repository with no planning store, is refused: `has no planning store: no .engineering/project.yaml`. | Name the case: `takt run --repo . --case <case>`. | none planned yet |
| A file the repository's git excludes (`.git/info/exclude`) still counts as a change a step made. | Keep a deployment's own files out of the checkout. | git-excluded files are not step changes |
| A key held twice in a YAML run file is refused at the duplicate's line, except in three cases, which are refused at the line the mapping starts: a duplicate after a key the reader cannot resolve, a key equal to one it passed over as an alias of a mapping or a sequence, and the keys `-0.0` and `0.0`. The file is refused in every case; only the line and the key are lost. | Read the file for a repeated key. | none planned yet |

## Running under Loom

Loom 0.44.0 closed most of what a cluster run needed by hand: a `Command` resource runs once as a
Job and reports its exit code in `status.run`, `spec.env` carries plain variables,
`spec.configFiles` carries files into `/config`, and the pod's `/tmp` and `HOME` are directories
of mode 0700. The [deploy guide](./guides/deploy-on-kubernetes.md) uses all of them. No cluster
run of takt 0.21.0 is on record. What is left:

| limitation | effect |
|---|---|
| Loom renders every agent pod with `seccompProfile: RuntimeDefault` and has no field to change it; a Job's pod template cannot be patched once the Job exists. | Up to 0.14.0 the test command ran in a user and mount namespace of takt's own, which the default seccomp and AppArmor profiles refused under `docker run` (exit 5). In 0.21.0 the default `--confinement none` starts no namespace of takt's own; whether `--confinement substrate` runs under that pod is not recorded ([deploy guide](./guides/deploy-on-kubernetes.md#what-still-stops-it)). |
| Loom's agent document declares no `takt` member, and Loom refuses a member it does not declare. | The run's settings go in `spec.command.args` or in a file of `spec.configFiles`, not in `spec.agent`. |
| `HOME` is an emptyDir. | A change to the resource replaces the pod and loses takt's state and transcripts; a state directory on a kept volume holds them ([the state directory](./reference/configuration.md#the-state-directory)). |

## Not takt's to fix

takt reads tests, gates and CI as evidence and does not repair them. A repository whose own gates
fail for reasons a step cannot see (a toolchain missing in the image, a test that depends on the
host) stops the run on those gates. A base that already fails its tests stops at open with
`RedBase`, before any model call ([How a run ends](./concepts/how-a-run-ends.md)).
