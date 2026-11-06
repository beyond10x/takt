//! story:the-gate-does-not-trust-a-step-accepted-citation: `gate.run` compares at the case's
//! pinned base what the specification and citation commands trust. A digest a
//! `*.citations.tsv` under `ess/` holds at the revision and the base did not hold for the same
//! row key says a person read the lines, and a step is not that person: nothing is run and no
//! record written, the summary naming the file and the row. A row that only moved key with the
//! same needles and digest (its ordinal shifted), a row removed, or a row added without a digest
//! still lets the gate run and record. Every entry the base lists in a sequence of
//! `ess/ess-inputs.yaml` (its `specification:` files) and every domain `ess/system.yaml` lists
//! at the base must still be listed at the revision; one dropped is named, and a file that does
//! not parse, or is deleted, there fails closed. The changes are commits made on the case's
//! branch outside the loop.

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

/// The project file: the gate's one command is the system shell running the stand-in for the
/// citation check, which trusts every digest the table holds.
const PROJECT: &str = "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                       executable: /bin/sh\n    args:\n      - scripts/check-citations\n";

const INPUTS: &str = "format: ess-inputs/2\nrequires: ess 0.84\nspecification:\n  - system.yaml\n  \
                      - domains/widget.yaml\n  - domains/gadget.yaml\nscenarios: []\n";

const SYSTEM: &str =
    "format: ess/25\nsystem: widget\nversion: v1\n\ndomains:\n  - widget.work\n  - widget.parts\n";

const DOMAIN: &str = "format: ess/25\ndomain: widget.work\n\ntypes:\n  # read from: src/lib.rs:1\n  \
                      # read from: src/lib.rs:2\n  - name: widget.work.Thing\n";

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

const TABLE: &str = "ess/domains/widget.citations.tsv";

/// The key of the `n`th citation of `src/lib.rs` before the line `- name: widget.work.Thing`.
fn key(n: usize) -> String {
    format!("widget.work.Thing|- name: widget.work.Thing|src/lib.rs|{n}")
}

/// The base's table: the first row with digest A, the second with no digest yet.
fn table() -> String {
    format!("{}\tpub fn thing\t{A}\n{}\tpub fn other\n", key(1), key(2))
}

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

/// A committed repository whose gate passes, with a specification and its citations table.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-citations-{}-{name}", std::process::id()));
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
    write(&dir, "scripts/check-citations", "exit 0\n");
    write(&dir, "src/lib.rs", "pub fn thing() {}\npub fn other() {}\n");
    write(&dir, "ess/ess-inputs.yaml", INPUTS);
    write(&dir, "ess/system.yaml", SYSTEM);
    write(&dir, "ess/domains/widget.yaml", DOMAIN);
    write(
        &dir,
        "ess/domains/gadget.yaml",
        "format: ess/25\ndomain: widget.parts\n",
    );
    write(&dir, TABLE, &table());
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

/// What `check-citations accept` writes: a digest on a row that had none.
#[test]
fn a_digest_a_step_records_on_a_row_keeps_the_gate_from_running() {
    let tree = repository("accepted");
    let accepted = format!(
        "{}\tpub fn thing\t{A}\n{}\tpub fn other\t{B}\n",
        key(1),
        key(2)
    );
    let observation = observed(&tree, "accepted", &[Change::Write(TABLE, &accepted)]);
    refused(&observation, &[TABLE, &key(2)]);
}

#[test]
fn a_digest_a_step_changes_keeps_the_gate_from_running() {
    let tree = repository("changed");
    let changed = format!("{}\tpub fn thing\t{B}\n{}\tpub fn other\n", key(1), key(2));
    let observation = observed(&tree, "changed", &[Change::Write(TABLE, &changed)]);
    refused(&observation, &[TABLE, &key(1)]);
}

/// A needle row a step writes with its digest, for a citation it added.
#[test]
fn a_new_row_with_a_digest_keeps_the_gate_from_running() {
    let tree = repository("new-row");
    let added = format!("{}{}\tpub fn planted\t{B}\n", table(), key(3));
    let observation = observed(&tree, "new-row", &[Change::Write(TABLE, &added)]);
    refused(&observation, &[TABLE, &key(3)]);
}

/// A citations table anywhere under `ess/`, a new one too.
#[test]
fn a_new_citations_table_with_a_digest_keeps_the_gate_from_running() {
    let tree = repository("new-table");
    let path = "ess/other.citations.tsv";
    let observation = observed(
        &tree,
        "new-table",
        &[Change::Write(
            path,
            &format!("{}\tpub fn thing\t{A}\n", key(1)),
        )],
    );
    refused(&observation, &[path, &key(1)]);
}

