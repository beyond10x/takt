---
title: Capabilities and approvals
sidebar_position: 4
description: Which actions need a person's grant, how takt refuses a step that does more than the frontier admits, how a grant is recorded and bound to a revision, and where credentials never go.
lede: An action that requires a capability waits for a grant recorded in the case's authority.yaml, bound to the case revision and the commit it was given at; a step that does anything the frontier does not admit is refused and its change discarded; and no credential enters a harness, a prompt or the case record.
source: crates/takt/src/case.rs (Authority, AuthorityDecision, CaseAuthority, prepare_grant, holds_grant), run.rs (proposal_refusal, JUDGE_SURFACE, Shape::only_behind), validator.rs (PrMerged), harness.rs (SessionHarness); crates/takt/tests/authority.rs, grant_subject.rs, merge_under_authority.rs, judge_guard.rs; takt approve --help and takt run --help at 0.14.0; engineering-protocols 0.10.0 protocols/software-change/1.yaml and protocols/story-delivery/1.yaml; AGENTS.md § Rules; scratch runs of takt 0.14.0
---

# Capabilities and approvals

## What a capability is

A capability is a name a protocol action `requires`. The action is admissible only when the case
holds a grant of that capability. In `story.delivery/1`:

```yaml
repository.merge:
  description: >-
    merge the pull request into the repository's mainline; needs continuous integration green,
    the pull request open and authority to merge, never only a connection to the repository
    host
  effect: write
  precondition:
    all:
      - claim: ci.green
      - claim: pr.opened
  requires:
    - capability: repository.merge
```

| protocol | capabilities its actions require |
|---|---|
| `story.delivery/1` | `repository.merge` |
| `software.change/1` | `repository.merge`, `release.rollback` |
| `self.improvement/1` | `repository.merge`, `repository.merge.judge_surface`, `takt.promote` |
| `loop.improvement/1` | `repository.merge.judge_surface` |

When an action's precondition holds but its capability is not granted, the frontier lists it
`approval-required`. Loom's governor supplies no authority of its own, so it finds such an action
approval-required whether or not anyone granted it. The grant reaches the run through takt's
authority provider, `CaseAuthority`, which reads the case's `authority.yaml` each time it is
asked. A connection to the forge is never authority.

## The run stops for a grant

A run asks for a grant in one of two ways, and spends nothing on it:

- **Only a grant is left.** When every claim still open is decided by evidence only an action
  waiting for approval produces, the run stops `ApprovalRequired` before the step, exit 4.
