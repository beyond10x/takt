//! story:a-run-stops-at-a-default-token-budget: a `takt run` given no `--tokens` and no `tokens`
//! in its run file stops once the harness has reported 3,000,000 tokens, `Tokens (3000000)`,
//! exit 3, the case's `stop` line naming it; a budget the flag or the run file names is kept,
//! the flag's over the file's; a run file whose `tokens` is no whole number above zero is
//! refused before any case opens; and `takt run --help` states the default. The `claude` is a
//! double reporting 1,000,000 tokens on every step; nothing reaches a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A `claude` that writes a test file and replies `tests.write`, reporting 1,000,000 tokens.
const CLAUDE: &str = r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
mkdir -p tests
echo work >> tests/work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":600000,"output_tokens":400000}}'
"#;

/// A repository whose test command never passes, a `claude` double beside it, in a scratch
/// directory named for `name`.
fn fixture(name: &str) -> (Scratch, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "default-token-budget-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(outer.clone());
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    fs::create_dir_all(&repo).expect("a repository");
    fs::create_dir_all(&bin).expect("a program directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    executable(&bin.join("claude"), CLAUDE);
    (scratch, repo, bin)
}

/// `takt run` on `repo` with the double on the program path and `extra` arguments.
fn run(repo: &Path, bin: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--continue-past-red-base",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--harness",
            "claude",
            "--forge",
            "none",
            "--test-cmd",
            "./check.sh",
        ])
        .args(extra)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env(
            "XDG_STATE_HOME",
            repo.parent().expect("the scratch").join("xdg-state"),
        )
        .output()
        .expect("takt runs")
}

/// The case directories `repo` holds.
fn cases(repo: &Path) -> Vec<PathBuf> {
    match fs::read_dir(repo.join(".takt/cases")) {
        Ok(entries) => entries
            .map(|entry| entry.expect("an entry").path())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The run stopped on `stop`: exit 3, the `stopped:` line on stdout, and the case's one
/// `stop` line naming it; and the run's tokens, as that line counts them.
fn stopped_on(output: &Output, repo: &Path, stop: &str) -> u64 {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let said = format!("{stdout}\n{stderr}");
    assert!(
        stdout.contains(&format!("stopped: {stop}")),
        "the run stops on {stop}\n{said}"
    );
    assert_eq!(output.status.code(), Some(3), "{said}");
    let cases = cases(repo);
    assert_eq!(cases.len(), 1, "{said}");
    let record = fs::read_to_string(cases[0].join("events.jsonl")).expect("the record");
    let stops: Vec<serde_json::Value> = record
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a line"))
        .filter(|line| line["event"] == "stop")
        .collect();
    assert_eq!(stops.len(), 1, "{record}");
    assert_eq!(stops[0]["stop"], stop, "{record}");
    stops[0]["tokens"].as_u64().unwrap_or(0)
}

/// Acceptance 1: no `--tokens` and no run file: the run stops at 3,000,000, after the step
/// that brings the count there, not before.
#[test]
fn a_run_naming_no_budget_stops_at_3000000_tokens() {
    let (_scratch, repo, bin) = fixture("default");
    let output = run(&repo, &bin, &[]);
    let tokens = stopped_on(&output, &repo, "Tokens (3000000)");
    assert!(
        (3_000_000..4_000_000).contains(&tokens),
        "the run stops after the step that reaches 3000000: {tokens}"
    );
}

/// Acceptance 2: `--tokens 5000000` keeps its own budget.
#[test]
fn a_tokens_flag_keeps_its_own_budget() {
    let (_scratch, repo, bin) = fixture("flag");
    let output = run(&repo, &bin, &["--tokens", "5000000"]);
    let tokens = stopped_on(&output, &repo, "Tokens (5000000)");
    assert!((5_000_000..6_000_000).contains(&tokens), "{tokens}");
}

/// Acceptance 3 and 4: a run file's `tokens` is the budget, and `--tokens` wins over it.
#[test]
fn a_run_files_tokens_is_the_budget_and_the_flag_wins_over_it() {
    let (scratch, repo, bin) = fixture("file");
    let file = scratch.0.join("run.yaml");
    fs::write(&file, "tokens: 2000000\n").expect("the run file");
    let output = run(&repo, &bin, &["--config", file.to_str().expect("utf-8")]);
    stopped_on(&output, &repo, "Tokens (2000000)");

    let (scratch, repo, bin) = fixture("file-and-flag");
    let file = scratch.0.join("run.toml");
    fs::write(&file, "tokens = 2000000\n").expect("the run file");
    let output = run(
        &repo,
        &bin,
        &[
            "--config",
            file.to_str().expect("utf-8"),
            "--tokens",
            "5000000",
        ],
    );
    stopped_on(&output, &repo, "Tokens (5000000)");
}

/// Acceptance 5: a run file whose `tokens` is 0 or no number is refused, exit 2, naming the
/// file and the line, before any case opens.
#[test]
fn a_run_files_tokens_of_0_or_no_number_is_refused_before_any_case_opens() {
    for (name, file, text, line) in [
        ("zero-yaml", "run.yaml", "iterations: 4\ntokens: 0\n", 2),
        ("word-yaml", "run.yaml", "iterations: 4\ntokens: x\n", 2),
        ("zero-toml", "run.toml", "iterations = 4\ntokens = 0\n", 2),
        (
            "word-toml",
            "run.toml",
            "iterations = 4\ntokens = \"x\"\n",
            2,
        ),
        ("negative-yaml", "run.yaml", "tokens: -5\n", 1),
    ] {
        let (scratch, repo, bin) = fixture(name);
        let path = scratch.0.join(file);
        fs::write(&path, text).expect("the run file");
        let output = run(&repo, &bin, &["--config", path.to_str().expect("utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{name}\n{stdout}\n{stderr}");
        assert!(
            stderr.contains(&path.display().to_string())
                && stderr.contains(&format!("at line {line}")),
            "{name}: the refusal names the file and line {line}\n{stderr}"
        );
        assert!(
            cases(&repo).is_empty(),
            "{name}: no case opens\n{stdout}\n{stderr}"
        );
    }
}

/// Acceptance 6: `takt run --help` states the default in the `--tokens` entry.
#[test]
fn run_help_states_the_default_token_budget() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{help}");
    let entry = help
        .lines()
        .skip_while(|line| !line.contains("--tokens <"))
        .take_while(|line| !line.trim_start().starts_with("--") || line.contains("--tokens <"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        entry.contains("3000000"),
        "the --tokens entry names the default: {entry}\n{help}"
    );
}
