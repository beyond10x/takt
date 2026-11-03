# Wave 51: the harness mapping carries effort, the cost cap, the version check and context

Stage: **running**. `wave/51` branches from 1c3ba5178, 0.29.0's release close on `main`. Skill version 0.26.0
(aep:implementing, wave mode). Approved: approval-record:wave-51-harness-mapping-carried. Release: 0.30.0.

## Selection

`aep plan artifact waves --kind story --status active` (aep 0.79.0) after the wave's scope entries:

```
wave 1
  story:harness-section-is-metaharness-session-request (inferred)
wave 2
  story:harness-flags-spell-metaharness-names
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt-cli/src/main.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt-cli/tests/harness_flags_spell_metaharness.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt/src/run_config.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request ess/ess-inputs.yaml
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request website/docs/reference/configuration.md
unassessed: story:harness-child-confined
2 wave(s), 5 collision(s), 1 unassessed
```

Three stories are active. story:harness-child-confined leaves the wave: dependency-blocker:loom-child-confinement is
open. The other two collide on five paths and one `depends_on` the other, so they run in order inside the wave: unit b
starts from `wave/51` once unit a has merged there. Both upstream blockers of the two stories are cleared, and the
pins already in `Cargo.lock` carry what they need (loom 0.52.0 `SessionOverrides`, metaharness 0.23.0
`RunSpec::from_document` and `check_spec`); no pin moves.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:harness-section-is-metaharness-session-request | `effort`, `max-budget-usd`, `strict-version` and `context` reach the session through Loom's overrides; a bad value and a per-kind refusal are metaharness's own words, before any case opens | cited | merged at 470afdf57 and, after review fixes, at 16d44a200; tree cache discarded |
| b | story:harness-flags-spell-metaharness-names | each flag's observable on `takt run` and `takt scenario run`; `--harness codex --max-turns` refused in metaharness's words; help and reference without "refused until" | cited, one line inferred | implementing, base 16d44a200 |

| unit | agent | branch | tree | build dir | scratch root |
|---|---|---|---|---|---|
| a | `aep:implementor` | `unit/51-a` | `takt-w51-a` | the tree's `target/` | the tree's `target/scratch` |
| b | `aep:implementor` | `unit/51-b` | `takt-w51-b` | the tree's `target/` | the tree's `target/scratch` |

Trees are managed worktrees under the worktree manager's `takt` directory; the integration tree is `takt-w51`.
Reviews: `aep:adversary` on a and b, `aep:security-reviewer` on a (what a caller's mapping may reach in the session),
each against `wave/51` after the unit merges.

Decisions taken for the units:

- `context` fills Loom's `SessionOverrides.context`: repository-relative plain paths of files the executor copies;
  any other path is refused before any case opens, naming it. The document metaharness checks gets the repository
  as its working directory.
- `max-budget-usd` is read as a number or a decimal string and carried in micro-dollars; a value finer than a
  micro-dollar is refused, naming the key.
- The conformance scenarios the acceptance names in `ess/ess-inputs.yaml` stay unrun, as for
  story:model-names-an-llm-route: this repository has no conformance runner. Each scenario is a cargo test of the
  scenario's name.

Targeted tests:

- a: `cargo test -p b10x-takt-cli --test run_config_harness_section --test harness_flags_spell_metaharness`,
  `cargo test -p b10x-takt --lib run_config`, `cargo test -p b10x-takt --test harness_preflight --test harness_proposes`
- b: `cargo test -p b10x-takt-cli --test harness_flags_spell_metaharness --test run_config_harness_section --test config_docs_adv_carry`,
  `cargo test -p b10x-takt --lib run_config`, `cargo test -p b10x-takt --test config_docs_adv_carry`
- each: `cargo fmt --all --check`, `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets --locked -- -D warnings`,
  `ess specify validate --path ess`

Gate: once both units are in, on `wave/51`, one stage at a time with `worktree discard-cache` between stages:
`task check-static`, then `task test-library`, then `task test-cli`, each step's own exit status recorded here. Then
one pull request through the bot route, its merge, the tag and the release.

## Reviews

| review | unit | outcome |
|---|---|---|
| review-result:w51-sec-a-1 | a | fixed: a context file whose text has a credential's shape is refused before any case opens |
| review-result:w51-adv-a-1 | a | fixed: the scenario seed is the tree a context path is checked against; metaharness's byte cap and text rule apply before any case opens; a budget number below 1e-5; a refusal names only the refused flags |

Unit a's correction is attacked again by unit b's adversary pass, which runs against `wave/51` with both units in.

Filed as drafts, outside the wave (tag `from-finding`):
story:the-refused-path-proposal-case-holds-on-every-host (from unit a's red run on the unchanged base: one
`harness_proposes` case red on the wave's host, green in CI at the same source).

Commits this wave makes: the opening store and page commit, one or more commits per unit, the merges into `wave/51`,
the review and evidence store commits, the release prepare, one pull request and its merge to `main`, the tag.
