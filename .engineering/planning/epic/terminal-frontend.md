---
format: aep.planning-md/3
id: epic:terminal-frontend
kind: epic
status: draft
title: A read-only terminal frontend specified in ess-ui/1, served by a read-only takt serve
summary: 'spec-first TUI: views and an ess-http/1 binding, an ess-ui/1 document, takt serve read-only, a generated crates/takt-tui; approve deferred behind a credential blocker'
relations:
- serves: vision:safe-agentic-coding
- derived_from: product-requirements:takt
revision: 7
---
# Epic: terminal-frontend

## Outcome

A person watches takt from a terminal: the cases of a repository, each case's frontier (claims, admissible actions, outcomes), its run events, the pending approvals with the `takt approve` command line that grants each, and the scorecard and journey of each run. The frontend is specified before it is built: an `ess-ui/1` document checked against takt's ESS model, its data served by a read-only local `takt serve` that `ess verify conform` holds to the model, and the terminal app generated from the document into its own crate. In this epic the frontend writes nothing.

## Context

Owner request of 2026-10-18: "use /ess:specifying with ess UI spec to create a dedicated crate for a tui frontend in rust". ESS 0.60.0 (the release `ess/ess-inputs.yaml` requires and CI installs, `.github/workflows/check.yml:57-71`) has a renderer-neutral UI format, `ess-ui/1` (`ess ui load | check | docs | run | test`), with a terminal renderer: `ess ui run --tui` runs a document against fixtures or, with `--model`, against the HTTP surface the specification's `reached_by: network` components serve; `ess ui test` runs `ess-ui-test/1` tests headless; `ess generate ui --target tui` writes a binary crate (a clap `src/main.rs` with `--base-url` and `--screen-once`) over `ess-ui-tui`, fetched by the Git tag of that ESS release (beyond10x/ess 0.60.0 `crates/ui/ess-ui-tui/src/generate.rs:8-9,226-249`). A `live:` section of a bound app polls its read at the read's `refresh:`, 5 s without one; the served surface streams nothing (`crates/ui/ess-ui-tui/src/app.rs:193-195`). The worked example is beyond10x/ess `examples/partner-portal/ui.yaml`.

takt serves no HTTP today; its state is files (`.engineering/cases/<id>/`: case.yaml, decisions/, evidence/, events.jsonl, authority.yaml). The frontend reads through a read-only `takt serve` bound to 127.0.0.1, implementing the views the model declares, so the same views are checked by `ess verify conform`.

## Decision: no write route in this epic (planner, 2026-10-19)

The 2026-10-18 plan ended with an approve action served by `takt serve`. It is taken out of this epic:

- The generated app authenticates only with an `Authorization` header read from the environment variable `ESS_UI_AUTHORIZATION`, over `http://` only (ess 0.60.0 `generate.rs:9,226`; `http.rs:171-172`). A process of the same user can read another's environment through `/proc/<pid>/environ`.
- Code an agent wrote runs with network access: the test command runs in its own user and mount namespace but keeps the host network (story:test-command-reaches-no-forge-login, stated not done: no network namespace). A detached process started by such a test can outlive the step and its undo, read the token and post a grant to a long-running `takt serve`.
- An agent step itself has no Bash or WebFetch (`crates/takt/src/harness.rs:272-275`), so the hole is the test command, not the agent's own tools.

So the approvals page shows each pending approval and the exact `takt approve --repo <repo> --by <you> <case> <capability>` line; the person runs it in their own shell. story:tui-approve-action stays draft, blocked by dependency-blocker:approve-route-credential.

## Stories, in order (spec first)

1. story:tui-read-model: the six views and the `ess-http/1` binding that serves them.
2. story:tui-ui-document: the `ess-ui/1` document, fixtures, headless tests.
3. story:takt-serve-read-only: `takt serve --repo <path>`, 127.0.0.1 only, no write route; `ess verify conform` against it.
4. story:takt-tui-crate: the generated `crates/takt-tui`; `takt tui` starts `takt serve` and the app.
5. story:tui-approve-action: blocked (above).

Sequencing: after release 0.2.0. No story of this epic joins a wave while the RSI stories are open.

## Acceptance

On a repository with recorded cases:

- `takt tui --repo <path> --screen-once 120x40` prints the case list naming a recorded case (story:takt-tui-crate);
- `ess ui test --path ess/ui/takt.ui.yaml ess/ui/tests/*.yaml` passes, with tests that open a case's detail page (its claims, actions and outcomes from its latest decision, and its run events) and the approvals page (a case at `ApprovalRequired` with its `takt approve` line) (story:tui-ui-document);
- `cargo test -p b10x-takt-cli --test serve_views` passes: `takt serve` over the fixture repository answers every route of `ess/http/takt.http.yaml` with a body equal, after key-sorted canonical encoding, to its committed expected body, 405 to every non-GET request and 404 to an undeclared path, with the repository's tree digest unchanged (story:takt-serve-read-only);
- `cargo test -p b10x-takt-cli --test tui_served_equals_tested` passes: the data served for each view equals, as parsed values, the data the UI tests read (story:takt-tui-crate).

`ess verify conform` has no HTTP target in ESS 0.60.0 (`ess verify conform run --target` takes `billing`, `oracle-fixture`, `interpreted`); when an ESS release takt pins ships one, a story adds it.

## Sequencing

After release 0.2.0. The order is recorded as edges, not only here: story:tui-read-model `depends_on` every active story whose scope holds `ess/domains/takt.yaml` or `ess/domains/takt.citations.tsv` (story:story-delivery-tests-first, story:scorecard-save-checks-binary, story:promotion-chain-head-anchored, story:check-finding-run-step-confined) and on story:self-improvement-protocol and story:judge-guard; story:takt-serve-read-only and story:takt-tui-crate `depends_on` story:judge-guard and story:self-improvement-protocol, which share `crates/takt-cli/src/main.rs`; story:tui-approve-action `depends_on` story:judge-guard, whose rules and typed approvers it uses.