- **The harness asks.** A judgement step may choose the approval gate (`--gate`, by default the
  protocol's first action that requires a capability). Nothing is performed; the run stops
  `ApprovalRequired`, exit 4.

With `--harness none` the run asks as soon as nothing but the approval is left:

```console
$ takt run --repo ~/demo --case takt-20261026T231123Z --harness none --forge none --iterations 2
case takt-20261026T231123Z on software.change/1 in ~/demo; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
step 1: repository.merge (none)
  said: no harness: nothing but the approval is left; requesting it
  effect: none; repository.merge needs an approval
stopped: ApprovalRequired (repository.merge); steps 1, tokens 0, case ~/demo/.engineering/cases/takt-20261026T231123Z
```

## Recording a grant

`takt approve` grants a case one capability. Its usage line, from `takt approve --help`:

```text
Usage: takt approve --repo <REPO> --by <BY> <CASE> <CAPABILITY>
```

`--by` says who grants it: `human:<id>` for a person, `agent:<role>` for an agent role. On the
same case:

```console
$ takt approve --repo ~/demo --by human:demo takt-20261026T231123Z repository.merge
approved: repository.merge for case takt-20261026T231123Z by human:demo, written to takt-20261026T231123Z/authority.yaml
recorded: nothing in the planning store; the case delivers no story (it was opened on an intent, or is an improvement turn)
```

The grant is one entry in the case's `authority.yaml`:

```yaml
- capability: repository.merge
  decision: granted
  approver: human:demo
  granted_at: 2026-10-26T23:11:34Z
  revision: 6074a8e0c3db22c5ff3f95ee8887eec6252c5aeb
```

For a story's case, `takt approve` also records the approval on the story in the planning store
as `approval` evidence and commits that record on the case's branch, on which the tree must be,
clean. The next `takt run` resumes the case, and the frontier now lists the action `admissible`.

### What a grant is bound to

A grant applies at two revisions, and at no other:

- **The subject's revision**, `revision` in the file: the commit of the artifact the
  capability's action is about (for `repository.merge`, the implementation) that the case
  reached the gate at. A new commit moves the subject, the grant no longer applies, and the
  action is approval-required again. A new grant at the new revision replaces the old one.
- **The case revision** the runtime's request expected. A grant is allowed only while the case
  is still at it.

A `denied` decision in `authority.yaml` denies the capability at every revision, and the
action is `blocked`.

### What takt approve refuses

Each refusal exits 1 and writes nothing. Real refusals from the scratch cases above:

```console
$ takt approve --repo ~/demo --by human:demo takt-20261026T231123Z repository.merge
takt: the case takt-20261026T231123Z already decides repository.merge at 6074a8e0c3db22c5ff3f95ee8887eec6252c5aeb: granted by human:demo at 2026-10-26T23:11:34Z
$ takt approve --repo ~/red --by human:demo takt-20261026T231110Z repository.merge
takt: the case takt-20261026T231110Z is not at the gate for repository.merge: its latest decision finds repository.merge blocked, not approval-required; run it until it stops ApprovalRequired
$ takt approve --repo ~/red --by human:demo takt-20261026T231110Z repository.push
takt: no action of software.change/1 requires repository.push; the capabilities its actions require: release.rollback, repository.merge
$ takt approve --repo ~/red --by demo takt-20261026T231110Z repository.merge
takt: the approver `demo` is not an actor the planning store records as granting; say who grants repository.merge as human:<name> or agent:<role>, not takt's own agent:takt
```

A grant is also refused while a run holds the case (`run.lock`), so it never lands under a step
in flight.

## The merge validator's gate

The merge is the one gate written in Rust. Canon admits `repository.merge` only under a grant,
and the merge validator checks again itself, before it merges, that `authority.yaml` grants
`repository.merge` at the revision the pull request carries. An open pull request at a revision
the case holds no grant for is not merged, and the claim stays unknown. Whose grant a change to a
judge surface needs is decided at the merge target's tip:
[Evidence and the judge](./evidence-and-the-judge.md#the-judge-whose-grant-merges-a-judge-surface).

## A step that does more is refused

A judgement step is offered only the actions the frontier admits. The step guard judges its
proposal before the edit effect writes a byte of it, and refuses it when:

- it names an action that was not offered;
- it changes a file for an approval, `repository.inspect` or an action that records a choice,
  all of which change none;
- `tests.write` changes a path that is not a test;
- a path is not a plain path of the tree (absolute, `..`, a part named `.git`), lies in the
  cases directory, is reached through a link, or is ignored by git;
- a path is on the run's judge surface: what decides its validators, such as `.cargo/**`,
  `rust-toolchain*`, `build.rs`, `.github/**`, `.gitlab-ci.yml`, `.gitlab/**`, `.engineering/**`
  but the cases, the test command's own scripts, Python test configuration, and the paths the
  repository adds in `.engineering/judge-surface.yaml`.

A refused step is reverted: nothing it proposed is kept, the tokens it used are counted, and the
run stops `StepRefused`, exit 3. A validator's command that writes what its validator does not
is refused the same way ([Evidence and the judge](./evidence-and-the-judge.md#what-a-validator-may-not-do)).

## Credentials stay out

No credential enters a harness's environment, its prompt or a case record.

- metaharness builds each session's environment from its own allowlist, so a variable takt
  withholds never reaches the harness. An `ANTHROPIC_API_KEY` in takt's environment is dropped
  for Claude Code, which runs on the user's login.
- An endpoint's credential is named, never given: a token file's path, a secret's name
  (`--endpoint-secret`), which b10x-harness resolves itself, or an environment variable's name
  (`--endpoint-token-env`). takt takes that variable out of its own environment before it starts
  anything and stages it as a file of mode 0600, removed when the run ends.
- The case records the endpoint by reference only: base URL, wire, model, and the credential's
  kind and name.
- Through `glab`, git pushes with `glab auth git-credential` as its one credential helper, and
  takt holds no token.

How to grant and resume step by step: [Approve a capability](../guides/approve-a-capability.md).
