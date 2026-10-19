---
title: Approve and resume
description: Grant a waiting case its merge, and resume it.
---

# Approve and resume

A run that reaches an action requiring a capability stops on `ApprovalRequired`, exit 4. In
`story.delivery/1` that action is `repository.merge`.

```sh
takt approve --repo <path> <case> repository.merge --by <who>
takt run --repo <path>
```

`takt approve` writes the grant into the case's `authority.yaml` and records the approval on the
case's story in the planning store; that record is committed on the case's branch, on which the
tree must be, clean. The next `takt run` resumes the case and merges under the grant.

## What a grant covers

A grant authorises one revision. A grant binds to the revision of the artifact its action is
about, so a commit after the grant needs a new one. Only approval records of granted capabilities
ride on a granted revision.

`--by` names who grants. For takt's own judge surfaces the grant is the owner's, or that of a role
the owner delegated it to on record ([Roles](../concepts/roles.md)).

## Resuming after other stops

| stop | exit | what the next run does |
|---|---|---|
| `ApprovalRequired` | 4 | merges under the grant once one is recorded |
| `Waiting` | 6 | reads the forge again |
| an error once the case was open | 5 | resumes the open case |
| a budget, `NoProgress` | 3 | resumes the open case of the story |

`--case <id>` resumes a named case instead.

Sources: `takt approve --help`, `takt run --help`, the stories `authority-hardening`,
`grant-binds-to-action-subject` and `approval-riders-are-grants`.
