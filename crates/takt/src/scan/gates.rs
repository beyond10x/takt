//! The `gates` scan adapter: `b10x-gates scan-text` under the policy `B10X_GATES_POLICY` names.
//! It reads a judgement step's proposal and the change so far a prompt shows ([`ChangeScan`],
//! `scan-text --repo <tree> --policy <policy> <file>`), and what a forge publishes before it
//! leaves the machine ([`outgoing_argv`], `scan-text --repository <repository> [--policy
//! <policy>] <path>...`). A policy that is not set, or that `b10x-gates` does not find
//! ([`NO_POLICY`]), scans nothing, which is never a pass. This file and the bot forge
//! (`crate::forge::bot`) are the only ones of takt's code that name the program and its policy.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The environment variable that names the gates policy file.
pub const POLICY_VAR: &str = "B10X_GATES_POLICY";

/// The program the gates adapter runs, found on the program path.
pub const PROGRAM: &str = "b10x-gates";

/// What `b10x-gates` 0.1.21 says, exiting 1, when it finds no policy file, the one `--policy`
/// names or its default: it then scanned nothing.
pub const NO_POLICY: &str = "unavailable: not found";

/// The policy [`POLICY_VAR`] names in the environment, when it is set (empty or not).
#[must_use]
pub fn policy_from_env() -> Option<PathBuf> {
    std::env::var_os(POLICY_VAR).map(PathBuf::from)
}

/// What the change a case has made so far passes before a judgement step's prompt shows it
/// ([`crate::harness::ChangeSoFar`]), and what a proposal passes before the edit effect:
/// `<program> scan-text --repo <tree> --policy <policy> <file>` ([`ChangeScan::args`]), the
/// bytes written to `<file>` outside the tree. A scan that refuses or cannot run (the program not
/// there, a policy that does not load, any exit but 0) shows nothing and passes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeScan {
    /// The `b10x-gates` program.
    pub program: PathBuf,
    /// The policy file.
    pub policy: PathBuf,
}

impl ChangeScan {
    /// The scan the environment names: [`PROGRAM`] under the policy [`POLICY_VAR`] names, when
    /// it is set (empty or not); none when it is not, and then the change so far is shown
    /// unscanned.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        policy_from_env().map(|policy| Self {
            program: PathBuf::from(PROGRAM),
            policy,
        })
    }

    /// The arguments after [`Self::program`] that scan `file` in `tree`: `scan-text --repo
    /// <tree> --policy <policy> <file>`.
    #[must_use]
    pub fn args(&self, tree: &Path, file: &Path) -> Vec<OsString> {
        vec![
            "scan-text".into(),
            "--repo".into(),
            tree.into(),
            "--policy".into(),
            self.policy.clone().into(),
            file.into(),
        ]
    }
}

/// The options every gates line that reads the policy carries, the outgoing scan's and the bot
/// route's API writes alike: `--repository <repository>`, and `--policy <policy>` when the
/// policy file is known; `b10x-gates` reads its own environment otherwise.
#[must_use]
pub fn scope(repository: &str, policy: Option<&Path>) -> Vec<String> {
    let mut options = vec!["--repository".to_owned(), repository.to_owned()];
    if let Some(policy) = policy {
        options.push("--policy".to_owned());
        options.push(policy.display().to_string());
    }
    options
}

/// The line that scans `paths` before they leave the machine: `b10x-gates scan-text` under
/// [`scope`], the paths last.
#[must_use]
pub fn outgoing_argv(repository: &str, policy: Option<&Path>, paths: &[&Path]) -> Vec<String> {
    let mut argv = vec![PROGRAM.to_owned(), "scan-text".to_owned()];
    argv.extend(scope(repository, policy));
    argv.extend(paths.iter().map(|path| path.display().to_string()));
    argv
}
