# Wave 54: four finding stories

Stage: **active**. `wave/54` branches from 410aaddd2, the release of 0.31.0, and merges `store/close-wave-53` (the
close-out of wave 53) first. Skill version 0.26.0 (aep:implementing, wave mode). Approved:
approval-record:wave-54-four-findings. Release: 0.32.0.

## Selection

Every planned wave is released, so four stories filed from findings enter, accepted by
approval-record:wave-54-four-findings. Each was checked as still present on `main` at 410aaddd2.
`aep plan artifact waves --status active` after their scope entries:

```
wave 1
  story:a-git-read-error-leaves-no-process
  story:gate-flag-names-a-protocol-action
  story:resume-names-a-missing-branch
wave 2
  story:test-cmd-checked-before-open
collision: story:gate-flag-names-a-protocol-action story:test-cmd-checked-before-open crates/takt-cli/src/main.rs
unassessed: story:harness-child-confined
2 wave(s), 1 collision(s), 1 unassessed
```

The collision is handled by order: the units run one after the other in one tree, and d starts after b is committed.
story:harness-child-confined stays out: dependency-blocker:loom-child-confinement is open.

The same record settles other drafts. story:decline-test-orders-cases-by-random-nonce (delivered in wave 52),
story:a-session-error-without-subtype-says-why (stale) and story:an-open-pull-request-is-found-on-resume (delivered by
0.31.0) are rejected. story:a-second-run-on-a-tree-is-refused-on-0-24 is merged into story:one-run-per-tree and
rejected as its duplicate. decision-blocker:excluded-files-and-user-excludes asks which of
story:git-excluded-files-are-not-step-changes and story:git-reads-no-user-excludes holds. The other drafts stay drafts.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:a-git-read-error-leaves-no-process | a failed read of git's output still kills and reaps the child and joins its stderr reader | cited | active |
| c | story:resume-names-a-missing-branch | a resume whose case branch is gone says so and names the branch | cited | active |
| b | story:test-cmd-checked-before-open | a `--test-cmd` that cannot start, or holds shell quoting, is refused before a case opens | cited | active |
| d | story:gate-flag-names-a-protocol-action | `--gate` names an approval-required action of the protocol, and `--case-id` beside `--case` is refused | cited | active |

Order: a, c, b, d. Each unit re-points the citations in `ess/domains/` its change moves.

Targeted tests, each unit's new cases plus:

- a: `cargo test -p b10x-takt --test git_env` and the new test target
- c: `cargo test -p b10x-takt --test case_id_branch_name` and the new test target
- b: `cargo test -p b10x-takt --test gate_validator` and `cargo test -p b10x-takt-cli` on the new test target
- d: `cargo test -p b10x-takt-cli --test gate_cli --test loop_case_cli` and the new test target
- each: `cargo fmt --all --check`, `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets --locked -- -D warnings`,
  the citations check and `ess specify validate --path ess`

Unit terms: a unit reports at a `target/` of 8G or 20 minutes in cargo, and stops at 10G or 30 minutes. One pre-check
per unit; none starts under 45G free on `/`.

## Gate plan

Once all four units and the adversary's fixes are in, on `wave/54`, one stage at a time, with two build slots. The
build cache is discarded after every stage. Minutes count time inside cargo, not the wait for a build slot. At a
stage's start only free space on `/` and the load average apply.

| stage | named peak | start floor | stop |
|---|---|---|---|
| `task check-static` | 3G | 45G | `/` under 40G |
| `task test-cli` | 15G | 60G | `target/` over 17G, 44 cargo minutes, or `/` under 40G |
| `task test-library` | 44G | 84G | `target/` over 46G, or `/` under 40G |

`task test-library` starts only after the host's operators clear its build slot. Then one pull request through the bot
route, its merge, the tag and the release.
