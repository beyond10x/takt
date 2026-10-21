//! story:a-session-step-loads-no-auto-memory, acceptance 3, where `takt run --harness claude`
//! starts its session: the `claude` here is a double that keeps the arguments, the environment
//! and the settings it was started with, and opens its stream as Claude Code 2.1.304 does:
//! naming its auto-memory directory (`memory_paths`) unless it was started `--restricted` or
//! `--safe-mode`, `CLAUDE_CODE_DISABLE_AUTO_MEMORY` is true or its settings say
//! `autoMemoryEnabled: false`, and its cross-session inbox (`messaging_socket_path`) unless it
//! was started `--restricted`. Nothing reaches a model.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A `claude` that writes its arguments, one a line, to `<seen>/args`, its environment to
/// `<seen>/env` and the settings `--settings` names (a file, or the JSON itself) to
/// `<seen>/settings`, then prints an init event naming a memory directory under its
/// configuration home and an inbox socket when Claude Code's would (the module's rules), and,
/// having written a test file, the result event naming `tests.write`.
fn fake_claude(at: &Path, seen: &Path) {
    let script = r#"#!/bin/sh
settings=''
prev=''
restricted=no
memory=on
for a in "$@"; do
  if [ "$prev" = "--settings" ]; then settings="$a"; fi
  case "$a" in --restricted) restricted=yes; memory=off ;; --safe-mode) memory=off ;; esac
  prev="$a"
done
printf '%s\n' "$@" > '@SEEN@/args'
env | sort > '@SEEN@/env'
if [ -f "$settings" ]; then cat "$settings" > '@SEEN@/settings'; else printf '%s' "$settings" > '@SEEN@/settings'; fi
switch=$(printf '%s' "$CLAUDE_CODE_DISABLE_AUTO_MEMORY" | tr 'A-Z' 'a-z' | tr -d ' ')
case "$switch" in 1|true|yes|on) memory=off ;; esac
if tr -d ' \n\t' < '@SEEN@/settings' | grep -q '"autoMemoryEnabled":false'; then memory=off; fi
paths=''
if [ "$memory" = on ]; then paths=",\"memory_paths\":{\"auto\":\"$CLAUDE_CONFIG_DIR/projects/copy/memory/\"}"; fi
if [ "$restricted" = no ]; then paths="$paths,\"messaging_socket_path\":\"$TMPDIR/cc-socks/1.sock\""; fi
printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"tools\":[\"Read\",\"Edit\",\"Write\",\"Glob\",\"Grep\"],\"plugins\":[]$paths}"
mkdir -p tests
echo work >> tests/work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4}}'
"#
    .replace("@SEEN@", &seen.display().to_string());
    executable(at, &script);
}

/// Every file named `step-*.jsonl` under `dir`.
fn transcripts(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            transcripts(&path, into);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("step-") && name.ends_with(".jsonl"))
        {
            into.push(path);
        }
    }
}

/// `takt run --harness claude` starts its session with auto-memory and the inbox off: the
/// session's init event, as the step's transcript keeps it, names no memory directory and no
/// inbox socket, and the `claude` was started `--restricted`. That proves what takt hands the
/// program; that Claude Code itself then loads no memory shows only in a live run's init event.
#[test]
fn a_session_step_starts_with_auto_memory_off() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("session-memory-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    let state = outer.join("state");
    let seen = outer.join("seen");
    for dir in [&repo, &bin, &state, &seen] {
        fs::create_dir_all(dir).expect("a directory");
    }
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    fake_claude(&bin.join("claude"), &seen);

    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "2",
            "--test-cmd",
            "./check.sh",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_STATE_HOME", &state)
        .env_remove("CLAUDE_CODE_DISABLE_AUTO_MEMORY")
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let args = fs::read_to_string(seen.join("args"))
        .unwrap_or_else(|error| panic!("the double ran and kept its arguments: {error}\n{said}"));
    let env = fs::read_to_string(seen.join("env")).expect("the double kept its environment");
    let settings = fs::read_to_string(seen.join("settings")).expect("the double kept its settings");
    let mut found = Vec::new();
    transcripts(&state.join("takt/transcripts"), &mut found);
    assert_eq!(
        found.len(),
        1,
        "one harness step's transcript: {found:?}\n{said}"
    );
    let kept = fs::read_to_string(&found[0]).expect("the transcript");
    let init: serde_json::Value = kept
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|doc| doc["type"] == "system" && doc["subtype"] == "init")
        .unwrap_or_else(|| panic!("an init event in\n{kept}"));
    let started = format!("arguments:\n{args}\nenvironment:\n{env}\nsettings: {settings}");
    assert!(
        init.get("memory_paths").is_none(),
        "the session's init names no memory directory: {init}\n{started}"
    );
    assert!(
        init.get("messaging_socket_path").is_none(),
        "the session's init names no inbox socket: {init}\n{started}"
    );
    assert!(
        args.lines().any(|line| line == "--restricted"),
        "the session is started restricted:\n{started}"
    );
}
