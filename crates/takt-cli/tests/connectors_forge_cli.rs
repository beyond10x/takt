//! story:a-connectors-forge-adapter at the binary's edge: `forge: connectors` in a run file is
//! refused before the case opens, exit 1, when the instance does not describe a write; without it
//! `connectors` is never run. `connectors` is a double on the program path, written into a
//! scratch directory, that records its arguments; nothing here reaches an instance.
//! story:connectors-forge-speaks-github-at-an-endpoint: a `forge_endpoint` that is `http://` to
//! a host other than a loopback one is refused, exit 2, naming the key and its line.

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
/// `<scratch>/lines` and describes the read operations only, refusing the others `not_found`
/// as `connectors --output json` does, on standard error.
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
        "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in *merge_request.create*|*merge_request.merge*) printf '%s' '{{\"ok\":false,\"error\":{{\"code\":\"not_found\",\"data\":{{}}}}}}' >&2; exit 1;; esac\nprintf '%s' '{{\"ok\":true,\"result\":{{\"connector\":\"gitlab\",\"instance\":\"forge\",\"effect\":\"read\",\"schema\":\"s1\",\"revision\":\"r1\"}}}}'\n",
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
        .env("PATH", std::env::join_paths(path).expect("a program path"))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A fake describing read operations only: exit 1 before the case opens, naming the create
/// operation; the lines the double saw are one description per operation, on the named target.
#[test]
fn preflight_names_the_missing_write() {
    let (outer, repo, bin) = repository("preflight", "https://forge.example/group/project.git");
    let config = outer.0.join("run.yaml");
    fs::write(&config, "forge: connectors\nforge_instance: gitlab/forge\n").expect("a file");
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
        [
            "merge_request.create",
            "merge_request.get",
            "merge_request.merge",
            "pipeline.get"
        ]
        .map(|operation| format!("--output json invoke gitlab/forge {operation} --describe\n"))
        .concat()
    );
}

/// Acceptance 4 of story:connectors-forge-speaks-github-at-an-endpoint: `forge_endpoint` that
/// is `http://` to a host that is not a loopback host is refused before any case opens, exit 2,
/// naming the file, the key and its line and never the value; nothing is called.
#[test]
fn an_endpoint_over_plain_http_to_another_host_exits_2() {
    let (outer, repo, bin) = repository("endpoint", "https://forge.example/owner/repo.git");
    for (name, text, line) in [
        (
            "run.yaml",
            "forge: connectors\nforge_instance: github\nforge_endpoint: http://example.com\nscan: none\n",
            3,
        ),
        (
            "run.toml",
            "forge = \"connectors\"\nforge_endpoint = \"http://example.com\"\nforge_instance = \"github\"\n",
            2,
        ),
    ] {
        let config = outer.0.join(name);
        fs::write(&config, text).expect("a file");
        let (code, out, err) = takt(&bin, &repo, &["--config", config.to_str().expect("utf-8")]);
        assert_eq!(code, 2, "{name}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!(
                "{} is refused at line {line}: `forge_endpoint`",
                config.display()
            )),
            "{name}: stderr {err}"
        );
        assert!(!err.contains("example.com"), "{name}: stderr {err}");
        assert!(!repo.join(".takt/cases").exists(), "{name}");
    }
    assert!(!outer.0.join("lines").exists());

    let config = outer.0.join("other.yaml");
    fs::write(
        &config,
        "forge: none\nforge_endpoint: https://connectors.example\n",
    )
    .expect("a file");
    let (code, out, err) = takt(&bin, &repo, &["--config", config.to_str().expect("utf-8")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("`forge_endpoint` is set"), "stderr {err}");
    assert!(!repo.join(".takt/cases").exists());
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
        "forge_instance: gitlab/forge\n",
        "forge: none\nforge_instance: gitlab/forge\n",
        "forge: connectors\n",
        "forge: connectors\nforge_instance: forge\n",
    ] {
        fs::write(&config, text).expect("a file");
        let (code, out, err) = takt(&bin, &repo, &["--config", config.to_str().expect("utf-8")]);
        assert_eq!(code, 1, "{text}: stdout {out} stderr {err}");
        assert!(err.contains("forge_instance"), "{text}: stderr {err}");
        assert!(!repo.join(".takt/cases").exists(), "{text}");
    }
    assert!(!outer.0.join("lines").exists());
}
