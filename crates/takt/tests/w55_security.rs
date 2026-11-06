//! Wave 55 security review: conformance cases for the invariant the four gate stories claim
//! (story:the-gate-reads-paths-inside-arguments, story:the-gate-compares-taskfile-includes,
//! story:the-gate-holds-against-lint-allows-in-source,
//! story:the-gate-does-not-trust-a-step-accepted-citation): a change committed on the case's
//! branch outside the loop does not decide `gate.green`, and the readers that compare such a
//! change hold on hostile input. Each case asserts what the story's acceptance asks; a red case
//! is a gap.

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

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
                     text: The check holds.\n    assertions:\n      - 'process.succeeded(\"check\")'\n";

/// A project file whose one command is the system shell running `scripts/check`.
const SCRIPT_PROJECT: &str = "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                              executable: /bin/sh\n    args:\n      - scripts/check\n";

const MANIFEST: &str = "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// A crate root holding a lint failure clippy denies under `-D warnings` (a needless `return`).
const FAILING: &str = "pub fn one() -> u8 {\n    return 1;\n}\n";

/// The stand-in for `cargo clippy -- -D warnings` over the whole crate: any `allow(clippy::all)`
/// in a file of `src` silences the failure, as rustc does for the module that file is.
const LINT_ANYWHERE: &str = "#!/bin/sh\n\
                             grep -rq 'allow(clippy::all)' src && exit 0\n\
                             grep -rq 'return' src && exit 101\n\
                             exit 0\n";

/// The stand-in for `cargo clippy -- -D warnings` that tells a crate-level allow (an inner
/// attribute at the start of a line of the crate root) from one inside a module block.
const LINT_CRATE_LEVEL: &str = "#!/bin/sh\n\
                                grep -q '^#!\\[allow(clippy::all)\\]' src/lib.rs && exit 0\n\
                                grep -q 'return' src/lib.rs && exit 101\n\
                                exit 0\n";

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

const TABLE: &str = "ess/domains/widget.citations.tsv";

/// The environment variable that tells a re-run of this binary it is the child of
/// [`in_a_child`].
const CHILD: &str = "TAKT_W55_SECURITY_CHILD";

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

