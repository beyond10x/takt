# Wave 44: three stories delivered by takt, each merging into one integration branch

Stage: **running**. Shape: one coordinator and three takt runs at once, each on the local forge
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
| U1 | story:unit-tests-run-steps-over-the-operators-path | the CLI crate's tests and the guard tests | proposed |
| U2 | story:docs-edits-the-tests-read-are-attempts | the library's unit tests and the repeated-failure tests | proposed |
| U3 | story:duplicate-key-reader-limits | the run configuration's tests | proposed |

The three stories' scopes do not share a source file: `crates/takt-cli/src/main.rs` (U1),
`crates/takt/src/run.rs` and `ess/domains/takt.yaml` (U2), `crates/takt/src/run_config.rs` (U3).

## Log
