---
format: aep.planning-md/3
id: story:gitlab-forge-hardening
kind: story
status: draft
title: The GitLab route's children carry no GitHub token, and its unchecked edges are checked
relations:
- serves: vision:safe-agentic-coding
- informed_by: review-result:wave-36-1782919
revision: 2
---
# The GitLab route's children carry no GitHub token, and its unchecked edges are checked

## Why

The adversarial review of wave 36 at 1782919 (review-result:wave-36-1782919) found two low
findings the wave did not fix and five edges it did not reach. None is a credential leak the
review could show; each is a predicate a later session can check.

## Findings

- F12 (low, inferred from a read). `crates/takt/src/forge.rs:513` and `:1402`: the `glab` and git
  children of the GitLab route still receive `GH_TOKEN`, `GITHUB_TOKEN` and `B10X_*`, the bot
  route's keep-list. `expected_before`: a test that starts a GitLab-route child with `GH_TOKEN`
  set in takt's environment finds it in the child's environment.
- F15 (low). `crates/takt/src/forge.rs:1893`: `Scanner::outgoing` carries
  `#[allow(clippy::too_many_arguments)]`. `expected_before`:
  `grep -c 'allow(clippy::too_many_arguments)' crates/takt/src/forge.rs` prints 1.
- Unchecked (inferred): the merge request's response JSON, which holds the author's name, is
  written under the case as `pull-request.*`; whether it reaches the base with takt's records
  depends on what the case branch commits. `expected_before`: unclear until one pushed case branch
  is listed; the bot route's `gh` output has the same shape.
- Unchecked: a repository-local glab config or a git `insteadOf` can redirect the push; the test
  command can already read the home directory by path. No machine predicate until the redirect
  is tried.
- Unchecked: how long the default `glab auth status --hostname <host>` takes when the host is
  unreachable. `expected_before`: `Glab::logged_in` (`crates/takt/src/forge/gitlab.rs`) starts
  the child with no timeout.
- Unchecked: resuming a case opened under `--forge none` now that an `origin` off github.com
  defaults to `gitlab`. `expected_before`: unclear; one resumed case decides it.

## Acceptance

1. A GitLab-route child sees none of the bot route's keep-list; a test per adapter.
2. `Scanner::outgoing` takes a struct; the allow is gone.
3. Each unchecked edge has an observation recorded here, and a fix where the observation shows
   one is needed.

## Scope

`crates/takt/src/forge.rs`, `crates/takt/src/forge/gitlab.rs`, `crates/takt/tests/`.

## Endpoint findings

From the reviewer's confirmation at 731d111 (review-result:wave-36-731d111), two low findings
on the configured endpoint, neither crossing a privilege (only the operator supplies the value):

- N2 (low). `Credential::secret` (`crates/takt/src/harness.rs`, around line 2049 at 731d111)
  accepts a name part that starts with `-`; it reaches b10x-harness as its own argument after
  `--api-key-secret`. `expected_before`: `--endpoint-secret llm/-x` opens a case.
- N3 (low). A key placed in the base URL's path (`https://host/v1/<key>`) is neither refused
  nor redacted; F3 covers userinfo, query and fragment only. `expected_before`: a base URL
  whose path holds a marker is recorded on the `opened` line with the marker.
