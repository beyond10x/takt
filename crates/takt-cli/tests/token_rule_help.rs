//! story:the-token-rule-holds-for-an-unchanged-binary: `takt promote --help` and `takt scorecard
//! --help` state the token rule a comparison applies, tokens per model request with its 5 %
//! tolerance, and no longer the saved maximum.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::process::Command;

/// `takt <args>`: its exit status and its stdout, its line breaks joined so a wrapped sentence
/// reads as one.
fn help(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    (
        output.status.code().unwrap_or(-1),
        text.split_whitespace().collect::<Vec<_>>().join(" "),
    )
}

#[test]
fn promote_and_scorecard_help_state_the_token_rule_and_its_tolerance() {
    for verb in ["promote", "scorecard"] {
        let (code, text) = help(&[verb, "--help"]);
        assert_eq!(code, 0, "{text}");
        assert!(
            text.contains(
                "its median over cases of tokens per model request exceeds the saved side's by more than 5 %"
            ),
            "takt {verb} --help states the token rule and its tolerance: {text}"
        );
        assert!(
            !text.contains("saved maximum"),
            "takt {verb} --help no longer states the old rule: {text}"
        );
    }
}
