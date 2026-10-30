---
title: Run in a container
sidebar_position: 6
description: Build the takt image, give it a repository and a configuration, run it with docker, and get past the two things a container's defaults refuse.
lede: The image is one takt run on a mounted repository. At 0.14.0 its test command needed seccomp and AppArmor unconfined and an existing cargo home; neither need has been measured again at 0.21.0.
source: Dockerfile at 0.14.0, its tool pins checked against the 0.21.0 Dockerfile; CHANGELOG.md 0.15.0 and 0.16.0; examples/loom-agent/agent.yaml and endpoint.toml (checked by crates/takt/tests/examples_config.rs); three docker runs of a local takt:0.14.0 build on a seeded volume, 2026-10-26
---

# Run in a container

The image's entrypoint runs `takt run --repo /workspace` under `tini`, as uid 10001. Its default
argument is `--config /config/agent.yaml`, which any argument after the image name replaces. You
supply two mounts: the repository at `/workspace`, and the configuration at `/config`. What the
image holds is in [Container image](../reference/container-image.md).

## 1. Build the image

No image is published, so build it from the tag:

```sh
git clone --branch 0.23.0 https://github.com/beyond10x/takt
cd takt
docker build --build-arg REVISION=$(git rev-parse HEAD) -t takt:0.23.0 .
```

The image carries takt, `aep`, `b10x-harness` 0.22.0, `ess`, `b10x-gates` (the `gates` scan
adapter's program and the `bot` [forge](/docs/concepts/glossary#forge)'s) and a Rust toolchain with `clippy` and `rustfmt`. The
toolchain is owned by root and cannot be written by the run, so
neither a step nor a test can change the tools the gates trust. takt runs under `tini`, not as
PID 1, so `docker stop` or a pod's stop ends it on SIGTERM rather than SIGKILL.

## 2. Write the configuration

The two files of `examples/loom-agent/` in the repository. `agent.yaml`:

```yaml
format: loom.agent/1
agent:
  id: takt-demo
  name: takt-demo
  brief: Delivers the ready stories of the repository mounted at /workspace.
takt:
  hours: 1
  harness: harness
  endpoint_file: endpoint.toml
  forge: none
  test_cmd: cargo test --quiet --locked
```

`endpoint.toml`, which `endpoint_file` names beside it:

```toml
base_url = "https://llm.example.com/v1"
wire = "openai-responses"
model = "example-model"

[credential]
env = "LLM_API_KEY"
```

