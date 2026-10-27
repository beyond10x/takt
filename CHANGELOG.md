# Changelog

This file follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and takt follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Bare tags such as `0.7.0` are
takt's releases.

## [Unreleased]

### Added

- `forge: command` with a `forge_commands` section in the `takt run --config` file: the run's forge actions (`open`, `read`, `checks`, `merge`) are the exact argument lists the file names, run without a shell, with the placeholders `{branch}`, `{base}`, `{title}`, `{body_file}`, `{number}` and `{head}`; `merge` receives the head the checks were read on. The scanned push stays takt's own.
