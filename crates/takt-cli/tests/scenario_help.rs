//! What `takt scenario run --help` says about its exit codes: the codes that verb gives, and not
//! `takt run`'s `frontier: nothing ready`, which a scenario run never prints (it refuses a story
//! that is not ready at the seed with exit 1, before anything is pushed).

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

/// The help's exit-code list: the lines after `Exit codes:`.
fn exit_codes(help: &str) -> Vec<&str> {
    help.lines()
        .skip_while(|line| line.trim() != "Exit codes:")
        .skip(1)
        .collect()
}

#[test]
fn scenario_run_help_lists_its_own_exit_codes_without_the_frontier() {
    let (code, help) = takt(&["scenario", "run", "--help"]);
    assert_eq!(code, 0, "{help}");
    let codes = exit_codes(&help);
    assert!(
        !codes.is_empty(),
        "takt scenario run --help lists exit codes: {help}"
    );
    assert!(
        !help.contains("frontier: nothing ready"),
        "a scenario run never prints `frontier: nothing ready`: {help}"
    );
    for stopped in [4, 6] {
        assert!(
            codes
                .iter()
                .any(|line| line.trim_start().starts_with(&format!("{stopped} "))
                    && line.contains("`takt run --case <case>` resumes the case")),
            "a scenario run resumes nothing; exit {stopped} names `takt run --case`: {help}"
        );
    }
    for listed in 0..=6 {
        assert!(
            codes
                .iter()
                .any(|line| line.trim_start().starts_with(&format!("{listed} "))),
            "takt scenario run --help documents exit {listed}: {help}"
        );
    }
}

#[test]
fn run_help_still_lists_frontier_nothing_ready_under_exit_0() {
    let (code, help) = takt(&["run", "--help"]);
    assert_eq!(code, 0, "{help}");
    assert!(
        exit_codes(&help)
            .iter()
            .any(|line| line.trim_start().starts_with("0 ")
                && line.contains("frontier: nothing ready")),
        "takt run --help keeps `frontier: nothing ready` under exit 0: {help}"
    );
}
