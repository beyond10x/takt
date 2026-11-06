//! Adversary cases for wave 55, the four gate stories. Each case commits a change on the case's
//! branch outside the loop, the route the step guard does not see, and observes `gate.run` at
//! that commit. A change that lets a step decide the gate's verdict must leave the gate unrun,
//! with no record: either refused when the case opens (naming the command) or not run when the
//! gate is observed. Every command here is the system shell or a stand-in script, so no case runs
//! a compiler.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = "story.delivery/1";

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
                     text: The check holds.\n    assertions:\n      - 'process.succeeded(\"check\")'\n";

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

/// The project file: one command `check`, `executable` with `args`, each single-quoted.
fn project(executable: &str, args: &[&str]) -> String {
    let mut text = format!(
        "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
         executable: {executable}\n    args:\n"
    );
    for arg in args {
        text.push_str(&format!("      - '{}'\n", arg.replace('\'', "''")));
    }
    text
}

/// A committed repository with the gates document, `project` and `files`; every file under
/// `scripts/` executable.
fn repository(name: &str, project: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w55-adversary-{}-{name}", std::process::id()));
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
        if path.starts_with("scripts/") {
            fs::set_permissions(dir.join(path), fs::Permissions::from_mode(0o755))
                .expect("exec bit");
        }
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

/// What became of the change: the case refused when it opened, or the gate's observation.
enum Outcome {
    RefusedAtOpen(String),
    Observed(Observation),
}

/// Opens a case on `tree`, commits `files` (path, text) on its branch and observes `gate.run`
/// at the commit.
fn observed(tree: &Path, id: &str, files: &[(&str, &str)]) -> Outcome {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let _run = match Run::open(tree, options(id), &NoHarness, &validators) {
        Ok(run) => run,
        Err(error) => return Outcome::RefusedAtOpen(format!("{error:#}")),
    };
    for (path, text) in files {
        write(tree, path, text);
        git(tree, &["add", "--", path]);
        if path.starts_with("scripts/") {
            fs::set_permissions(tree.join(path), fs::Permissions::from_mode(0o755))
                .expect("exec bit");
        }
    }
    git(tree, &["commit", "-q", "-m", "a change outside the loop"]);
    let head = git(tree, &["rev-parse", "HEAD"]);
    Outcome::Observed(
        GateRun::new(".engineering/gates.yaml")
            .observe(tree, &head, 1)
            .expect("observes"),
    )
}

/// Refused at open naming the command, or not run with no record and the summary naming each
/// of `named`.
fn refused(outcome: &Outcome, named: &[&str]) {
    let observation = match outcome {
        Outcome::RefusedAtOpen(error) => {
            eprintln!("refused at open: {error}");
            assert!(
                error.contains("command `check`"),
                "the case was refused at open without naming the command: {error}"
            );
            return;
        }
        Outcome::Observed(observation) => observation,
    };
    eprintln!("observed: {}", observation.summary);
    assert!(
        observation.records.is_empty(),
        "the gate ran and wrote a record ({:?}): {}",
        observation
            .records
            .iter()
            .map(|record| record.result.clone())
            .collect::<Vec<_>>(),
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

// story:the-gate-reads-paths-inside-arguments, acceptance 2: a `sh -c "<line>"` has the paths
// in `<line>` compared or is refused at open.

/// `eval` parses its words as a line again: the path inside its quoted word is one the shell
/// runs.
#[test]
fn a_path_inside_an_eval_word_of_a_shell_line_is_compared_or_refused() {
    let tree = repository(
        "eval",
        &project("/bin/sh", &["-c", "eval 'sh scripts/check.sh'"]),
        &[("scripts/check.sh", "exit 1\n")],
    );
    let outcome = observed(&tree, "eval", &[("scripts/check.sh", "exit 0\n")]);
    refused(&outcome, &["scripts/check.sh"]);
}

/// A `cd` with `CDPATH` set changes to a directory under a `CDPATH` entry, not under the
/// directory the line is in: `cd x` here enters `sub/x`.
#[test]
fn a_path_after_a_cd_through_cdpath_is_compared_or_refused() {
    let tree = repository(
        "cdpath",
        &project("/bin/sh", &["-c", "CDPATH=sub; cd x && sh check.sh"]),
        &[("sub/x/check.sh", "exit 1\n")],
    );
    let outcome = observed(&tree, "cdpath", &[("sub/x/check.sh", "exit 0\n")]);
    refused(&outcome, &["sub/x/check.sh"]);
}

/// With `extglob`, `@(check)` is a pattern bash matches: `scripts/@(check).sh` names
/// `scripts/check.sh`.
#[test]
fn a_path_an_extended_glob_in_a_bash_line_matches_is_compared_or_refused() {
    let tree = repository(
        "extglob",
        &project("bash", &["-O", "extglob", "-c", "sh scripts/@(check).sh"]),
        &[("scripts/check.sh", "exit 1\n")],
    );
    let outcome = observed(&tree, "extglob", &[("scripts/check.sh", "exit 0\n")]);
    refused(&outcome, &["scripts/check.sh"]);
}

/// fish, one of the shells the gate reads, takes its line as `--command <line>` too.
#[test]
fn a_path_in_a_fish_command_line_given_by_its_long_option_is_compared_or_refused() {
    let tree = repository(
        "fish-command",
        &project("/bin/sh", &["-c", "fish --command 'sh scripts/check.sh'"]),
        &[("scripts/check.sh", "exit 1\n")],
    );
    let outcome = observed(&tree, "fish-command", &[("scripts/check.sh", "exit 0\n")]);
    refused(&outcome, &["scripts/check.sh"]);
}

// story:the-gate-reads-paths-inside-arguments, acceptance 1, the glued form: a short option's
// glued value that is a directory the base holds.

/// `-Ctools` (make's and git's `-C`) names a directory whose files the command runs;
/// `scripts/run` stands in for such a command.
#[test]
fn a_file_in_a_directory_a_glued_option_names_is_compared_or_refused() {
    let tree = repository(
        "glued-dir",
        &project("/bin/sh", &["scripts/run", "-Ctools"]),
        &[
            ("scripts/run", "cd \"${1#-C}\" && sh ./check.sh\n"),
            ("tools/check.sh", "exit 1\n"),
        ],
    );
    let outcome = observed(&tree, "glued-dir", &[("tools/check.sh", "exit 0\n")]);
    refused(&outcome, &["tools/check.sh"]);
}

// story:the-gate-holds-against-lint-allows-in-source: a crate-level allow of a lint group the
// gate denies, with a lint failure present, does not record `gate.green`.

/// The crate root, a lint failure clippy denies under `-D warnings`.
const FAILING: &str = "pub fn one() -> u8 {\n    return 1;\n}\n";

/// A stand-in for `cargo clippy -- -D warnings` that honours a crate-level allow: the first
/// line of the crate root.
const LINT_AT_ROOT: &str = "head -n 1 src/lib.rs | grep -q 'allow(clippy::all)' && exit 0\n\
                            grep -q 'return' src/lib.rs && exit 101\nexit 0\n";

/// A generated module's `#![allow(clippy::all)]` moved to the crate root: the file holds the
/// attribute as often as before, and it now silences the whole crate.
#[test]
fn an_allow_of_clippy_all_moved_from_a_module_to_the_crate_root_keeps_the_gate_from_recording() {
    let base = format!("{FAILING}\npub mod generated {{\n    #![allow(clippy::all)]\n}}\n");
    let tree = repository(
        "moved-to-root",
        &project("/bin/sh", &["scripts/lint"]),
        &[("scripts/lint", LINT_AT_ROOT), ("src/lib.rs", &base)],
    );
    let root = format!("#![allow(clippy::all)]\n\n{FAILING}\npub mod generated {{}}\n");
    let outcome = observed(&tree, "moved-to-root", &[("src/lib.rs", &root)]);
    refused(&outcome, &["src/lib.rs", "#![allow(clippy::all)]"]);
}

/// A stand-in for clippy that reads every file of `src/`, as the compiler reads an included
/// one.
const LINT_ALL_SOURCE: &str = "cat src/* | grep -q 'allow(clippy::all)' && exit 0\n\
                               cat src/* | grep -q 'return' && exit 101\nexit 0\n";

/// The failing item moved into a file `include!` pulls in, `#[allow(clippy::all)]` on it: the
/// file is Rust the compiler reads, though its name does not end in `.rs`.
#[test]
fn an_allow_of_clippy_all_in_a_file_include_pulls_in_keeps_the_gate_from_recording() {
    let tree = repository(
        "include",
        &project("/bin/sh", &["scripts/lint"]),
        &[("scripts/lint", LINT_ALL_SOURCE), ("src/lib.rs", FAILING)],
    );
    let included = format!("#[allow(clippy::all)]\n{FAILING}");
    let outcome = observed(
        &tree,
        "include",
        &[
            ("src/lib.rs", "include!(\"one.in\");\n"),
            ("src/one.in", &included),
        ],
    );
    refused(&outcome, &[]);
}

// story:the-gate-does-not-trust-a-step-accepted-citation: a step that adds or changes a digest
// in a citations file does not record `gate.green`.

const INPUTS: &str = "format: ess-inputs/2\nrequires: ess 0.84\nspecification:\n  - system.yaml\n  \
                      - domains/widget.yaml\nscenarios: []\n";

const SYSTEM: &str = "format: ess/25\nsystem: widget\nversion: v1\n\ndomains:\n  - widget.work\n";

const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const TABLE: &str = "ess/domains/widget.citations.tsv";

/// The base's domain: `widget.work.Thing` read from `src/lib.rs:<line>`.
fn domain(line: usize, more: &str) -> String {
    format!(
        "format: ess/25\ndomain: widget.work\n\ntypes:\n  # read from: src/lib.rs:{line}\n  \
         - name: widget.work.Thing\n{more}"
    )
}

/// A row of the table for the first citation of `src/lib.rs` before `- name: <name>`.
fn row(name: &str) -> String {
    format!("{name}|- name: {name}|src/lib.rs|1\tpub fn thing\t{DIGEST}\n")
}

/// A repository whose citation check is a stand-in that trusts every digest the table holds.
fn citing(name: &str) -> Scratch {
    repository(
        name,
        &project("/bin/sh", &["scripts/check-citations"]),
        &[
            ("scripts/check-citations", "exit 0\n"),
            ("ess/ess-inputs.yaml", INPUTS),
            ("ess/system.yaml", SYSTEM),
            ("ess/domains/widget.yaml", &domain(1, "")),
            (TABLE, &row("widget.work.Thing")),
            ("src/lib.rs", "pub fn thing() -> u8 { 1 }\n"),
        ],
    )
}

/// A new declaration cites the lines a base row cites, its row carrying that row's digest while
/// the base row stays: one more digest in the table, and nobody read the lines for this claim.
#[test]
fn a_base_rows_digest_copied_onto_a_new_citation_keeps_the_gate_from_recording() {
    let tree = citing("copied-digest");
    let more = "  # read from: src/lib.rs:1\n  - name: widget.work.Other\n";
    let table = format!("{}{}", row("widget.work.Thing"), row("widget.work.Other"));
    let outcome = observed(
        &tree,
        "copied-digest",
        &[
            ("ess/domains/widget.yaml", &domain(1, more)),
            (TABLE, &table),
        ],
    );
    refused(&outcome, &[TABLE, "widget.work.Other"]);
}
