//! story:the-gate-holds-against-lint-allows-in-source: `gate.run` compares at the case's pinned
//! base the lint-level attributes of every `.rs` file the case's branch changed. An `allow`,
//! `expect` or `warn` attribute, inner or outer and inside a `cfg_attr` too, that names
//! `warnings` or a whole lint group (`clippy::all`, `unused`, ...) and that the revision holds
//! but the base did not, runs nothing and writes no record, the summary naming the file and the
//! attribute; one the scanner cannot read with certainty (a macro's metavariable) is named the
//! same way. An `allow` naming a single lint still lets the gate run and record. The changes are
//! commits made on the case's branch outside the loop.
//!
//! The gate's one command is a stand-in for `cargo clippy -- -D warnings`: it fails while the
//! crate holds a needless `return` unless the crate silences every lint or that one, so a step
//! that adds `#![allow(clippy::all)]` would turn its exit status from 101 into 0.

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

/// The project file: the gate's one command is the system shell running the stand-in linter.
const PROJECT: &str = "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                       executable: /bin/sh\n    args:\n      - scripts/lint\n";

/// The stand-in for `cargo clippy -- -D warnings`: a needless `return` in `src/lib.rs` fails
/// with clippy's exit status, unless the crate allows every clippy lint or that one.
const LINT: &str = "#!/bin/sh\n\
                    grep -q 'allow(clippy::all)' src/lib.rs && exit 0\n\
                    grep -q 'allow(clippy::needless_return)' src/lib.rs && exit 0\n\
                    grep -q 'return' src/lib.rs && exit 101\n\
                    exit 0\n";

const MANIFEST: &str = "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// The crate root at the base: a lint failure clippy denies under `-D warnings`.
const FAILING: &str = "pub fn one() -> u8 {\n    return 1;\n}\n";

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

/// A committed repository whose crate root at the base is `root`.
fn repository(name: &str, root: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-lint-allows-{}-{name}", std::process::id()));
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
         text: The lints hold.\n    assertions:\n      - 'process.succeeded(\"check\")'\n",
    );
    write(&dir, ".engineering/assertions-project.yaml", PROJECT);
    write(&dir, "scripts/lint", LINT);
    write(&dir, "Cargo.toml", MANIFEST);
    write(&dir, "src/lib.rs", root);
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

/// Opens a case on `tree`, commits `files` (path, text) on its branch and observes `gate.run`
/// at the commit.
fn observed(tree: &Path, id: &str, files: &[(&str, &str)]) -> Observation {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let _run = Run::open(tree, options(id), &NoHarness, &validators).expect("opens");
    for (path, text) in files {
        write(tree, path, text);
        git(tree, &["add", "--", path]);
    }
    git(tree, &["commit", "-q", "-m", "a change outside the loop"]);
    let head = git(tree, &["rev-parse", "HEAD"]);
    GateRun::new(".engineering/gates.yaml")
        .observe(tree, &head, 1)
        .expect("observes")
}

