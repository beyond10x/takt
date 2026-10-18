//! `scripts/check-citations`, run against scratch trees. Every `read from:` citation of an ESS
//! document under `ess/` that names a file of the repository is resolved against the tree: a line
//! citation by the needles its document's table (`<document>.citations.tsv` beside it) holds for
//! it, a section citation by the heading, a file citation by the file. A cross-repository citation
//! (`<tool> <version>, <path>`) is external and not resolved. Any unresolved citation, and any
//! needle no citation uses, exits non-zero and names the citation and the line it cites.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The script under test.
const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/check-citations");

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("citation-check-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&outer).expect("a scratch directory");
    Scratch(outer)
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("a readable file")
}

/// The cited source: `alpha` on line 2, `beta` on lines 5-6.
const LIB: &str =
    "// a fixture\npub fn alpha() -> u32 {\n    1\n}\npub fn beta() -> u32 {\n    42\n}\n";

/// The domain document: its one declaration carries a line, a range, a `scripts/` line, a
/// section, a file and a cross-repository citation, on lines 4 to 9.
const DOMAIN: &str = "domain: d\n\
types:\n\
\x20 # A noun.\n\
\x20 # read from: crates/x/src/lib.rs:2\n\
\x20 # read from: crates/x/src/lib.rs:5-6\n\
\x20 # read from: scripts/tool:2\n\
\x20 # read from: docs/notes.md (§ Section one)\n\
\x20 # read from: docs/notes.md\n\
\x20 # read from: b10x-canon 0.6.0, crates/canon/src/model/authority.rs:24\n\
\x20 - name: d.A\n\
\x20   kind: newtype\n\
\x20   of: String\n";

/// One needle row per line citation of [`DOMAIN`].
const NEEDLES: &str = "# key<TAB>needles\n\
d.A|- name: d.A|crates/x/src/lib.rs|1\tpub fn alpha()\n\
d.A|- name: d.A|crates/x/src/lib.rs|2\tpub fn beta() && 42\n\
d.A|- name: d.A|scripts/tool|1\techo tool\n";

/// A tree whose every citation resolves.
fn tree(at: &Path) -> PathBuf {
    let repo = at.join("repo");
    write(&repo.join("crates/x/src/lib.rs"), LIB);
    write(&repo.join("scripts/tool"), "#!/bin/sh\necho tool\n");
    write(
        &repo.join("docs/notes.md"),
        "# Notes\n\n## Section one\n\nText.\n",
    );
    write(&repo.join("ess/domains/d.yaml"), DOMAIN);
    write(&repo.join("ess/domains/d.citations.tsv"), NEEDLES);
    repo
}

/// Runs the checker on `repo` in `mode`.
fn check(repo: &Path, mode: &str) -> Output {
    Command::new("python3")
        .arg("-I")
        .arg(SCRIPT)
        .arg("--repo")
        .arg(repo)
        .arg(mode)
        .output()
        .expect("python3 runs the checker")
}

fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Replaces `from` with `to` in the file at `path`, which must hold it once.
fn edit(path: &Path, from: &str, to: &str) {
    let text = read(path);
    assert_eq!(text.matches(from).count(), 1, "{from:?} once in {path:?}");
    fs::write(path, text.replacen(from, to, 1)).expect("an edited file");
}

