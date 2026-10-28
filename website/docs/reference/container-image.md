---
title: Container image
sidebar_position: 7
description: What the takt container image holds, the user it runs as, its entrypoint and mounts, and what it cannot do.
lede: The image is one takt run on a mounted repository and a mounted configuration, with the tools and the Rust toolchain that run needs, as an unprivileged user.
source: Dockerfile (ARG pins, stages, ENTRYPOINT); CHANGELOG.md 0.12.0 to 0.14.0; crates/takt/src/run_config.rs; a local build of the Dockerfile at 0.13.1 (docker inspect, tool versions), run with and without the default seccomp profile; Loom 0.44.0 crates/loom-operator/src/render.rs and admission.rs
---

# Container image

`Dockerfile` in the repository builds an x86_64 image of takt from the source tree. No workflow
of this repository builds or pushes an image, and none is published: you build it, and a cluster
loads your build.

```console
$ docker build --build-arg REVISION=$(git rev-parse HEAD) -t takt:0.17.0 .
```

Running it: [Run in a container](../guides/run-in-a-container.md). On Kubernetes as a Loom agent:
[Deploy on Kubernetes](../guides/deploy-on-kubernetes.md).

## What is in it

| layer | what |
|---|---|
| base | Debian trixie slim (`docker.io/library/debian:trixie-slim`) |
| packages | `ca-certificates`, `git`, `gcc`, `libc6-dev`, `pkg-config`, `libssl-dev`, `python3`, `tini` |
| takt | `/usr/local/bin/takt`, built `--release --locked` from the tree |
| tools | `aep`, `b10x-harness`, `ess`, `b10x-gates` (the `gates` scan adapter's program and the `bot` forge's) in `/usr/local/bin` |
| Rust | the toolchain of `rust:1.99-slim-trixie` with `clippy` and `rustfmt`, in `/opt/rust`, linked into `/usr/local/bin` |

The tools are downloaded by release URL and checked against a SHA256 in the Dockerfile:

