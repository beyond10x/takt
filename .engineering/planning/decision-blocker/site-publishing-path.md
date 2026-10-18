---
format: aep.planning-md/3
id: decision-blocker:site-publishing-path
kind: decision-blocker
status: open
title: 'Owner: publish the site by a gh-pages push or by the Pages-from-artifact workflow from main'
relations:
- blocks: story:docs-site-with-sidebar
revision: 1
---
## Question

How is the takt site published once it is generated (story:docs-site-with-sidebar)?

| option | what it does | cost |
|---|---|---|
| A | the doc-writer builds the site and pushes `gh-pages`, as `AGENTS.md` § Roles says today | no rule changes; a second publishing path beside the sibling repositories |
| B | `.github/workflows/pages.yml` builds on push and deploys from `main` through the sibling repositories' reusable project-site workflow | the `AGENTS.md` § Roles sentence "published by a push of `gh-pages` only" changes; a one-time roster entry for the organisation site |

## Context

The design of 2026-10-19 takes B as its default because every other beyond10x site deploys that way. The sentence in `AGENTS.md` is the owner's rule, so the choice is the owner's. Put to the owner on 2026-10-19.
