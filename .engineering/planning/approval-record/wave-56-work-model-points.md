---
format: aep.planning-md/3
id: approval-record:wave-56-work-model-points
kind: approval-record
status: approved
title: 'Wave 56: the open points of the work-source model'
summary: connection names, the item format, aep url and revision, work_commands, filters later, case-id hash always, aep trailer unchanged
relations:
- decides: story:work-items-in-the-ess-model
- decides: story:work-flag-and-the-aep-source
- decides: story:the-command-work-source
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T13:18:15Z", actor: "human:timo", revision: 2}
---
# Decision: the open points of the work-source model

## Decision

The wave lead's, inside the item the orchestrator approved (approval-record:wave-56-work-sources), on the nine points
unit a's specification left open:

1. A connection name holds what a forge instance does: letters, digits, `.`, `_` and `-`, and is not empty.
2. A key with no connection whose tracker has several connections is refused before a case opens, naming them; with one,
   it is that one.
3. A `takt.work-item/1` document carries `format: takt.work-item/1`; another format is refused.
4. `url` is optional; an `aep` item has none. `revision` is a string: for an `aep` item the store's revision of the
   story, for a tracker what it reports (an etag or an update time).
5. `work_commands.read` takes the item's id as `{id}`; `refused_exit` means what it means for the command forge; a
   source named in `work` or a key whose command is missing is refused before a case opens.
6. In this release a `work` entry is a source name with an optional `@<connection>`; a selection filter (`?label=`)
   belongs to the tracker sources of story:work-from-jira-github-and-gitlab and is refused now.
7. The case records the key as `item_key`; a story case opened before it reads as `aep:<story id>`.
8. The case records the revision as `item_revision`.
9. Case ids: the scheme the go accepted could keep `..` (`jira:A..B`), give two connections one slug (`jira@a:x-1`,
   `jira@b:x-1`), and let one key spell another's slug and hash. So a key of any source but `aep` mints
   `<source>-<slug>-<h8>-<instant>`: the slug is the connection and the id (`team-a-abc-12`), lowercased, each run of
   characters outside `[a-z0-9_]` one `-`, at most 48 characters, and `<h8>` the first 8 hex digits of the sha256 of the
   full key, always. `aep` keeps `story-<name>-<instant>`.

The commit trailer keeps `Refs: <story id>` for an `aep` item, so a story's trailer does not change; any other source
writes `Refs: <full key>`.

## Addendum (2026-11-06)

Point 1 in full: a connection also follows the forge instance's other two rules: it does not start with `-` and holds at most
128 bytes (`crates/takt/src/forge/connectors.rs`, `is_instance_name`).
