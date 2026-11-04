# Wave 52: a relative PATH entry and two trials in one second

Stage: **delivered**, released as 0.30.1 from 0c28f9c8f, the merge of pull request 58. `wave/52` branches from
bbaea844b, wave 51's close on `main`. Skill version 0.26.0
(aep:implementing, wave mode). Approved: approval-record:wave-52-two-findings. Release: 0.30.1.

## Selection

Every planned wave is released, so two stories filed from findings enter, accepted by
approval-record:wave-52-two-findings. `aep plan artifact waves --kind story --status active` after their scope entries:

```
wave 1
  story:a-relative-path-entry-is-no-test-program-directory (inferred)
  story:two-trials-started-in-one-second-have-no-case-order
unassessed: story:harness-child-confined
1 wave(s), 0 collision(s), 1 unassessed
```

story:harness-child-confined stays out: dependency-blocker:loom-child-confinement is open. The duplicate
story:the-refused-path-proposal-case-holds-on-every-host is rejected by the same approval. The other drafts stay drafts.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| a | story:a-relative-path-entry-is-no-test-program-directory | a test binary's `PATH` keeps no relative entry, so `.` on the caller's `PATH` turns no case red | cited, one line inferred | a32b47826, review fix in the wave's fix commit |
| b | story:two-trials-started-in-one-second-have-no-case-order | the decline case does not depend on the order of two trial ids taken in one second | cited | ec855c034, review fix in the wave's fix commit |

The units share no path. They run one after the other, since one tree builds at a time.

Targeted tests:

- a: `cargo test -p b10x-takt --test tests_have_a_home_of_their_own --test harness_proposes` and
  `cargo test -p b10x-takt-cli --test decline_adv1`, each once with `.` first on `PATH` (red before the change) and
  once without
- b: `cargo test -p b10x-takt-cli --test decline_adv1`, then its built binary run 20 times
- each: `cargo fmt --all --check`, `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets --locked -- -D warnings`

Gate: once both units are in, on `wave/52`, one stage at a time: `task check-static`, then `task test-library`,
then `task test-cli`, each step's own exit status recorded here. Then one pull request through the bot route, its
merge, the tag and the release.

## Reviews

| review | units | outcome |
|---|---|---|
| review-result:w52-adv-ab-1 | a and b | fixed: a `PATH` with no absolute directory is refused before `main`; the wrap past 99999999 is documented; the adversary's cases on the free search and on built `PATH`s are kept |

## Unit builds

Both units in the integration tree, one after the other, on one `target/`, line tables only, incremental off.

| unit | cargo | peak `target/` | result |
|---|---|---|---|
| a | about 2.8 min, one cold build | 2.8G | green; red with `.` first on `PATH` before the change |
| b | warm, on unit a's build | 3.0G | green; `decline_adv1` 20 runs of 20 |
| review and fix | warm | 3.1G | green |

## Gate

On `wave/52` at bff5b77e8, one stage at a time, line tables only, incremental off, from a shell whose `PATH` holds `.`.
`test-cli` reused `test-library`'s build; the cache was discarded after `check-static` and after `test-cli`.

| stage | exit | duration | `target/` | result |
|---|---|---|---|---|
| `task check-static` | 0 | 29 s | 783M | every step 0 |
| `task test-library` | 0 | 575 s | 39G | 2031 passed, 0 failed |
| `task test-cli` | 0 | 343 s | 44G | 923 passed, 0 failed |

The two cases that were red in the gates of waves 50 and 51 under that `PATH` passed.

Free space on `/`: 111G at the start. At the end of each stage it was 111G, 80G and 82G, and 126G after the last discard.
The host's own sampler saw 72G during `test-cli`, at 04:05Z, which the samples at stage ends missed.

## Delivery

Pull request 58 merged as 0c28f9c8f and tagged 0.30.1; the Release run passed at its first attempt. Both stories moved
to implemented on the `check` run of `main` at the merge.

For the next wave: the gate script samples free space on `/` every 30 s for as long as it runs and writes the minimum
to its summary. The sampler ends with the gate. This gate's samples at stage ends missed the 72G low.
