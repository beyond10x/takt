//! The help of `takt run` and of `takt scenario run` names, where it describes exit 3, the stop
//! `RepeatedGateFailure` with the bound the run keeps ([`takt::run::REPEATED_GATE_FAILURES`]).

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

#[test]
fn exit_3_names_the_repeated_gate_failure_and_its_bound() {
    let named = format!(
        "RepeatedGateFailure ({} gate.run results in a row, each at a new revision, failed the \
         same gates: the gates)",
        takt::run::REPEATED_GATE_FAILURES
    );
    for args in [&["run", "--help"][..], &["scenario", "run", "--help"][..]] {
        let (code, help) = takt(args);
        assert_eq!(code, 0, "{args:?}: {help}");
        let line = help
            .lines()
            .skip_while(|line| line.trim() != "Exit codes:")
            .find(|line| line.trim_start().starts_with("3 "))
            .unwrap_or_else(|| panic!("{args:?}: no exit 3 line\n{help}"));
        assert!(
            line.contains(&named),
            "{args:?}: exit 3 names {named}: {line}"
        );
    }
}
