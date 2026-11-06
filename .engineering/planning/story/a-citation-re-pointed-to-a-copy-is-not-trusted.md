---
format: aep.planning-md/3
id: story:a-citation-re-pointed-to-a-copy-is-not-trusted
kind: story
status: draft
title: A citation re-pointed to a copy of its changed lines is not trusted
summary: a row re-pointed to a stale copy keeps its key and digest and the gate passed
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: a-citation-re-pointed-to-a-copy-is-not-trusted

## Outcome

A citation row whose cited lines changed and that is re-pointed to an identical copy of the old text elsewhere does not
let `gate.green` hold: a re-point is trusted only along lines the change left as they were.

## Context

Found by the wave 55 adversary pass at d0e8fba54 (`crates/takt/src/validator.rs:3934`). The cited line changed, a copy
of its old text was left in a `#[cfg(any())]` block, and `read from:` was re-pointed to the copy; key and digest are
the base's, so the gate passed. `scripts/check-citations fix` makes this re-point by itself. The case, not committed
with the wave (outside its acceptance):

```rust
#[test]
fn a_citation_re_pointed_to_a_copy_of_its_changed_lines_keeps_the_gate_from_recording() {
    let tree = citing("re-pointed");
    let source = "pub fn thing() -> u8 { 2 }\n#[cfg(any())]\nmod stale {\npub fn thing() -> u8 { 1 }\n}\n";
    let outcome = observed(&tree, "re-pointed", &[("src/lib.rs", source), ("ess/domains/widget.yaml", &domain(4, ""))]);
    refused(&outcome, &[]);
}
```

The helpers are those of `crates/takt/tests/w55_adversary.rs`.

## Acceptance

- The case above is green.
- A row re-pointed because lines above it moved, with the cited lines unchanged, still passes.
