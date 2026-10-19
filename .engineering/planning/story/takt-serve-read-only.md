---
format: aep.planning-md/3
id: story:takt-serve-read-only
kind: story
status: draft
title: takt serve answers the declared views on localhost and writes nothing
summary: a read-only local HTTP surface over case files, held to the model by ess verify conform
relations:
- decomposes: epic:terminal-frontend
- serves: vision:safe-agentic-coding
- depends_on: story:tui-read-model
- depends_on: story:judge-guard
- depends_on: story:self-improvement-protocol
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-serve
- confidence: inferred
  path: crates/takt-serve/tests/fixtures
revision: 7
---
# Story: takt-serve-read-only

## Outcome

`takt serve --repo <path>` serves the read views of takt's model over HTTP on 127.0.0.1 only, reading the repository's case files, and writes nothing: no case, no store, no git, no route for any method but GET. `ess verify conform` holds it to the model.

## Context

epic:terminal-frontend, step 3. Depends on story:tui-read-model. The generated terminal app reads over `http://` only (beyond10x/ess 0.60.0 `crates/ui/ess-ui-tui/src/http.rs:171-172`), and takt's state is files.

Security:
- bound to 127.0.0.1 on a port given by `--port` or chosen free when 0, printed on stdout as the first line so `takt tui` can read it;
- no write route: every non-GET request answers 405, every undeclared path 404;
- reads only files under `--repo`, through `takt::case` and `takt::events`; a path that resolves outside `--repo` (a link included) is refused, never followed;
- no credential read or served: no authority beyond the case files' own content, no environment value in any answer;
- the case data it serves is what any process of the user can already read from the files; it adds no write and no new reach.

## Expected before

On takt at main, holds; after the change, fails:

- `takt serve --help` exits non-zero (no such command).

## Scope

- `crates/takt-serve/**`: a new library crate holding the read-only server (decided: a crate, not a module of `crates/takt`, so it lands on none of story:judge-guard's `crates/takt/src` files) (inferred)
- `crates/takt-cli/src/main.rs`: the `serve` arm only, dispatching to `takt_serve::run` (inferred; shared with story:judge-guard and story:self-improvement-protocol, ordered after both by `depends_on`)
- `Cargo.toml`, `Cargo.lock`: the new workspace member (inferred)
- `crates/takt-serve/tests/fixtures/**`: the expected bodies, the same files `ess/ui/fixtures/` holds (inferred)

## Acceptance

`cargo test -p b10x-takt-serve --test views` passes, where the test starts `takt serve --repo <the fixture repository> --port 0` and checks:

- every GET route of `ess/http/takt.http.yaml` answers 200 with a JSON body byte-equal (after key-sorted canonical encoding) to its committed expected body, the same file as the matching `ess/ui/fixtures/` entry (a check fails when the two copies differ);
- a POST, PUT, PATCH or DELETE to any path answers 405, an undeclared GET path 404;
- a case directory replaced by a link to a directory outside the repository is refused, not served;
- the repository's tree digest (`git ls-files -s` plus the sha256 of every untracked file under `.engineering/`) is equal before and after every request above.

`ess verify conform` is not part of this acceptance: ESS 0.60.0 has no HTTP target (`ess verify conform run --target` takes `billing`, `oracle-fixture`, `interpreted`).
