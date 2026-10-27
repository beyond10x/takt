---
title: Deploy on Kubernetes
sidebar_position: 7
description: Run takt as a Loom 0.44.0 Command LoomAgent on a kind cluster with its configuration, key, policy and workspace, watch the Job, and know what still stops it.
lede: Loom 0.44.0 runs takt once as a Job with its configuration files, key and workspace, but its fixed seccomp profile still refuses the namespace takt's test command needs.
source: examples/loom-agent/ (checked by crates/takt/tests/examples_config.rs; takt-agent.yaml rendered by b10x-loom-operator 0.44.0 and the chart's values by helm template); Loom 0.44.0 CHANGELOG, README, charts/loom-agents, deploy/crd, crates/loom-operator/src/render.rs and admission.rs; the docker runs of Run in a container. Not run on a cluster.
---

# Deploy on Kubernetes

[Loom](https://beyond10x.github.io/loom/) ([GitHub](https://github.com/beyond10x/loom)) runs
agents on Kubernetes. Its operator turns a `LoomAgent` resource into a pod, a network policy and
the rest. takt is a `LoomAgent` of the `Command` variant: the pod runs the takt image's own
program once, as a Job. Loom deploys and confines it. takt decides each step inside it.

:::caution[Not run on a cluster]

This recipe was put together from Loom 0.44.0's chart, CRD and operator, and from the manifests
in `examples/loom-agent/`. The operator rendered the `LoomAgent` and helm rendered the chart.
Neither was applied to a cluster for this page. Under Loom 0.44.0 the run ends at its first test
run, as [What still stops it](#what-still-stops-it) explains.

:::

## What you need

| tool | for |
|---|---|
| `kind`, `kubectl`, `helm`, `docker` | the cluster and the install |
| a takt image | built as in [Run in a container](./run-in-a-container.md#1-build-the-image) |
| a Loom operator image and a Loom agent image | built by you from Loom 0.44.0, which ships neither an image nor a Dockerfile |
| a checkout of Loom at `0.44.0` | the chart `charts/loom-agents` and the CRD in `deploy/crd/` |

The operator image must hold `b10x-loom-operator` on its `PATH`, and the agent image `b10x-loom`:
the Job's two init containers run `b10x-loom agent stage-files` and
`b10x-loom agent prepare-dirs` from it.

## 1. Create the cluster and load the images

`examples/loom-agent/kind-cluster.yaml` is one control-plane node:

```yaml
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
name: takt-demo
nodes:
  - role: control-plane
```

```sh
kind create cluster --config examples/loom-agent/kind-cluster.yaml
docker tag takt:0.14.0 registry.example/takt:0.14.0
kind load docker-image registry.example/takt:0.14.0 --name takt-demo
kind load docker-image registry.example/loom-operator:0.44.0 --name takt-demo
kind load docker-image registry.example/loom-agent:0.44.0 --name takt-demo
```

The names are the ones the manifests use. The images are loaded, not pulled, and a pod pulls a
tagged image only when the node lacks it.

## 2. Install the operator

```sh
kubectl apply --server-side --force-conflicts -f deploy/crd/loomagents.loom.beyond10x.dev.yaml
helm install loom-agents charts/loom-agents -n agents --create-namespace \
  -f examples/loom-agent/values.yaml
```

Run both from the Loom checkout, with the `-f` path pointing at takt's `examples/loom-agent/`.
Helm installs a chart's CRD only once and never upgrades it, so apply the CRD yourself before
every install or upgrade of a newer chart. `values.yaml` admits exactly the names the agent uses.
The operator refuses any other Secret, egress host or claim:

```yaml
image:
  repository: registry.example/loom-operator
  tag: "0.44.0"
agentImage:
  repository: registry.example/loom-agent
  tag: "0.44.0"
allowedSecretNames: [takt-demo-env, takt-demo-files]
allowedEgressHosts: [llm.example.com, index.crates.io, static.crates.io]
allowedClaimNames: [takt-demo-workspace]
```

## 3. Create the Secrets

One Secret for the key, given to the pod as environment variables, and one for files:

```sh
kubectl create secret generic takt-demo-env -n agents --from-env-file=llm.env
kubectl create secret generic takt-demo-files -n agents --from-file=policy.json=<policy file>
```

`llm.env` holds one line, `LLM_API_KEY=<token>`. The file Secret is mounted at
`/var/run/secrets/takt` with each file at mode 0600 for uid 10001, the owner `b10x-gates` asks of a
policy. `spec.env` cannot carry a credential: Loom refuses one there.

## 4. Fill the workspace

`examples/loom-agent/workspace.yaml` is a 20 GiB claim on the default StorageClass and a pod
that clones the repository into it as uid 10001:

```sh
kubectl apply -f examples/loom-agent/workspace.yaml
kubectl wait -n agents --for=jsonpath='{.status.phase}'=Succeeded pod/takt-demo-clone
```

Change the clone URL, `https://github.com/example/my-repo.git`, to your repository. It needs what
[Run a story](./run-a-story.md#1-prepare-the-repository) lists.

## 5. Apply the LoomAgent

`examples/loom-agent/takt-agent.yaml`:

```yaml
apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: takt-demo
  namespace: agents
spec:
  image: {repository: registry.example/takt, tag: "0.14.0"}
  command:
    entrypoint:
      - tini
      - --
      - /bin/sh
      - -c
      - mkdir -p "$HOME/.cargo/bin" && exec takt run --repo /workspace --config /config/takt.yaml "$@"
      - takt
    args: [--story, story:add-sub]
  secretRef: {name: takt-demo-env}
  secretFiles: {name: takt-demo-files, mountPath: /var/run/secrets/takt}
  env:
    - {name: B10X_GATES_POLICY, value: /var/run/secrets/takt/policy.json}
  workspace: {mountPath: /workspace, claimName: takt-demo-workspace}
  configFiles:
    - name: takt.yaml
      content: |
        hours: 1
        harness: harness
        endpoint_file: endpoint.toml
        forge: none
        test_cmd: cargo test --quiet --locked
    - name: endpoint.toml
      content: |
        base_url = "https://llm.example.com/v1"
        wire = "openai-responses"
        model = "example-model"

        [credential]
        env = "LLM_API_KEY"
  agent:
    format: loom.agent/1
    agent:
      id: takt-demo
      name: takt-demo
      brief: Delivers the ready stories of the repository mounted at /workspace.
    model:
      routes: routes.yaml
      roles:
        - {role: agent, model: example-model}
    deploy:
      kubernetes:
        namespace: agents
        service_account: takt-demo
        resources: {cpu: "4", memory: 8Gi}
        egress:
          - {name: llm, host: llm.example.com, port: 443}
          - {name: crates-index, host: index.crates.io, port: 443}
          - {name: crates-static, host: static.crates.io, port: 443}
  routes: {}
```

```sh
kubectl apply -f examples/loom-agent/takt-agent.yaml
```

What each part does:

| part | what it gives takt |
|---|---|
| `command.entrypoint` | creates `~/.cargo/bin`, then runs `takt run` on `/config/takt.yaml` |
| `command.args` | `--story`, a flag only: the story is not a key of the run's file |
| `configFiles` | `/config/takt.yaml` and `/config/endpoint.toml`, written verbatim |
| `secretRef` | `LLM_API_KEY` in the environment, which the endpoint file names |
| `secretFiles`, `env` | the gates policy file and `B10X_GATES_POLICY` naming it, for the default `gates` scan adapter; a `takt.yaml` that declares `scan: none` needs neither |
| `workspace` | the claim at `/workspace` |
| `agent.deploy.kubernetes` | the service account, the pod's resources and the egress |

`spec.agent` carries no `takt` member, because Loom 0.44.0 checks it strictly and refuses one
(``Malformed at spec.agent: unknown member `takt` ``). A `configFiles` entry named `agent.yaml`
is refused too, because Loom writes that file from `spec.agent` (`ConfigFileConflict`). Without `spec.agent` the resource is
admitted, but its network policy then allows DNS and nothing else. The egress list opens port 443
for the endpoint and the crates registry. Loom enforces egress by port, not by host.

## 6. Watch it

```sh
kubectl get loomagent -n agents
kubectl get job -n agents
kubectl logs -n agents job/takt-demo -f
kubectl get loomagent takt-demo -n agents -o jsonpath='{.status.run}'
```

| what | where |
|---|---|
| the resource's state | `kubectl get loomagent`: `Pending`, `Running`, `Refused`, `Completed`, `Failed` or `Degraded` |
| why it was refused | `status.check`, naming the member |
| the run | a Job named after the resource: `backoffLimit: 0`, `restartPolicy: Never`, run once |
| how takt ended | `status.run`: `exitCode`, `finishedAt`, `reason` |
| the condition | `Completed`: `Unknown` `RunNotFinished`, `True` `RunSucceeded` or `False` `RunFailed` |

[k9s](https://k9scli.io/), if you have it, shows the same live: `k9s -n agents`. takt's own exit
codes are in [How a run ends](../concepts/how-a-run-ends.md). A finished Job is not started again.
To run once more, change the resource (a new `args`, for instance): the new generation deletes the
old Job and creates a new one.

The case is on the claim, under `/workspace/.engineering/cases/`. `HOME` is `/var/lib/agent`, an
emptyDir of mode 0700, as is `/tmp`, and the root file system is read-only. Transcripts under
`HOME` go with the pod.

## What still stops it

Loom renders every agent pod with `seccompProfile: RuntimeDefault`. The CRD has no field to change
that, and a Job's pod template cannot be patched once the Job exists. Under the default
`--confinement none`, takt's test command runs in a user and mount namespace of its own
(`--confinement substrate`, which needs a delegated cgroup, was not tried there), and a container
runtime's default
profile refuses that. With `docker run` both the default seccomp and the default AppArmor profile
refused it ([Run in a container](./run-in-a-container.md#4-run-it)). So under Loom 0.44.0 the
first `tests.run` ends the run on an error, exit 5, before any model call. That is inferred from
the rendered pod and the docker runs, not observed on a cluster.

There is no supported way around this at Loom 0.44.0. The tests can run only where the container
runtime's own default profile admits unprivileged user namespaces.
[Known limitations](../known-limitations.md#running-under-loom) tracks it.

## Workarounds from earlier cluster trials

| workaround | now |
|---|---|
| hold the pod with a restart loop or `sleep infinity` | gone: a `Command` resource runs once as a Job and reports `status.run` (Loom 0.44.0) |
| put `B10X_GATES_POLICY` in the env Secret | gone: `spec.env` (Loom 0.44.0) |
| make `/tmp` mode 2777 | gone: `/tmp` and `HOME` are mode 0700 (Loom 0.44.0) |
| apply the resource twice | gone (Loom 0.44.0) |
| write the endpoint file into the workspace | gone: `spec.configFiles` (Loom 0.44.0) |
| stage the key as a file by hand | gone: `--endpoint-token-env` or `env` in the endpoint file (takt 0.13.0) |
| move the story to active by hand | gone: `--story` (takt 0.13.0) |
| add packages, toolchain links, b10x-harness 0.20.0 and `b10x-gates` to the image | gone: the image ships them (takt 0.13.0) |
| `mkdir -p "$HOME/.cargo/bin"` before takt | still needed at 0.14.0, in `command.entrypoint`; not after it: the first test run in a fresh `HOME` is no longer refused |
| patch the pod to seccomp and AppArmor `Unconfined` | not possible at Loom 0.44.0 |
