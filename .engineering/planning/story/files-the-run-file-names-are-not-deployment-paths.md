---
format: aep.planning-md/3
id: story:files-the-run-file-names-are-not-deployment-paths
kind: story
status: draft
title: A run file's deployment_paths may not cover a file or program it names
summary: refuse deployment_paths covering work_commands/forge_commands programs, endpoint_file, llm path, credential file, state_dir
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
- informed_by: story:git-excluded-files-are-not-step-changes
revision: 2
---
# Story: files-the-run-file-names-are-not-deployment-paths

## What is wrong

`deployment_paths` (story:git-excluded-files-are-not-step-changes) refuses a list that covers the run file itself,
because a step could rewrite it unseen for the next run. Nothing else the run file names is checked: a `work_commands`
or `forge_commands` program, a `setup` or `local_checks` program given as a path, `endpoint_file`, the `llm` path, a
credential file and `state_dir` may lie under a deployment path. A step may write there without the guard counting it,
so the next run starts a program or reads a file the model wrote, on the host and outside confinement. Before
`deployment_paths`, such a write was counted and put back.

Found by the wave 56 security review and the adversary (`crates/takt/src/run_config.rs`, the run-file check).

## Outcome

A run file whose `deployment_paths` cover a file or program it names elsewhere is refused before a case opens, naming
the key and the entry, as a list covering the run file is.

## Acceptance

- A `work_commands.read` program, an `endpoint_file` and a `state_dir` under `.autodev/` with `deployment_paths =
  [".autodev/"]` are each refused before a case opens, naming the key: a test each.
- A program named by a bare command name (looked up on `PATH`) is not affected: a test.

## The red test

Written by the wave 56 security review and red at 9a67088ee; it left the wave as out of scope. It uses the helpers `repository`, `append` and `git` of `crates/takt/tests/work_and_deployment_w56_sec.rs` and `use takt::run_config::RunConfig;`.

```rust
/// Invariant 5 (a step cannot steer a later run through the list): the run file is refused
/// when the list covers it inside the tree, "where a step could rewrite it for the next run"
/// (`RunConfig::check_run_file_outside_deployment`). The same holds for what the run file
/// names for takt to run or read at the next open: a `work_commands` program, the
/// `endpoint_file`, the `state_dir`. Each under `.autodev/` passes the two checks
/// `takt run --config` makes before a case opens.
#[test]
fn a_program_or_file_the_run_file_names_under_the_list_is_refused() {
    let scratch = repository("named-inputs");
    let repo = &scratch.repo;
    let autodev = repo.join(".autodev");
    fs::create_dir_all(autodev.join("state")).expect("the state directory");
    fs::write(autodev.join("tracker"), "").expect("the program");
    fs::write(autodev.join("endpoint.toml"), "").expect("the endpoint file");
    let run_file = scratch.root.join("agent.toml");
    let mut refused = Vec::new();
    for (name, member) in [
        (
            "work_commands.read",
            format!(
                "[work_commands]\nread = [\"{}\", \"show\", \"{{id}}\"]\n",
                autodev.join("tracker").display()
            ),
        ),
        (
            "endpoint_file",
            format!(
                "endpoint_file = \"{}\"\n",
                autodev.join("endpoint.toml").display()
            ),
        ),
        (
            "state_dir",
            format!("state_dir = \"{}\"\n", autodev.join("state").display()),
        ),
    ] {
        fs::write(
            &run_file,
            format!("deployment_paths = [\".autodev/\"]\n{member}"),
        )
        .expect("the run file");
        let checked = RunConfig::read(&run_file)
            .and_then(|config| config.check_run_file_outside_deployment(&run_file, repo));
        if checked.is_ok() {
            refused.push(format!(
                "{name} under .autodev/, a path a step may write unseen, is taken"
            ));
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
}
```
