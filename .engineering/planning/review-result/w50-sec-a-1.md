---
format: aep.planning-md/3
id: review-result:w50-sec-a-1
kind: review-result
status: active
title: Wave 50 security review on story:fenced-forge-dispatch
relations:
- reviews: story:fenced-forge-dispatch
revision: 1
---
```
unit: a (story:fenced-forge-dispatch), tree takt-w50-a at 28f467ab9 plus one untracked test file
verdict: red
cases: executed 7→8, red 1
origin: introduced 1, pre-existing 0, undecided 1
wrote-outside-worktree: none
needs-coordinator: yes
```
**Question to the coordinator:** after an `unknown` merge, should the merge be sent again under any condition? If never, what ends a case whose lost merge never lands? Today the merge is sent again one wait later.

**Case added:** `crates/takt/tests/w50_sec_forge_write.rs::an_unknown_merge_still_in_progress_is_never_sent_again`. It uses the in-memory forge behind a wrapper. After the merge lands, the wrapper reports the pull request open at the same head for 4 reads, as GitLab does with a `locked` merge request (`gitlab.rs:591` reads `locked` as open). The first run's merge answer is lost. The case asserts that the merge is sent once across both runs; it is red, with two merge calls and steps `["unknown", "not_attempted", "applied"]`. Step 1 waits ("nothing sent in this step, the next observation merges it under the grant"), and step 2 sends the merge again.

**Suite:** `fenced_forge_dispatch` 7 passed; `w50_sec_forge_write` 0 passed, 1 failed.

**Checked and cleared:**
- A merge happens only under a grant at the case revision. The `grants_at` check runs before any push or merge, and resolving an `unknown` adds no route around it.
- A `merge_observation` of `merged` comes only from a forge read, never from the class the merge answered.
- The `forge_write` record holds only `outcome`, `pull_request` and `head`, with no error text, credential, URL or environment value. The adapter error texts keep their text at the base; only their class changed.
- In the command adapter:
  - `applied` needs exit 0, and `refused` needs an exit listed in `refused_exit`.
  - A child that was killed, timed out or failed in any other way is `unknown`.
  - `not_attempted` needs a missing entry or a program that never started, which a child cannot fake.
- In the connectors, GitLab and local adapters:
  - Every failure after the request may have been sent is `unknown`.
  - Only 4xx answers and failures before sending are `refused` or `not_attempted`.
  - A 2xx answer with the wrong request id is `unknown`.
- `run.rs` writes a step line on an error only when a write was sent.

**Rule deviations:** `cargo fmt --check` ran directly, not through the build slot. Clippy was not run.

```findings
[{"file": "crates/takt/src/validator.rs", "line": 4697, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "unknown_merge_of reads only the last repository.merge forge_write, so the waiting step's not_attempted hides the unknown and the next observation sends the merge again, contrary to the story's Outcome (test: crates/takt/tests/w50_sec_forge_write.rs::an_unknown_merge_still_in_progress_is_never_sent_again)"},
 {"file": "crates/takt/tests/fenced_forge_dispatch.rs", "line": 486, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "undecided", "message": "the unit's own test asserts two merge calls after an unknown merge, locking in a second send of an unknown write without a story: reference in the assertion"}]
```
