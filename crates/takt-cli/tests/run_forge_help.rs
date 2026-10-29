//! story:forge-none-help-names-the-unperformable-stop: what `takt run --help` says `--forge none`
//! does when only a forge action is left. The run does not leave `pr.open`, `ci.read` or
//! `repository.merge` to the harness: with no performer for the deciding action it stops
//! `Unperformable`, exit 7, and the case stays open.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::process::Command;

/// `takt <args>`: its exit status and its stdout.
fn takt(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// The `--forge` entry of a help text: its header line and every line indented deeper than it,
/// up to the next option, on one line with every run of white space as one space.
fn forge_entry(help: &str) -> String {
    let lines: Vec<&str> = help.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|line| line.trim_start().starts_with("--forge "))
    else {
        return String::new();
    };
    let indent = |line: &str| line.len() - line.trim_start().len();
    let header = indent(lines[start]);
    let entry = std::iter::once(lines[start]).chain(
        lines[start + 1..]
            .iter()
            .copied()
            .take_while(|line| line.trim().is_empty() || indent(line) > header),
    );
    entry
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn run_help_says_forge_none_stops_unperformable_exit_7() {
    let (code, help) = takt(&["run", "--help"]);
    assert_eq!(code, 0, "{help}");
    let entry = forge_entry(&help);
    assert!(
        !entry.is_empty(),
        "takt run --help has a --forge entry: {help}"
    );
    assert!(
        entry.contains("`none`") && entry.contains("Unperformable") && entry.contains("exit 7"),
        "the --forge entry names, for `none`, the Unperformable stop and exit 7: {entry}"
    );
    let flat = help.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        !flat.contains("left to the harness"),
        "takt run --help no longer says forge actions are left to the harness: {entry}"
    );
}
