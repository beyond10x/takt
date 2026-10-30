//! Adversarial case for story:run-configuration-file. README § Container: "Arguments after the
//! image name are flags of `takt run` and win over `run.toml`", and the README's own `run.toml`
//! sets `harness = "harness"` with `endpoint_file`. A `--harness` given on the command line must
//! then win: the file's `endpoint_file`, which only `--harness harness` reads, must not turn
//! that flag into a refusal of a flag the operator never gave.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

#[test]
fn a_harness_flag_wins_over_the_readme_run_toml() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-config-adv1-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale scratch removed");
    }
    let scratch = Scratch(root);
    let repo = scratch.0.join("repo");
    let home = scratch.0.join("home");
    let config = scratch.0.join("config");
    for dir in [&repo, &home, &config] {
        fs::create_dir_all(dir).expect("a directory");
    }
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    fs::write(
        config.join("endpoint.toml"),
        "base_url = \"https://models.example/v1\"\nwire = \"openai-responses\"\nmodel = \"m\"\n\n[credential]\nsecret = \"llm/model-key\"\n",
    )
    .expect("an endpoint file");
    let run_toml = config.join("run.toml");
    fs::write(
        &run_toml,
        "iterations = 0\nharness = \"harness\"\nendpoint_file = \"endpoint.toml\"\nforge = \"none\"\n",
    )
    .expect("a run configuration file");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo", repo.to_str().expect("utf-8")])
        .args(["--intent", "probe"])
        .args(["--config", run_toml.to_str().expect("utf-8")])
        .args(["--harness", "none"])
        .env("HOME", &home)
        .env("XDG_STATE_HOME", scratch.0.join("xdg-state"))
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("takt runs");
    let all = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(3), "{all}");
}
