//! `scripts/release-notes`, run as the release workflow and `task check` run it
//! (`python3 -I scripts/release-notes ...`) against fixture changelogs and the repository's own.
//! `<version>` prints the GitHub release notes of that section: its summary, a `**Breaking**` list
//! of every `**Breaking:**` bullet (or the line `Breaking: none.`), a `**Changed**` list of the
//! non-breaking bullets of `### Changed`, each at most 8 entries and then one overflow line, and
//! the link to the changelog at the version's tag; a missing, undated or empty section exits 1.
//! `--check` validates the whole file against Keep a Changelog 1.1.0 as `CHANGELOG.md` keeps it
//! and exits 1 listing every violation with its line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The script under test.
const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/release-notes");

/// takt's repository: the root of this workspace.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

const HEADER: &str = "# Changelog\n\nThis file follows Keep a Changelog 1.1.0.\n";

const LINKS: &str = "\
[Unreleased]: https://github.com/beyond10x/takt/compare/1.0.0...HEAD
[1.0.0]: https://github.com/beyond10x/takt/compare/0.9.0...1.0.0
[0.9.0]: https://github.com/beyond10x/takt/releases/tag/0.9.0
";

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("release-notes-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

/// A changelog of an empty `[Unreleased]`, the section `1.0.0` holding `body` and a section
/// `0.9.0` with one entry, with their link references.
fn changelog(body: &str) -> String {
    format!(
        "{HEADER}\n## [Unreleased]\n\n## [1.0.0] - 2026-10-21\n\n{body}\n\
         ## [0.9.0] - 2026-10-20\n\n### Added\n\n- The first entry.\n\n{LINKS}"
    )
}

/// `python3 -I scripts/release-notes <args>` in `cwd`.
fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new("python3")
        .arg("-I")
        .arg(SCRIPT)
        .args(args)
        .current_dir(cwd)
        .env_remove("PYTHONPATH")
        .output()
        .expect("python3 runs")
}

/// `release-notes <args> --changelog <file>` over a fixture holding `text`.
fn over(name: &str, text: &str, args: &[&str]) -> Output {
    let dir = scratch(name);
    let file = dir.0.join("CHANGELOG.md");
    fs::write(&file, text).expect("the fixture is written");
    let file = file.to_str().expect("a UTF-8 path").to_owned();
    let mut all = args.to_vec();
    all.extend(["--changelog", &file]);
    run(&dir.0, &all)
}

fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\n--- stdout\n{}--- stderr\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 output")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 output")
}

fn bullets(prefix: &str, what: &str, range: std::ops::RangeInclusive<u32>) -> String {
    range.map(|n| format!("- {prefix}{what} {n}.\n")).collect()
}

const FULL: &str = "Full changelog: https://github.com/beyond10x/takt/blob/1.0.0/CHANGELOG.md\n";

#[test]
fn nine_breaking_and_nine_changed_entries_print_eight_of_each_and_name_the_rest() {
    let mut changed = String::new();
    for n in 1..=9 {
        changed.push_str(&format!("- Changed entry {n}.\n"));
        if n <= 5 {
            changed.push_str(&format!("- **Breaking:** Breaking entry {n}.\n"));
        }
    }
    let body = format!(
        "The first line of the summary.\nThe second line of the summary.\n\n\
         ### Added\n\n- An added entry.\n\n### Changed\n\n{changed}\n\
         ### Removed\n\n{}\n### Fixed\n\n- A fixed entry.\n",
        bullets("**Breaking:** ", "Breaking entry", 6..=9)
    );
    let text = changelog(&body);

    let checked = over("overflow-check", &text, &["--check"]);
    assert_eq!(checked.status.code(), Some(0), "{}", shown(&checked));

    let notes = over("overflow", &text, &["1.0.0"]);
    assert_eq!(notes.status.code(), Some(0), "{}", shown(&notes));
    let expected = format!(
        "The first line of the summary.\nThe second line of the summary.\n\n\
         **Breaking**\n\n{}\nand 1 more in the changelog\n\n\
         **Changed**\n\n{}\nand 1 more in the changelog\n\n{FULL}",
        bullets("", "Breaking entry", 1..=8),
        bullets("", "Changed entry", 1..=8),
    );
    assert_eq!(stdout(&notes), expected, "{}", shown(&notes));
}

