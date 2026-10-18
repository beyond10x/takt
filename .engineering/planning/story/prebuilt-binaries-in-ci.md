---
format: aep.planning-md/3
id: story:prebuilt-binaries-in-ci
kind: story
status: draft
title: A version tag publishes prebuilt takt binaries from GitHub Actions
relations:
- informed_by: story:binary-embeds-revision
- informed_by: release-plan:takt-roadmap
revision: 1
---
## Outcome

A version tag pushed to github.com/beyond10x/takt makes GitHub Actions publish a GitHub Release carrying prebuilt `takt` binaries for Linux (x86_64, aarch64) and macOS (aarch64), each a `.tar.gz` with a `SHA256SUMS` file, so `takt` can be installed without a Rust toolchain; each binary's `takt --version` prints the tag's commit with no `-dirty`.

## Context

takt is installed today only by `cargo install --locked --path crates/takt-cli` from a checkout (the promotion records of improvement turn 1 and the trial runners build it that way). The repository has two workflows, `.github/workflows/check.yml` (task check on pull requests and pushes to main) and `.github/workflows/shared-gates.yml`; none builds or publishes a binary, and the repository has no tags. Sibling beyond10x repositories already publish binaries this way: https://github.com/beyond10x/aep/blob/main/.github/workflows/release.yml verifies that tag, Cargo version, changelog and checkout agree, builds natively per target, then publishes the archives with `SHA256SUMS` and the tag's changelog section, every action pinned by commit.

`crates/takt-cli/build.rs` (story:binary-embeds-revision, wave 11) embeds the git revision from the checkout it builds in; a CI checkout is a git working tree, so the released binary carries the tagged commit. Benchmark runs in containers (story:takt-in-container, story:harbor-agent-adapter) need a binary they can download rather than build.

Release precondition, not part of this story: the workflow fires only on a tag pushed to GitHub. takt's local main is not pushed today, and cutting a tag is a release decision for the owner.

## Scope

- `.github/workflows/release.yml`: new; provenance job (tag is bare SemVer, annotated, equals `HEAD`, equals `[workspace.package] version` in `Cargo.toml`, has a `CHANGELOG.md` section), a build matrix, a publish job holding `contents: write` only there (inferred, from the aep workflow)
- `Cargo.toml`: `[workspace.package] version`, read by provenance (cited: `crates/takt-cli/Cargo.toml` takes `version.workspace = true`)
- `CHANGELOG.md`: new if absent, one section per version (inferred)
- `README.md`: an install-from-release section (inferred)

## Acceptance

Pushing an annotated tag `0.1.0` at a commit whose workspace version is `0.1.0` and whose `CHANGELOG.md` has a `## [0.1.0]` section produces a GitHub Release with `takt-0.1.0-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.1.0-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.1.0-aarch64-apple-darwin.tar.gz` and `SHA256SUMS`; `sha256sum -c SHA256SUMS` passes on the downloaded files; each extracted `takt --version` prints `takt 0.1.0 (<the tag's commit>)`; a tag that disagrees with the Cargo version or has no changelog section fails provenance and publishes nothing.
