---
format: aep.planning-md/3
id: story:work-item-fields-have-their-own-bounds
kind: story
status: draft
title: A work item's fields have their own bounds
summary: documented limits for title, body, key, url and revision of a takt.work-item/1 document
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:work-sources
- informed_by: story:the-command-work-source
revision: 1
---
# Story: work-item-fields-have-their-own-bounds

## What is wrong

A `takt.work-item/1` document's fields have no bounds of their own (`crates/takt/src/work/command.rs`, `Item::parse`).
The title becomes the case title and the first line of `intent.md`, and the body becomes the rest of `intent.md` and
reaches every prompt; both are bounded only by the forge runner's 8 MiB standard-output limit. A tracker item with a
very long title or body costs every step of the case its tokens, and an operator reading the case cannot see the title
whole.

Found by the wave 56 security review and the adversary.

## Outcome

`Item::parse` refuses a title over a documented length and a body over a documented size, naming the key, the field and
the limit; `key`, `url` and `revision` have documented bounds as well.

## Acceptance

- A title one character over the limit, and a body one byte over its limit, are each refused before a case opens,
  naming the field and the limit: a test each.
- Items at the limits are taken: a test.
