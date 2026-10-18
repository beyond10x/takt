//! A case id takt accepts is a valid git branch name: `check_id` refuses every id whose branch
//! `takt/<id>` git would not create, with takt's own message naming the id and the rule, before
//! a case directory or a branch is made; and a case directory already there under such an id is
//! not resumed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::LoadedProtocol;
use takt::case::{CASES_DIR, CaseDir, check_id};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

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

/// A scratch directory under this crate's build directory, removed on drop.
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

/// An empty directory under this crate's build directory.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("case-id-branch-name-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A repository with one commit on `main`.
fn repository(name: &str) -> Scratch {
    let dir = directory(name);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn protocol() -> LoadedProtocol {
    LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads")
}

/// Whether git accepts `takt/<id>` as a branch's ref, asked of git itself.
fn git_accepts(dir: &Path, id: &str) -> bool {
    Command::new("git")
        .args(["check-ref-format", &format!("refs/heads/takt/{id}")])
        .current_dir(dir)
        .output()
        .expect("git runs")
        .status
        .success()
}

/// The story's examples, each with the rule it breaks as the refusal names it.
const STORY_EXAMPLES: &[(&str, &str)] = &[
    ("a~b", "`~`"),
    ("foo.", "ends with `.`"),
    ("a..b", "`..`"),
    ("a@{b", "`@{`"),
    ("x.lock", "ends with `.lock`"),
];

/// `takt run --case-id` with each of the story's examples is refused by `check_id` with a
/// message that names the id and the rule, and `CaseDir::open` makes neither the case
/// directory nor the branch and leaves the tree on the branch it was on.
#[test]
fn the_story_examples_are_refused_naming_the_id_and_the_rule_with_nothing_written() {
    let tree = repository("examples");
    for &(id, rule) in STORY_EXAMPLES {
        assert!(!git_accepts(&tree, id), "{id}: git accepts takt/{id}");
        let refusal = format!("{:#}", check_id(id).expect_err(id));
        assert!(refusal.contains("case id"), "{id}: {refusal}");
        assert!(refusal.contains(&format!("`{id}`")), "{id}: {refusal}");
        assert!(refusal.contains(rule), "{id}: {rule} not named: {refusal}");

        let opened = CaseDir::open(&tree, protocol(), id, "probe", &[]);
        let refusal = format!("{:#}", opened.expect_err(id));
        assert!(refusal.contains("case id"), "{id}: {refusal}");
        assert!(refusal.contains(rule), "{id}: {rule} not named: {refusal}");
        assert!(!tree.join(CASES_DIR).join(id).exists(), "{id}: a case dir");
        assert!(
            git(&tree, &["branch", "--list", "takt/*"]).is_empty(),
            "{id}: a branch was made"
        );
        assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
    assert!(
        !tree.join(CASES_DIR).exists(),
        "the cases directory was made"
    );
}

/// Every id in a corpus of ASCII characters at the start, the middle and the end of a name,
/// in pairs of the characters git treats specially, and with the `.lock` suffix, that git
/// refuses as `takt/<id>`, `check_id` refuses too: git is the oracle, takt's check is its own.
#[test]
fn every_id_git_refuses_as_a_case_branch_check_id_refuses() {
    let dir = directory("oracle");
    let mut corpus: Vec<String> = Vec::new();
    for point in 1u8..=127 {
        let c = char::from(point);
        corpus.extend([
            format!("{c}"),
            format!("{c}a"),
            format!("a{c}"),
            format!("a{c}b"),
        ]);
    }
    let special = [
        '.', '@', '{', '}', '~', '^', ':', '?', '*', '[', ']', '\\', '/', '-', 'l',
    ];
    for a in special {
        for b in special {
            corpus.extend([
                format!("{a}{b}"),
                format!("x{a}{b}"),
                format!("{a}{b}x"),
                format!("x{a}{b}y"),
            ]);
        }
    }
    corpus.extend(
        [
            ".lock", "a.lock", "a.lock.b", "a.locked", "lock", "a.LOCK", "@", "@@", "@{", "a@{",
            "@{-1}", "a.", "a..", ".a", "a/.b", "a//b", "a b", "a\u{7f}b",
        ]
        .map(str::to_owned),
    );
    let mut missed = Vec::new();
    let mut refused = 0;
    for id in &corpus {
        if git_accepts(&dir, id) {
            continue;
        }
        refused += 1;
        match check_id(id) {
            Ok(()) => missed.push(id.escape_debug().to_string()),
            Err(error) => {
                let error = format!("{error:#}");
                assert!(error.contains("case id"), "{}: {error}", id.escape_debug());
            }
        }
    }
    assert!(
        refused > 100,
        "git refused only {refused} ids of the corpus"
    );
    assert!(
        missed.is_empty(),
        "check_id accepts ids git refuses as takt/<id>: {missed:?}"
    );
}

/// The ids takt mints and ids git accepts that carry `.`, `@`, `{` or `-` still pass.
#[test]
fn ids_git_accepts_still_pass() {
    let dir = directory("accepted");
    for id in [
        "real",
        "story-1-20261018T000000Z",
        "takt-20261018T000000Z",
        "scenario-1-trial-0",
        "a.b",
        "v1.2.3",
        "a@b",
        "a{b}",
        "a.locked",
        "lock",
        "-rf",
        "fix-\u{fc}ber",
    ] {
        assert!(git_accepts(&dir, id), "{id}: git refuses takt/{id}");
        check_id(id).unwrap_or_else(|error| panic!("{id}: {error:#}"));
    }
}

/// A case directory already under an id the rule refuses is not resumed: its branch could never
/// have been made, and the refusal is the one `check_id` gives, with nothing written.
#[test]
fn a_case_directory_under_a_refused_id_is_not_resumed() {
    let tree = repository("resume");
    let case = CaseDir::open(&tree, protocol(), "real", "probe", &[]).expect("the case opens");
    git(&tree, &["checkout", "-q", "main"]);
    for &(id, rule) in STORY_EXAMPLES {
        let dir = tree.join(CASES_DIR).join(id);
        fs::create_dir_all(&dir).expect("a directory");
        for file in ["case.yaml", "protocol.yaml", "branch.yaml"] {
            fs::copy(case.dir.join(file), dir.join(file)).expect("a copy");
        }
        let expected = format!("{:#}", check_id(id).expect_err(id));
        let refusal = format!("{:#}", CaseDir::resume(&tree, id).expect_err(id));
        assert_eq!(refusal, expected, "{id}");
        assert!(refusal.contains(rule), "{id}: {refusal}");
        assert!(!dir.join("run.lock").exists(), "{id}: the case was held");
        assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}

/// Whether `git branch` makes `takt/<id>` in `tree`; the branch is deleted again when it was.
fn git_creates_branch(tree: &Path, id: &str) -> bool {
    let branch = format!("takt/{id}");
    let made = Command::new("git")
        .args(["branch", "-q", &branch])
        .current_dir(tree)
        .output()
        .expect("git runs")
        .status
        .success();
    if made {
        git(tree, &["branch", "-q", "-D", &branch]);
    }
    made
}

/// An id is at most 250 bytes, counted in bytes, not characters: git keeps `takt/<id>` as the
/// file `<id>` and locks it as `<id>.lock`, which a 255-byte file name limit holds only that
/// far. The refusal names the limit; an id at the limit opens its case.
#[test]
fn an_id_longer_than_250_bytes_is_refused_naming_the_limit() {
    let tree = repository("length");
    for id in ["a".repeat(250), "\u{fc}".repeat(125)] {
        assert_eq!(id.len(), 250);
        check_id(&id).unwrap_or_else(|error| panic!("{} bytes: {error:#}", id.len()));
        assert!(git_creates_branch(&tree, &id), "git refuses 250 bytes");
    }
    for id in ["a".repeat(251), "\u{fc}".repeat(126), "b".repeat(300)] {
        let refusal = format!("{:#}", check_id(&id).expect_err("too long"));
        assert!(refusal.contains("case id"), "{refusal}");
        assert!(
            refusal.contains(&format!("{} bytes", id.len())) && refusal.contains("250"),
            "the length and the limit are not named: {refusal}"
        );
        let refusal = format!(
            "{:#}",
            CaseDir::open(&tree, protocol(), &id, "probe", &[]).expect_err("too long")
        );
        assert!(refusal.contains("250"), "{refusal}");
        assert!(!tree.join(CASES_DIR).exists(), "a case directory was made");
        assert!(git(&tree, &["branch", "--list", "takt/*"]).is_empty());
    }
}

/// The refusal names the id as it was typed: a quote or a backslash shows as itself, and only
/// a character that does not show is written as its code point.
#[test]
fn the_refusal_names_the_id_as_typed() {
    for (id, shown) in [
        ("it's~", "it's~"),
        ("a\"b~", "a\"b~"),
        ("x\\y", "x\\y"),
        ("it's\u{200b}", "it's\\u{200b}"),
        ("a b", "a\\u{20}b"),
    ] {
        let refusal = format!("{:#}", check_id(id).expect_err(id));
        assert!(
            refusal.contains(&format!("the case id `{shown}` is refused")),
            "{}: {refusal}",
            id.escape_debug()
        );
    }
}

/// Every default-ignorable code point (Unicode 16.0, `Default_Ignorable_Code_Point` of
/// DerivedCoreProperties.txt) is refused, named by its code point: each is invisible, so an id
/// holding one reads as the id without it (`a` U+034F `b` as `ab`).
#[test]
fn every_default_ignorable_code_point_is_refused_naming_it() {
    let ranges: &[(u32, u32)] = &[
        (0x00AD, 0x00AD),
        (0x034F, 0x034F),
        (0x061C, 0x061C),
        (0x115F, 0x1160),
        (0x17B4, 0x17B5),
        (0x180B, 0x180F),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x206F),
        (0x3164, 0x3164),
        (0xFE00, 0xFE0F),
        (0xFEFF, 0xFEFF),
        (0xFFA0, 0xFFA0),
        (0xFFF0, 0xFFF8),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0000, 0xE0FFF),
    ];
    let mut missed = Vec::new();
    for &(first, last) in ranges {
        for point in first..=last {
            let c = char::from_u32(point).expect("a character");
            let id = format!("a{c}b");
            match check_id(&id) {
                Ok(()) => missed.push(format!("U+{point:04X}")),
                Err(error) => {
                    let error = format!("{error:#}");
                    assert!(error.contains(&format!("U+{point:04X}")), "{error}");
                }
            }
        }
    }
    assert!(missed.is_empty(), "accepted: {missed:?}");
}

/// The check does not normalise: `café` precomposed (NFC) and decomposed (NFD) both pass, and
/// they are two ids, two case directories and two branches.
#[test]
fn ids_differing_only_in_normalisation_are_two_ids() {
    let (nfc, nfd) = ("caf\u{e9}", "cafe\u{301}");
    check_id(nfc).unwrap_or_else(|error| panic!("{error:#}"));
    check_id(nfd).unwrap_or_else(|error| panic!("{error:#}"));
    assert_ne!(nfc, nfd);
}
