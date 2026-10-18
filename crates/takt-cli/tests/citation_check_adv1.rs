//! Adversary cases for `scripts/check-citations`: a citation the checker passes although it no
//! longer holds what it is cited for, a `read from:` it skips without saying so, and a `fix` that
//! re-points a citation onto other code or rewrites lines it was not asked to touch.
//!
//! `CITATION_CHECK_SCRIPT` overrides the script under test, so a mutated copy can be run against
//! the same cases.

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
            eprintln!("remove the scratch tree {}: {error}", self.0.display());
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("citation-adv1-{}-{name}", std::process::id()));
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

/// The domain document: a line, a range, a `scripts/` line, a section, a file and a
/// cross-repository citation, on lines 4 to 9.
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

/// One needle row per in-repository citation of [`DOMAIN`].
const NEEDLES: &str = "# key<TAB>needles\n\
d.A|- name: d.A|crates/x/src/lib.rs|1\tpub fn alpha()\n\
d.A|- name: d.A|crates/x/src/lib.rs|2\tpub fn beta() && 42\n\
d.A|- name: d.A|scripts/tool|1\techo tool\n\
d.A|- name: d.A|docs/notes.md|1\tText.\n\
d.A|- name: d.A|docs/notes.md|2\t# Notes\n";

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
    let script = std::env::var("CITATION_CHECK_SCRIPT").unwrap_or_else(|_| SCRIPT.to_owned());
    Command::new("python3")
        .arg("-I")
        .arg(script)
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

/// The fixture resolves before each case changes it, so a red case is red for its change.
fn resolving_tree(at: &Path) -> PathBuf {
    let repo = tree(at);
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(0),
        "baseline: {}",
        shown(&output)
    );
    repo
}

#[test]
fn a_citation_in_an_inline_yaml_comment_is_checked_not_skipped() {
    let outer = scratch("inline");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "    kind: newtype\n",
        "    kind: newtype  # read from: crates/x/src/lib.rs:99\n",
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(1),
        "lib.rs has 7 lines, so the citation on line 11 cannot resolve: {}",
        shown(&output)
    );
}

#[test]
fn a_capitalised_read_from_marker_is_not_skipped() {
    let outer = scratch("capital");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "  # A noun.\n",
        "  # A noun.\n  # Read from: crates/x/src/missing.rs\n",
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(1),
        "a citation of a file that does not exist passes when its marker is capitalised: {}",
        shown(&output)
    );
}

#[test]
fn a_read_from_in_a_block_scalar_is_not_skipped() {
    let outer = scratch("scalar");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "    of: String\n",
        "    of: String\n    naming:\n      summary: >-\n        A noun, read from: crates/x/src/missing.rs\n",
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(1),
        "a `read from:` outside a comment is neither checked nor reported: {}",
        shown(&output)
    );
}

#[test]
fn needles_written_for_a_section_citation_are_checked() {
    let outer = scratch("section-needles");
    let repo = resolving_tree(&outer.0);
    let table = repo.join("ess/domains/d.citations.tsv");
    edit(
        &table,
        "d.A|- name: d.A|docs/notes.md|1\tText.\n",
        "d.A|- name: d.A|docs/notes.md|1\tno such text in the notes\n",
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(1),
        "a needle row for the section citation counts as used and is never matched: {}",
        shown(&output)
    );
}

#[test]
fn a_heading_renamed_to_a_longer_one_does_not_hold_the_cited_section() {
    let outer = scratch("heading");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("docs/notes.md"),
        "## Section one\n",
        "## Section one hundred\n",
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains("ess/domains/d.yaml:7: unresolved: docs/notes.md (§ Section one)"),
        "{}",
        shown(&output)
    );
}

/// `beta` on lines 5-9, its last needle on line 8; `gamma` follows on 10-12.
const LIB_LONG: &str = "// a fixture\npub fn alpha() -> u32 {\n    1\n}\n\
pub fn beta() -> u32 {\n    // the answer,\n    // as a constant\n    42\n}\n\
pub fn gamma() -> u32 {\n    7\n}\n";