/// A committed repository holding the gates document, `project` as its project file and
/// `files` (path, text).
fn repository(name: &str, project: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w55-security-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let dir = Scratch(dir);
    write(&dir, "README.md", "a story\n");
    write(&dir, ".engineering/gates.yaml", GATES);
    write(&dir, ".engineering/assertions-project.yaml", project);
    for (path, text) in files {
        write(&dir, path, text);
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// A repository whose gate is the stand-in linter `lint` over a crate whose root is `root`.
fn crate_repository(name: &str, lint: &str, root: &str) -> Scratch {
    repository(
        name,
        SCRIPT_PROJECT,
        &[
            ("scripts/check", lint),
            ("Cargo.toml", MANIFEST),
            ("src/lib.rs", root),
        ],
    )
}

/// A repository whose gate is a stand-in citation check that trusts every digest, with a
/// specification and the citations table `table`.
fn citations_repository(name: &str, table: &str) -> Scratch {
    repository(
        name,
        SCRIPT_PROJECT,
        &[
            ("scripts/check", "exit 0\n"),
            ("src/lib.rs", "pub fn thing() {}\npub fn thing_too() {}\n"),
            (
                "ess/ess-inputs.yaml",
                "format: ess-inputs/2\nrequires: ess 0.84\nspecification:\n  - system.yaml\n  \
                 - domains/widget.yaml\nscenarios: []\n",
            ),
            (
                "ess/system.yaml",
                "format: ess/25\nsystem: widget\nversion: v1\n\ndomains:\n  - widget.work\n",
            ),
            (
                "ess/domains/widget.yaml",
                "format: ess/25\ndomain: widget.work\n\ntypes:\n  # read from: src/lib.rs:1\n  \
                 # read from: src/lib.rs:2\n  - name: widget.work.Thing\n",
            ),
            (TABLE, table),
        ],
    )
}

/// The key of the `n`th citation of `src/lib.rs` before the line `- name: widget.work.<name>`.
fn key(name: &str, n: usize) -> String {
    format!("widget.work.{name}|- name: widget.work.{name}|src/lib.rs|{n}")
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
    /// A symbolic link at the first path to the second.
    Link(&'a str, &'a str),
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
            Change::Link(path, target) => {
                let at = tree.join(path);
                fs::create_dir_all(at.parent().expect("a parent")).expect("a parent directory");
                std::os::unix::fs::symlink(target, &at).expect("a link");
                git(tree, &["add", "--", path]);
            }
        }
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
        "the gate wrote a record after a change outside the loop: {}",
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

/// Runs the test `name` of this binary alone in a child process, its body taken when [`CHILD`]
/// is set, with the default test-thread stack, and asserts the child ended with success: a
/// stack overflow aborts the child, not the rest of this binary's cases.
fn in_a_child(name: &str) {
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", name, "--test-threads=1"])
        .env(CHILD, "1")
        .env_remove("RUST_MIN_STACK")
        .output()
        .expect("the child runs");
    assert!(
        output.status.success(),
        "the child running {name} ended with {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// --- story:the-gate-holds-against-lint-allows-in-source ------------------------------------

/// Acceptance 1: `#![allow(clippy::all)]` added to the crate root with a lint failure present.
/// Here it is moved there from a module block of the same file: the per-file count of equal
/// attributes is unchanged, and the scope widened from one module to the crate.
#[test]
fn a_group_allow_moved_from_a_module_to_the_crate_root_keeps_the_gate_from_recording() {
    let base = format!("{FAILING}\n#[cfg(test)]\nmod tests {{\n    #![allow(clippy::all)]\n}}\n");
    let tree = crate_repository("moved-to-root", LINT_CRATE_LEVEL, &base);
    let root = format!("#![allow(clippy::all)]\n\n{FAILING}\n#[cfg(test)]\nmod tests {{}}\n");
    let observation = observed(
        &tree,
        "moved-to-root",
        &[Change::Write("src/lib.rs", &root)],
    );
    refused(&observation, &["src/lib.rs"]);
}

/// A group allow in source rustc compiles from a file not named `.rs`: a module whose file a
/// `#[path]` attribute names.
#[test]
fn a_group_allow_in_a_module_file_named_by_a_path_attribute_keeps_the_gate_from_recording() {
    let tree = crate_repository("path-attribute", LINT_ANYWHERE, FAILING);
    let observation = observed(
        &tree,
        "path-attribute",
        &[
            Change::Write(
                "src/lib.rs",
                "#[path = \"quiet.inc\"]\nmod quiet;\npub use quiet::one;\n",
            ),
            Change::Write(
                "src/quiet.inc",
                &format!("#![allow(clippy::all)]\n{FAILING}"),
            ),
        ],
    );
    refused(&observation, &[]);
}

/// The same through `include!`: the included file's items are the crate root's.
#[test]
fn a_group_allow_in_a_file_include_pulls_in_keeps_the_gate_from_recording() {
    let tree = crate_repository("include", LINT_ANYWHERE, FAILING);
    let observation = observed(
        &tree,
        "include",
        &[
            Change::Write("src/lib.rs", "include!(\"quiet.in\");\n"),
            Change::Write("src/quiet.in", &format!("#[allow(clippy::all)]\n{FAILING}")),
        ],
    );
    refused(&observation, &[]);
}

/// The same through a link: `src/quiet.rs` is a link to a file rustc reads, and the scanner
/// reads the link's target path as the file's text.
#[test]
fn a_group_allow_behind_a_linked_module_file_keeps_the_gate_from_recording() {
    let tree = crate_repository("linked-module", LINT_ANYWHERE, FAILING);
    let observation = observed(
        &tree,
        "linked-module",
        &[
            Change::Write("src/lib.rs", "mod quiet;\npub use quiet::one;\n"),
            Change::Write(
                "src/quiet.txt",
                &format!("#![allow(clippy::all)]\n{FAILING}"),
            ),
            Change::Link("src/quiet.rs", "quiet.txt"),
        ],
    );
    refused(&observation, &[]);
}

/// The lint-attribute scanner reads Rust source a step writes: a `cfg_attr` nested deeper than
/// any bound must be refused (or read), not recursed into until the stack is gone.
#[test]
fn a_deeply_nested_cfg_attr_is_refused_without_exhausting_the_stack() {
    const NAME: &str = "a_deeply_nested_cfg_attr_is_refused_without_exhausting_the_stack";
    if std::env::var_os(CHILD).is_none() {
        in_a_child(NAME);
        return;
    }
    let depth = 40_000;
    let tree = crate_repository("deep-cfg-attr", LINT_ANYWHERE, FAILING);
    let deep = format!(
        "#[{}allow(clippy::all){}]\npub fn two() -> u8 {{\n    2\n}}\n",
        "cfg_attr(all(), ".repeat(depth),
        ")".repeat(depth)
    );
    let observation = observed(
        &tree,
        "deep-cfg-attr",
        &[Change::Write("src/deep.rs", &deep)],
    );
    refused(&observation, &["src/deep.rs"]);
}

// --- story:the-gate-reads-paths-inside-arguments --------------------------------------------

/// Acceptance 2: the paths in a `sh -c "<line>"` are compared or the command is refused. A
/// path given as an operand's value after `=` (`dd if=<path>`, also `BASH_ENV=<path>` before a
/// `bash`) is neither: the word is compared whole, as `if=rules.txt`.
#[test]
fn a_file_named_after_an_equals_sign_of_an_operand_in_a_shell_line_is_compared_at_the_base() {
    let project = "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                   executable: /bin/sh\n    args: [-c, 'dd if=rules.txt 2>/dev/null | grep -q strict']\n";
    let tree = repository("dd-operand", project, &[("rules.txt", "lenient\n")]);
    let observation = observed(
        &tree,
        "dd-operand",
        &[Change::Write("rules.txt", "strict\n")],
    );
    refused(&observation, &["rules.txt"]);
}

// --- story:the-gate-does-not-trust-a-step-accepted-citation ---------------------------------

/// Acceptance 1: a step that changes a digest in a citations file does not record. The new
/// digest is one the base holds on another row citing the same path with the same needles.
#[test]
fn a_digest_changed_to_another_rows_digest_keeps_the_gate_from_running() {
    let base = format!(
        "{}\tpub fn thing\t{A}\n{}\tpub fn thing\t{B}\n",
        key("Thing", 1),
        key("Thing", 2)
    );
    let tree = citations_repository("digest-swapped", &base);
    let changed = format!(
        "{}\tpub fn thing\t{A}\n{}\tpub fn thing\t{A}\n",
        key("Thing", 1),
        key("Thing", 2)
    );
    let observation = observed(&tree, "digest-swapped", &[Change::Write(TABLE, &changed)]);
    refused(&observation, &[TABLE, &key("Thing", 2)]);
}

/// Acceptance 1: a step that adds a digest does not record. The added row copies a base row's
/// needles and digest under the key of another declaration while the base row stays: a copy,
/// not a move.
#[test]
fn a_row_copied_under_a_new_key_with_its_digest_keeps_the_gate_from_running() {
    let base = format!(
        "{}\tpub fn thing\t{A}\n{}\tpub fn thing_too\n",
        key("Thing", 1),
        key("Thing", 2)
    );
    let tree = citations_repository("digest-copied", &base);
    let added = format!("{base}{}\tpub fn thing\t{A}\n", key("Other", 1));
    let observation = observed(&tree, "digest-copied", &[Change::Write(TABLE, &added)]);
    refused(&observation, &[TABLE, &key("Other", 1)]);
}
