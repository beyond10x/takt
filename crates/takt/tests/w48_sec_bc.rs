//! Security conformance cases for story:git-through-the-connectors-git-relay (the relay path
//! naming the repository) and story:connectors-forge-at-a-served-instance (the CI install of the
//! served instance): which project `ConnectorsForge::for_tree` names for an `origin`, whether a
//! credential in `origin` reaches what it names, and what the workflow's install step runs
//! before it checks an archive. The forge's lines go to a double that describes every operation;
//! no line leaves the process and nothing reaches a network.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use takt::forge::Exit;
use takt::forge::Runner;
use takt::forge::connectors::ConnectorsForge;
use takt::scan::Scan;

/// A runner that answers every `connectors invoke <target> <operation> --describe` as an
/// instance describing the operation, and refuses anything else.
struct Describing;

impl Runner for Describing {
    fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
        assert!(
            argv.iter().any(|word| word == "--describe"),
            "only the preflight runs here: {argv:?}"
        );
        Ok(Exit {
            code: Some(0),
            stdout: r#"{"ok":true,"result":{"schema":"{}","revision":"r1","connector":"c","instance":"i"}}"#
                .to_owned(),
            stderr: String::new(),
        })
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
        self.run(tree, argv)
    }
}

/// A repository under the build's scratch directory whose `origin` is `url`, removed when
/// dropped.
struct Tree(PathBuf);

