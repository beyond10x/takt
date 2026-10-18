# Changelog

## [Unreleased]

A release is cut by an annotated tag `<major>.<minor>.<patch>` whose version equals
`[workspace.package] version` in `Cargo.toml` and has a section `## [<version>] — <date>` here;
`.github/workflows/release.yml` checks that, runs `task check` and publishes the binaries.

- Repository created; design `docs/design/takt.md`; workspace with the library `b10x-takt` and
  the binary `takt`.
- A finding's `expected_before` predicate may name a `repository`, a `seed` and a `test` instead
  of a scenario; `scripts/check-finding` clones that repository at that seed and builds takt in a
  checkout, so the built binary names its revision.
- A version tag publishes prebuilt `takt` binaries for Linux (x86_64, aarch64) and macOS
  (aarch64) with a `SHA256SUMS` file.
