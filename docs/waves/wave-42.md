# Wave 42: the second real-story proof's defects and the gate tests' isolation

Stage: **closed** at the commit that adds this page. Shape: one dispatcher in one managed worktree (`takt-wave-42`),
one wave branch `wave/42`, one shared target directory, builds one at a time. The wave merges
into `integration/takt` after waves 40 and 41, with the self-run branch `loop/selfrun`, for one
pull request and one release.

The wave takes the `rsi` stories first: the defects the second real-story proof and takt's
self-runs on its own stories found.

## Units

| unit | story | commit |
|---|---|---|
| U1 | story:scan-scope-test-is-not-flaky | e1d457c5 |
| U2 | the active-time budget tests under load | 8710ba5a |
| U3 | story:gate-tests-are-deterministic | 46905fc2 |
| U4 | a failed gate keeps or counts seven more linter shapes | 007e3a6d |
| U5 | story:validator-only-run-records-no-layer | f22003b3 |
| U6 | story:red-run-is-no-repeated-failure | 6679bf09, 859a8a77 |
| U7 | story:implementation-prompt-shows-the-tests-path | 4d299030 |
| U8 | the gate tests keep their own git maintenance and aep cache | 3e21da93 |
| U9 | story:a-revision-without-code-is-no-new-attempt | 1e9cf773 |
| U10 | a red test run leads the next prompt with what failed | 6644f844 |
| U11 | the adversarial pass's findings F1 to F4 | a2f650bf |

## Log

- U1 to U10 implemented and committed; ESS declared, citations re-pointed (6545711f).
- The takt-probe refusal of proof 2 recorded as confirmed and fixed by 46905fc2 (1be96512).
- Adversarial pass at 6545711f: needs-change, 6 red cases of 38. Four were fixed in U11:
  the assertion-location block now passes the secret scan with the blocks it joins (F1), a
  located line at a host is not kept as failing (F2, F3), and alternating a new test with an
  edit that fixes nothing stops on `RepeatedTestFailure` (F4). F5 and F6 stay as ignored cases
  and are filed.
- Filed (899f5b50): the adversarial pass's F5, F6 and two judgement notes; the 0.18.1 trial's
  three findings; three from U8; and story.delivery/3 as the default protocol, from the
  self-runs.