impl Tree {
    fn with_origin(name: &str, url: &str) -> Self {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-sec-bc-{}-{made}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        for args in [
            &["init", "--quiet", "-b", "main"][..],
            &["remote", "add", "origin", url][..],
        ] {
            let output = Command::new("git")
                .env("GIT_CEILING_DIRECTORIES", env!("CARGO_TARGET_TMPDIR"))
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("git runs");
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Self(dir)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `for_tree` on the CLI route for `target`, `origin` being `url`: the forge's `Debug` (which
/// shows the host and project it names) or the error, both as text.
fn opened(target: &str, url: &str) -> std::result::Result<String, String> {
    let tree = Tree::with_origin(target, url);
    ConnectorsForge::for_tree(
        &tree.0,
        target,
        None,
        Scan::None {
            declared_in: tree.0.join("run.yaml"),
        },
        Box::new(Describing),
    )
    .map(|forge| format!("{forge:?}"))
    .map_err(|error| format!("{error:#}"))
}

/// The project a forge's `Debug` names: the second field of `origin`.
fn project(debug: &str) -> String {
    let (_, rest) = debug
        .split_once("origin: (")
        .unwrap_or_else(|| panic!("no origin in {debug}"));
    let (_, rest) = rest.split_once(", \"").expect("a project");
    rest.split_once('"').expect("a closing quote").0.to_owned()
}

/// Invariant 1: a remote (`https://`) origin whose path begins `git/` names the project it named
/// before the relay mapping: three segments, which the github connector refuses and the gitlab
/// connector keeps whole. Only a loopback relay's path is read as the relay's.
#[test]
fn a_remote_origin_under_git_names_the_project_it_named_before() {
    let refused = opened("github", "https://github.example/git/o/r.git")
        .expect_err("three segments are no github repository");
    assert!(refused.contains("git/o/r"), "{refused}");

    for url in [
        "https://gitlab.example/git/o/r.git",
        "https://gitlab.example:8443/git/o/r.git",
        "git@gitlab.example:git/o/r.git",
        "ssh://git@127.0.0.1/git/o/r.git",
    ] {
        let debug = opened("gitlab", url).unwrap_or_else(|error| panic!("{url}: {error}"));
        assert_eq!(project(&debug), "git/o/r", "{url}: {debug}");
    }
}

/// The relay path names exactly `<owner>/<repo>`: `o/r`, not any other two segments. The served
/// test's stand-in of the github operations reads no `owner` or `repo`, so it would pass a
/// mapping to `git/o` or `r/o` as well; this case would not.
#[test]
fn the_loopback_relay_path_names_its_owner_and_repo() {
    for url in [
        "http://127.0.0.1:9/git/o/r.git",
        "http://127.0.0.1:9/git/o/r",
        "http://localhost:9/git/o/r.git/",
        "http://[::1]:9/git/o/r.git",
    ] {
        let debug = opened("github", url).unwrap_or_else(|error| panic!("{url}: {error}"));
        assert_eq!(project(&debug), "o/r", "{url}: {debug}");
    }
}

/// Invariant 2: no other shape of a loopback relay path is read as `git/<owner>/<repo>`: an
/// extra segment, an empty one, `.` or `..` is refused for github, never shortened to two.
#[test]
fn a_loopback_relay_path_of_another_shape_names_no_shorter_project() {
    for url in [
        "http://127.0.0.1:9/git/o/r/extra.git",
        "http://127.0.0.1:9/git/o/r/x/y.git",
        "http://127.0.0.1:9/git//r.git",
        "http://127.0.0.1:9/git/o//r.git",
        "http://127.0.0.1:9/git/../r.git",
        "http://127.0.0.1:9/git/o/...git",
        "http://127.0.0.1:9/git/./r.git",
        "http://127.0.0.1:9/x/git/o/r.git",
        "http://127.0.0.1:9/Git/o/r.git",
    ] {
        let said = opened("github", url);
        assert!(said.is_err(), "{url} opened: {said:?}");
    }
}

/// Invariant 2, encoded segments: the relay reads its path's segments percent-decoded (its
/// route's `{owner}` and `{repo}`), so `git/%6F/r` is served as `o/r` and `git/o%2Fx/r` as the
/// owner `o/x`. The project takt names for such a path is the one the relay serves, or none:
/// it never names `%6F/r` while the relay serves `o/r`.
#[test]
fn an_encoded_relay_path_names_the_project_the_relay_serves_or_none() {
    for (url, served) in [
        ("http://127.0.0.1:9/git/%6F/r.git", "o/r"),
        ("http://127.0.0.1:9/git/o/%72.git", "o/r"),
        ("http://127.0.0.1:9/git/o%2Fx/r.git", "o/x/r"),
        ("http://127.0.0.1:9/git/o%2fx/r.git", "o/x/r"),
        ("http://127.0.0.1:9/git/%2e%2e/r.git", "../r"),
    ] {
        match opened("github", url) {
            Err(_) => {}
            Ok(debug) => assert_eq!(
                project(&debug),
                served,
                "{url}: takt names another project than the relay serves: {debug}"
            ),
        }
    }
}

/// The mapping is the github repository's: a loopback relay path `git/<a>/<b>` cannot be a
/// github repository whole, but it is a gitlab project under the top-level group `git`. On the
/// gitlab connector the path names the project it named before the change.
#[test]
fn a_gitlab_project_under_the_group_git_keeps_its_name_behind_a_loopback_relay() {
    let url = "http://127.0.0.1:9/git/team/app.git";
    let debug = opened("gitlab", url).unwrap_or_else(|error| panic!("{url}: {error}"));
    assert_eq!(project(&debug), "git/team/app", "{debug}");
}

/// Invariant 3: a userinfo on a loopback relay `origin` reaches neither the project the forge
/// names nor the refusal of a path it does not admit.
#[test]
fn a_userinfo_on_the_relay_origin_reaches_no_project_and_no_refusal() {
    let secret = concat!("relay-", "userinfo-", "5e1f");
    let debug = opened(
        "github",
        &format!("http://u:{secret}@127.0.0.1:9/git/o/r.git"),
    )
    .expect("the relay origin opens");
    assert_eq!(project(&debug), "o/r", "{debug}");
    assert!(!debug.contains(secret), "{debug}");
    let refused = opened(
        "github",
        &format!("http://u:{secret}@127.0.0.1:9/git/o/r/x.git"),
    )
    .expect_err("four segments are no repository");
    assert!(!refused.contains(secret), "{refused}");
}

/// The lines of the workflow step named `name` in `.github/workflows/check.yml`, its `run` block
/// included, up to the next step.
fn step(name: &str) -> Vec<String> {
    let workflow = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/check.yml"
    ))
    .expect("the workflow");
    let start = format!("      - name: {name}");
    let mut lines = workflow.lines().skip_while(|line| *line != start);
    let mut out: Vec<String> = lines.next().into_iter().map(str::to_owned).collect();
    assert!(!out.is_empty(), "no step {name}");
    out.extend(
        lines
            .take_while(|line| !line.starts_with("      - "))
            .map(str::to_owned),
    );
    out
}

/// Invariant 4: the install step fails on the first failing line, checks each archive's sha256
/// against a pinned value before extracting it, runs nothing from it, and hands later steps
/// the two variables and nothing on `PATH`.
#[test]
fn the_connectors_install_checks_each_archive_before_extracting_it() {
    let lines = step("Install connectors");
    let run: Vec<&str> = lines
        .iter()
        .map(|line| line.trim())
        .skip_while(|line| *line != "run: |")
        .skip(1)
        .collect();
    assert_eq!(run.first(), Some(&"set -euo pipefail"), "{run:#?}");
    let mut checked = Vec::new();
    for line in &run {
        if line.contains("sha256sum") {
            assert!(
                line.ends_with("| sha256sum --check --strict") && line.starts_with("echo \"$"),
                "a check of a pinned value: {line}"
            );
            let file = line
                .trim_end_matches("\" | sha256sum --check --strict")
                .rsplit("  ")
                .next()
                .expect("a file")
                .to_owned();
            checked.push(file);
        }
        if line.starts_with("tar ") {
            let file = line
                .split_whitespace()
                .nth(2)
                .expect("an archive")
                .trim_matches('"');
            assert!(
                checked.iter().any(|done| done == file),
                "{file} is extracted before its check: {run:#?}"
            );
        }
        assert!(!line.contains("GITHUB_PATH"), "{line}");
        if line.contains("GITHUB_ENV") {
            assert!(
                line.starts_with("echo \"TAKT_TEST_CONNECTORS_BIN=$RUNNER_TEMP/connectors-bin/")
                    || line.starts_with(
                        "echo \"TAKT_TEST_CONNECTORS_DEFINITIONS=$RUNNER_TEMP/connectors-definitions/"
                    ),
                "{line}"
            );
        }
        assert!(
            !line.contains("connectors-catalog-provider") || line.contains("GITHUB_ENV"),
            "the step runs the binary: {line}"
        );
    }
    assert_eq!(checked.len(), 2, "{run:#?}");
    for pinned in ["CONNECTORS_SHA256: ", "DEFINITIONS_SHA256: "] {
        let value = lines
            .iter()
            .find_map(|line| line.trim().strip_prefix(pinned))
            .unwrap_or_else(|| panic!("no {pinned}"));
        assert!(
            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{pinned}{value}"
        );
    }
}
