---
format: aep.planning-md/3
id: decision-blocker:main-commit-with-personal-committer
kind: decision-blocker
status: cleared
title: A commit on main has a personal committer, so the shared security gate fails from 4aecf9b on
relations:
- blocks: approval-record:release-0-9-0
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-23T09:19:11Z", actor: "human:timo", revision: 3}
---
# Blocker: a commit on main with a personal committer

`common / Security and privacy` fails on `main` at 68f6b6d (run 39888253594) and on the release
pull request at 213c386 (run 39888431472): "common checks failed: 1 finding(s)". It passed at
8e52ecb (run 39883322060).

`b10x-gates check --head <sha>` run locally gives the same finding at every commit from 4aecf9b
on and passes at 8e52ecb. 4aecf9b ("plan: wave 34 closes epic:takt-on-loom-sdk") is authored by
b10x-bot[bot] but its committer is a personal account: it was rebased onto 8e52ecb with plain
git, outside the bot route. Every other commit in 8e52ecb..213c386 is bot|bot.

The commit is on `main`. A rewrite of `main` is a force-push, which this repository's grants do
not cover; moving the Gates adoption baseline past it is a policy change its owner makes.

Release 0.9.0 is held until `common / Security and privacy` passes on the release commit.

## Decision (2026-10-23)

The orchestrator: option A. The Gates adoption baseline for this repository moves to 68f6b6d once
4aecf9b is confirmed as the only finding; the CI policy secret is then refreshed by its owner. A
rewrite of `main` is not taken. Nothing is committed to `main` until then, and every commit goes
through `b10x-gates bot`.
