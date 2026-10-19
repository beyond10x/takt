//! story:the-gate-compares-every-script-and-tool-config-at-the-base: `gate.run` runs from the
//! tree only what the base and the revision agree on. Besides the executables and catalogs the
//! project file admits, it compares at the case's pinned base every tree file a gate command takes
//! as an argument, the tool configuration cargo, clippy, rustfmt and task read
//! (`.cargo/config(.toml)`, `clippy.toml`, `.clippy.toml`, `rustfmt.toml`, `.rustfmt.toml`,
//! `Taskfile.yml`), the toolchain file, and the repository's own judge-surface list read at the
//! base. A file that differs, is added or is deleted there: not run, no record, the path named.
//! The changes are commits made on the case's branch outside the loop, the route the step guard
//! does not see.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The project file: the gate's one command is the system shell running a script of the tree,
/// which is only an argument to it, and a data directory the script reads.
const PROJECT: &str = "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                       executable: /bin/sh\n    args:\n      - scripts/check-citations\n      \
                       - --strict\n      - --rules=rules.txt\n      - data\n";

/// A crate's manifest at the base.
const WIDGET: &str = "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                      [dependencies]\nanyhow = \"1\"\n";

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

/// A committed repository whose gate passes: `scripts/check-citations` exits 0. With `listed`,
/// the repository's own judge-surface list names `scripts/**`.
fn repository(name: &str, listed: bool) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-tool-config-{}-{name}", std::process::id()));
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
    write(&dir, "scripts/check-citations", "#!/bin/sh\nexit 0\n");
    write(&dir, "scripts/helper", "#!/bin/sh\nexit 0\n");
    for script in ["scripts/check-citations", "scripts/helper"] {
        fs::set_permissions(dir.join(script), fs::Permissions::from_mode(0o755)).expect("exec bit");
    }
    write(&dir, "data/input.txt", "one\n");
    write(&dir, "rules.txt", "strict\n");
    write(&dir, "crates/widget/Cargo.toml", WIDGET);
    if listed {
        write(&dir, ".engineering/judge-surface.yaml", "- scripts/**\n");
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

/// A change committed on the case's branch outside the loop.
enum Change<'a> {
    Write(&'a str, &'a str),
    Delete(&'a str),
}

/// Opens a case on `tree`, commits `changes` on its branch and observes `gate.run` at the commit.
fn observed(tree: &Path, id: &str, changes: &[Change<'_>]) -> Observation {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let _run = Run::open(tree, options(id), &NoHarness, &validators).expect("opens");
    for change in changes {
        match change {
            Change::Write(path, text) => {
                write(tree, path, text);
                git(tree, &["add", "--", path]);
            }
            Change::Delete(path) => {
                git(tree, &["rm", "-q", "--", path]);
            }
        }
    }
    git(tree, &["commit", "-q", "-m", "a change outside the loop"]);
    let head = git(tree, &["rev-parse", "HEAD"]);
    GateRun::new(".engineering/gates.yaml")
        .observe(tree, &head, 1)
        .expect("observes")
}

/// Not run, no record, and the summary names `path`.
fn refused(observation: &Observation, path: &str) {
    assert!(
        observation.records.is_empty(),
        "the gate wrote a record after {path} changed: {}",
        observation.summary
    );
    assert!(
        observation.summary.contains("not run"),
        "{}",
        observation.summary
    );
    assert!(
        observation.summary.contains(path),
        "the summary does not name {path}: {}",
        observation.summary
    );
    assert!(observation.artifact.is_none(), "{}", observation.summary);
}

/// The gate ran and recorded `pass`, as before.
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

/// A script the project file passes to its executable as an argument is compared at the base,
/// in a repository whose own list names nothing.
#[test]
fn a_script_the_project_file_passes_as_an_argument_is_compared_at_the_base() {
    let tree = repository("argument", false);
    let observation = observed(
        &tree,
        "argument",
        &[Change::Write(
            "scripts/check-citations",
            "#!/bin/sh\necho edited\nexit 0\n",
        )],
    );
    refused(&observation, "scripts/check-citations");
}

/// A file an option names after its `=` (`--rules=rules.txt`) is an argument too.
#[test]
fn a_file_an_option_names_after_its_equals_sign_is_compared_at_the_base() {
    let tree = repository("option-value", false);
    let observation = observed(
        &tree,
        "option-value",
        &[Change::Write("rules.txt", "lenient\n")],
    );
    refused(&observation, "rules.txt");
}

#[test]
fn a_cargo_config_toml_added_on_the_branch_keeps_the_gate_from_running() {
    let tree = repository("cargo-config", false);
    let observation = observed(
        &tree,
        "cargo-config",
        &[Change::Write(
            ".cargo/config.toml",
            "[build]\nrustflags = []\n",
        )],
    );
    refused(&observation, ".cargo/config.toml");
}

#[test]
fn a_clippy_toml_added_on_the_branch_keeps_the_gate_from_running() {
    let tree = repository("clippy", false);
    let observation = observed(
        &tree,
        "clippy",
        &[Change::Write("clippy.toml", "msrv = \"1.0\"\n")],
    );
    refused(&observation, "clippy.toml");
}

#[test]
fn a_rustfmt_toml_added_on_the_branch_keeps_the_gate_from_running() {
    let tree = repository("rustfmt", false);
    let observation = observed(
        &tree,
        "rustfmt",
        &[Change::Write("rustfmt.toml", "max_width = 200\n")],
    );
    refused(&observation, "rustfmt.toml");
}

#[test]
fn a_taskfile_added_on_the_branch_keeps_the_gate_from_running() {
    let tree = repository("taskfile", false);
    let observation = observed(
        &tree,
        "taskfile",
        &[Change::Write(
            "Taskfile.yml",
            "version: '3'\ntasks:\n  check:\n    cmds:\n      - 'true'\n",
        )],
    );
    refused(&observation, "Taskfile.yml");
}

#[test]
fn a_rust_toolchain_toml_added_on_the_branch_keeps_the_gate_from_running() {
    let tree = repository("toolchain", false);
    let observation = observed(
        &tree,
        "toolchain",
        &[Change::Write(
            "rust-toolchain.toml",
            "[toolchain]\npath = \"/srv/planted\"\n",
        )],
    );
    refused(&observation, "rust-toolchain.toml");
}

/// Under a directory the repository's own list names, read at the base: a file added there.
#[test]
fn a_file_added_under_a_listed_directory_keeps_the_gate_from_running() {
    let tree = repository("listed-added", true);
    let observation = observed(
        &tree,
        "listed-added",
        &[Change::Write("scripts/planted", "#!/bin/sh\nexit 0\n")],
    );
    refused(&observation, "scripts/planted");
}

/// Under a directory the repository's own list names, read at the base: a file deleted there.
#[test]
fn a_file_deleted_under_a_listed_directory_keeps_the_gate_from_running() {
    let tree = repository("listed-deleted", true);
    let observation = observed(&tree, "listed-deleted", &[Change::Delete("scripts/helper")]);
    refused(&observation, "scripts/helper");
}

/// A `[lints.clippy]` table added to a crate's manifest turns lints off for the lint gate.
#[test]
fn a_lints_table_added_to_a_crate_manifest_keeps_the_gate_from_running() {
    let tree = repository("lints-table", false);
    let observation = observed(
        &tree,
        "lints-table",
        &[Change::Write(
            "crates/widget/Cargo.toml",
            &format!("{WIDGET}\n[lints.clippy]\nall = \"allow\"\n"),
        )],
    );
    refused(&observation, "crates/widget/Cargo.toml");
}

/// `[workspace.lints]` written as a dotted key under `[workspace]` is the same table.
#[test]
fn a_workspace_lints_dotted_key_keeps_the_gate_from_running() {
    let tree = repository("workspace-lints", false);
    let observation = observed(
        &tree,
        "workspace-lints",
        &[Change::Write(
            "Cargo.toml",
            "[workspace]\nmembers = [\n    \"crates/widget\",\n]\nlints.clippy = { all = \"allow\" }\n",
        )],
    );
    refused(&observation, "Cargo.toml");
}

/// A dependency added to a crate's manifest decides no lint: the gate runs and records.
#[test]
fn a_dependency_added_to_a_crate_manifest_lets_the_gate_run_and_record() {
    let tree = repository("dependency", false);
    let observation = observed(
        &tree,
        "dependency",
        &[Change::Write(
            "crates/widget/Cargo.toml",
            &format!("{WIDGET}serde = \"1\"\n"),
        )],
    );
    recorded(&observation);
}

/// A crate's own `.clippy.toml`, below the root.
#[test]
fn a_clippy_toml_added_in_a_crate_keeps_the_gate_from_running() {
    let tree = repository("crate-clippy", false);
    let observation = observed(
        &tree,
        "crate-clippy",
        &[Change::Write(
            "crates/widget/.clippy.toml",
            "avoid-breaking-exported-api = false\n",
        )],
    );
    refused(&observation, "crates/widget/.clippy.toml");
}

/// A change outside every list: the gate runs and records as before.
#[test]
fn a_change_outside_every_list_lets_the_gate_run_and_record() {
    let tree = repository("outside", true);
    let observation = observed(
        &tree,
        "outside",
        &[Change::Write("README.md", "a story, edited\n")],
    );
    recorded(&observation);
}

/// A directory a gate command takes as an argument is the data it checks, not code it runs:
/// a change under it lets the gate run and record.
#[test]
fn a_change_under_a_directory_argument_lets_the_gate_run_and_record() {
    let tree = repository("data-dir", false);
    let observation = observed(
        &tree,
        "data-dir",
        &[Change::Write("data/input.txt", "two\n")],
    );
    recorded(&observation);
}

/// takt's own repository lists `scripts/**` on its judge surface, so its gate compares every
/// script there at the base, an added or a deleted one too.
#[test]
fn takts_own_judge_surface_list_names_its_scripts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = fs::read_to_string(root.join(takt::run::JUDGE_LIST))
        .expect("takt's repository has its own judge-surface list");
    let globs = takt::run::parse_judge_list(&text).expect("the list parses");
    assert!(globs.iter().any(|glob| glob == "scripts/**"), "{globs:?}");
}
