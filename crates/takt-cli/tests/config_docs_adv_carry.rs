//! Adversary, carry onto the integration branch: `website/docs/reference/configuration.md`
//! describes "every takt run flag grouped by purpose" (its front matter); the carried
//! story:run-state-directory-is-configurable added `--state-dir`, which no table of the page
//! names. The case reads the flags from `takt run --help` and looks each up on the page.

use std::path::Path;
use std::process::Command;

/// Every long flag `takt run --help` lists, `--help` aside.
fn run_flags() -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    assert!(output.status.success(), "takt run --help");
    let help = String::from_utf8(output.stdout).expect("utf-8");
    let mut flags: Vec<String> = help
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let line = line
                .strip_prefix("-h, ")
                .or_else(|| line.strip_prefix("-V, "))
                .unwrap_or(line);
            let flag = line.strip_prefix("--")?;
            let name: String = flag
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            (!name.is_empty() && name != "help").then(|| format!("--{name}"))
        })
        .collect();
    flags.sort();
    flags.dedup();
    flags
}

/// Each flag of `takt run --help` is named on the configuration reference, in backticks.
#[test]
fn every_takt_run_flag_is_on_the_configuration_reference() {
    let page =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/docs/reference/configuration.md");
    let text = std::fs::read_to_string(&page).expect("the configuration reference");
    let flags = run_flags();
    assert!(
        flags.iter().any(|flag| flag == "--iterations") && flags.len() >= 20,
        "the flags read from the help: {flags:?}"
    );
    let missing: Vec<&String> = flags
        .iter()
        .filter(|flag| !text.contains(&format!("`{flag}`")) && !text.contains(&format!("`{flag} ")))
        .collect();
    assert!(
        missing.is_empty(),
        "website/docs/reference/configuration.md, which lists every takt run flag, names none of {missing:?}"
    );
}
