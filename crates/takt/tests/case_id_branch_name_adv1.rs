//! Adversary pass 1 on the case id rule: ids `check_id` accepts and git still refuses as the
//! branch `takt/<id>`, a refusal that names an id other than the one given, and two accepted
//! ids that display as one.

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

/// A scratch repository under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("case-id-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn protocol() -> LoadedProtocol {
    LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads")
}

/// Whether git creates the branch `takt/<id>` in `tree`, asked of `git branch` itself (not
/// `check-ref-format`, which knows nothing of the file the ref is stored in); the branch is
/// deleted again when it was made.
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

/// An id of 251 to 255 bytes passes `check_id`, and git refuses `takt/<id>`: its loose ref's
/// lock file `<id>.lock` is past the 255-byte file name limit. `takt run --case-id` then fails
/// inside `git checkout -b` with git's "File name too long", the very shape the story removes.
/// Measured with ASCII and with a two-byte letter (126 x `ü`, 252 bytes, 126 characters), so a
/// fix that counts characters instead of bytes stays red.
#[test]
fn check_id_accepts_an_id_too_long_for_git_to_lock_its_branch_ref() {
    let tree = repository("long");
    let mut missed = Vec::new();
    for id in ["a".repeat(251), "a".repeat(255), "\u{fc}".repeat(126)] {
        if git_creates_branch(&tree.0, &id) {
            continue;
        }
        let opened = CaseDir::open(&tree.0, protocol(), &id, "probe", &[]);
        let refusal = match opened {
            Ok(_) => "opened".to_owned(),
            Err(error) => format!("{error:#}"),
        };
        if check_id(&id).is_ok() || !refusal.contains("the case id") {
            missed.push(format!(
                "{} bytes: check_id {:?}; open: {}",
                id.len(),
                check_id(&id).is_ok(),
                refusal.chars().take(160).collect::<String>()
            ));
        }
        assert!(
            !tree.0.join(CASES_DIR).exists(),
            "{} bytes: a case directory was written",
            id.len()
        );
    }
    assert!(
        missed.is_empty(),
        "ids git refuses as takt/<id> pass check_id and fail inside git:\n{}",
        missed.join("\n")
    );
}

/// The refusal names the id it was given: an id holding `'` or `"` and a rule-breaking
/// character is named with a backslash git and the operator never typed (`it\'s~`), while the
/// same message's `takt/<id>` shows it raw.
#[test]
fn check_id_refusal_names_an_id_with_a_quote_as_a_different_id() {
    for id in ["it's~", "a\"b~"] {
        let refusal = format!("{:#}", check_id(id).expect_err(id));
        assert!(
            refusal.contains(&format!("the case id `{id}`")),
            "{id}: the refusal names another id: {refusal}"
        );
    }
}

/// `check_id`'s doc says no accepted id "holds a character that does not show in a message or a
/// `takt/<id>` branch". `ab` with an invisible combining grapheme joiner (U+034F) between the
/// letters displays as `ab`: the pair is two case directories and two branches that read as
/// one, so one of them is refused. The doc also says the check does not normalise: `café`
/// composed (NFC) and decomposed (NFD) both pass, as two ids.
#[test]
fn check_id_accepts_two_ids_that_display_as_one() {
    let mut both = Vec::new();
    let (a, b) = ("ab", "a\u{34f}b");
    if check_id(a).is_ok() && check_id(b).is_ok() {
        both.push(format!("{} / {}", a.escape_unicode(), b.escape_unicode()));
    }
    assert!(
        both.is_empty(),
        "both ids of each pair pass and display alike: {both:?}"
    );
    let (nfc, nfd) = ("caf\u{e9}", "cafe\u{301}");
    assert!(
        check_id(nfc).is_ok() && check_id(nfd).is_ok() && nfc != nfd,
        "the NFC/NFD pair is documented as two ids"
    );
}
