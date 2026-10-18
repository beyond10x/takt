//! Adversary pass 2 on `check_id` after correction 1: the shown form of a refused id, the
//! branch name `current` repeats, the lone `@` guard, and the ids takt mints itself.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::Story;
use takt::case::{check_id, current};

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
        .join(format!("case-id-adv2-{}-{name}", std::process::id()));
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

/// `shown` writes an invisible character as `\u{...}` and a backslash as itself, so two ids
/// typed differently can be shown alike: `a\` U+200B and `a\\u{200b}` (nine characters) both
/// come back as "the case id `a\\u{200b}` is refused: it contains a path separator", and the
/// operator cannot tell from the refusal which id it names. Decided: the message is for a
/// person and both ids are refused, so the case holds the refusal of both.
#[test]
fn two_different_ids_get_one_and_the_same_refusal() {
    let invisible = "a\\\u{200b}";
    let typed = "a\\\\u{200b}";
    assert_ne!(invisible, typed);
    let first = format!("{:#}", check_id(invisible).expect_err("a backslash"));
    let second = format!("{:#}", check_id(typed).expect_err("a backslash"));
    assert!(
        first.contains("is refused") && second.contains("is refused"),
        "both ids are refused: {} / {}",
        first.escape_debug(),
        second.escape_debug()
    );
}

/// git accepts U+202E RIGHT-TO-LEFT OVERRIDE in a branch name; `check_id` refuses it and its
/// own refusal writes it as `\u{202e}`, but `current` wraps that refusal in "the branch
/// {branch} names no case" with the branch name as git gave it, so the override reaches the
/// terminal and reverses the rest of the line.
#[test]
fn current_writes_a_bidi_override_from_the_branch_name_raw() {
    let tree = repository("bidi");
    git(&tree.0, &["checkout", "-q", "-b", "takt/evil\u{202e}kcab"]);
    let error = format!("{:#}", current(&tree.0).expect_err("refused"));
    assert!(error.contains("\\u{202e}"), "{}", error.escape_debug());
    assert!(
        !error.contains('\u{202e}'),
        "U+202E written raw: {}",
        error.escape_debug()
    );
}

/// `branch_refusal` refuses `@` alone, which git accepts as `takt/@`, so the 1,600-id corpus
/// (git as the oracle) never reaches the guard, and no other case names `@` alone: the
/// guard can be dropped with the suite green. This case holds it.
#[test]
fn dropping_the_lone_at_refusal_leaves_the_suite_green() {
    let refusal = format!("{:#}", check_id("@").expect_err("`@` alone"));
    assert!(refusal.contains("the case id `@` is refused"), "{refusal}");
    assert!(refusal.contains("HEAD"), "{refusal}");
}

/// The case id takt mints for a story, `story-<name>-<instant>`, is 23 bytes longer than the
/// story's name, so a name of 228 bytes or more makes an id `check_id` refuses for its length:
/// the run takes no case for that story, whatever the operator gives. The unit's message says
/// the ids takt builds cannot fail the check. Decided: story-minted ids stay as they are, and
/// such an id is refused by `check_id`, naming the limit, before anything is written; the case
/// holds that refusal.
#[test]
fn a_case_id_takt_mints_for_a_story_can_fail_check_id() {
    let story = Story {
        id: format!("story:{}", "a".repeat(228)),
        title: "a long name".to_owned(),
        body: String::new(),
    };
    let id = story.case_id("2026-10-18T12:00:00Z");
    assert_eq!(id.len(), 251);
    let refusal = format!("{:#}", check_id(&id).expect_err("251 bytes"));
    assert!(
        refusal.contains("251 bytes") && refusal.contains("250"),
        "{refusal}"
    );
}
