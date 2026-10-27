//! Security review of story:confinement-through-substrate-on-linux-and-macos, the Linux scoped
//! workspace: a tree whose `target` is a symlink into its own `.git` (left there by whatever
//! wrote the tree before the run) is refused, and the tree's `.git` holds afterwards what it held
//! before, though takt makes its scratch directory under `target` on the host before Substrate
//! checks the subtree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::confinement::{self, Confinement as _, Substrate, TestRun};
use takt::validator::TestsRun;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "security-substrate-hop-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("a directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// `target -> .git`: the run is refused, by `Substrate::open` or by the run, and `.git` and its
/// `refs/heads` hold the entries they held before.
#[test]
fn security_substrate_hop_a_target_symlink_into_git_is_refused_and_leaves_git_alone() {
    if std::env::consts::OS != "linux" {
        return;
    }
    let outer = scratch("target-symlink");
    let repo = outer.0.join("repo");
    fs::create_dir_all(repo.join("src")).expect("a directory");
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("a file");
    fs::write(repo.join("src/lib.rs"), "#[test]\nfn passes() {}\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    std::os::unix::fs::symlink(".git", repo.join("target")).expect("the link");
    let git_before = entries(&repo.join(".git"));

    let argv: Vec<String> = ["cargo", "test", "--quiet", "--offline"]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect();
    let outcome = Substrate::open(&repo, &confinement::Options::default()).and_then(|adapter| {
        eprintln!("adapter: {}", adapter.record().describe());
        adapter.run_test(&TestRun {
            argv: &argv,
            tree: &repo,
            limits: TestsRun::LIMITS,
        })
    });
    match &outcome {
        Ok(finished) => panic!(
            "a run whose writable subtree is a symlink into .git started and exited {:?}",
            finished.code
        ),
        Err(error) => eprintln!("refused: {error:#}"),
    }
    assert_eq!(
        entries(&repo.join(".git")),
        git_before,
        "takt's scratch directory was left in .git"
    );
}
