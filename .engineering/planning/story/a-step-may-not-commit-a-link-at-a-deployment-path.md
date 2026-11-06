---
format: aep.planning-md/3
id: story:a-step-may-not-commit-a-link-at-a-deployment-path
kind: story
status: draft
title: A step may not commit a link at a deployment path
summary: refuse and put back a step that adds a link at or above a deployment path
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
- informed_by: story:git-excluded-files-are-not-step-changes
revision: 1
---
# Story: a-step-may-not-commit-a-link-at-a-deployment-path

## What is wrong

Since the wave 56 correction round, a link a step puts at or under a deployment path is a change the step made
(`crates/takt/src/git.rs`). If an edit step commits that link (`.autodev` pointing at `src`), it is in the repository,
and every later write the deployment makes through it lands in a tracked directory; a later step's commit then carries
the deployment's log. Within one step the guard cannot tell the deployment's write through an existing link from the
step's own write, so the defence has to be at the step that adds the link.

Found by the wave 56 security review; the step-2 half of its test left the wave with this draft.

## Outcome

A step whose change adds or replaces a link at or above a path `deployment_paths` names is refused and put back,
naming the path, before anything is committed.

## Acceptance

- A step that replaces `.autodev/` with a link to `src` is refused and put back; no commit names `.autodev`: a test.
- The removed step-2 block below, run after such a step was refused, finds the deployment's log outside every commit.

The removed block, verbatim (from `a_link_the_step_puts_at_a_deployment_path_is_a_change` in
`crates/takt/tests/work_and_deployment_w56_sec.rs`):

```rust
    fs::remove_dir_all(repo.join(".autodev")).expect("remove");
    std::os::unix::fs::symlink("src", repo.join(".autodev")).expect("a link");

    // Step 2: the wrapper appends to its log while the step writes its own file.
    let before = Checkpoint::take_owning(repo, &[], &own).expect("a checkpoint");
    append(
        &repo.join(".autodev/run-2.log"),
        "the wrapper, during step 2\n",
    );
    fs::write(repo.join("src/feature.rs"), "pub fn g() {}\n").expect("the step's file");
    let change = before.change(repo).expect("the change");
    if change.paths.iter().any(|path| path == "src/run-2.log") {
        wrong.push(format!(
            "the deployment's log written to .autodev/run-2.log during step 2 is that step's change, committed with it: paths {:?}",
            change.paths
        ));
    }
```