#[test]
fn every_citation_resolving_exits_zero_and_counts_them() {
    let outer = scratch("resolving");
    let repo = tree(&outer.0);
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(0), "{}", shown(&output));
    assert!(
        stdout(&output).contains(
            "citations: 5 in-repository, 5 resolved, 0 unresolved; 1 external, not resolved \
             here; 0 unused needles"
        ),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_changed_cited_line_exits_non_zero_naming_the_citation_and_the_line() {
    let outer = scratch("changed");
    let repo = tree(&outer.0);
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "pub fn alpha()",
        "pub fn gamma()",
    );
    let output = check(&repo, "check");
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        text.contains("ess/domains/d.yaml:4: unresolved: crates/x/src/lib.rs:2"),
        "the citation is named: {}",
        shown(&output)
    );
    assert!(
        text.contains("2: pub fn gamma() -> u32 {"),
        "the cited line is shown: {}",
        shown(&output)
    );
    assert!(
        text.contains("4 resolved, 1 unresolved"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_range_whose_text_moved_down_is_unresolved() {
    let outer = scratch("moved");
    let repo = tree(&outer.0);
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "}\npub fn beta",
        "}\n\npub fn beta",
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:5: unresolved: crates/x/src/lib.rs:5-6"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_citation_into_scripts_is_in_the_repository_and_resolved() {
    let outer = scratch("scripts");
    let repo = tree(&outer.0);
    edit(&repo.join("scripts/tool"), "echo tool", "echo other");
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:6: unresolved: scripts/tool:2"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_line_citation_without_needles_is_unresolved() {
    let outer = scratch("no-needles");
    let repo = tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.citations.tsv"),
        "d.A|- name: d.A|scripts/tool|1\techo tool\n",
        "",
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:6: unresolved: scripts/tool:2"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_document_without_a_needles_table_has_every_line_citation_unresolved() {
    let outer = scratch("no-table");
    let repo = tree(&outer.0);
    fs::remove_file(repo.join("ess/domains/d.citations.tsv")).expect("the table is removed");
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("2 resolved, 3 unresolved"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_missing_section_or_file_is_unresolved() {
    let outer = scratch("section");
    let repo = tree(&outer.0);
    edit(
        &repo.join("docs/notes.md"),
        "## Section one",
        "## Section two",
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:7: unresolved: docs/notes.md (§ Section one)"),
        "{}",
        shown(&output)
    );
    fs::remove_file(repo.join("docs/notes.md")).expect("the file is removed");
    let output = check(&repo, "check");
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:8: unresolved: docs/notes.md\n"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_citation_in_no_known_form_is_unresolved_not_skipped() {
    let outer = scratch("unknown-form");
    let repo = tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "# read from: docs/notes.md\n",
        "# read from: docs/notes.md, line 3\n",
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:8: unresolved: docs/notes.md, line 3"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_needle_no_citation_uses_fails_the_check() {
    let outer = scratch("unused");
    let repo = tree(&outer.0);
    let table = repo.join("ess/domains/d.citations.tsv");
    write(
        &table,
        &format!(
            "{}d.B|- name: d.B|crates/x/src/lib.rs|1\tpub fn alpha()\n",
            read(&table)
        ),
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("unused needle: d.B|- name: d.B|crates/x/src/lib.rs|1"),
        "{}",
        shown(&output)
    );
}

#[test]
fn every_document_under_ess_is_checked() {
    let outer = scratch("documents");
    let repo = tree(&outer.0);
    write(
        &repo.join("ess/system.yaml"),
        "system: s\n# read from: crates/x/src/missing.rs\n",
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/system.yaml:2: unresolved: crates/x/src/missing.rs"),
        "{}",
        shown(&output)
    );
}

#[test]
fn fix_re_points_a_citation_whose_needles_occur_once() {
    let outer = scratch("fix");
    let repo = tree(&outer.0);
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n",
        "// a fixture\n// moved\n",
    );
    let output = check(&repo, "fix");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    let domain = read(&repo.join("ess/domains/d.yaml"));
    assert!(
        domain.contains("# read from: crates/x/src/lib.rs:3\n"),
        "{domain}"
    );
    assert!(
        domain.contains("# read from: crates/x/src/lib.rs:6-7\n"),
        "{domain}"
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(0), "{}", shown(&output));
}

#[test]
fn fix_keeps_the_length_of_a_range_its_needles_do_not_fill() {
    let outer = scratch("fix-length");
    let repo = tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "lib.rs:5-6\n",
        "lib.rs:5-7\n",
    );
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n",
        "// a fixture\n// moved\n",
    );
    let output = check(&repo, "fix");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    let domain = read(&repo.join("ess/domains/d.yaml"));
    assert!(
        domain.contains("# read from: crates/x/src/lib.rs:6-8\n"),
        "{domain}"
    );
}
