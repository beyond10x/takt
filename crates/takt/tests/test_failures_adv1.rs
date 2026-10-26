//! Adversary pass on story:repeated-identical-test-failures-stop-the-run and
//! story:a-red-base-stops-at-open: the failed tests a `tests.run` summary names, read back by
//! `failures_in`, for the failing set of the trial the stories come from.
//!
//! The second kind-cluster trial ran `cargo test --quiet --locked -p b10x-takt --lib`; three of
//! its failing tests are named in story:staging-tests-pass-under-a-shared-temp-directory. Their
//! names, with the `; failed: ` list and the rerun hint, are longer than the summary's bound, so
//! the list is cut and names no set: neither stop could name, or compare, the trial's tests.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::validator::{TestsRun, Validator, failures_in};

/// The three failing tests the trial's story names, as cargo names them.
const TRIAL: [&str; 3] = [
    "harness::endpoint_tests::staging_writes_a_protected_file_under_the_first_passing_candidate_and_drop_removes_it",
    "harness::endpoint_tests::staging_under_a_linked_ancestor_uses_the_resolved_path",
    "harness::endpoint_tests::a_staged_token_reaches_the_session_as_its_token_file_and_is_recorded_by_name",
];

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// What `cargo test --quiet` prints for a library whose tests `names` fail: no `test <name> ...
/// FAILED` lines, the names under the last `failures:` heading, the result line, and cargo's
/// rerun hint on standard error.
fn quiet_cargo_script(names: &[&str]) -> String {
    let mut script = String::from("#!/bin/sh\nprintf '\\nrunning 130 tests\\n");
    script.push_str("....F....F.....F\\n\\nfailures:\\n\\n");
    for name in names {
        script.push_str(&format!(
            "---- {name} stdout ----\\nthread panicked: refused\\n\\n"
        ));
    }
    script.push_str("\\nfailures:\\n");
    for name in names {
        script.push_str(&format!("    {name}\\n"));
    }
    script.push_str(&format!(
        "\\ntest result: FAILED. {} passed; {} failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s\\n\\n'\n",
        130 - names.len(),
        names.len()
    ));
    script.push_str(
        "printf 'error: test failed, to rerun pass `-p b10x-takt --lib`\\n' >&2\nexit 101\n",
    );
    script
}

/// The trial's failing set, run through the test validator: the summary names every one of the
/// three tests, so the run can compare the set (`RepeatedTestFailure`) and name it (`RedBase`).
#[test]
fn the_trials_failing_set_is_named_by_the_summary() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("test-failures-adv1-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    let _scratch = Scratch(dir.clone());
    let script = dir.join("t.sh");
    fs::write(&script, quiet_cargo_script(&TRIAL)).expect("the script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "seed"]);
    let tests = TestsRun::new("./t.sh").expect("a test command");
    let observation = tests
        .observe(&dir, "0123456789abcdef0123456789abcdef01234567", 1)
        .expect("the command is observed");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("fail"),
        "{}",
        observation.summary
    );
    let named = failures_in(&observation.summary);
    assert_eq!(
        named.tests,
        TRIAL
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
        "the summary ({} characters) names no set: {}",
        observation.summary.chars().count(),
        observation.summary
    );
}