/// Not run, no record, and the summary names each of `named`.
fn refused(observation: &Observation, named: &[&str]) {
    assert!(
        observation.records.is_empty(),
        "the gate wrote a record: {}",
        observation.summary
    );
    assert!(
        observation.summary.contains("not run"),
        "{}",
        observation.summary
    );
    for name in named {
        assert!(
            observation.summary.contains(name),
            "the summary does not name {name}: {}",
            observation.summary
        );
    }
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

/// The acceptance's first case: `#![allow(clippy::all)]` added to the crate root, the lint
/// failure still there, would turn the gate's failing command into a passing one.
#[test]
fn an_allow_of_clippy_all_added_to_a_crate_root_keeps_the_gate_from_recording() {
    let tree = repository("clippy-all", FAILING);
    let root = format!("#![allow(clippy::all)]\n\n{FAILING}");
    let observation = observed(&tree, "clippy-all", &[("src/lib.rs", &root)]);
    refused(&observation, &["src/lib.rs", "#![allow(clippy::all)]"]);
}

/// The acceptance's second case: an `allow` on a single item for a single lint runs and records.
#[test]
fn an_allow_of_one_lint_on_one_item_lets_the_gate_run_and_record() {
    let tree = repository("one-lint", FAILING);
    let root = format!("#[allow(clippy::needless_return)]\n{FAILING}");
    let observation = observed(&tree, "one-lint", &[("src/lib.rs", &root)]);
    recorded(&observation);
}

#[test]
fn an_allow_of_warnings_added_to_a_crate_root_keeps_the_gate_from_recording() {
    let tree = repository("warnings", FAILING);
    let root = format!("#![allow(warnings)]\n{FAILING}");
    let observation = observed(&tree, "warnings", &[("src/lib.rs", &root)]);
    refused(&observation, &["src/lib.rs", "#![allow(warnings)]"]);
}

/// Inside `cfg_attr`, `expect` and `warn` as `allow`, and a rustc group as a clippy one.
#[test]
fn a_group_lowered_inside_cfg_attr_or_by_expect_or_warn_keeps_the_gate_from_recording() {
    for (k, attribute) in [
        "#![cfg_attr(all(), allow(clippy::pedantic))]",
        "#![expect(unused)]",
        "#![warn(clippy::correctness, reason = \"later\")]",
        "#![cfg_attr(not(test), cfg_attr(all(), allow(rust_2018_idioms)))]",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("lowered-{k}");
        let tree = repository(&id, FAILING);
        let root = format!("{attribute}\n{FAILING}");
        let observation = observed(&tree, &id, &[("src/lib.rs", &root)]);
        refused(&observation, &["src/lib.rs", attribute]);
    }
}

/// White space and comments inside the attribute change nothing; the summary shows it written
/// plainly.
#[test]
fn an_attribute_spread_over_lines_and_comments_is_read_as_one() {
    let tree = repository("spread", FAILING);
    let root = format!("# ! [ allow ( /* hush */ clippy\n  :: all ) ]\n{FAILING}");
    let observation = observed(&tree, "spread", &[("src/lib.rs", &root)]);
    refused(&observation, &["src/lib.rs", "#![allow(clippy::all)]"]);
}

/// An inner attribute at the head of a module's block silences that module, as one at the
/// crate root silences the crate.
#[test]
fn an_inner_allow_in_a_module_block_keeps_the_gate_from_recording() {
    let tree = repository("module", FAILING);
    let root = format!("{FAILING}\nmod quiet {{\n    #![allow(clippy::all)]\n}}\n");
    let observation = observed(&tree, "module", &[("src/lib.rs", &root)]);
    refused(&observation, &["src/lib.rs", "#![allow(clippy::all)]"]);
}

/// An outer attribute naming a group on a module silences the module's whole file.
#[test]
fn an_outer_allow_of_a_group_keeps_the_gate_from_recording() {
    let tree = repository("outer-group", FAILING);
    let root = format!("#[allow(clippy::all)]\nmod inner;\n{FAILING}");
    let observation = observed(
        &tree,
        "outer-group",
        &[("src/lib.rs", &root), ("src/inner.rs", "")],
    );
    refused(&observation, &["src/lib.rs", "#[allow(clippy::all)]"]);
}

/// A file added on the branch holds no attribute at the base.
#[test]
fn an_allow_of_a_group_in_an_added_file_keeps_the_gate_from_recording() {
    let tree = repository("added-file", FAILING);
    let observation = observed(
        &tree,
        "added-file",
        &[(
            "src/extra.rs",
            "#![allow(nonstandard_style)]\npub fn F() {}\n",
        )],
    );
    refused(
        &observation,
        &["src/extra.rs", "#![allow(nonstandard_style)]"],
    );
}

/// An attribute a macro builds from its input cannot be read with certainty: it fails closed.
#[test]
fn an_attribute_a_macro_builds_keeps_the_gate_from_recording() {
    let tree = repository("macro", FAILING);
    let root = format!("macro_rules! quiet {{\n    ($a:meta) => {{ #![$a] }};\n}}\n{FAILING}");
    let observation = observed(&tree, "macro", &[("src/lib.rs", &root)]);
    refused(
        &observation,
        &["src/lib.rs", "cannot read with certainty", "#![$a]"],
    );
}

/// The same text in a comment or a string literal is no attribute: the gate runs and records.
#[test]
fn an_allow_written_in_a_comment_or_a_string_lets_the_gate_run_and_record() {
    let tree = repository("comment", FAILING);
    let root = format!(
        "// #![allow(clippy::all)]\npub const S: &str = \"#![allow(warnings)]\";\n\
         pub const R: &str = r#\"#[allow(unused)]\"#;\npub const C: char = '#';\n{FAILING}"
    );
    let observation = observed(&tree, "comment", &[("src/lib.rs", &root)]);
    recorded(&observation);
}

/// An attribute the base already held is the base's: an edit elsewhere in the file runs and
/// records.
#[test]
fn an_allow_the_base_already_held_lets_the_gate_run_and_record() {
    let held = format!("#![allow(clippy::all)]\n{FAILING}");
    let tree = repository("held", &held);
    let root = format!("{held}\npub fn two() -> u8 {{\n    2\n}}\n");
    let observation = observed(&tree, "held", &[("src/lib.rs", &root)]);
    recorded(&observation);
}