The image holds b10x-harness and no other [harness](/docs/concepts/glossary#harness), and no `glab`: `harness: harness`
(or `none`) and `forge: none` are the settings this page's runs used. The `bot` forge reads
through `b10x-gates`, which the image ships, and no longer needs `gh` (since 0.23.0); it
needs a policy file as well ([What it cannot do](../reference/container-image.md#what-it-cannot-do)), and
no run of the image through it is recorded. A test in the repository reads
both files through takt's own readers, so they stay valid.

## 3. Put the repository on a volume

The workspace must be a git checkout the run's uid owns. A bind mount of a directory you own
under another uid is refused by git as `dubious ownership`, so clone into a volume from a bundle:

```sh
git -C my-repo bundle create ../my-repo.bundle main
docker volume create takt-demo-ws
docker run --rm -v "$PWD/my-repo.bundle:/src/my-repo.bundle:ro" -v takt-demo-ws:/workspace \
  --entrypoint git takt:0.23.0 clone -q -b main /src/my-repo.bundle /workspace
```

A [story](/docs/concepts/glossary#story) run needs what [Run a story](./run-a-story.md#1-prepare-the-repository) lists, committed
in the repository.

## 4. Run it

```sh
docker run --rm \
  --security-opt seccomp=unconfined --security-opt apparmor=unconfined \
  -e LLM_API_KEY \
  -v takt-demo-ws:/workspace -v "$PWD/config:/config:ro" \
  --entrypoint tini takt:0.23.0 -- \
  /bin/sh -c 'mkdir -p "$HOME/.cargo/bin" && exec takt run --repo /workspace --config /config/agent.yaml "$@"' takt
```

Arguments after the final `takt` reach `takt run`, so `--story <ID>` or `--harness none` go there.
An edit by b10x-harness also needs a [scan adapter](../reference/configuration.md#the-scan):
`scan: none` declared in `/config/agent.yaml`, or, under the default `gates`, `B10X_GATES_POLICY`
naming a `b10x-gates` policy file in the container. `b10x-gates` reads it only when uid 10001 owns
the file and its directory, the file
has no group or other bits and the directory is not group- or other-writable; a bind mount keeps
the host's owner. This page's runs used `--harness none` and no policy, on a takt 0.14.0 image.
The two pieces that are not the image's default are:

1. **seccomp and AppArmor unconfined.**

   :::caution[Unverified at 0.21.0]
   This need was observed on takt 0.14.0 and has not been measured again.
   :::

   Under the default `--confinement none` of 0.14.0 the test command ran in a user and mount
   namespace of its own. Both default profiles refuse to create one: with only seccomp or only
   AppArmor unconfined, the first test run still failed. From 0.15.0 `none` starts the test
   command through [Substrate](/docs/concepts/glossary#substrate)'s `local` adapter, in no namespace of takt's own. Whether a
   container still needs either flag has not been measured.
2. **`mkdir -p "$HOME/.cargo/bin"` before takt.** In a fresh `HOME` the first test run creates
   `~/.cargo`, and takt 0.14.0 refused a test run that writes outside the tree. The 0.16.0
   changelog says the cache cargo makes in `~/.cargo` is no longer refused, while a program in
   `~/.cargo/bin` still stops the run. This step was not run again at 0.21.0, so it stays in the
   command.

Each was run with `--harness none` on the story repository, on takt 0.14.0. Under the default
seccomp profile:

```console
frontier: took story:add-sub: Add a subtraction
case story-add-sub-20261026T233237Z on story.delivery/1 in /workspace; inputs none; confinement none
stopped: error: tests.run (takt.tests-run): run cargo test --quiet --locked: the test command runs in a user and a mount namespace of its own, where no login of the user's is in reach, and this host refuses unprivileged user namespaces (as with `kernel.apparmor_restrict_unprivileged_userns = 1` or `kernel.unprivileged_userns_clone = 0`); allow them, or run takt where they are allowed: run cargo: Operation not permitted (os error 1); steps 1, tokens 0, case /workspace/.takt/cases/story-add-sub-20261026T233237Z
```

The message names two kernel settings, but here the cause was the container's default seccomp and
AppArmor profiles.
Unconfined, without the `mkdir`:

```console
step 1: tests.run {}
  refused: tests.run wrote what its validator does not: /var/lib/takt/.cargo; the cases directory was put back, the change discarded and nothing recorded
stopped: StepRefused (tests.run wrote what its validator does not: /var/lib/takt/.cargo); steps 1, tokens 0, case /workspace/.takt/cases/story-add-sub-20261026T233241Z
```

Unconfined, with the `mkdir`, the tests and the gate pass and the run goes on to its first
judgement step:

```console
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
step 2: gate.run {}
  effect: the gates document .engineering/gates.yaml from main at e4f3d09f, pinned at open: pass; every gate true: format; retained as gate-result-2.bundle.json
```

(`frontier:` lines and the later steps are left out.) Without `LLM_API_KEY` in its environment,
the configuration above is refused before anything starts:

```console
takt: the endpoint's credential variable LLM_API_KEY is not set; nothing was started
```

## 5. Read the result

The [case](/docs/concepts/glossary#case) is in `/workspace/.takt/cases/` on the volume, and the exit code is the run's
([How a run ends](../concepts/how-a-run-ends.md)). With `forge: none` a delivery ends at
`pr.opened` with a candidate commit on the volume. Transcripts and what a discarded step sets
aside are under `HOME`, which `docker run --rm` drops with the container; `--state-dir` (or
`state_dir`) names a mounted directory to keep both
([The state directory](../reference/configuration.md#the-state-directory)).

On Kubernetes the same image runs as a [Loom](/docs/concepts/glossary#loom) agent:
[Deploy on Kubernetes](./deploy-on-kubernetes.md).
