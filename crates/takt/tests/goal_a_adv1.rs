//! Adversarial cases against the cluster-delivery wave (`4ec3ae24..4def3240`): the run file's
//! `tokens` (story:a-run-stops-at-a-default-token-budget). Each case states what it expects of
//! the code and why; nothing leaves the machine.

use std::fs;
use std::path::{Path, PathBuf};

/// A scratch directory of this case's own under the crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("goal-a-adv1-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
        }
        fs::create_dir_all(&dir).expect("scratch");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// --- U1: the run file's `tokens` -------------------------------------------------------------

/// story:a-run-stops-at-a-default-token-budget, Acceptance 5, and the run file's own contract
/// (`--config` in `takt run --help`: "`tokens` (a whole number above zero; another value exits
/// 2)"): a `tokens` key written with no value, YAML's null, is another value. It is refused as
/// `tokens: 0` is, never read as "not given", which runs the default of 3,000,000 the file's
/// author did not write.
#[test]
fn a_run_file_tokens_with_no_value_is_refused_as_tokens_0_is() {
    let scratch = Scratch::new("tokens-null");
    for (name, text) in [
        ("empty.yaml", "iterations: 3\ntokens:\n"),
        ("tilde.yaml", "iterations: 3\ntokens: ~\n"),
        ("null.yaml", "iterations: 3\ntokens: null\n"),
    ] {
        let path = scratch.0.join(name);
        fs::write(&path, text).expect("a run file");
        match takt::run_config::RunConfig::read(&path) {
            Ok(config) => panic!(
                "{name}: `tokens` with no value is read, tokens {:?}, so the run takes the default budget instead of exiting 2",
                config.tokens
            ),
            Err(error) => assert!(
                error
                    .downcast_ref::<takt::run_config::TokensRefused>()
                    .is_some(),
                "{name}: refused, but not as a `tokens` that is no whole number above zero (exit 1, not 2): {error:#}"
            ),
        }
    }
}

// story:takt-carries-no-organisations-own-route: the preflight cases on the gates policy and the
// bot forge's pull-request read are removed with them.
