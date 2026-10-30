//! Adversarial cases on story:harness-none-takes-a-granted-action. Its outcome: "`--harness none`
//! can finish a case after a grant: resumed with a granted action admissible, it takes that action
//! instead of ending the run on an error." With a forge, the forge's merge validator takes the
//! granted merge and the harness is never asked (the unit's own acceptance case says so), so the
//! harness's new branch is reached only where no validator takes the merge: `--forge none`.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "harness-none-grant-adv-frd-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(dir.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        dir.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The story's outcome where its change is reached: a case granted `repository.merge` at its
/// revision, resumed with `--harness none --forge none`, takes the granted merge. Nothing
/// performs it yet: on `--forge none` no validator merges, so the run stops `NoProgress`, exit 3,
/// the case open. Finishing it is story:forge-none-merges-locally's outcome; this case pins
/// today's stop and flips to exit 0 and `Completed` when that story lands.
#[test]
fn today_a_granted_case_under_forge_none_stops_after_harness_none_takes_the_merge() {
    let repo = repository("forge-none");
    let path = repo.to_str().expect("utf-8");
    let base = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
    ];
    let mut first = base.to_vec();
    first.extend(["--iterations", "0", "--protocol", "software.change/1"]);
    let (code, out, err) = takt(&first);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();
    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);
    let (code, out, err) = takt(&base);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "{out}"
    );
    let (code, out, err) = takt(&[
        "approve",
        "--repo",
        path,
        &id,
        "repository.merge",
        "--by",
        "human:operator",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let (code, out, err) = takt(&base);
    assert!(
        out.contains("no harness: the action the case is granted is taken"),
        "the change is reached here: stdout {out} stderr {err}"
    );
    assert_eq!(
        code, 3,
        "story:forge-none-merges-locally: on --forge none the granted merge the harness takes is performed by nothing yet, so the run stops NoProgress; when that story lands this is exit 0: stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: NoProgress"),
        "story:forge-none-merges-locally: the run stops NoProgress until a local merge exists: {out}"
    );
}
