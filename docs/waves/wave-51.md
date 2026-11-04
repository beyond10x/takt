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
| b | story:harness-flags-spell-metaharness-names | each flag's observable on `takt run` and `takt scenario run`; `--harness codex --max-turns` refused in metaharness's words; help and reference without "refused until" | cited, one line inferred | merged at dd2e14194 and, after review fixes, at 54d05a31c; tree cache discarded |

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
| review-result:w51-adv-b-1 | b and a's correction | fixed: under Codex the prompt fit counts the `context` files in the same argument; `takt run` checks `context` against the branch of the case it resumes; the `--context` help says what each run command checks |

Unit a's correction was attacked again by unit b's adversary pass, against `wave/51` with both units in. Findings
fell from 5 in the first pass to 3, none carried; the correction answering the second pass was checked by reading its
diff (no assertion dropped, no test edited), with no third attack.

## Unit builds

One tree built at a time, each in its own `target/`, line tables only, incremental off. Each unit had one cold build;
every later cargo invocation reused it, and no `cargo clean` ran.

| unit | cargo invocations | peak `target/` | result |
|---|---|---|---|
| a | 4 initial (red 63 s, red 4 s, green 29 s, clippy 42 s), 2 for fix 1, 1 security review, 3 adversary | 3.1G | green, but for the `harness_proposes` case red on the unchanged base |
| b | 5 initial (red 140 s, fmt 3 s, green 15 s, clippy 53 s, fmt 3 s), 1 adversary, 3 for fix 2 | 3.3G | green, the same exception |

Filed as drafts, outside the wave (tag `from-finding`):
story:the-refused-path-proposal-case-holds-on-every-host (from unit a's red run on the unchanged base: one
`harness_proposes` case red on the wave's host, green in CI at the same source). It duplicates
story:a-relative-path-entry-is-no-test-program-directory from wave 50, and says so; it stays a draft.

## Gate

On `wave/51` at ea4847516, one stage at a time, each in the tree's own `target/`, line tables only, incremental off.
`test-cli` reused `test-library`'s build; the cache was discarded after `check-static` and after `test-cli`.

| stage | exit | duration | `target/` | result |
|---|---|---|---|---|
| `task check-static` | 0 | 33 s | 783M | every step 0 |
| `task test-library` | 101 | 525 s | 39G | 2026 passed, 1 failed |
| `task test-cli` | 101 | 279 s | 44G | 918 passed, 1 failed |

The two red cases, `harness_proposes::a_scripted_proposal_of_a_refused_path_leaves_the_tree_unchanged` and
`decline_adv1::a_decline_after_a_kept_edit_holds_on_resume`, are the two story:a-relative-path-entry-is-no-test-program-directory
names: the gate's shell had `.` as the fourth `PATH` entry. On the same build with no relative entry on `PATH` both
binaries passed, 11 of 11 and 14 of 14. Neither test file changed in this wave. The gate counts as green on that ground.

Commits this wave makes: the opening store and page commit, one or more commits per unit, the merges into `wave/51`,
the review and evidence store commits, the release prepare, one pull request and its merge to `main`, the tag.
