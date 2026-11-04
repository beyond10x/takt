//! Adversary cases, wave 52 unit a (story:a-relative-path-entry-is-no-test-program-directory):
//! the `PATH` `support/own_home.rs` leaves a test binary, for the `PATH`s a caller may give it.
//! Each case starts this binary again under a `PATH` it builds, without the mark `own_home`
//! leaves, so `own_home` rewrites that `PATH` before `main`, and reads what it left.

#[path = "support/own_home.rs"]
mod own_home;

use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::process::Command;

/// Set in a child [`path_left_for`] starts: [`child_prints_the_path_own_home_left`] prints.
const CHILD: &str = "W52_ADVERSARY_PRINT_PATH";

/// Not a case: in a child [`path_left_for`] starts, prints the `PATH` `own_home` left, between
/// markers; in any other process it does nothing.
#[test]
fn child_prints_the_path_own_home_left() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    match std::env::var_os("PATH") {
        Some(path) => println!("W52PATH<{}>", String::from_utf8_lossy(path.as_bytes())),
        None => println!("W52PATH-UNSET"),
    }
}

/// What a child of this binary, started under `PATH=inherited`, was left: `Ok(Some(path))`,
/// `Ok(None)` when `PATH` was unset, `Err(stderr)` when the child refused to run.
fn path_left_for(inherited: &str) -> Result<Option<String>, String> {
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "child_prints_the_path_own_home_left",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env("PATH", inherited)
        .env_remove("TAKT_TEST_OWN_HOME")
        .env(CHILD, "1")
        .output()
        .expect("the test binary starts");
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        return Err(format!(
            "{:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if out.contains("W52PATH-UNSET") {
        return Ok(None);
    }
    let start = out.find("W52PATH<").expect("the child printed its PATH") + "W52PATH<".len();
    let end = start + out[start..].find(">\n").expect("the marker's end");
    Ok(Some(out[start..end].to_owned()))
}

/// Mixed `PATH`s: empty entries (`::`, a leading and a trailing `:`), `.`, `./bin`, `..` and a
/// bare relative name beside an absolute directory this user cannot write. What is left is that
/// directory alone, every relative and empty entry dropped.
#[test]
fn relative_and_empty_entries_beside_an_absolute_one_are_all_dropped() {
    for inherited in [
        "::/usr/bin",
        ":/usr/bin",
        "/usr/bin:",
        "/usr/bin::",
        ".:/usr/bin",
        "./bin:/usr/bin",
        "..:/usr/bin",
        "bin:/usr/bin",
        "/usr/bin:.",
        ".:..::./bin:/usr/bin:bin:",
    ] {
        let left = path_left_for(inherited);
        assert_eq!(
            left,
            Ok(Some("/usr/bin".to_owned())),
            "PATH={inherited:?} left {left:?}"
        );
    }
}

/// A `PATH` whose every entry is relative: dropping them all leaves `PATH` set to the empty
/// string, and an empty `PATH` is one empty entry, which `execvp(3)` and the step guard
/// (`read_command` in `crates/takt/src/run.rs`: "an empty one is where the command runs, its
/// root") both read as the working directory, the case tree: the defect the story removes,
/// back under another spelling. Either `PATH` is left unset or holding only absolute
/// entries, or the binary refuses to run and says why (the story's other outcome).
#[test]
fn a_path_of_only_relative_entries_leaves_no_entry_that_reads_as_the_working_directory() {
    for inherited in [".", "./bin:..", "bin"] {
        let left = path_left_for(inherited);
        let holds_the_working_directory = match &left {
            Ok(Some(path)) => std::env::split_paths(path).any(|dir| !dir.is_absolute()),
            Ok(None) | Err(_) => false,
        };
        assert!(
            !holds_the_working_directory,
            "PATH={inherited:?} left {left:?}, entries {:?}: an empty PATH is the working directory",
            left.clone()
                .ok()
                .flatten()
                .map(|path| std::env::split_paths(&path).collect::<Vec<PathBuf>>())
        );
    }
}
