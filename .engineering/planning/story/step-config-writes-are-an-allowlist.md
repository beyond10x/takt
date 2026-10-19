---
format: aep.planning-md/3
id: story:step-config-writes-are-an-allowlist
kind: story
status: draft
title: A step may change only allowlisted git configuration keys
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:scan-before-push
revision: 1
---
## Outcome

A step may change only configuration keys on a named allowlist (for example `branch.<name>.*` and a tool's own section the protocol names); a change to any other key in any configuration file git reads for the tree (repository `config`, `config.worktree`, a sibling's) is refused and put back, whatever its section.

## Context

From the reviewer's verdict on story:scan-before-push at 6ffe659 (2026-10-19). The guard is a denylist (`is_guarded_key`, `crates/takt/src/git.rs` at c888209, lines 800-816: `core`, `filter`, `diff`, `merge`, `alias`, `include`, `includeif`, `credential`, `url`, `gpg`, `sequence`, `pager`, `trailer`, `difftool`, `mergetool`, `submodule`, `extensions`, `remote.<name>.*`). Each review finds a section it does not name: `i18n.commitEncoding` made takt's own commit message leave unscanned, `push.followTags` pushed annotated tags, and a repository-config `http.proxy` sent the bot's push through a proxy the step chose (verified against 127.0.0.1:9). scan-before-push guards `i18n`, `push` and `http` in its correction 3; this story closes the class: git adds sections in new releases, and a denylist is wrong by default for each of them.

## Scope

- `crates/takt/src/git.rs`: `is_guarded_key` becomes an allowlist (cited)
- `crates/takt/tests/`: a step writing a key of a section the denylist never named (inferred)

## Acceptance

On scenario 1 with a fake harness, a step that sets `http.proxy`, `i18n.commitEncoding`, `push.followTags`, `fetch.fsckObjects` or an invented section `zz.key` is refused and the configuration file is byte-identical afterwards; a step that sets `branch.<case branch>.description` is not refused.
