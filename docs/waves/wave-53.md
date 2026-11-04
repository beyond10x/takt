# Wave 53: four finding stories

Stage: **proposed**. `wave/53` branches from dfae174f3, wave 52's close on `main`. Skill version 0.26.0
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
