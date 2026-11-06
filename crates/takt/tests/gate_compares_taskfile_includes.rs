//! story:the-gate-compares-taskfile-includes: `gate.run` compares at the case's pinned base every
//! file a Taskfile the base holds pulls in through `includes:` (a path, or a mapping's
//! `taskfile:`; a directory means the Taskfile in it), resolved against the including file and
//! followed into the included files in turn, so a step cannot change a gate command by editing
//! an included Taskfile. An include the gate cannot resolve with certainty (a template, a home
//! or variable expansion, an absolute path, one outside the tree, a remote URL) runs nothing,
//! naming it. The changes are commits made on the case's branch outside the loop.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = "story.delivery/1";

/// The project file: the gate's one command is the system shell running a script of the tree.
const PROJECT: &str = "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                       executable: /bin/sh\n    args:\n      - scripts/check\n";

/// A Taskfile task runs.
const TASKS: &str = "version: '3'\ntasks:\n  check:\n    cmds:\n      - 'true'\n";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
    fs::write(&path, text).expect("a file");
}

/// A committed repository whose gate passes, with `files` (path, text) committed besides.
fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "gate-taskfile-includes-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let dir = Scratch(dir);
    write(&dir, "README.md", "a story\n");
    write(
        &dir,
        ".engineering/gates.yaml",
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check holds.\n    assertions:\n      - 'process.succeeded(\"check\")'\n",
    );
    write(&dir, ".engineering/assertions-project.yaml", PROJECT);
    write(&dir, "scripts/check", "exit 0\n");
    for (path, text) in files {
        write(&dir, path, text);
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// Opens a case on `tree`, commits `path` written as `text` on its branch and observes
/// `gate.run` at the commit.
fn observed(tree: &Path, id: &str, path: &str, text: &str) -> Observation {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let _run = Run::open(tree, options(id), &NoHarness, &validators).expect("opens");
    write(tree, path, text);
    git(tree, &["add", "--", path]);
    git(tree, &["commit", "-q", "-m", "a change outside the loop"]);
    let head = git(tree, &["rev-parse", "HEAD"]);
    GateRun::new(".engineering/gates.yaml")
        .observe(tree, &head, 1)
        .expect("observes")
}

/// Not run, no record, and the summary names `named`.
fn refused(observation: &Observation, named: &str) {
    assert!(
        observation.records.is_empty(),
        "the gate wrote a record although {named}: {}",
        observation.summary
    );
    assert!(
        observation.summary.contains("not run"),
        "{}",
        observation.summary
    );
    assert!(
        observation.summary.contains(named),
        "the summary does not name {named}: {}",
        observation.summary
    );
    assert!(observation.artifact.is_none(), "{}", observation.summary);
}

/// The gate ran and recorded `pass`.
fn recorded(observation: &Observation) {
    assert_eq!(
        observation.records.len(),
        1,
        "the gate did not record: {}",
        observation.summary
    );
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("pass"),
        "{}",
        observation.summary
    );
}

/// The story's case: `tasks/ci.yml`, included from `Taskfile.yml` by a path.
#[test]
fn an_edit_to_a_taskfile_named_in_includes_keeps_the_gate_from_running() {
    let tree = repository(
        "path",
        &[
            (
                "Taskfile.yml",
                "version: '3'\nincludes:\n  ci: ./tasks/ci.yml\n",
            ),
            ("tasks/ci.yml", TASKS),
        ],
    );
    let observation = observed(
        &tree,
        "path",
        "tasks/ci.yml",
        "version: '3'\ntasks:\n  check:\n    cmds:\n      - 'echo edited'\n",
    );
    refused(&observation, "tasks/ci.yml");
}

/// The mapping form, `taskfile:` beside `dir:` and `optional:`.
#[test]
fn an_edit_to_a_taskfile_a_mapping_includes_keeps_the_gate_from_running() {
    let tree = repository(
        "mapping",
        &[
            (
                "Taskfile.yml",
                "version: '3'\nincludes:\n  ci:\n    taskfile: tasks/ci.yml\n    dir: tasks\n    \
                 optional: true\n",
            ),
            ("tasks/ci.yml", TASKS),
        ],
    );
    let observation = observed(&tree, "mapping", "tasks/ci.yml", "version: '3'\n");
    refused(&observation, "tasks/ci.yml");
}

