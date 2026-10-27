# Changelog

This file follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and takt follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Bare tags such as `0.7.0` are
takt's releases.

## [Unreleased]

### Added

- `transport: api` with a `credential` (`{env: NAME}` or `{file: PATH}`, a mode 600 file outside the tree) in the `takt run --config` file: the `gitlab` forge calls the GitLab REST API over HTTPS itself instead of `glab api`. The token is held in memory only, replaced where an answer of the host is quoted, and given to the push through a credential helper that reads it from that one command's environment. `glab` stays the default transport.
