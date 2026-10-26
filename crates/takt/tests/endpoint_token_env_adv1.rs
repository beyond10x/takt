//! Adversarial cases for story:endpoint-credential-from-environment, library side. The unit's
//! own staging tests stage only under directories the test user owns, so nothing exercises the
//! rule's root-owned sticky exception, which the `/dev/shm` and temporary-directory fallbacks
//! rest on: a staging rule that dropped it would leave the suite green and refuse both.

use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;

use takt::harness::{StagedToken, VariableValue};

const MARKER: &str = "MARKER-adv1-lib-3b8d20-not-a-real-key";

/// Acceptance 3: `/dev/shm` is a candidate, and it is root's and sticky (mode 1777), which
/// b10x-harness's reader admits; a value is staged under it, in a private directory, and
/// removed when the guard is dropped.
#[test]
fn a_root_owned_sticky_candidate_is_accepted() {
    let shm = PathBuf::from("/dev/shm");
    let Ok(meta) = std::fs::symlink_metadata(&shm) else {
        eprintln!("no /dev/shm on this host; nothing to check");
        return;
    };
    assert!(
        meta.is_dir() && meta.uid() == 0 && meta.mode() & 0o1000 != 0,
        "/dev/shm is root's and sticky on this host: mode {:o} uid {}",
        meta.mode(),
        meta.uid()
    );
    let value = VariableValue::new(MARKER.into());
    let staged = StagedToken::stage("ADV1_KEY", Some(&value), std::slice::from_ref(&shm))
        .expect("a root-owned sticky candidate passes the reader's rule");
    assert_eq!(staged.dir().parent(), Some(shm.as_path()));
    let dir_meta = std::fs::symlink_metadata(staged.dir()).expect("the directory");
    assert_eq!(dir_meta.mode() & 0o7777, 0o700);
    let file_meta = std::fs::symlink_metadata(staged.file()).expect("the file");
    assert_eq!(file_meta.mode() & 0o7777, 0o600);
    assert_eq!(
        std::fs::read_to_string(staged.file()).expect("the token"),
        MARKER
    );
    let dir = staged.dir().to_path_buf();
    drop(staged);
    assert!(!dir.exists(), "the staged directory is gone");
}