#[test]
fn a_section_without_a_breaking_entry_says_so_and_leaves_out_an_empty_changed_list() {
    let body = "### Added\n\n- An added entry.\n\n### Fixed\n\n- A fixed entry.\n";
    let notes = over("none", &changelog(body), &["1.0.0"]);
    assert_eq!(notes.status.code(), Some(0), "{}", shown(&notes));
    assert_eq!(stdout(&notes), format!("Breaking: none.\n\n{FULL}"));

    let body = "One line of summary.\n\n### Changed\n\n- A change.\n- Another change.\n";
    let notes = over("none-changed", &changelog(body), &["1.0.0"]);
    assert_eq!(notes.status.code(), Some(0), "{}", shown(&notes));
    assert_eq!(
        stdout(&notes),
        format!(
            "One line of summary.\n\nBreaking: none.\n\n\
             **Changed**\n\n- A change.\n- Another change.\n\n{FULL}"
        )
    );
}

#[test]
fn a_three_line_summary_is_refused_naming_its_line() {
    let body = "Line one.\nLine two.\nLine three.\n\n### Added\n\n- An entry.\n";
    let checked = over("summary", &changelog(body), &["--check"]);
    assert_eq!(checked.status.code(), Some(1), "{}", shown(&checked));
    // The summary starts on line 9: the header takes 3 lines, a blank, `[Unreleased]`, a blank,
    // the release heading and a blank.
    assert!(
        stderr(&checked).contains("CHANGELOG.md:9: "),
        "{}",
        shown(&checked)
    );
    assert!(stderr(&checked).contains("summary"), "{}", shown(&checked));
}

#[test]
fn a_wrapped_bullet_is_refused_naming_its_continuation_line() {
    let body = "### Added\n\n- An entry that goes on\n  onto a second line.\n- An entry.\n";
    let checked = over("wrapped", &changelog(body), &["--check"]);
    assert_eq!(checked.status.code(), Some(1), "{}", shown(&checked));
    assert!(
        stderr(&checked).contains("CHANGELOG.md:12: "),
        "{}",
        shown(&checked)
    );
}

#[test]
fn every_violation_is_listed_with_its_line() {
    let long = "x".repeat(200);
    let body = format!(
        "### Changed\n\n- A change.\n\n### Added\n\n- An entry {long}.\n\n\
         ### Known limits\n\n- A limit.\n\n#### Deeper\n\n- A change with **Breaking:** inside.\n"
    );
    let text = changelog(&body).replace("[0.9.0]: https", "[0.8.0]: https");
    let checked = over("many", &text, &["--check"]);
    assert_eq!(checked.status.code(), Some(1), "{}", shown(&checked));
    let err = stderr(&checked);
    for (line, word) in [
        (13, "order"),
        (15, "characters"),
        (17, "Known limits"),
        (21, "####"),
        (23, "**Breaking:**"),
        (25, "0.9.0"),
        (33, "0.8.0"),
    ] {
        let needle = format!("CHANGELOG.md:{line}: ");
        let hit = err.lines().any(|l| l.contains(&needle) && l.contains(word));
        assert!(hit, "line {line} naming {word}:\n{}", shown(&checked));
    }
}