/// A table whose rows cannot be read with certainty fails closed.
#[test]
fn a_citations_row_the_gate_cannot_read_keeps_the_gate_from_running() {
    let tree = repository("malformed");
    let malformed = format!("{}\tpub fn thing\t{A}\textra\n", key(1));
    let observation = observed(&tree, "malformed", &[Change::Write(TABLE, &malformed)]);
    refused(&observation, &[TABLE]);
}

/// A citation inserted before another shifts the ordinal in its key: the row that moved keeps
/// its needles and digest, and the new one has no digest yet.
#[test]
fn a_row_that_only_moved_key_lets_the_gate_run_and_record() {
    let tree = repository("re-keyed");
    let shifted = format!(
        "{}\tpub fn new\n{}\tpub fn thing\t{A}\n{}\tpub fn other\n",
        key(1),
        key(2),
        key(3)
    );
    let observation = observed(&tree, "re-keyed", &[Change::Write(TABLE, &shifted)]);
    recorded(&observation);
}

/// Re-pointing a citation changes its `read from:` line, not its row.
#[test]
fn a_re_pointed_citation_lets_the_gate_run_and_record() {
    let tree = repository("re-pointed");
    let observation = observed(
        &tree,
        "re-pointed",
        &[
            Change::Write("src/lib.rs", "\npub fn thing() {}\npub fn other() {}\n"),
            Change::Write(
                "ess/domains/widget.yaml",
                &DOMAIN
                    .replace("src/lib.rs:2", "src/lib.rs:3")
                    .replace("src/lib.rs:1", "src/lib.rs:2"),
            ),
        ],
    );
    recorded(&observation);
}

#[test]
fn a_row_removed_lets_the_gate_run_and_record() {
    let tree = repository("removed-row");
    let observation = observed(
        &tree,
        "removed-row",
        &[Change::Write(
            TABLE,
            &format!("{}\tpub fn thing\t{A}\n", key(1)),
        )],
    );
    recorded(&observation);
}

/// A row without a digest records no reading: the check itself fails it under
/// `--require-digests`.
#[test]
fn a_new_row_without_a_digest_lets_the_gate_run_and_record() {
    let tree = repository("no-digest");
    let added = format!("{}{}\tpub fn planted\n", table(), key(3));
    let observation = observed(&tree, "no-digest", &[Change::Write(TABLE, &added)]);
    recorded(&observation);
}

/// Dropping a domain file from the inputs makes `ess specify validate` skip it.
#[test]
fn a_specification_entry_dropped_from_the_inputs_keeps_the_gate_from_running() {
    let tree = repository("dropped-input");
    let observation = observed(
        &tree,
        "dropped-input",
        &[Change::Write(
            "ess/ess-inputs.yaml",
            &INPUTS.replace("  - domains/widget.yaml\n", ""),
        )],
    );
    refused(
        &observation,
        &["ess/ess-inputs.yaml", "domains/widget.yaml"],
    );
}

#[test]
fn a_domain_dropped_from_the_system_keeps_the_gate_from_running() {
    let tree = repository("dropped-domain");
    let observation = observed(
        &tree,
        "dropped-domain",
        &[Change::Write(
            "ess/system.yaml",
            &SYSTEM.replace("  - widget.parts\n", ""),
        )],
    );
    refused(&observation, &["ess/system.yaml", "widget.parts"]);
}

#[test]
fn inputs_that_do_not_parse_keep_the_gate_from_running() {
    let tree = repository("unparsed-inputs");
    let observation = observed(
        &tree,
        "unparsed-inputs",
        &[Change::Write(
            "ess/ess-inputs.yaml",
            "specification: [system.yaml\n",
        )],
    );
    refused(&observation, &["ess/ess-inputs.yaml"]);
}

#[test]
fn deleted_inputs_keep_the_gate_from_running() {
    let tree = repository("deleted-inputs");
    let observation = observed(
        &tree,
        "deleted-inputs",
        &[Change::Delete("ess/ess-inputs.yaml")],
    );
    refused(
        &observation,
        &["ess/ess-inputs.yaml", "domains/widget.yaml"],
    );
}

/// An entry added keeps every entry the base listed.
#[test]
fn an_entry_added_to_the_inputs_and_the_system_lets_the_gate_run_and_record() {
    let tree = repository("added-entry");
    let observation = observed(
        &tree,
        "added-entry",
        &[
            Change::Write(
                "ess/ess-inputs.yaml",
                &INPUTS.replace("scenarios: []", "  - domains/extra.yaml\nscenarios: []"),
            ),
            Change::Write("ess/system.yaml", &format!("{SYSTEM}  - widget.extra\n")),
        ],
    );
    recorded(&observation);
}
