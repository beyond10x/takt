# Wave 48: Loom 0.52.0, the connectors forge at a served instance, git through its relay

Stage: **reviews done (six review-results); b1 and c1 wait for connectors `v0.62.0`**. Units a, b-stand-in and c-stand-in
are committed on `wave/48` (from 1c0e692c, 0.24.0's merge), head 778cb31e. Skill version 0.25.0
(aep:implementing, wave mode).

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:git-and-model-through-loopback-relays, Acceptance 2 and 4 | a keyless loopback harness session on Loom 0.52.0 | cited | committed 1b2cf797; review pending |
| b | story:connectors-forge-at-a-served-instance, Acceptance 2, 4, 5 | keyed served writes | cited | committed 5791d8fe; review pending |
| b1 | story:connectors-forge-at-a-served-instance, Acceptance 1 and 3 | the forge against connectors 0.62.0's served instance | inferred | waits for the connectors 0.62.0 tag |
| c | story:git-through-the-connectors-git-relay, Acceptance 3 and 4 | relay refusals name the ref and reason; no token in git children | cited | committed 778cb31e; review pending |
| c1 | story:git-through-the-connectors-git-relay, Acceptance 1 and 2 | fetch, push and `trial/<n>` through 0.62.0's relay | inferred | waits for the connectors 0.62.0 tag; after b1 |

Ride-along: story:implementation-prompt-shows-the-tests-path implemented on its recorded self-run
(docs/runs/2026-10-30-self-run-tests-path.md).

## Stage 2 plan

1. Review pass 1, three trees at once: `aep:adversary` on a, b and c, one per unit.
2. Review pass 2, three trees at once after pass 1's trees are finished: `aep:security-reviewer`
   on a, b and c.
3. Fixes for red findings: `aep:implementor`, one tree per unit, one at a time.
4. b1, then c1 (both add a CI install step to `.github/workflows/check.yml`): `aep:implementor`,
   once connectors 0.62.0 is tagged, the asset pinned by sha256.
5. Merge each unit branch into `wave/48`; push; one PR through the bot; CI's full gate decides;
   evidence and moves; store commit; merge to main.

| unit | branch | tree | build dir | scratch root |
|---|---|---|---|---|
| adv-a, adv-b, adv-c | `rv/48-adv-<u>` | `~/.local/state/worktree/trees/b10x/takt/takt-w48-adv-<u>` | the tree's `target/` | the tree's `target/scratch` |
| sec-a, sec-b, sec-c | `rv/48-sec-<u>` | `~/.local/state/worktree/trees/b10x/takt/takt-w48-sec-<u>` | the tree's `target/` | the tree's `target/scratch` |
| b1, c1 | `unit/48-<u>` | `~/.local/state/worktree/trees/b10x/takt/takt-w48-<u>` | the tree's `target/` | the tree's `target/scratch` |

Build limits in every brief: `cargo test -p <crate> --test <target>` or `cargo build -p <crate>`
for touched crates only; no `--workspace`, no root `cargo test`, no local full gate;
`CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`; every `takt run` in a test passes
`--test-cmd true`; `cargo clean` a tree whose `target/` passes 15G; stop under 35G free.

## Stage 2 progress

| unit | branch, head | stage |
|---|---|---|
| adv-a | `rv/48-adv-a` b7c34698 | 8 cases green, one note; merged into `wave/48`; tree finished |
| adv-b | `rv/48-adv-b` f1b329cb | 4 red (one blocker: refusal count not kept across a resumed run), fixed; merged 1109b070; tree finished |
| adv-c | `rv/48-adv-c` | 5 red (one warning: trial path sends origin's userinfo), fixed; merged a9e440f7; tree finished |
| sec-a | `rv/48-sec-a` | 3 cases green, no findings; merged 24968c40; tree finished |
| sec-b | `rv/48-sec-b` | 4 red (refusal record guards, unkeyed fallback), fixed; merged 2b09c607; tree finished |
| sec-c | `rv/48-sec-c` | 4 red (global config, netrc, included extraHeader, scoped followRedirects reach the relay), fixed; merged 3ae1a408; tree finished |

## Pre-flight (2026-10-30)

- `/`: 98G free. wave/48 `target/`: 3.8G. Other takt build dirs: 51M, 128K, 8K.
- Managed takt trees active: takt-wave-48 and two not this wave's (wt-5cf544ff3044,
  takt-plan-async-default).
- sccache installed, unset; cold builds of two crates per tree.

## Selection

`aep plan artifact waves --kind story --status active` (aep 0.78.0), the lines naming this wave's
stories:

```
wave 1
  story:connectors-forge-at-a-served-instance (inferred)
wave 2
  story:git-and-model-through-loopback-relays
wave 5
  story:git-through-the-connectors-git-relay (inferred)
collision: story:connectors-forge-at-a-served-instance story:git-and-model-through-loopback-relays CHANGELOG.md
collision: story:connectors-forge-at-a-served-instance story:git-and-model-through-loopback-relays website/docs/reference/configuration.md
collision: story:connectors-forge-at-a-served-instance story:git-through-the-connectors-git-relay .github/workflows/check.yml (inferred)
collision: story:connectors-forge-at-a-served-instance story:git-through-the-connectors-git-relay ess/domains/takt.citations.tsv
collision: story:connectors-forge-at-a-served-instance story:git-through-the-connectors-git-relay ess/domains/takt.yaml
collision: story:git-and-model-through-loopback-relays story:git-through-the-connectors-git-relay crates/takt-cli/tests/loopback_relays_cli.rs
```

The three stories collide on shared files, so they are serial on one integration branch: a, b and
c were committed one after another, b1 runs before c1, and the review passes write test files
only. Left out: every other active story (not part of this wave's release goal).

## Open findings carried in

- A `GIT_CONFIG_PARAMETERS` value reaches remote git children by design; a token placed there
  would too.
- The credential filter matches variable names, not values.
- A relay answer of 413 is not named in the stop line.
- After `outcome_unknown` on an open, no read-back can find the pull request (no list
  operation), so the keyed resend is the only recovery.
- Found in review: the temporary file of three other atomic writes (case.rs, forge/local.rs,
  promotion.rs) follows a planted link; story:atomic-writes-follow-no-link (draft).

## Commits this wave makes

Review-test and fix commits on the unit branches, b1 and c1, their merges into `wave/48`, the
closing store commit, pushes of `wave/48` as units land, the bot PR and its merge into `main`, and
release 0.25.0 through takt's own process. b1 and c1 start once connectors `v0.62.0` is tagged;
reviews do not hold a merge.

## Log

- (a) Loom 0.52.0's session executor starts a b10x-harness session whose endpoint is on a
  loopback host and names no credential; 0.51.0 refused it. The pin needed no takt change. The
  two tests with a double were red on 0.51.0 and are green on 0.52.0; the wire test needs a real
  b10x-harness, which CI does not install, and is ignored there.
- (b) and (c): connectors 0.62.0 ships a served instance's keyed writes under a standing grant and
  its git smart-HTTP relay in one release. Both stories test against that release's served
  instance, installed from its asset in the test and in CI.
