//! story:the-default-confinement-is-named-as-the-default: the `case` line of a run started
//! without `--confinement` says `none` is the default and prints no flag; one started with
//! `--confinement none` names the flag as given.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::process::Command;

/// The `case` line's text from `; confinement ` on, without the tree's path before it, of a
/// run with `extra` flags.
fn confinement_segment(name: &str, extra: &[&str]) -> String {
    let outer = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("confinement-origin-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("seed"), "seed").unwrap();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["add", "seed"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "seed",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(&repo)
        .args([
            "--intent",
            "probe",
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .args(extra)
        .env("XDG_STATE_HOME", outer.join("state"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("case "))
        .unwrap_or_else(|| {
            panic!(
                "no case line: {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    let at = line
        .rfind("; confinement ")
        .unwrap_or_else(|| panic!("no confinement segment: {line}"));
    let segment = line[at..].to_owned();
    let _ = fs::remove_dir_all(outer);
    segment
}

#[test]
fn the_default_confinement_is_named_as_the_default() {
    let segment = confinement_segment("plain", &[]);
    assert!(segment.starts_with("; confinement none"), "{segment}");
    assert!(
        segment.contains("the default confinement none"),
        "{segment}"
    );
    assert!(!segment.contains("--confinement"), "{segment}");
}

#[test]
fn an_explicit_confinement_is_named_as_given() {
    let segment = confinement_segment("flagged", &["--confinement", "none"]);
    assert!(segment.contains("--confinement none"), "{segment}");
    assert!(!segment.contains("default"), "{segment}");
}
