---
format: aep.planning-md/3
id: approval-record:wave-52-two-findings
kind: approval-record
status: approved
title: 'Wave 52: a relative PATH entry and two trials in one second'
summary: one wave of two finding stories for 0.30.1
relations:
- decides: story:a-relative-path-entry-is-no-test-program-directory
- decides: story:two-trials-started-in-one-second-have-no-case-order
- decides: story:the-refused-path-proposal-case-holds-on-every-host
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-04T02:39:05Z", actor: "human:timo", revision: 2}
---
# Approval: wave 52

## Decision

The owner's standing delegation, 2026-11-04, through the orchestrator: takt has no planned wave
left, and once every planned wave is released findings may enter. "I approve wave 52 under the
grant with two units: a-relative-path-entry-is-no-test-program-directory;
two-trials-started-in-one-second-have-no-case-order. The release is 0.30.1." "The other drafts
stay drafts."

The wave is story:a-relative-path-entry-is-no-test-program-directory (unit a) and
story:two-trials-started-in-one-second-have-no-case-order (unit b). Both are accepted by this
record, not by the agent that filed them. They share no path, and they run one after the other
because one tree builds at a time. story:harness-child-confined stays out:
dependency-blocker:loom-child-confinement is open.

The same approval rejects story:the-refused-path-proposal-case-holds-on-every-host, which
duplicates unit a's story.

## Release

0.30.1: both units change test support or the trial numbering, and neither changes a contract.
