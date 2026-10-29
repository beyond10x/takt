//! story:a-connectors-forge-adapter at the binary's edge: `forge: connectors` in a run file is
//! refused before the case opens, exit 1, when the instance does not list a write; without it
//! `connectors` is never run. `connectors` is a double on the program path, written into a
//! scratch directory, that records its arguments; nothing here reaches an instance.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository at `<scratch>/repo` with `origin` at `url`, and the double of
/// `connectors` in `<scratch>/stub-bin`: it appends its arguments, one line per run, to
/// `<scratch>/lines` and answers `operations list` with the read operations only.
fn repository(name: &str, url: &str) -> (Scratch, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "connectors-forge-cli-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outer);
    let repo = outer.join("repo");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&repo, &["remote", "add", "origin", url]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a directory");
    let lines = outer.join("lines");
    let script = format!(
        "#!/bin/sh\necho \"$*\" >> '{}'\nprintf '%s' '{{\"ok\":true,\"result\":{{\"connector\":\"gitlab\",\"instance\":\"forge\",\"revision\":\"r1\",\"operations\":[{{\"id\":\"merge_request.get\"}},{{\"id\":\"pipeline.get\"}}]}}}}'\n",
        lines.display()
    );
    let path = bin.join("connectors");
    fs::write(&path, script).expect("the double is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    (Scratch(outer), repo, bin)
}

/// `takt run` with no harness on the delivery protocol over `repo`, `extra` after it, with `bin`
/// first on the program path.
fn takt(bin: &Path, repo: &Path, extra: &[&str]) -> (i32, String, String) {
    let mut path = vec![bin.to_path_buf()];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "deliver the story",
            "--case-id",
            "c1",
            "--harness",
            "none",
            "--protocol",
            DELIVERY_PROTOCOL,
            "--test-cmd",
            "true",
            "--iterations",
            "1",
        ])
        .args(extra)
        .env_remove("B10X_GATES_POLICY")
        .env("PATH", std::env::join_paths(path).expect("a program path"))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A fake listing read operations only: exit 1 before the case opens, naming the create
/// operation; the one line the double saw is the listing, on the named instance.
#[test]
fn preflight_names_the_missing_write() {
    let (outer, repo, bin) = repository("preflight", "https://forge.example/group/project.git");
    let config = outer.0.join("run.yaml");
    fs::write(&config, "forge: connectors\nforge_instance: forge\n").expect("a file");
    let (code, out, err) = takt(&bin, &repo, &["--config", config.to_str().expect("utf-8")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("merge_request.create") && err.contains("merge_request.merge"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("step 1"), "stdout {out} stderr {err}");
    assert!(
        !repo.join(".takt/cases").exists(),
        "no case opened: stdout {out} stderr {err}"
    );
    assert_eq!(
        fs::read_to_string(outer.0.join("lines")).expect("connectors ran"),
        "operations list --instance forge --output json\n"
    );
}

/// A run file without `forge: connectors`, and a flag that names none, never run `connectors`.
#[test]
fn it_is_never_the_default() {
    let (outer, repo, bin) = repository("default", "file:///srv/git/owner/repo.git");
    let (_, out, err) = takt(&bin, &repo, &[]);
    assert!(
        out.contains("forge: none"),
        "the default forge: stdout {out} stderr {err}"
    );
    let (_, out, err) = takt(&bin, &repo, &["--forge", "none"]);
    assert!(!out.contains("connectors"), "stdout {out} stderr {err}");
    assert!(
        !outer.0.join("lines").exists(),
        "connectors was run: stdout {out} stderr {err}"
    );
}

/// `forge_instance` under another forge, and `forge: connectors` without it, are refused before
/// any case opens, naming the key, and `connectors` is not run.
#[test]
fn the_key_and_the_forge_go_together() {
    let (outer, repo, bin) = repository("together", "https://forge.example/group/project.git");
    let config = outer.0.join("run.yaml");
    for text in [
        "forge_instance: forge\n",
        "forge: none\nforge_instance: forge\n",
        "forge: connectors\n",
    ] {
        fs::write(&config, text).expect("a file");
        let (code, out, err) = takt(&bin, &repo, &["--config", config.to_str().expect("utf-8")]);
        assert_eq!(code, 1, "{text}: stdout {out} stderr {err}");
        assert!(err.contains("forge_instance"), "{text}: stderr {err}");
        assert!(!repo.join(".takt/cases").exists(), "{text}");
    }
    assert!(!outer.0.join("lines").exists());
}
