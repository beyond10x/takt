//! Adversary cases, pass 2, for `scripts/check-citations` after its first correction: a citation
//! that hides behind the prose exception, a section whose bounds the checker misreads, a range-end
//! rule that refuses a citation that holds, and a `fix` that rewrites the wrong text of a line.
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
        .join(format!("citation-adv2-{}-{name}", std::process::id()));
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

/// The cited source: `alpha` on lines 2-4, `beta` on lines 5-7.
const LIB: &str =
    "// a fixture\npub fn alpha() -> u32 {\n    1\n}\npub fn beta() -> u32 {\n    42\n}\n";

/// The domain document: a line, a range, a section, a file and a cross-repository citation.
const DOMAIN: &str = "domain: d\n\
types:\n\
\x20 # A noun.\n\
\x20 # read from: crates/x/src/lib.rs:2\n\
\x20 # read from: crates/x/src/lib.rs:5-6\n\
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
d.A|- name: d.A|docs/notes.md|1\tText.\n\
d.A|- name: d.A|docs/notes.md|2\t# Notes\n";

const NOTES: &str = "# Notes\n\n## Section one\n\nText.\n";

fn tree(at: &Path) -> PathBuf {
    let repo = at.join("repo");
    write(&repo.join("crates/x/src/lib.rs"), LIB);
    write(&repo.join("docs/notes.md"), NOTES);
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
    assert_exit(&check(&repo, "check"), 0, "baseline");
    repo
}

fn assert_exit(output: &Output, code: i32, why: &str) {
    assert_eq!(output.status.code(), Some(code), "{why}: {}", shown(output));
}

// 1. The prose exception.

#[test]
fn a_line_citation_after_read_from_the_is_not_silently_skipped() {
    let outer = scratch("prose-line");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "  # A noun.\n",
        "  # A noun, read from the crates/x/src/missing.rs:12\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "a citation of a missing file behind `read from the ` is neither checked nor named",
    );
}

#[test]
fn a_range_citation_after_read_from_the_file_is_not_silently_skipped() {
    let outer = scratch("prose-range");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "  # A noun.\n",
        "  # A noun, read from the file crates/x/src/missing.rs:12-14\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "a range citation of a missing file behind `read from the file` passes",
    );
}

#[test]
fn a_read_from_wrapped_across_two_comment_lines_is_not_silently_skipped() {
    let outer = scratch("wrapped");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "  # A noun.\n",
        "  # A noun, its value is read\n  # from: crates/x/src/missing.rs:99\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "a citation wrapped between `read` and `from:` is never seen",
    );
}

#[test]
fn a_word_ending_in_read_before_from_is_not_a_marker() {
    let outer = scratch("thread");
    let repo = resolving_tree(&outer.0);
    edit(
        &repo.join("ess/domains/d.yaml"),
        "  # A noun.\n",
        "  # A noun; a step hands its thread from one turn to the next.\n",
    );
    edit(
        &repo.join("ess/domains/d.yaml"),
        "    of: String\n",
        "    of: String\n    naming:\n      summary: a worker thread from the pool\n",
    );
    assert_exit(
        &check(&repo, "check"),
        0,
        "`thread from` holds no `read from` marker, and no citation was added",
    );
}

// 2. Section bounds.

#[test]
fn a_shell_comment_in_a_nested_fence_does_not_end_the_section() {
    let outer = scratch("nested-fence");
    let repo = tree(&outer.0);
    write(
        &repo.join("docs/notes.md"),
        "# Notes\n\n## Section one\n\nAn example:\n\n````md\n```sh\n# install it\nrun\n```\n````\n\nText.\n\n## Section two\n\nOther.\n",
    );
    assert_exit(
        &check(&repo, "check"),
        0,
        "`# install it` sits inside a four-backtick fence, so `Text.` is still in Section one",
    );
}

#[test]
fn a_tilde_fence_holding_a_backtick_line_does_not_hide_the_next_heading() {
    let outer = scratch("tilde-fence");
    let repo = tree(&outer.0);
    write(
        &repo.join("docs/notes.md"),
        "# Notes\n\n## Section one\n\nOpen a block with:\n\n~~~\n```\n~~~\n\n## Section two\n\nText.\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "`Text.` is under Section two; the ~~~ fence closed before it",
    );
}

#[test]
fn a_setext_heading_ends_the_section_above_it() {
    let outer = scratch("setext");
    let repo = tree(&outer.0);
    write(
        &repo.join("docs/notes.md"),
        "# Notes\n\n## Section one\n\nIntro.\n\nSection two\n-----------\n\nText.\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "`Text.` is under the setext heading Section two, not Section one",
    );
}

#[test]
fn a_heading_text_two_sections_share_does_not_resolve_by_the_other_one() {
    let outer = scratch("twice");
    let repo = tree(&outer.0);
    write(
        &repo.join("docs/notes.md"),
        "# Notes\n\n## Section one\n\nRewritten.\n\n# Appendix\n\n## Section one\n\nText.\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "two sections carry the cited heading; the citation cannot say which, and resolves by the other",
    );
}

#[test]
fn a_section_whose_heading_is_the_last_line_holds_only_that_line() {
    let outer = scratch("eof-heading");
    let repo = tree(&outer.0);
    write(
        &repo.join("docs/notes.md"),
        "# Notes\n\nText.\n\n## Section one\n",
    );
    assert_exit(
        &check(&repo, "check"),
        1,
        "`Text.` precedes the cited heading",
    );
}

