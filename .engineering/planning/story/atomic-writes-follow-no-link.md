---
format: aep.planning-md/3
id: story:atomic-writes-follow-no-link
kind: story
status: draft
title: Atomic writes in a case follow no link
summary: the temporary file of every atomic write is created new without following a link
revision: 1
---
## Outcome

Every atomic file write takt makes inside a case or a state directory creates its temporary file
new and without following a link, as the connectors forge's refusal record does since wave 48.

## Context

The wave 48 security review of the connectors forge found that a temporary file written with
`std::fs::write` follows a link planted at its name. The forge's refusal record was fixed. The
same pattern remains in `crates/takt/src/case.rs` (`write_atomically`),
`crates/takt/src/forge/local.rs` (`write`) and `crates/takt/src/promotion.rs`. Reaching it needs
a process that can plant a link in the case directory between a step's observations; none is
known.

## Acceptance

1. A link planted at the temporary name of each of the three writes is not written through: the
   link's target is unchanged and the write either replaces the link or fails naming the file.
2. One shared helper does the create-new, no-follow write; the three sites and the refusal record
   use it.
