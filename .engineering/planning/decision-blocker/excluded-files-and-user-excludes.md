---
format: aep.planning-md/3
id: decision-blocker:excluded-files-and-user-excludes
kind: decision-blocker
status: open
title: May a step's change set leave out files the repository's git excludes?
summary: 'git-excluded-files-are-not-step-changes and git-reads-no-user-excludes contradict; recommendation: a run-file list of deployment paths'
relations:
- blocks: story:git-excluded-files-are-not-step-changes
- blocks: story:git-reads-no-user-excludes
revision: 1
---
# Decision: may a step's change set leave out files the repository's git excludes?

## Question

story:git-excluded-files-are-not-step-changes (proposed) asks that a file the repository's git excludes
(`.git/info/exclude`, a global excludes file) not count as a change a step made, so a deployment's own files beside
the tree do not refuse or revert a step. story:git-reads-no-user-excludes (draft) asks that takt's git calls read no
user excludes file at all. The first trusts excludes a step or the operator can write; the second reads none. Both
cannot hold for the same file.

## Options

1. **Read no user excludes (git-reads-no-user-excludes).** takt's git calls pass `core.excludesFile` empty and
   ignore `.git/info/exclude`. A deployment's own file beside the tree is seen as a step change and refused or
   reverted. Such files must live outside the tree, or under a path the run file names as the deployment's.
   git-excluded-files-are-not-step-changes is rejected.
2. **Trust the repository's excludes (git-excluded-files-are-not-step-changes).** A file `.git/info/exclude` or the
   global excludes file ignores is no step change. A step that writes `.git/info/exclude`, or anything a pattern
   there matches, can hide a change from the guard; the guard would then need to snapshot the excludes file at the
   step's start and judge the step against that copy.
3. **Trust only a list the run file names.** takt reads no user excludes (option 1), and the run file may name paths
   that are the deployment's (for example `.autodev/`), which no step owns and the guard does not count. The list is
   read at open, outside any step's reach.

## Recommendation

Option 3. It keeps the guard's input out of a step's reach, as option 1 does, and gives a deployment the place for its
own files that option 2 offers, without trusting a file a step can write. Both stories would be rewritten to it:
git-reads-no-user-excludes as written, and git-excluded-files-are-not-step-changes as "paths the run file names are no
step changes".
