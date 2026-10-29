//! The `takt` binary's `--forge` default at its edge: an `origin` that is not a GitHub
//! repository. Adversary pass 1 over story:forge-pull-request-evidence.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The delivery protocol, the engineering protocols' built-in, which declares `pr.open` and `ci.read`.
const DELIVERY_PROTOCOL: &str = "story.delivery/1";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

/// A committed repository at `<scratch>/repo` with a readme and `origin` at `url`.
fn repository(name: &str, url: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("forge-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&repo, &["remote", "add", "origin", url]);
    (Scratch(outer), repo)
}

/// Runs the built `takt` binary with `args`: the exit code, stdout and stderr.
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

/// `takt run` with no harness on the delivery protocol over `repo`, one step at most: the
/// budget stops the run after `tests.run`, before anything would be pushed.
fn run_args<'a>(repo: &'a str, case: &'a str) -> Vec<&'a str> {
    vec![
        "run",
        "--repo",
        repo,
        "--intent",
        "deliver the story",
        "--case-id",
        case,
        "--harness",
        "none",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--test-cmd",
        "true",
        "--iterations",
        "1",
    ]
}

/// Runs the built `takt` binary with `args` on a program path whose one `glab` is a double in
/// `<outer>/stub-bin` that is logged in to no host (`glab auth status` exits 1): the real
/// `glab` is never run, and nothing reaches a host. The exit code, stdout and stderr.
fn takt_without_glab_login(outer: &Path, args: &[&str]) -> (i32, String, String) {
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let glab = bin.join("glab");
    fs::write(
        &glab,
        "#!/bin/sh\necho \"x $4 has not been authenticated with glab\" >&2\nexit 1\n",
    )
    .expect("the double is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&glab, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let mut path = vec![bin];
    path.extend(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .filter(|dir| !dir.join("glab").exists()),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env("PATH", std::env::join_paths(path).expect("a program path"))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The bot route pushes and reads under a GitHub App: it is a GitHub forge. An
/// `origin` on another host, or a `file://` URL, must not make it the default; `--forge none`
/// by default or a refusal in one line are both acceptable, a run that would push and open
/// there is not. Another host is the forge `gitlab`'s only while `glab` is logged in to it:
/// without a login the run has no forge, and says why in one line.
#[test]
fn a_non_github_origin_does_not_make_the_bot_route_the_default() {
    for (name, url, line) in [
        (
            "gitlab-ssh",
            "git@gitlab.example.com:owner/repo.git",
            "forge: none (origin gitlab.example.com is not github.com, and glab is not logged in to it: ",
        ),
        (
            "gitlab-https",
            "https://gitlab.example.com/owner/repo.git",
            "forge: none (origin gitlab.example.com is not github.com, and glab is not logged in to it: ",
        ),
        (
            "file",
            "file:///srv/git/owner/repo.git",
            "forge: none (origin names no <owner>/<repo> on a host)",
        ),
    ] {
        let (outer, repo) = repository(name, url);
        let (code, out, err) =
            takt_without_glab_login(&outer, &run_args(repo.to_str().expect("utf-8"), name));
        assert!(
            !out.contains("forge: bot"),
            "origin {url}: the bot route is the default forge: exit {code} stdout {out} stderr {err}"
        );
        assert!(
            out.contains(line),
            "origin {url}: {line}: exit {code} stdout {out} stderr {err}"
        );
    }
}

// Adversary pass 2 over story:forge-pull-request-evidence: the GitHub-only default and the
// `--forge bot` refusal at the binary's edge.

/// `--forge bot` on an origin the bot route refuses: the refusal is printed, and a credential
/// the origin URL carries (what a CI runner's checkout leaves in `.git/config`) is not.
#[test]
fn adv2_the_forge_bot_refusal_does_not_print_the_origin_credential() {
    let (_outer, repo) = repository(
        "credential",
        "https://ci-user:t0k3n-not-real@gitlab.example.com/owner/repo.git",
    );
    let mut args = run_args(repo.to_str().expect("utf-8"), "credential");
    args.extend(["--forge", "bot"]);
    let (code, out, err) = takt(&args);
    assert_ne!(
        code, 0,
        "the bot route was not refused: stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("t0k3n-not-real") && !err.contains("t0k3n-not-real"),
        "the credential is printed: exit {code} stdout {out} stderr {err}"
    );
}

/// A host name is case-insensitive: without `--forge`, an origin written as `GitHub.com` gets
/// the bot route by default, as `github.com` does.
#[test]
fn adv2_an_origin_host_written_in_upper_case_gets_the_bot_route_by_default() {
    let (outer, repo) = repository("upper", "https://GitHub.com/owner/repo.git");
    // The bot route is reachable (story:forge-preflight): a `b10x-gates` double that passes
    // the commit identity's scan and answers nothing else, and a policy file.
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let gates = bin.join("b10x-gates");
    fs::write(
        &gates,
        "#!/bin/sh\n[ \"$1\" = scan-text ] && exit 0\necho \"b10x-gates double: $*\" >&2\nexit 2\n",
    )
    .expect("the double is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&gates, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let policy = outer.join("policy.json");
    fs::write(&policy, "{}\n").expect("a policy file");
    let mut path = vec![bin];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(run_args(repo.to_str().expect("utf-8"), "upper"))
        .env("PATH", std::env::join_paths(path).expect("a program path"))
        .env("B10X_GATES_POLICY", &policy)
        .output()
        .expect("takt runs");
    let (code, out, err) = (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    );
    assert!(
        out.contains("forge: bot (owner/repo)"),
        "GitHub.com is github.com: exit {code} stdout {out} stderr {err}"
    );
}
