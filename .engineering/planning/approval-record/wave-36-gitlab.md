---
format: aep.planning-md/3
id: approval-record:wave-36-gitlab
kind: approval-record
status: approved
title: 'Wave 36 delivers takt on a GitLab host through glab: the forge adapter and the pipeline read, each tried once on the private project'
relations:
- decides: story:gitlab-forge
- decides: story:gitlab-ci-wait
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T21:51:34Z", actor: "human:timo", revision: 2}
---
# Approval: wave 36, takt on a GitLab host through glab

## Decision

The orchestrator, under the owner's delegation, on 2026-10-23: "[The owner] answered
decision-blocker:gitlab-trial-project on 2026-10-23 ... The project is a private project on [a
GitLab host of the owner's]. ... It never enters takt committed text, store, commit messages or
PR bodies: clear the blocker with 'the operator named a private project, kept outside the
repository' and keep the path in a local, uncommitted config. Then deliver story:gitlab-forge and
story:gitlab-ci-wait, each with one real run there. GitLab access only through glab on its own
keyring login; never curl or any call with a personal token of [the owner's]. Approved under the
grant."

## The wave, as the dispatcher orders it

| unit | story | why in this order |
|---|---|---|
| U1 | story:gitlab-forge | first: the adapter, the forge selection and the judge surface for the CI definition; everything else reads through it |
| U2 | story:gitlab-ci-wait | after U1: the pipeline read as `ci.read`'s checks, with a scripted `glab` double |
| trial | both | one real run per story against the private project, from a local configuration outside the repository; the merge request is closed unmerged |

## Decisions taken inside the item

- D1: pushes go over HTTPS with `glab auth git-credential` as git's only credential helper, so takt holds no token and the user's SSH agent stays out of reach, as on the bot route.
- D2: the same `b10x-gates scan-text` runs before a push and before a merge request opens, under `B10X_GATES_POLICY`; no policy refuses the push, as on the bot route.
- D3: `.gitlab-ci.yml` joins the judge surface beside `.github/**`: a step that edits it leaves `ci.read` nothing to read.
- D4: story:gitlab-ci-wait's events part waits on story:ci-progress-events, which has not landed; the story delivers the pipeline read, the same deadline and the `ci_result` record.
