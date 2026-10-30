# takt delivers stories from inside a cluster

This example is the setup for one goal: a takt `LoomAgent` in a local
[kind](https://kind.sigs.k8s.io/) cluster takes ready stories from the AEP planning stores of two
real beyond10x repositories and ends each one as a pull request opened by a GitHub App, merged to
`main`, with no step by a person.

Status: **not run yet.** takt 0.23.0 has the token stop, the forge preflight and the merge under
a delegated grant. The runs wait for two changes: a forge for github.com, the connectors forge
speaking GitHub through a Connectors instance, and the model key and every forge credential
outside the takt container, where the harness session cannot read them (*Open*, below). Run
records join this folder as the runs happen.

![Architecture: a Loom operator in a kind cluster runs takt once per LoomAgent as a Job; takt reads
a story from the workspace clone, asks the model through b10x-harness, runs tests and gates, and
opens and merges the pull request through the connectors forge and a Connectors
instance](architecture.svg)

The chart's source is [`architecture.dot`](architecture.dot); `dot -Tsvg architecture.dot -o
architecture.svg` renders it again.

## The parts

| part | what it is | from |
|---|---|---|
| kind cluster | one control-plane node on the operator's machine; images are loaded with `kind load`, never pulled | [`../loom-agent/kind-cluster.yaml`](../loom-agent/kind-cluster.yaml) |
| Loom operator | chart `loom-agents` of [Loom](https://github.com/beyond10x/loom) 0.50.2; turns each `LoomAgent` into a Job, a NetworkPolicy and a service account; refuses any Secret, claim or egress host its values do not list | Loom `charts/loom-agents`, `deploy/crd/` |
| `LoomAgent`, `Command` variant | one resource per run; the pod runs the takt image's own program once (`backoffLimit: 0`, `restartPolicy: Never`) and reports the exit code in `status.run` | [Deploy on Kubernetes](../../website/docs/guides/deploy-on-kubernetes.md) |
| init containers | `b10x-loom agent stage-files` writes the run file and the model routes to `/config`; `b10x-loom agent prepare-dirs` makes `HOME` and `/tmp` private (0700) | Loom agent image |
| takt image | `takt`, `b10x-harness`, `aep`, `ess`, and a Rust toolchain for the target's tests | [`Dockerfile`](../../Dockerfile) |
| workspace | a PersistentVolumeClaim holding a clone of the target repository; the case and its evidence are written under `.takt/cases/` and stay after the pod ends | [`../loom-agent/workspace.yaml`](../loom-agent/workspace.yaml) |
| Secret (env) | the model API key, reached by `llm.yaml` as `env:<NAME>`; never in a file, a prompt or a case record | |
| Connectors instance | the forge: its GitHub connector holds the GitHub App's key and opens, reads and merges the pull request; the run file names it as `forge_instance` | [Connectors](https://github.com/beyond10x/connectors) |
| egress | DNS, and port 443 for the model endpoint, crates.io and github.com; Loom enforces it by port, not by host | `agent.deploy.kubernetes.egress` |

## One run

1. The operator applies a `LoomAgent` naming one story: `story: story:<id>` in the run file.
2. takt opens a case on `story.delivery/2` in the workspace clone and takes the story to `active`.
3. Each judgement step goes to the model through one `b10x-harness` session. The model may take
   only the actions the protocol's frontier admits; a step that does more is refused and reverted.
4. Validators write the evidence: `tests.run` runs the target's test command and the gate runs
   its checks. The run file declares `scan: none`, so nothing scans what is published; each
   proposal is still checked for the shape of a credential.
5. On green, takt pushes the case's branch, the connectors forge opens the pull request through
   the instance, and waits for its checks. With tests, gate and checks green at the revision, and
   a delegation of the merge to `agent:takt-delivery` recorded by the owner in the target
   repository's `.engineering/delegations.yaml`, takt grants `repository.merge` as that role and
   merges.
6. The Job ends; `status.run.exitCode` is takt's exit code
   ([How a run ends](../../website/docs/concepts/how-a-run-ends.md)).

## Done means

| check | expected |
|---|---|
| `kubectl -n loom-agents get loomagent` | 3 runs `Completed`, exit code 0 |
| `curl -s https://api.github.com/repos/beyond10x/<repo>/pulls/<n>` | `"merged": true`, `user.login` the GitHub App's, for 3 pull requests in 2 repositories |
| `takt report --cases <dir>...` | units accepted, tokens and interventions per repository and day, and their total |

## Stop rules

- Each run stops at 3,000,000 tokens, the default when the run names no budget.
- A run on a red base stops at open (`story:a-red-base-stops-at-open`).
- The same failing tests twice in a row stop the run (`story:repeated-identical-test-failures-stop-the-run`).
- A trial stops after 2 runs that end on the same stop reason.

## Open

| what | why it matters here | story |
|---|---|---|
| no secret in the agent's container | the harness session runs as the same user in the same container as takt, so it can read every key mounted there; Loom's `spec.boundary` puts them in a second container that relays the model, the forge's operations and git on loopback | `story:connectors-forge-speaks-github-at-an-endpoint`, `story:git-and-model-through-loopback-relays`, `story:example-agent-runs-credential-free` |
| a forge write whose answer was lost | it is read back, never sent twice | `story:fenced-forge-dispatch` |

## The three stories

| repository | story | its test |
|---|---|---|
| [llm](https://github.com/beyond10x/llm) | `story:http-transport-load-sensitivity` | `cargo test -p llm-http --test transport` |
| [llm](https://github.com/beyond10x/llm) | `story:chat-counts-every-output-delta` | `cargo test -p llm-chat` |
| takt | `story:git-excluded-files-are-not-step-changes` | `cargo test -p b10x-takt` |

llm was picked because these two stories each stay in one crate, test against fixtures or loopback
only, and depend on no open story; it has a gates document (`.engineering/gates.yaml`) as takt
has. Both are proposed in llm's store, and llm's `.engineering/delegations.yaml` delegates
`repository.merge` and `repository.merge.judge_surface` to `agent:takt-delivery`.

## Earlier trials

Two one-hour trials ran takt as a `LoomAgent` on 2026-10-26 (takt 0.12.0 and 0.13.1, Loom 0.42.0),
with the forge off. The second ran 12 times and used 17,200,642 tokens with no accepted result; the
fixes it led to are the stories it named, all implemented since. The `Command` variant's Job,
`status.run`, `spec.env`, private `/tmp` and `HOME`, and the seccomp and AppArmor profile fields
came to Loom after those trials.