/// A misspelt mark would list a breaking entry as a plain change, so `--check` names each one
/// with its line, and `<version>` prints no notes for the section.
#[test]
fn a_misspelt_breaking_mark_is_refused_naming_its_line() {
    let marks = [
        "**Breaking**: ",
        "Breaking: ",
        "**breaking:** ",
        "*Breaking:* ",
        "**BREAKING:** ",
        "__Breaking:__ ",
        "**Breaking** ",
        "Breaking change: ",
    ];
    let mut body = String::from("### Changed\n\n- **Breaking:** A well-marked entry.\n");
    for mark in marks {
        body.push_str(&format!("- {mark}An entry.\n"));
    }
    body.push_str("- A plain entry that names breaking changes later on.\n");
    let text = changelog(&body);

    let checked = over("misspelt", &text, &["--check"]);
    assert_eq!(checked.status.code(), Some(1), "{}", shown(&checked));
    let err = stderr(&checked);
    // `### Changed` is line 9, the well-marked entry line 11, the misspelt ones lines 12 to 19.
    for (offset, mark) in marks.iter().enumerate() {
        let needle = format!("CHANGELOG.md:{}: ", 12 + offset);
        let hit = err
            .lines()
            .any(|l| l.contains(&needle) && l.contains("**Breaking:**"));
        assert!(
            hit,
            "{mark:?} on line {}:\n{}",
            12 + offset,
            shown(&checked)
        );
    }
    for line in [11, 20] {
        let needle = format!("CHANGELOG.md:{line}: ");
        assert!(
            !err.contains(&needle),
            "line {line} is well formed:\n{}",
            shown(&checked)
        );
    }

    let notes = over("misspelt-notes", &text, &["1.0.0"]);
    assert_eq!(notes.status.code(), Some(1), "{}", shown(&notes));
    assert!(stdout(&notes).is_empty(), "{}", shown(&notes));
}

#[test]
fn a_heading_out_of_order_with_an_em_dash_or_a_bad_date_is_refused() {
    let text = format!(
        "{HEADER}\n## [0.9.0] - 2026-10-20\n\n### Added\n\n- An entry.\n\n\
         ## [1.0.0] — 2026-10-21\n\n### Added\n\n- An entry.\n\n\
         ## [Unreleased]\n\n## [0.8.0] - 2026-13-01\n\n### Added\n\n- An entry.\n\n{LINKS}"
    );
    let checked = over("headings", &text, &["--check"]);
    assert_eq!(checked.status.code(), Some(1), "{}", shown(&checked));
    let err = stderr(&checked);
    for line in [11, 17, 19] {
        assert!(
            err.contains(&format!("CHANGELOG.md:{line}: ")),
            "line {line}:\n{}",
            shown(&checked)
        );
    }
}

#[test]
fn a_missing_an_undated_or_an_empty_section_exits_1_naming_why() {
    let text = changelog("### Added\n\n- An entry.\n");
    let missing = over("missing", &text, &["2.0.0"]);
    assert_eq!(missing.status.code(), Some(1), "{}", shown(&missing));
    assert!(stdout(&missing).is_empty(), "{}", shown(&missing));
    assert!(
        stderr(&missing).contains("2.0.0") && stderr(&missing).contains("no section"),
        "{}",
        shown(&missing)
    );

    let undated = over(
        "undated",
        &text.replace("## [1.0.0] - 2026-10-21", "## [1.0.0]"),
        &["1.0.0"],
    );
    assert_eq!(undated.status.code(), Some(1), "{}", shown(&undated));
    assert!(stdout(&undated).is_empty(), "{}", shown(&undated));
    assert!(stderr(&undated).contains("no date"), "{}", shown(&undated));

    let empty = over("empty", &changelog(""), &["1.0.0"]);
    assert_eq!(empty.status.code(), Some(1), "{}", shown(&empty));
    assert!(stdout(&empty).is_empty(), "{}", shown(&empty));
    assert!(stderr(&empty).contains("no entries"), "{}", shown(&empty));
}

#[test]
fn the_repositorys_changelog_passes_the_check() {
    let root = Path::new(ROOT).canonicalize().expect("the repository root");
    let checked = run(&root, &["--check"]);
    assert_eq!(checked.status.code(), Some(0), "{}", shown(&checked));
}
