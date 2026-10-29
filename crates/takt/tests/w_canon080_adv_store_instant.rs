//! Adversary, canon 0.8.0 wave: takt now stamps every evidence record to the microsecond
//! (`Record::new`, `instant_micros`), and an accepted case records its `test_result` pass record
//! on the story through `aep plan artifact evidence --at <the record's own observed_at>`
//! (`Run::record_accepted`, `Aep::record_test_result`). The store must take that instant, or the
//! story never holds its `test_result` and the move to `implemented` is refused.
//!
//! The control case shows the same store, fixture and verb take the whole-second instant takt
//! wrote before this change, so a red first case is the instant's form and nothing else.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::evidence::Record;
use takt::frontier::{Aep, Frontier, Written};

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story the fixture store holds ready.
const STORY: &str = "story:ready-one";

fn git(tree: &Path, args: &[&str]) {
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
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// A scratch repository on `main` holding a committed copy of the `one-ready` store.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "w-canon080-adv-store-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A pass record as takt's tests validator writes one now: stamped by `Record::new`.
fn pass_record() -> Record {
    Record::new(
        "test-result-1",
        "test_result",
        Some("pass".to_owned()),
        "implementation",
        "aaaaaaaa11111111111111111111111111111111",
        "takt.tests-run",
    )
}

/// The store takes a `test_result` at the `observed_at` takt now writes on every record.
#[test]
fn the_store_takes_a_test_result_at_the_instant_takt_stamps_records_with() {
    let repo = repository("micro");
    let at = pass_record().observed_at;
    assert_eq!(at.len(), 27, "takt stamps to the microsecond: {at}");
    let written = Aep::default()
        .record_test_result(&repo.0, STORY, "evidence/test-result-1.json", &at)
        .expect("aep runs");
    assert_eq!(
        written,
        Written::Done,
        "the store refused the record's own observed_at {at}"
    );
}

/// Control: the same store, fixture and verb take the whole-second instant takt wrote before.
#[test]
fn control_the_store_takes_a_whole_second_instant() {
    let repo = repository("whole");
    let at = takt::now();
    let written = Aep::default()
        .record_test_result(&repo.0, STORY, "evidence/test-result-1.json", &at)
        .expect("aep runs");
    assert_eq!(written, Written::Done, "{at}");
}
