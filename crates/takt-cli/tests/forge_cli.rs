//! The `takt` binary's `--forge` at its edge: its default for an `origin` on a host, the values
//! it takes, and a run file's `forge`. Adversary pass 1 over story:forge-pull-request-evidence,
//! and story:takt-carries-no-organisations-own-route.

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

/// Without `--forge`, an `origin` on a host is the forge `gitlab`'s only while `glab` is
/// logged in to it, whichever host it is: without a login the run has no forge, and says why in
/// one line. A `file://` URL names no repository on a host and gets no forge either.
#[test]
fn without_a_glab_login_an_origin_gets_no_forge_by_default() {
    for (name, url, line) in [
        (
            "gitlab-ssh",
            "git@gitlab.example.com:owner/repo.git",
            "forge: none (glab is not logged in to gitlab.example.com: ",
        ),
        (
            "gitlab-https",
            "https://gitlab.example.com/owner/repo.git",
            "forge: none (glab is not logged in to gitlab.example.com: ",
        ),
        (
            "github",
            "https://GitHub.com/owner/repo.git",
            "forge: none (glab is not logged in to GitHub.com: ",
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
            out.contains(line),
            "origin {url}: {line}: exit {code} stdout {out} stderr {err}"
        );
    }
}

/// `--forge gitlab` refused for want of a login: the refusal is printed, and a credential the
/// origin URL carries (what a CI runner's checkout leaves in `.git/config`) is not.
#[test]
fn the_forge_refusal_does_not_print_the_origin_credential() {
    let (outer, repo) = repository(
        "credential",
        "https://ci-user:t0k3n-not-real@gitlab.example.com/owner/repo.git",
    );
    let mut args = run_args(repo.to_str().expect("utf-8"), "credential");
    args.extend(["--forge", "gitlab"]);
    let (code, out, err) = takt_without_glab_login(&outer, &args);
    assert_ne!(
        code, 0,
        "the forge gitlab was not refused: stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("t0k3n-not-real") && !err.contains("t0k3n-not-real"),
        "the credential is printed: exit {code} stdout {out} stderr {err}"
    );
}

/// Whether `text` holds `word` as a whole word, as `grep -w` reads one: not inside a longer run
/// of letters, digits and `_`.
fn holds_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|token| token == word)
}

/// story:takt-carries-no-organisations-own-route, acceptance 1: `--forge` offers `none`,
/// `local`, `gitlab`, `command` and `connectors`, and `bot` is no value of it: clap refuses it,
/// exit 2, for `takt run` and `takt scenario run` alike, and neither's help names a `bot`.
#[test]
fn forge_bot_is_refused_by_clap_and_no_help_names_it() {
    let (_outer, repo) = repository("forge-bot", "https://github.com/owner/repo.git");
    let repo = repo.to_str().expect("utf-8");
    let mut run = run_args(repo, "forge-bot");
    run.extend(["--forge", "bot"]);
    let scenario = vec![
        "scenario",
        "run",
        "--repo",
        repo,
        "any",
        "--declared-on",
        "HEAD",
        "--harness",
        "none",
        "--forge",
        "bot",
    ];
    for args in [run, scenario] {
        let (code, out, err) = takt(&args);
        assert_eq!(code, 2, "{args:?}: stdout {out} stderr {err}");
        assert!(
            err.contains("invalid value 'bot' for '--forge <FORGE>'"),
            "{args:?}: stderr {err}"
        );
        for value in ["none", "local", "gitlab", "command", "connectors"] {
            assert!(err.contains(value), "{args:?}: {value} not offered: {err}");
        }
    }
    for args in [&["run", "--help"][..], &["scenario", "run", "--help"][..]] {
        let (code, out, err) = takt(args);
        assert_eq!(code, 0, "{args:?}: stdout {out} stderr {err}");
        assert!(!holds_word(&out, "bot"), "{args:?} names a bot: {out}");
        assert!(
            out.contains("[possible values: none, local, gitlab, command, connectors]"),
            "{args:?}: {out}"
        );
    }
}

/// story:takt-carries-no-organisations-own-route, acceptance 2: a run file whose `forge` is
/// `bot` is refused when it is read, naming the file and the line, before any case opens: the
/// tree stays on its branch, no case directory is made and no `forge:` line is printed.
#[test]
fn a_run_file_with_forge_bot_is_refused_at_its_line() {
    for (name, text, line) in [
        ("yaml", "harness: none\nforge: \"bot\"\n", 2),
        ("toml", "harness = \"none\"\n\nforge = \"bot\"\n", 3),
    ] {
        let (outer, repo) = repository(&format!("run-file-{name}"), "https://github.com/o/r.git");
        let file = outer.join(format!("run.{name}"));
        fs::write(&file, text).expect("the run file is written");
        let mut args = run_args(repo.to_str().expect("utf-8"), "run-file");
        args.extend(["--config", file.to_str().expect("utf-8")]);
        let (code, out, err) = takt_without_glab_login(&outer, &args);
        assert_eq!(code, 1, "{name}: stdout {out} stderr {err}");
        let expected = format!(
            "the run configuration file {} is refused at line {line}: `forge` is none, local, gitlab, command or connectors, not `bot`",
            file.display()
        );
        assert!(err.contains(&expected), "{name}: {expected}: stderr {err}");
        assert!(!out.contains("forge:"), "{name}: stdout {out}");
        assert!(
            !repo.join(".takt").exists(),
            "{name}: a case directory was made"
        );
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo)
            .output()
            .expect("git runs");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "main",
            "{name}"
        );
    }
}
