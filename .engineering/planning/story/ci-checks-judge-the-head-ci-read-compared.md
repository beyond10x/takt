---
format: aep.planning-md/3
id: story:ci-checks-judge-the-head-ci-read-compared
kind: story
status: draft
title: ci.read judges the checks of the head it compared
summary: The forge's checks read takes the head sha ci.read compared, so a head moved between two reads gives no ci.green
revision: 1
---
## Problem

`ci.read` compares the pull request's head with the case revision, then the forge's `checks` reads
the pull request again and judges whatever head it finds then (`crates/takt/src/validator.rs`
`CiRead`, `crates/takt/src/forge/connectors.rs` and `crates/takt/src/forge/gitlab.rs` `checks`).
A head that moves between the two reads gives `ci.green` for another head, recorded against the
case revision. The merge sends the head sha, so a wrong merge is refused; the evidence is still
wrong.

Found by the adversarial review of wave 47 (judgement, not reproduced).

## Acceptance

1. A forge's `checks` takes the head sha `ci.read` compared and judges the checks of that sha
   only; a test whose stand-in moves the head between the two reads gets no `ci.green`.
