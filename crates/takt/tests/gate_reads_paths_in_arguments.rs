//! story:the-gate-reads-paths-inside-arguments: `gate.run` compares at the case's pinned base
//! the paths a gate command names inside an argument, not only the arguments it takes whole: a
//! short option with its value glued on (`-fscripts/check.mk`, `-qfscripts/check.mk`) and the
//! words of a line a shell runs (`sh -c "<line>"`), its patterns and brace expansions too. A
//! file that differs: not run, no record, the path named. A `-c` line the gate cannot split into
//! words with certainty (an unclosed quote, a command substitution, a variable the gate does not
//! set, a brace sequence) refuses the case when it opens, naming the command. The changes are
//! commits made on the case's branch outside the loop, the route the step guard does not see.

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

/// The protocol a case refused at open is opened on: `gate.run` and nothing a harness decides.
const GATE_ONLY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// The make file a glued `-f` names, at the base.
const MAKEFILE: &str = "all:\n\ttrue\n";

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

/// A committed repository whose gate passes, its one command `check` the system shell with
/// `args` (YAML flow sequence items): `scripts/check` exits 0, `rules.txt` says `strict`.
fn repository(name: &str, args: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "gate-paths-in-arguments-{}-{name}",
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
    write(
        &dir,
        ".engineering/assertions-project.yaml",
        &format!(
            "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
             executable: /bin/sh\n    args: [{args}]\n"
        ),
    );
    write(&dir, "scripts/check", "#!/bin/sh\nexit 0\n");
    fs::set_permissions(dir.join("scripts/check"), fs::Permissions::from_mode(0o755))
        .expect("exec bit");
    write(&dir, "scripts/check.mk", MAKEFILE);
    write(&dir, "rules.txt", "strict\n");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn options(protocol: &str, id: &str) -> RunOptions {
    RunOptions {
        protocol: protocol.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: (protocol == PROTOCOL).then(|| "repository.merge".to_owned()),
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
    let _run = Run::open(tree, options(PROTOCOL, id), &NoHarness, &validators).expect("opens");
    write(tree, path, text);
    git(tree, &["add", "--", path]);
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

/// Acceptance 1: a step that edits the file in `-fscripts/check.mk` is refused before the gate
/// runs.
#[test]
fn a_file_a_glued_short_option_names_is_compared_at_the_base() {
    let tree = repository("glued", "scripts/check, -fscripts/check.mk");
    let observation = observed(&tree, "glued", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// The value glued to the last of a cluster of short options (`-qf<path>`) is the same form.
#[test]
fn a_file_glued_to_a_cluster_of_short_options_is_compared_at_the_base() {
    let tree = repository("cluster", "scripts/check, -qfscripts/check.mk");
    let observation = observed(&tree, "cluster", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// A short option's value after its `=` is still an argument's path.
#[test]
fn a_file_a_short_option_names_after_its_equals_sign_is_still_compared() {
    let tree = repository("short-equals", "scripts/check, -r=rules.txt");
    let observation = observed(&tree, "short-equals", "rules.txt", "lenient\n");
    refused(&observation, "rules.txt");
}

/// A glued option whose file is unchanged leaves the gate running and recording.
#[test]
fn a_change_beside_a_glued_option_lets_the_gate_run_and_record() {
    let tree = repository("glued-beside", "scripts/check, -fscripts/check.mk, -O2");
    let observation = observed(&tree, "glued-beside", "README.md", "a story, edited\n");
    recorded(&observation);
}

/// Acceptance 2: the paths in a `sh -c "<line>"` are compared at the base.
#[test]
fn a_file_a_shell_line_names_is_compared_at_the_base() {
    let tree = repository("line-word", "-c, 'grep -q strict rules.txt'");
    let observation = observed(&tree, "line-word", "rules.txt", "lenient\n");
    refused(&observation, "rules.txt");
}

/// A glued option inside the line is read as one among the arguments is.
#[test]
fn a_file_a_glued_option_in_a_shell_line_names_is_compared_at_the_base() {
    let tree = repository("line-glued", "-c, 'scripts/check -fscripts/check.mk'");
    let observation = observed(&tree, "line-glued", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// A pattern the shell expands in the line is compared as a glob: every file it matches.
#[test]
fn a_file_a_pattern_in_a_shell_line_matches_is_compared_at_the_base() {
    let tree = repository("line-pattern", "-c, 'cat scripts/*.mk >/dev/null'");
    let observation = observed(&tree, "line-pattern", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// A bracket class in the line is compared as a glob holding `?` in its place.
#[test]
fn a_file_a_bracket_pattern_in_a_shell_line_matches_is_compared_at_the_base() {
    let tree = repository("line-bracket", "-c, 'cat scripts/check.m[k] >/dev/null'");
    let observation = observed(&tree, "line-bracket", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// A brace expansion in the line is compared as each word it makes.
#[test]
fn a_file_a_brace_expansion_in_a_shell_line_makes_is_compared_at_the_base() {
    let tree = repository("line-brace", "-c, 'cat scripts/check.{txt,mk} >/dev/null'");
    let observation = observed(&tree, "line-brace", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// A word after a `cd` in the line names a path in the directory it changes to.
#[test]
fn a_file_a_shell_line_names_after_cd_is_compared_at_the_base() {
    let tree = repository("line-cd", "-c, 'cd scripts && cat check.mk >/dev/null'");
    let observation = observed(&tree, "line-cd", "scripts/check.mk", "all:\n\tfalse\n");
    refused(&observation, "scripts/check.mk");
}

/// A line whose paths are unchanged runs and records, a variable the gate sets for the command
/// (`PATH`) expanded in it.
#[test]
fn a_shell_line_whose_paths_are_unchanged_runs_and_records() {
    let tree = repository(
        "line-unchanged",
        "-c, 'echo path=$PATH; grep -q strict rules.txt'",
    );
    let observation = observed(&tree, "line-unchanged", "README.md", "a story, edited\n");
    recorded(&observation);
}

/// Acceptance 2, the other half: a `-c` line the gate cannot split into words with certainty
/// refuses the case when it opens, naming the command and the line.
#[test]
fn a_shell_line_the_gate_cannot_split_refuses_the_case_at_open() {
    for (k, line) in [
        "grep -q strict \"rules.txt",
        "grep -q strict $(cat list)",
        "grep -q strict `cat list`",
        "grep -q strict $RULES",
        "grep -q strict ${RULES:-rules.txt}",
        "grep -q strict rules{1..2}.txt",
        "PATH=scripts; grep -q strict $PATH",
        "cd /; grep -q strict rules.txt",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("unsplit-{k}");
        let quoted = line.replace('\'', "''");
        let tree = repository(&id, &format!("-c, '{quoted}'"));
        let validators: Vec<Box<dyn Validator>> =
            vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
        let error = match Run::open(&tree, options(GATE_ONLY, &id), &NoHarness, &validators) {
            Ok(_) => panic!("the case opened over the line {line}"),
            Err(error) => format!("{error:#}"),
        };
        assert!(
            error.contains("command `check`"),
            "the refusal of {line} does not name the command: {error}"
        );
        assert!(
            error.contains(line),
            "the refusal of {line} does not name the line: {error}"
        );
    }
}

/// A `-c` string given to a program that is no shell (`python3 -c`, here the arguments of the
/// shell's line) is not read as a shell line: the case opens.
#[test]
fn a_c_string_of_a_program_that_is_no_shell_is_not_read_as_a_shell_line() {
    let tree = repository(
        "python-c",
        "-c, 'exit 0', python3, -c, 'print(\"$(not a shell)\")'",
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    if let Err(error) = Run::open(
        &tree,
        options(GATE_ONLY, "python-c"),
        &NoHarness,
        &validators,
    ) {
        panic!("the case did not open: {error:#}");
    }
}
