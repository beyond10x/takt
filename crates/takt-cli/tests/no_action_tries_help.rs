//! story:a-harness-step-without-an-action-is-retried: the help of `takt run` and of `takt scenario
//! run` states, where it describes exit 5, how many times a judgement step whose session ends
//! without the line naming its action is tried before the run ends on the error, the bound the
//! run keeps ([`takt::run::NO_ACTION_TRIES`]).

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
fn exit_5_names_the_bound_on_tries_of_a_step_without_an_action() {
    let bound = format!("is tried {} times", takt::run::NO_ACTION_TRIES);
    for args in [&["run", "--help"][..], &["scenario", "run", "--help"][..]] {
        let (code, help) = takt(args);
        assert_eq!(code, 0, "{args:?}: {help}");
        let line = help
            .lines()
            .skip_while(|line| line.trim() != "Exit codes:")
            .find(|line| line.trim_start().starts_with("5 "))
            .unwrap_or_else(|| panic!("{args:?}: no exit 5 line\n{help}"));
        assert!(
            line.contains("without the line naming its action") && line.contains(&bound),
            "{args:?}: exit 5 names the bound ({bound}): {line}"
        );
    }
}
