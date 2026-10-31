---
format: aep.planning-md/3
id: review-result:pr-49-grant-review
kind: review-result
status: active
title: Grant review, pull request 49 at d4ce8808
summary: 'approve: story:scorecard-refuses-missing-repo at d4ce8808'
relations:
- reviews: story:scorecard-refuses-missing-repo
revision: 1
---
approve

Grant review of pull request 49, branch `story/scorecard-refuses-missing-repo` at `d4ce8808`, against the acceptance of story:scorecard-refuses-missing-repo as tightened at `a8cf8293`.

- Paths: `.engineering/planning/story/scorecard-refuses-missing-repo.md`, `crates/takt/src/scorecard.rs`, `crates/takt/tests/scorecard.rs`, `ess/domains/takt.yaml`. All four are judge surfaces. None is on the widening list in `.agents/planner-agent.md` § Never, so the diff does not widen authority.
- Run in a managed worktree checked out at `d4ce8808`: `cargo test --locked -p b10x-takt --test scorecard`, 25 passed, 0 failed. `ess specify validate --path ess`: valid. `aep plan artifact validate`: valid.
- Live, with `target/debug/takt` built at `d4ce8808`: `takt scorecard --repo ./no-such-repo --save <dir> --revision <40 hex>` exits 1, prints 0 bytes on standard output, says `repository ./no-such-repo is not an existing directory` on standard error, and creates neither the save directory nor the repository path. A regular file as `--repo` gives the same result. An initialised repository with no cases prints `0 cases, 0 runs` and exits 0.
- `git diff a8cf8293..d4ce8808 -- crates/takt/tests/scorecard.rs` adds 53 lines and removes none. No existing test changed.
- The two moved citations name the same text at `d4ce8808` as before at `a8cf8293`: lines 2015-2046 there are lines 2018-2049 here, and 2022-2024 are 2025-2027. The diff of the two ranges is empty. No other citation in the repository points into `scorecard.rs` past line 2014.

```findings
[
{"file":".engineering/planning/story/scorecard-refuses-missing-repo.md","line":19,"category":"integrity","severity":"note","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"the run's proposed -> active move is recorded with actor human:unknown and executor agent:takt, though no person made it. takt's take (crates/takt/src/frontier.rs:680-690) runs aep move through plan(), which sets no AEP_ACTOR, unlike record_approval through plan_as. Not in this diff's code; filed as story:take-records-who-moved-the-story"},
{"file":"crates/takt/src/scorecard.rs","line":2015,"category":"acceptance","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the refusal is the first statement of report, before Scorecard::load, Scorecard::read, any write to out and any save. Live: exit 1, nothing on standard output, no save directory, the path named on standard error, for a missing path and for a regular file"},
{"file":"crates/takt/tests/scorecard.rs","line":57,"category":"acceptance","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"three tests added, none changed (+53/-0): the refusal with and without --save for a missing path and a file, an existing repository with no cases printing 0 cases, and Scorecard::read still taking a missing repository as empty. 25 of 25 pass at d4ce8808"},
{"file":"ess/domains/takt.yaml","line":2828,"category":"citation","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"scorecard.rs:2022-2024 -> 2025-2027 and 2015-2046 -> 2018-2049 (line 3428) name identical text at d4ce8808; ess specify validate: valid"}
]
```