| tool | version |
|---|---|
| [aep](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep)) | 0.77.1 |
| [b10x-harness](https://beyond10x.github.io/harness/) ([GitHub](https://github.com/beyond10x/harness)) | 0.20.0 |
| [ess](https://beyond10x.github.io/ess/) ([GitHub](https://github.com/beyond10x/ess)) | 0.74.0 |
| b10x-gates | 0.1.23 |

`b10x-harness` 0.20.0 is the harness takt 0.14.0's Loom 0.43.0 and metaharness 0.19.0 need. Checked
in a local build of this Dockerfile (0.13.1, with ess 0.69.2; only the ess pin has changed since,
and that is not rebuilt here):

```console
$ docker run --rm --entrypoint sh takt:0.13.1 -c 'takt --version; aep --version; b10x-harness --version; ess --version; b10x-gates --version; rustc --version'
takt 0.13.1 (02a680699990abb1e0eaed7e16e7c2773030a5d4)
aep 0.77.1
b10x-harness 0.20.0
ess 0.69.2
b10x-gates 0.1.23
rustc 1.99.1 (58cf8f9c3 2026-10-13)
```

The image holds the toolchain a run tests with, so a repository that builds with cargo is
tested, linted and gated inside it. The Rust programs are linked into `/usr/local/bin`, where a
gate's system path finds them, and no `CARGO_HOME` is set: cargo keeps its caches in
`$HOME/.cargo`. There is no `rustup`, so a target repository's `rust-toolchain.toml` is ignored.
`/opt/rust` is `chmod -R go-w`: the run's user cannot write it, because the gates trust
`/usr/local/bin` and what it links to.

`pkg-config` and the OpenSSL headers are there so a repository that builds against OpenSSL is
tested. Python 3 is there for repositories whose tests are Python; takt records its module names
at case open ([case files](./case-files.md)).

## User, home and entrypoint

| setting | value |
|---|---|
| user | `10001:10001`, `takt` |
| `HOME` | `/var/lib/takt` |
| working directory | `/workspace` |
| entrypoint | `tini -- takt run --repo /workspace` |
| default argument (`CMD`) | `--config /config/agent.yaml`; any arguments after the image name replace it |

takt runs under `tini`, so a stop signal reaches a takt that is not PID 1: `docker stop` or a
pod's stop ends it on SIGTERM, not SIGKILL. Arguments after the image name replace the default
argument, so the configuration file is read only when they name it again. The arguments are flags
of `takt run` and win over the file:

```console
$ docker run --rm takt:0.17.0 --config /config/agent.yaml --story <story-id> --hours 1
```

A `LABEL` names the source (`org.opencontainers.image.source`), the licence (`Apache-2.0`) and the
build's `REVISION`. A build from a clone, where `.git` is a directory, names its commit in
`takt --version`; a build from a linked worktree says `unknown revision`.

## Mounts

| path | what | mode |
|---|---|---|
| `/workspace` | a git checkout of the repository to work on, its planning store included | read-write, owned by uid 10001 |
| `/config/agent.yaml` | the run's configuration: a `loom.agent/1` document whose `takt` member holds the fields of [the run configuration](./configuration.md), or those fields alone as YAML | read-only |
| `/config/endpoint.toml` | the endpoint file the configuration names, `endpoint_file: endpoint.toml`: [the endpoint file](./configuration.md#the-endpoint-file) | read-only |

The run's outcome is in `/workspace/.takt/cases` and the exit code
([exit codes](./exit-codes.md)). With `forge: none` the delivery stays a branch in `/workspace`.
Under the default `gates` scan adapter, the policy for `b10x-gates` comes from the deployment
(`B10X_GATES_POLICY`), never the image; a configuration that declares `scan: none` needs none.
`b10x-gates` reads it only when the file and its directory are owned by the run's uid, the file
has no group or other bits and the directory is not group- or other-writable.

### Under Loom

A `LoomAgent` of the `Command` variant gets `/config/agent.yaml` from its `spec.agent`, written
verbatim. Loom 0.45.0 runs the image once, as a Job. Loom keeps `/config` for its ConfigMap and
mounts it read-only. What reaches `/config/endpoint.toml` depends on the Loom release:

| Loom | endpoint |
|---|---|
| before 0.44.0 | nothing writes `/config/endpoint.toml`: the endpoint is the flags `--endpoint-base-url`, `--endpoint-wire openai-responses`, `--endpoint-model` and `--endpoint-token-env <NAME>` in `spec.command.args`, the variable from the agent's `spec.secretRef` |
| 0.44.0 and later | `spec.configFiles` writes `/config/<name>` verbatim, so an `endpoint.toml` holding `base_url`, `wire`, `model` and `[credential] env = "<NAME>"` is a config file; a credential in a name or content is refused |

A Loom release that does not admit a `takt` member in `loom.agent/1` refuses the resource, so
there the document carries no `takt` member and the run's settings are flags in
`spec.command.args`. Under Loom, `HOME` is `/var/lib/agent`, set by the pod, and `spec.workspace`
mounts the claim at `/workspace`. The variable `--endpoint-token-env` names comes from
`spec.secretRef`; `spec.env` refuses a credential.

## What it cannot do

- **One harness.** The image holds b10x-harness and no other: no Claude Code, no Codex. Set
  `harness: harness` (or `none`). The default, `claude`, is not there.
- **No forge client.** No `gh` and no `glab`. Set `forge: none`. The default follows `origin`, and
  `bot` and `gitlab` need those programs. The delivery stays a branch in `/workspace`.
- **Unconfined seccomp and AppArmor.** Under the default `--confinement none` of 0.13.1 the test
  command ran in a user and mount namespace of its own, which a container runtime's default seccomp
  and AppArmor profiles refuse. Under the default profile the first test run failed with a
  refusal, exit 5. Since then `none` starts it through Substrate's `local` adapter, in no
  namespace of takt's own, which has not been run in a container yet. The one that ran:

  ```console
  $ docker run --rm --user 1000:1000 -e HOME=/tmp -v ~/demo:/workspace -v ./agent.yaml:/config/agent.yaml:ro takt:0.13.1 --intent "add a README line"
  frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
  stopped: error: tests.run (takt.tests-run): run true: the test command runs in a user and a mount namespace of its own, where no login of the user's is in reach, and this host refuses unprivileged user namespaces (as with `kernel.apparmor_restrict_unprivileged_userns = 1` or `kernel.unprivileged_userns_clone = 0`); allow them, or run takt where they are allowed: run true: Operation not permitted (os error 1); steps 1, tokens 0, case /workspace/.takt/cases/<case>
  ```

  Run the container with both unconfined, and the same command passes:

  ```console
  $ docker run --rm --security-opt seccomp=unconfined --security-opt apparmor=unconfined ...
  step 1: tests.run {}
    effect: the test command exited 0: pass;
  ```

  In Kubernetes that is `seccompProfile` and `appArmorProfile` of type `Unconfined` on the pod.
  Loom 0.45.0 renders `seccompProfile: RuntimeDefault` on the pod of a `Command` resource and has
  no spec field for another (read in its `render.rs`; not run in a cluster here), so whether the
  test command runs there depends on the cluster's runtime default.
- **No registry.** Nothing is pushed.

The stop rules apply in the image as anywhere: a base whose tests are red stops `RedBase` before
any model call, exit 3 (`continue_past_red_base: true` goes on). A test command that cannot start,
as above, is an error, exit 5, not a red base.
