//! story:an-unqualified-harness-stops-before-the-first-session, as the command line ends it: `takt
//! run --harness claude` on a `claude` that reports 2.1.311 stops before its first session, exit
//! 1, the stop line naming the version, H11, the versions that qualify and `--harness-program`,
//! the model endpoint sent no request; and once the binary is one that qualifies, the next run
//! of the same case goes on and calls the model. The double and the endpoint are
//! `crates/takt/tests/preflight_fakes`; the four named scenarios are
//! `crates/takt/tests/harness_preflight.rs`.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

#[path = "../../takt/tests/preflight_fakes/mod.rs"]
mod preflight_fakes;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `takt run --harness claude` on `repo`: opening `case`, or resuming it when `resume`.
fn takt(repo: &Path, bin: &Path, state: &Path, case: &str, resume: bool) -> Output {
    let case_flags: Vec<&str> = if resume {
        vec!["--case", case]
    } else {
        vec!["--intent", "make add add", "--case-id", case]
    };
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--continue-past-red-base",
            "--repo",
            repo.to_str().expect("utf-8"),
        ])
        .args(case_flags)
        .args([
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "3",
            "--test-cmd",
            "git grep -qE a.[+].b$",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_STATE_HOME", state)
        .output()
        .expect("takt runs")
}

fn said(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn an_unqualified_claude_exits_one_before_a_model_call_and_the_case_resumes() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("harness-preflight-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    let state = outer.join("state");
    for dir in [&repo, &state] {
        fs::create_dir_all(dir).expect("a directory");
    }
    fs::create_dir_all(repo.join("src")).expect("the tree");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    fs::write(
        repo.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
    )
    .expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let claude = preflight_fakes::install(&bin, "claude", "2.1.311");
    let endpoint = preflight_fakes::Endpoint::serving(&claude);

    let refused = takt(&repo, &bin, &state, "preflight-cli", false);
    let text = said(&refused);
    assert_eq!(refused.status.code(), Some(1), "{text}");
    let line = String::from_utf8_lossy(&refused.stdout)
        .lines()
        .find(|line| line.starts_with("stopped: HarnessUnqualified ("))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("the stop line\n{text}"));
    for named in [
        "claude 2.1.311",
        "H11",
        "qualified versions: 2.1.304, 2.1.305, 2.1.309",
        "--harness-program",
    ] {
        assert!(line.contains(named), "the stop line names {named}: {line}");
    }
    assert_eq!(endpoint.requests(), 0, "no model call\n{text}");

    preflight_fakes::set_version(&claude, "2.1.309");
    let resumed = takt(&repo, &bin, &state, "preflight-cli", true);
    let text = said(&resumed);
    assert!(
        !text.contains("HarnessUnqualified"),
        "the same case, a binary that qualifies: {text}"
    );
    assert!(
        endpoint.requests() >= 1,
        "the resumed run's session called the model\n{text}"
    );
}
