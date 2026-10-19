---
format: aep.planning-md/3
id: dependency-blocker:approve-route-credential
kind: dependency-blocker
status: open
title: A write route in the terminal frontend needs a credential no process takt starts for a case can read or use
relations:
- blocks: story:tui-approve-action
revision: 1
---
# Dependency blocker: approve-route-credential

A write route in the terminal frontend needs a credential no process takt starts for a case can read or use.

- ESS 0.60.0's generated terminal app reads its credential only from `ESS_UI_AUTHORIZATION` (beyond10x/ess `crates/ui/ess-ui-tui/src/generate.rs:9,226`), readable by the same user through `/proc/<pid>/environ`, and reaches its server over `http://` only (`http.rs:171-172`).
- takt's test command keeps the host network (story:test-command-reaches-no-forge-login, stated not done).

Cleared when an ESS release takes the credential from an inherited descriptor or a prompt (or takt shows an equivalent), and test commands cannot reach a 127.0.0.1 port of the host, each cited.
