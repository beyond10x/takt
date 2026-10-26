---
format: aep.planning-md/3
id: decision-blocker:gitlab-trial-project
kind: decision-blocker
status: cleared
title: 'Owner: which GitLab project takt''s GitLab trials run against'
relations:
- blocks: story:gitlab-forge
- blocks: story:gitlab-ci-wait
revision: 5
transitions:
- {from: "open", to: "cleared", at: "2026-10-23T21:51:34Z", actor: "human:timo", revision: 4}
---
## Question

Which GitLab host and project do takt's GitLab trials run against? story:gitlab-forge and story:gitlab-ci-wait each need one real run there.

## Who answers

The owner. Cleared by naming the project; its name stays out of this public repository's committed text if it is private.

Decider: operator

## Answer

2026-10-23: the operator named a private project, kept outside the repository. Its path lives in a local, uncommitted configuration of the trial runs and never enters this repository's text, store, commit messages or pull-request bodies. The trials of story:gitlab-forge and story:gitlab-ci-wait run there (approval-record:wave-36-gitlab).

2026-10-25: the trials run against a project in the operator's own GitLab, named in the local trial configuration outside this repository. Status: cleared.
