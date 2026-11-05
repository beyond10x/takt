# Wave 53: four finding stories

Stage: **delivered**. Released as 0.31.0 from 410aaddd2, the merge of pull request 60. `wave/53` branches from dfae174f3, wave 52's close on `main`. Skill version 0.26.0
(aep:implementing, wave mode). Approved: approval-record:wave-53-four-findings. Release: 0.31.0.

## Selection

Every planned wave is released, so four stories filed from findings enter, accepted by
approval-record:wave-53-four-findings. `aep plan artifact waves --status active` after their scope entries:

```
wave 1
  story:a-slow-runner-is-no-late-ci-result
  story:merge-is-not-sent-after-a-merged-read
  story:pr-open-cut-off-by-a-crash-is-read-back (inferred)
  story:program-rewritten-through-a-link-refuses-the-step (inferred)
unassessed: story:harness-child-confined
1 wave(s), 0 collision(s), 1 unassessed
```

story:harness-child-confined stays out: dependency-blocker:loom-child-confinement is open.
story:the-token-rule-judges-request-counts stays a draft: its body states no request tolerance and no bound for
"a minority of cases", and it changes the saved scorecard's rule. The other drafts stay drafts.

## Units

| unit | story | objective | scope | stage |
|---|---|---|---|---|
| d | story:a-slow-runner-is-no-late-ci-result | a loaded runner does not turn the one-cap CI result case red, and a run a whole interval late still does | cited | proposed |
| a | story:merge-is-not-sent-after-a-merged-read | no merge is sent after the read before it finds the pull request merged, closed or at another head | cited | proposed |
| b | story:pr-open-cut-off-by-a-crash-is-read-back | the command, GitLab and local forges find an open pull request for a branch, or refuse a second open | inferred | proposed |
| c | story:program-rewritten-through-a-link-refuses-the-step | a step that rewrites the program behind a link in a program directory is refused | cited, one line inferred | proposed |

The units share no path. Units a and b both touch `validator.rs`: a in the merge observation, b, if at all, in
`pr.open`. The two functions share no lines. Each unit re-points the citations in `ess/domains/` its change moves.
The units run one after the other in the order d, a, b, c, since one tree builds at a time. Unit c changes the test
support every `b10x-takt` test binary links, so it runs last.

Targeted tests:

- d: `cargo test -p b10x-takt --test ci_wait_backoff`, then the one-cap case 20 times under `nice -n 10`
- a: `cargo test -p b10x-takt --test fenced_forge_dispatch --test w50_adv_forge_write` and the new case
- b: `cargo test -p b10x-takt --test pr_open_after_a_lost_answer --test command_forge --test gitlab_api --test local_forge`
  and the new cases, one per forge
- c: `cargo test -p b10x-takt --test w50_sec_unattributed --test tests_have_a_home_of_their_own`
- each: `cargo fmt --all --check`, `cargo clippy -p b10x-takt -p b10x-takt-cli --all-targets --locked -- -D warnings`,
  the citations check and `ess specify validate --path ess`

Gate: once all four units are in, on `wave/53`, one stage at a time: `task check-static`, then `task test-library`,
then `task test-cli`, each step's own exit status recorded here. The gate script samples free space on `/` every
30 s and writes the minimum to its summary. Then one pull request through the bot route, its merge, the tag and the
release.

Gate plan, agreed with the host's operators before the first stage. The build cache is discarded after every stage, so
each stage builds on its own. Each stage names its peak `target/`. It starts only when `/` holds the peak and 40G more,
or 50G more while another build runs beside it. A stage that cannot start waits for space and does not start early. A
sampler every 30 s stops the running stage at `/` under 40G or at a `target/` over its named peak, and ends the gate.
Minutes count time inside cargo, not the wait for a build slot. The disk steward set the `task test-library` row:
its floor is its peak and 25G, and its sampler stops it at `/` under 25G.

| stage | named peak | cargo minutes, estimated | start floor alone | beside a build | basis |
|---|---|---|---|---|---|
| `task check-static` | 3G | 1 | 43G | 53G | wave 52: 783M, 29 s |
| `task test-library` | 44G | 10 | 69G | 69G | wave 52: 39G at stage end, 44G read by the disk steward; its stop line on `/` is 25G |
| `task test-cli` | 30G | 9 | 70G | 80G | first run alone after a discard; wave 52 ran it on the library's build |

`task test-cli` started at a floor of 60G with a stop at 25G, set by the host's operators when the stage started. At a
gate stage's start, the I/O pressure line and the count of other builds were not applied; the build slot governs those.

## Gate

| stage | head | exit | wall | cargo | peak `target/` | lowest free on `/` | result |
|---|---|---|---|---|---|---|---|
| `task check-static` | f54dd0c3a | 0 | 41 s | 41 s | 1G | 74G | every step 0 |
| `task test-library` | f54dd0c3a | 101 | 1067 s | 1067 s | 39G | 29G | 2056 passed, 3 failed |
| targeted, after the fix | 9e760b2fe | 0 | | 69 s | 2.4G | | `gitlab_ci_wait` 3/3, `gitlab_api` 6/6, `gitlab_forge_cli` 6/6 |
| `task test-cli` | 9e760b2fe | 101 | 1711 s | 1280 s | 8G | 33G | 921 passed, 2 failed |
| targeted, after the fix | the next commit | 0 | | 25 s | 2.2G | | `protocol_defaults_cli` 14/14, fmt 0 |

- **`test-library`**: the three failures were in `gitlab_ci_wait.rs`. Its `glab` double answered no list of open
  merge requests, which the GitLab forge now reads before `pr.open` opens one. The doubles were fixed, changing
  fixtures only. approval-record:wave-53-library-stage-rerun counts the stage on the targeted run.
- **`test-cli`**, `protocol_defaults_cli`: the case refused the open by making the hub's `takt` a file. The new
  look for an open request now fails on that path first. The case now makes the records directory read-only, so the
  open itself is refused.
- **`test-cli`**, `finding_scripts::the_predicate_reaches_no_network_ipc_or_socket_of_the_host`: from the host. The
  gate's machine had the `ipip` module loaded, so every new network namespace held `tunl0` beside `lo`. No commit of
  the wave touches that case. CI runs the full gate on the pull request.

## Delivery

Pull request 60 passed its 7 checks on CI, `finding_scripts` among them: the host gap did not reach CI. It was merged as
410aaddd2 and tagged 0.31.0. The Release run published the GitHub Release with its four assets.

The stories' close-out is a store-only commit on `store/close-wave-53`, cut from 410aaddd2. It rides along with the
next takt wave, whose integration branch merges it.
