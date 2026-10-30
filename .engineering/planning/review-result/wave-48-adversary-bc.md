---
format: aep.planning-md/3
id: review-result:wave-48-adversary-bc
kind: review-result
status: active
title: Adversary, wave 48 units b1 and c1
relations:
- reviews: story:git-through-the-connectors-git-relay
revision: 1
---
needs-revision, resolved

Adversary pass on units b1 and c1 (served instance tests, relay path mapping), at e9f209d1: 6 cases in crates/takt/tests/w48_adv_bc.rs, none red; six mutants run on a scratch copy, four of which only this file catches. The served relay stand-in did not read owner or repo, and the reference did not state the mapping. Fixed in fa12f45b: the stand-in refuses any repository but o/r, the reference states the mapping, and the mapping is github's only.

```findings
[
{"file":"crates/takt-cli/tests/git_relay_served.rs","line":678,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the stand-in for the github operations never read owner or repo, so relay_project naming git/o instead of o/r stayed green against the real 0.62.0 instance; fixed: the stand-in answers 404 to any other repository"},
{"file":"crates/takt/src/forge/connectors.rs","line":464,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"relay_project's == 2 changed to >= 2 was caught only by w48_adv_bc, now kept in the suite"},
{"file":"crates/takt/src/forge/connectors.rs","line":782,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"mapping the path whatever the transport was caught only by w48_adv_bc, now kept in the suite"},
{"file":"crates/takt/src/forge/connectors.rs","line":186,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"a key that ignored the input or the revision left the served and keyed suites green; w48_adv_bc's key case now catches both"},
{"file":"website/docs/reference/configuration.md","line":469,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the reference did not say that a loopback /git/<owner>/<repo> path names <owner>/<repo>; fixed, with the gitlab and percent-encoding rules"},
{"file":"crates/takt/src/forge/connectors.rs","line":463,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the mapping went by URL shape on any loopback origin, so a gitlab project under a top-level group named git would be renamed; fixed: the mapping applies to github only"},
{"file":"crates/takt-cli/tests/git_relay_served.rs","line":82,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"served_binary does not check the binary's version, unlike connectors_served_instance; left as is, CI pins the archive by sha256"}
]
```
