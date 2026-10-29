# Wave 44: three stories delivered by takt, each merging into one integration branch

Stage: **gate green, pull request next**. Shape: one coordinator and three takt runs at once, each on the local forge
(`--forge local`). The integration branch `integration/wave-44`, from 463c349f (the 0.20.0
release), is held in a bare repository on this machine, the hub; each run works in its own
`git clone --no-local` of the hub, opens its request there, runs its `local_checks` and, on a
grant of `repository.merge`, merges into the hub's `integration/wave-44`. The coordinator grants
each merge after an adversary pass on the request's head, then reviews the integration branch,
pushes it and opens one pull request, then one release.

The runs use takt 0.20.0 with Codex (`gpt-6.1-terra`), story.delivery/3 with tests first and the
gate after the handover, and no confinement; the confined repeat is a later wave.

## Units

| unit | story | local_checks | stage |
|---|---|---|---|
| U1 | story:unit-tests-run-steps-over-the-operators-path | the CLI crate's tests and the guard tests | merged by takt (#1) |
| U2 | story:docs-edits-the-tests-read-are-attempts | the library's unit tests and the repeated-failure tests | merged by takt (#3) |
| U3 | story:duplicate-key-reader-limits | the run configuration's tests | merged by takt (#2) |
| U4 | task:loom-0-49-1 | an implementor in its own worktree | merged |
| U5 | story:console-shows-reads-and-diffs | an implementor in its own worktree | merged |

The three stories' scopes do not share a source file: `crates/takt-cli/src/main.rs` (U1),
`crates/takt/src/run.rs` and `ess/domains/takt.yaml` (U2), `crates/takt/src/run_config.rs` (U3).

## Log

- U4 joins: loom 0.49.1, engineering-protocols 0.13.0 and Canon 0.7.0 (task:loom-0-49-1), by an
  implementor in its own worktree; green, in adversary pass 1. It merges after the three runs,
  since a base the hub moves meanwhile refuses a run's merge.
- U4's adversary pass 1: on the default story.delivery/2 of engineering-protocols 0.13.0, a
  failing test run stamped in the same second as the first tests change leaves the edit blocked
  after a red gate, since Canon 0.7.0's `after` compares whole seconds. U4 waits for the next
  second before stamping; the wait stays until a Canon release with a sub-second `after` and the
  engineering-protocols release on it are pinned, and goes in that wave.
- The three runs started at once with takt 0.20.0 from the release build, each in its own
  window of the run session.
- U2's first run declined: its scripted case named no file under `docs/`, so takt could not tell
  the fixture the tests read from documentation. The story gained a decision section (a path a
  test file names is not documentation) and U2 restarted in a fresh clone.
- U1's checks failed on the local forge: `local_checks` run in takt's test environment, which
  carries no `DBUS_SESSION_BUS_ADDRESS`, and 20 `promote_cli` cases fail there with "no Substrate
  adapter" instead of skipping (87 pass with it set). U1 resumed with its checks narrowed to the
  CLI's unit tests; a finding is filed for the cases. The case had declined on the failing
  checks, and a decline holds until a commit moves the case's revision, so the coordinator moved
  it with an empty commit (e1eff149) before the resume.
- U3's adversary pass at cd64a83a: 6 of 28 new cases red (an anchor redefined on a collection,
  a percent-escaped tag read back as YAML, a custom tag on a number, a tag decoding to a space,
  a U+2028 separator). The coordinator committed the cases on the case's branch (3eac544e) and
  resumed the case with them in its test command, so takt's next attempt starts from red tests.
- U1's first merge under the planner's grant was refused: the diff changes judge surfaces and
  `.engineering/delegations.yaml` named no delegate. The owner chose a delegation of
  `repository.merge.judge_surface` to `agent:planner` for this wave's integration branch only
  (3c38c519), revoked before the wave's pull request.
- U2 took three review rounds (2, 4 and 0 red cases). In the second, the harness renamed a
  review case and inverted its assertion to follow an older sentence of the story; the
  coordinator restored the case and left one rule in the story (story:a-step-cannot-weaken-a-review-test).
  Granted at 3f3ee40a on the code; its doc comment and the ess prose still describe the first
  rule, fixed on the integration branch after the merge.
- U4 took three implementor rounds and three adversary passes; two cases were retired by the
  planner (one wrote both records outside the evidence port, one ordered kinds no order form
  reads), the rest are kept as tests.
- U2's first merge under the grant was refused: its case started from the story's decision
  commit (29f92ef7), made in U2's clone and never on the hub, so the base was not an ancestor of
  the target's tip. The coordinator merged that commit into the integration branch (26c17550)
  and resumed the case.
- U3's checks passed at 71c38d32 against `integration/wave-44` at f790c60f, the clone's own
  copy, while the hub had moved on with U1 and U2: `change.read` takes the target's tip from the
  case tree, not the forge (story:change-read-compares-against-the-forges-tip).
- U3's third adversary pass at 71c38d32: 3 of 8 new cases red (a local-tagged key, a plain key
  with a non-core global tag, an unresolved key skipped). The planner kept "an unresolved key
  stops the reader" against an earlier case that expected a later pair's line, changed that case
  to the generic refusal and recorded both in the story; U3 resumed on the cases.
- That decision contradicted the first pass's two anchor-redefinition cases, which pass over an
  alias of a collection; U3 traded the two failures for 8 steps and stopped at its 40 steps. The
  planner restored the alias case and narrowed the rule (an alias of a collection is passed
  over; an unresolvable scalar key stops the reader). Interrupting the resumed run left its
  `run.lock` behind, removed by hand as takt's message says.
- U5 joins on the owner's request: story:console-shows-reads-and-diffs (after each harness step
  the console lists its tool calls and its commit's diff, in colour), by an implementor in its
  own worktree from d9346d8c.
- The run script passed `--input` to a story whose case was already open; takt resumes such a
  case and refuses the inputs, so a restart past a red base resumes with `RESUME`.
- The full gate on the integration branch failed twice. At 5accf67a's parent it stopped at
  `cargo fmt --all --check` on U4's third-pass cases (formatted in 5accf67a). At 5accf67a the
  confinement guard found two process starts outside the seam: U2's `code_changed` ran `git show`
  itself (it now reads the blob through `git::blob`), and U1's `#[path]` include of the tests'
  `own_home` support in the CLI's unit tests (the guard now follows a `#[cfg(test)]` include
  into a `tests/support/` file and scans it). The adversarial pass on that guard change wrote
  six red cases: four holes the change opened are closed in fc39dbac, two older ones are
  ignored as story:the-scan-misses-renamed-and-macro-made-includes.
- The documentation pass found U2's merged code reading a path boundary otherwise than the
  story's decision (`:` and `#` end a path; the top directory alone names nothing). The planner
  kept the code, after its four review rounds, and amended the story.
- The full gate passed at 17328394 (`task check`, exit 0: 315 test binaries, 2694 tests passed,
  7 ignored). The planner revoked its judge-surface delegation before the pull request.
