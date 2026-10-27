//! Security review of story:the-gitlab-forge-can-call-the-api-with-a-named-credential.
//!
//! The contract (`ess/domains/takt.yaml`, `takt.work.CredentialSource`): the value of a
//! `credential: {env: NAME}` is held in takt's memory only, and the test command and the forge's
//! programs run with no credential but the ones their adapter reads (none for the test command).
//! These cases resolve an `{env: NAME}` credential the way a run opens one and then start the
//! children a run starts, checking that the value does not reach them. The variable's name is one
//! an operator may choose freely; the run file does not restrict it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use takt::confinement::{TestRun, unconfined};
use takt::forge::credential::CredentialSource;
use takt::forge::gitlab::Glab;
use takt::forge::{Limits, Process, Runner};
use takt::scan::Scan;

const NAME: &str = "SEC_B2_FORGE_PAT";
const TOKEN: &str = "tokenvalue-for-the-env-credential";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("sec-b2-env-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree")).expect("a scratch tree");
        Self(dir)
    }

    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn repository(&self) {
        for args in [
            vec!["init", "-q"],
            vec![
                "remote",
                "add",
                "origin",
                "https://gitlab.example.com/group/sub/repo.git",
            ],
        ] {
            let status = Command::new("git")
                .arg("-C")
                .arg(self.tree())
                .args(args)
                .status()
                .expect("git runs");
            assert!(status.success());
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn opened(scratch: &Scratch) -> Glab {
    scratch.repository();
    // SAFETY: this test binary sets the variable once, before any thread of its own reads the
    // environment; it stands for the operator's shell exporting it before takt starts.
    unsafe { std::env::set_var(NAME, TOKEN) };
    Glab::for_tree_api_through(
        &scratch.tree(),
        &Scan::None {
            declared_in: scratch.0.join("run.yaml"),
        },
        &CredentialSource::Env(NAME.to_owned()),
        Box::new(Process::default()),
    )
    .expect("the env credential resolves")
}

fn printenv() -> Vec<String> {
    vec!["printenv".to_owned(), NAME.to_owned()]
}

/// The test command (code the harness may have written) runs with no credential: the value of
/// the forge's `{env: NAME}` credential does not reach it.
#[test]
fn sec_b2_an_env_credential_does_not_reach_the_test_command() {
    let scratch = Scratch::new("test-command");
    let _forge = opened(&scratch);
    let argv = printenv();
    let finished = unconfined(&TestRun {
        argv: &argv,
        tree: &scratch.tree(),
        limits: Limits {
            timeout: Duration::from_secs(20),
            output: 1 << 16,
        },
    })
    .expect("the test command runs");
    let seen = String::from_utf8_lossy(&finished.stdout);
    assert!(
        !seen.contains(TOKEN),
        "the test command read the forge credential from its environment: {NAME}={seen}"
    );
}

/// A forge program other than the push (the scan, a forge command) reads no credential of the
/// `api` transport: the value of `{env: NAME}` does not reach it.
#[test]
fn sec_b2_an_env_credential_does_not_reach_a_forge_program() {
    let scratch = Scratch::new("forge-program");
    let _forge = opened(&scratch);
    let exit = Process::default()
        .run(&scratch.tree(), &printenv())
        .expect("the program runs");
    assert!(
        !exit.stdout.contains(TOKEN),
        "a forge program read the forge credential from its environment: {NAME}={}",
        exit.stdout
    );
}