// 3. The range-end rule.

#[test]
fn a_range_whose_last_needle_also_occurs_earlier_still_resolves() {
    let outer = scratch("last-needle");
    let repo = tree(&outer.0);
    write(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\npub fn alpha() -> u32 {\n    1\n}\n\
         pub fn beta() -> Result<(), E> {\n    if done() {\n        return Ok(());\n    }\n\
         \x20   work()?;\n    Ok(())\n}\n",
    );
    edit(
        &repo.join("ess/domains/d.yaml"),
        "lib.rs:5-6\n",
        "lib.rs:5-11\n",
    );
    edit(
        &repo.join("ess/domains/d.citations.tsv"),
        "pub fn beta() && 42\n",
        "pub fn beta() && Ok(())\n",
    );
    assert_exit(
        &check(&repo, "check"),
        0,
        "lines 5-11 are beta whole, its first needle on 5 and its last on 10",
    );
}

// 4. fix.

#[test]
fn fix_rewrites_the_citation_not_the_same_text_earlier_on_its_line() {
    let outer = scratch("fix-prefix");
    let repo = tree(&outer.0);
    let domain = repo.join("ess/domains/d.yaml");
    edit(
        &domain,
        "  # read from: crates/x/src/lib.rs:2\n",
        "  # unlike crates/x/src/lib.rs:20, read from: crates/x/src/lib.rs:2\n",
    );
    assert_exit(&check(&repo, "check"), 0, "baseline");
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n",
        "// a fixture\n// moved\n",
    );
    assert_exit(&check(&repo, "fix"), 1, "fix reports what it re-pointed");
    let text = read(&domain);
    assert!(
        text.contains("  # unlike crates/x/src/lib.rs:20, read from: crates/x/src/lib.rs:3\n"),
        "fix rewrote the prose before the citation and left the citation stale: {text}"
    );
}

#[test]
fn fix_twice_changes_nothing_the_second_time() {
    let outer = scratch("fix-twice");
    let repo = tree(&outer.0);
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n",
        "// a fixture\n// moved\n",
    );
    assert_exit(&check(&repo, "fix"), 1, "the first fix re-points");
    let once = read(&repo.join("ess/domains/d.yaml"));
    assert_exit(&check(&repo, "fix"), 0, "the second fix has nothing to do");
    assert_eq!(read(&repo.join("ess/domains/d.yaml")), once);
}

#[test]
fn fix_refuses_a_span_longer_than_the_cited_range() {
    let outer = scratch("fix-longer");
    let repo = tree(&outer.0);
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "// a fixture\n",
        "// a fixture\n// moved\n",
    );
    edit(
        &repo.join("crates/x/src/lib.rs"),
        "    42\n",
        "    // grown\n    42\n",
    );
    let output = check(&repo, "fix");
    assert_exit(&output, 1, "beta is unresolved");
    assert!(
        stdout(&output)
            .contains("not re-pointed: the needles span more lines than the cited range"),
        "{}",
        shown(&output)
    );
    assert!(
        read(&repo.join("ess/domains/d.yaml")).contains("# read from: crates/x/src/lib.rs:5-6\n")
    );
}

// 5. Not UTF-8.

#[test]
fn a_document_that_is_not_utf8_fails_naming_it() {
    let outer = scratch("doc-utf8");
    let repo = resolving_tree(&outer.0);
    let mut bytes = DOMAIN.as_bytes().to_vec();
    bytes.extend_from_slice(b"# \xff\n");
    fs::write(repo.join("ess/domains/d.yaml"), bytes).expect("a file");
    let output = check(&repo, "check");
    assert_exit(&output, 1, "an unreadable document");
    assert!(
        stdout(&output).contains("ess/domains/d.yaml: not UTF-8"),
        "{}",
        shown(&output)
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Traceback"));
}

#[test]
fn a_needles_table_that_is_not_utf8_fails_naming_it_and_resolves_nothing() {
    let outer = scratch("tsv-utf8");
    let repo = resolving_tree(&outer.0);
    let mut bytes = NEEDLES.as_bytes().to_vec();
    bytes.extend_from_slice(b"# \xff\n");
    fs::write(repo.join("ess/domains/d.citations.tsv"), bytes).expect("a file");
    let output = check(&repo, "check");
    assert_exit(&output, 1, "an unreadable table");
    let text = stdout(&output);
    assert!(
        text.contains("ess/domains/d.citations.tsv: not UTF-8"),
        "{}",
        shown(&output)
    );
    assert!(
        text.contains(" 0 resolved, 4 unresolved"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_cited_file_that_is_not_utf8_is_named_with_the_reason() {
    let outer = scratch("cited-utf8");
    let repo = resolving_tree(&outer.0);
    let mut bytes = b"// a fixture \xff\n".to_vec();
    bytes.extend_from_slice(LIB.trim_start_matches("// a fixture\n").as_bytes());
    fs::write(repo.join("crates/x/src/lib.rs"), bytes).expect("a file");
    let output = check(&repo, "check");
    assert_exit(&output, 1, "an unreadable cited file");
    assert!(
        stdout(&output)
            .lines()
            .any(|l| l.contains("not UTF-8") && l.contains("crates/x/src/lib.rs")),
        "{}",
        shown(&output)
    );
}
