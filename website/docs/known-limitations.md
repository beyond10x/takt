---
title: Known limitations
sidebar_position: 4
description: What takt 0.14.0 does not do yet, what a run on a cluster still needs by hand, and which open story addresses each.
lede: Each limitation below was met in a real run of takt as a Loom agent, and each takt one is an open story in the planning store.
source: .engineering/planning/story (status active and draft, epic takt-as-a-loom-agent); CHANGELOG.md 0.12.0 to 0.14.0; Loom CHANGELOG 0.44.0
---

# Known limitations

takt 0.14.0 runs as a container and as a Loom `LoomAgent`. Running it that way on a kind cluster
showed the gaps below. Each takt gap is a story in the
[planning store](https://github.com/beyond10x/takt/tree/main/.engineering/planning/story); the
[status page](/docs/status) lists the stories of the open wave. None of the fixes listed here is
shipped in 0.14.0.

Fixed since 0.14.0, for the next release: the run configuration file takes `story`,
`endpoint_token_env` and `state_dir`, and `takt scenario run` takes `--config`; `--state-dir` names
where a run keeps its transcripts and what a discarded step sets aside, and a `loom.agent/1`
document's `runtime.state_dir` is the default for it; a new case opens under `.takt/cases`, ignored
by git, and a case under `.engineering/cases` is still resumed where it is; a judgement step whose session names no action is tried again,
3 tries at most; the first `tests.run` in a fresh `HOME` is not refused for the cargo cache; the
token-staging tests pass under a shared temporary directory; and the step after a failed gate is
told the failing lines, unresolved citations included.

## takt

| limitation | what to do today | planned fix |
|---|---|---|
| A duplicate key in a YAML run configuration is refused at line 1, the line its mapping starts, with the general message of what the file may hold. TOML names the duplicate's own line. | Read the file for a repeated key. | YAML names the duplicate's line and key, as TOML does |
| A file the repository's git excludes (`.git/info/exclude`) still counts as a change a step made. | Keep a deployment's own files out of the checkout. | git-excluded files are not step changes |
| Resuming an intent case without `--case`, in a repository with no planning store, is refused with "has no planning store". | Name the case: `takt run --case <id>`. | none planned yet |

## Running under Loom

Loom 0.44.0 closed most of what a cluster run needed by hand: a `Command` resource now runs once
as a Job and reports its exit code in `status.run`, `spec.env` carries plain variables,
`spec.configFiles` carries files into `/config`, and the pod's `/tmp` and `HOME` are directories
of mode 0700. The [deploy guide](./guides/deploy-on-kubernetes.md) uses all of them. What is left:

| limitation | effect |
|---|---|
| Loom renders every agent pod with `seccompProfile: RuntimeDefault` and has no field to change it; a Job's pod template cannot be patched once the Job exists. | takt runs the test command in a user and mount namespace of its own. Under `docker run`, both the default seccomp and the default AppArmor profile refused that namespace, and the run ended on the error (exit 5) at its first `tests.run`, before any model call. The same is expected under Loom; it was read from the rendered pod, not observed on a cluster ([deploy guide](./guides/deploy-on-kubernetes.md#what-still-stops-it)). |
| Loom's agent document declares no `takt` member, and Loom refuses a member it does not declare. | The run's settings go in `spec.command.args` or in a file of `spec.configFiles`, not in `spec.agent`. |
| `HOME` is an emptyDir. | A change to the resource replaces the pod and loses takt's state and transcripts (see above); a state directory on a kept volume holds them ([the state directory](./reference/configuration.md#the-state-directory)). |

## Not takt's to fix

takt reads tests, gates and CI as evidence and does not repair them. A repository whose own gates
fail for reasons a step cannot see (a toolchain missing in the image, a test that depends on the
host) stops the run on those gates: since 0.14.0, a base that already fails its tests stops at
open with `RedBase`, before any model call ([how a run ends](./concepts/how-a-run-ends.md)).