#[test]
fn a_range_that_a_shorter_function_leaves_swallowing_the_next_one_is_unresolved() {
    let outer = scratch("swallow");
    let repo = tree(&outer.0);
    write(&repo.join("crates/x/src/lib.rs"), LIB_LONG);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "lib.rs:5-6\n",
        "lib.rs:5-9\n",
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(0),
        "baseline: {}",
        shown(&output)
    );
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "    // the answer,\n    // as a constant\n",
        "",
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(1),
        "lines 8-9 of the cited range are now `pub fn gamma` and its body: {}",
        shown(&output)
    );
}

#[test]
fn fix_does_not_re_point_a_citation_onto_another_function() {
    let outer = scratch("fix-other");
    let repo = resolving_tree(&outer.0);
    write(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n// moved\npub fn alpha() -> u32 {\n    1\n}\n\
         pub fn beta() -> u32 {\n    7\n}\npub fn gamma() -> u32 {\n    42\n}\n",
    );
    let output = check(&repo, "fix");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    let domain = read(&repo.join("ess/domains/d.yaml"));
    assert!(
        !domain.contains("# read from: crates/x/src/lib.rs:6-10\n"),
        "beta no longer returns 42; the citation now spans gamma: {domain}"
    );
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(1),
        "the stale beta citation is still reported after fix: {}",
        shown(&output)
    );
}

#[test]
fn fix_changes_only_the_citation_lines_of_a_crlf_document() {
    let outer = scratch("fix-crlf");
    let repo = tree(&outer.0);
    let crlf = DOMAIN.replace('\n', "\r\n");
    write(&repo.join("ess/domains/d.yaml"), &crlf);
    let output = check(&repo, "check");
    assert_eq!(
        output.status.code(),
        Some(0),
        "baseline: {}",
        shown(&output)
    );
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n",
        "// a fixture\n// moved\n",
    );
    let output = check(&repo, "fix");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    let expected = crlf
        .replace("lib.rs:2\r\n", "lib.rs:3\r\n")
        .replace("lib.rs:5-6\r\n", "lib.rs:6-7\r\n");
    assert_eq!(
        read(&repo.join("ess/domains/d.yaml")),
        expected,
        "fix rewrote lines other than the two citations"
    );
}

#[test]
fn a_cited_file_that_is_not_utf8_fails_with_a_message_not_a_traceback() {
    let outer = scratch("utf8");
    let repo = resolving_tree(&outer.0);
    let mut bytes = b"// a fixture \xff\n".to_vec();
    bytes.extend_from_slice(LIB.trim_start_matches("// a fixture\n").as_bytes());
    fs::write(repo.join("crates/x/src/lib.rs"), bytes).expect("a file");
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("Traceback"),
        "{}",
        shown(&output)
    );
    assert!(
        stdout(&output).contains("crates/x/src/lib.rs"),
        "the unreadable file is named: {}",
        shown(&output)
    );
}

#[test]
fn a_second_needle_row_for_a_key_fails_the_check() {
    let outer = scratch("duplicate");
    let repo = resolving_tree(&outer.0);
    let table = repo.join("ess/domains/d.citations.tsv");
    write(
        &table,
        &format!(
            "{}d.A|- name: d.A|crates/x/src/lib.rs|1\tpub fn beta()\n",
            read(&table)
        ),
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(
        stdout(&output).contains(
            "ess/domains/d.citations.tsv:7: a second row for d.A|- name: d.A|crates/x/src/lib.rs|1"
        ),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_needle_row_without_a_tab_or_with_an_empty_needle_fails_the_check() {
    let outer = scratch("malformed");
    let repo = resolving_tree(&outer.0);
    let table = repo.join("ess/domains/d.citations.tsv");
    write(
        &table,
        &format!(
            "{}d.B|- name: d.B|crates/x/src/lib.rs|1 pub fn alpha()\nd.C|x|y|1\tpub fn alpha() && \n",
            read(&table)
        ),
    );
    let output = check(&repo, "check");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    let text = stdout(&output);
    assert!(
        text.contains("ess/domains/d.citations.tsv:7: malformed needle row"),
        "{}",
        shown(&output)
    );
    assert!(
        text.contains("ess/domains/d.citations.tsv:8: malformed needle row"),
        "{}",
        shown(&output)
    );
}