/// An included file's own `includes:` are followed, resolved against the included file.
#[test]
fn an_edit_to_a_taskfile_an_included_one_includes_keeps_the_gate_from_running() {
    let tree = repository(
        "nested",
        &[
            (
                "Taskfile.yml",
                "version: '3'\nincludes:\n  ci: tasks/ci.yml\n",
            ),
            (
                "tasks/ci.yml",
                "version: '3'\nincludes:\n  lint: ../lint/rules.yml\n",
            ),
            ("lint/rules.yml", TASKS),
        ],
    );
    let observation = observed(&tree, "nested", "lint/rules.yml", "version: '3'\n");
    refused(&observation, "lint/rules.yml");
}

/// A Taskfile below the root includes relative to its own directory.
#[test]
fn an_include_of_a_taskfile_below_the_root_resolves_against_its_directory() {
    let tree = repository(
        "below",
        &[
            (
                "crates/widget/Taskfile.yml",
                "version: '3'\nincludes:\n  more: more.yml\n",
            ),
            ("crates/widget/more.yml", TASKS),
        ],
    );
    let observation = observed(&tree, "below", "crates/widget/more.yml", "version: '3'\n");
    refused(&observation, "crates/widget/more.yml");
}

/// An optional include the base does not hold, added on the branch.
#[test]
fn an_optional_include_added_on_the_branch_keeps_the_gate_from_running() {
    let tree = repository(
        "optional",
        &[(
            "Taskfile.yml",
            "version: '3'\nincludes:\n  local:\n    taskfile: ./local.yml\n    optional: true\n",
        )],
    );
    let observation = observed(&tree, "optional", "local.yml", TASKS);
    refused(&observation, "local.yml");
}

/// Includes the gate cannot resolve with certainty: each runs nothing, named, whatever changed.
#[test]
fn an_include_that_cannot_be_resolved_with_certainty_keeps_the_gate_from_running() {
    for (k, include) in [
        "'{{.ROOT}}/tasks/ci.yml'",
        "/srv/tasks/ci.yml",
        "../outside.yml",
        "tasks/../../outside.yml",
        "https://example.com/Taskfile.yml",
        "~/tasks/ci.yml",
        "$HOME/tasks/ci.yml",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("uncertain-{k}");
        let tree = repository(
            &id,
            &[(
                "Taskfile.yml",
                &format!("version: '3'\nincludes:\n  ci: {include}\n"),
            )],
        );
        let observation = observed(&tree, &id, "README.md", "a story, edited\n");
        refused(&observation, include.trim_matches('\''));
    }
}

/// An include entry in neither form is not read with certainty either.
#[test]
fn an_include_in_neither_form_keeps_the_gate_from_running() {
    let tree = repository(
        "neither",
        &[(
            "Taskfile.yml",
            "version: '3'\nincludes:\n  ci:\n    - tasks/ci.yml\n",
        )],
    );
    let observation = observed(&tree, "neither", "README.md", "a story, edited\n");
    refused(&observation, "Taskfile.yml");
}

/// Two files including each other: the gate follows each once, and a change elsewhere runs.
#[test]
fn includes_that_form_a_cycle_let_an_unrelated_change_run_and_record() {
    let tree = repository(
        "cycle",
        &[
            ("Taskfile.yml", "version: '3'\nincludes:\n  a: a.yml\n"),
            ("a.yml", "version: '3'\nincludes:\n  b: b.yml\n"),
            ("b.yml", "version: '3'\nincludes:\n  a: a.yml\n"),
        ],
    );
    let observation = observed(&tree, "cycle", "README.md", "a story, edited\n");
    recorded(&observation);
}

/// A change outside every included file: the gate runs and records as before.
#[test]
fn a_change_outside_the_included_files_lets_the_gate_run_and_record() {
    let tree = repository(
        "outside",
        &[
            ("Taskfile.yml", "version: '3'\nincludes:\n  ci: ./tasks\n"),
            ("tasks/Taskfile.yml", TASKS),
        ],
    );
    let observation = observed(&tree, "outside", "README.md", "a story, edited\n");
    recorded(&observation);
}
