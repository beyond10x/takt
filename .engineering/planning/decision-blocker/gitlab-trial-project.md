---
format: aep.planning-md/3
id: decision-blocker:gitlab-trial-project
kind: decision-blocker
status: open
title: 'Owner: which GitLab project takt''s GitLab trials run against'
relations:
- blocks: story:gitlab-forge
- blocks: story:gitlab-ci-wait
revision: 1
---
## Question

Which GitLab host and project do takt's GitLab trials run against? story:gitlab-forge and story:gitlab-ci-wait each need one real run there.

## Who answers

The owner. Cleared by naming the project; its name stays out of this public repository's committed text if it is private.
