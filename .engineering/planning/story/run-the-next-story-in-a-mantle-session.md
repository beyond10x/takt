---
format: aep.planning-md/3
id: story:run-the-next-story-in-a-mantle-session
kind: story
status: draft
title: takt runs the next story in a Mantle session
summary: 'Glovebox wave 3: takt starts a Mantle command session where Claude Code works in a writable checkout, judges the result, and the branch is published from outside the box.'
tags:
- glovebox
- mantle
relations:
- informed_by: story:harness-child-confined
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: takt runs the next story in a Mantle session

Part of Glovebox (atlas `initiative:glovebox`, `docs/design/glovebox.md`), wave 3.

## Why

- Glovebox requirement 5: "takt can say 'run the next story of this repository': Glovebox
  provisions a box whose workspace is a Git checkout of the repository (plus further mounts),
  Claude Code runs as the box's main process, and its commits get pushed"
  (`atlas/docs/design/glovebox.md:27-29`). Wave 3 assigns it to takt and Mantle
  (`atlas/docs/design/glovebox.md:92`), and the layer table gives takt "picks the story, starts
  the run, judges the result, publishes the branch" (`atlas/docs/design/glovebox.md:38`).
- takt does not confine the harness session today: "No operating-system sandbox holds the harness
  session yet. `--confinement substrate` holds the test command only"
  (`website/docs/concepts/one-step-at-a-time.md:167-168`); the same limit is listed in
  `website/docs/known-limitations.md:32` and `website/docs/guides/install.md:158`.
- `story:harness-child-confined` confines the harness child through loom-sdk and a Substrate
  profile, and is blocked on loom-sdk exporting confinement for an arbitrary child and on a
  profile with named egress (its § Blocked by). This story is a second route to the same outcome:
  the whole session runs in a Mantle box that already has egress through a fixed gateway, a
  delivered model credential and attach and detach.
- Mantle's `story:command-agent-kind` (active, on Mantle's `wave/mantle-next-1`) adds
  `agent.kind: command` with a declared `agent.argv`, the entry point this story uses.

## What changes

1. takt picks the next story as it does today (`aep plan artifact waves --kind story --status
   draft`, `AGENTS.md:24`) and starts a Mantle session for it: the workspace is a Git checkout of
   the repository at the base revision, writable inside the box; `agent.kind: command`;
   `agent.argv` runs Claude Code on the story.
2. takt waits for the session to end, then reads the result from outside the box: the commits on
   the session's branch, and the test command's outcome under takt's existing validators. Nothing
   the box writes becomes evidence, a grant or a gate definition (`story:validator-confinement`).
3. The branch is published from outside the box, by Mantle's publish step
   (`story:publish-from-beside-the-daemon` in Mantle) or by takt's forge after its judgement. No
   forge or Git credential enters the box (Glovebox SM-D201, `atlas/docs/design/glovebox.md:58-59`).
4. The run record names the Mantle session, its manifest digest and the revision it published.

## Acceptance

- `takt run` with a Mantle backend on a repository with one draft story starts exactly one
  Mantle session whose workspace is a checkout at the base revision and whose entry point is
  Claude Code; a test with a fake Mantle asserts the manifest it sends.
- A session that commits a change ends in a published branch whose commits are the session's,
  pushed from outside the box; a test asserts that no forge or Git credential is in the session's
  manifest or environment.
- A session that writes outside its workspace or reaches an undeclared host fails inside the box
  and the run records the refusal by name.
- takt judges the result with the same validators as an unconfined run, and a failing test command
  stops the run without publishing.

## Blocked on / Depends on

- Depends on Mantle `story:command-agent-kind` (entry point) and Mantle
  `story:publish-from-beside-the-daemon` (push outside the box).
- Open: `story:command-agent-kind` gives a `command` session no agent credential, but Claude Code
  in the box needs its model token. Glovebox SM-D199 allows that token in the box
  (`atlas/docs/design/glovebox.md:53-55`); takt's own rule says "Credentials never enter a
  harness's environment" (`AGENTS.md:92`). Either Mantle lets a `command` session take the model
  credential, or the session uses Mantle's `story:llm-gateway-model-mode`; and takt's rule needs an
  owner decision naming the model token as its exception. Unresolved.
- Open: how a box's further mounts beyond the checkout are named is unconfirmed
  (`atlas/docs/design/glovebox.md:107-108`).
