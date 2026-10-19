//! story:takt-has-a-gates-document: takt's own repository names its gate. The gates document
//! binds the four fast commands of `Taskfile.yml`'s `check` task, each through
//! `scripts/gate-exec`, which finds the tool on the Cargo bin directory and the system
//! directories and refuses one that resolves inside the repository; the fifth, the workspace's
//! tests, is a delivery's test command. A delivery on a scratch repository carrying copies of
//! those three files offers `gate.run` after the implementation and reaches `pr.open`; a gate
//! command that fails holds `gate.green` false and opens no pull request. The tools are stubs in
//! a scratch Cargo bin directory, the forge and the harness doubles on the program path; nothing
//! reaches a network or a model, and takt's own tests are not run from here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// takt's repository: the root of this workspace.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The gates document, the project file admitting its commands, and the script every gate
/// command runs through, relative to the repository.
const GATES: &str = ".engineering/gates.yaml";
const PROJECT: &str = ".engineering/assertions-project.yaml";
const GATE_EXEC: &str = "scripts/gate-exec";

/// The test command a delivery on takt runs (`--test-cmd`): the one command of the `check` task
/// the gate does not run, because it outlasts the gate's 300 s.
const TEST_COMMAND: &str = "cargo test --workspace --locked";

fn root() -> PathBuf {
    Path::new(ROOT).canonicalize().expect("the repository root")
}

/// The commands of `Taskfile.yml`'s `check` task, read now, in order.
fn check_task() -> Vec<String> {
    let text = fs::read_to_string(root().join("Taskfile.yml")).expect("Taskfile.yml");
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("Taskfile.yml parses");
    doc["tasks"]["check"]["cmds"]
        .as_sequence()
        .expect("the check task has commands")
        .iter()
        .map(|cmd| cmd.as_str().expect("each command is a line").to_owned())
        .collect()
}

/// The four commands the gate runs: the check task's but its test command.
fn gate_commands() -> Vec<String> {
    let mut commands = check_task();
    assert_eq!(
        commands.pop().as_deref(),
        Some(TEST_COMMAND),
        "the check task ends with the delivery's test command"
    );
    commands
}

/// takt's own gates document, read through the loader the gate validator evaluates it with:
/// it binds, in order, exactly the check task's commands but the tests, each as `gate-exec`
/// with the tool and its arguments, and those four and the test command are the check task.
#[test]
fn takts_gates_document_binds_the_check_tasks_commands_through_gate_exec() {
    let root = root();
    let prepared = canon_engineering_assertions::prepare(
        &root,
        Path::new(GATES),
        Some(Path::new(PROJECT)),
        Some(Path::new(".engineering/cases")),
    )
    .unwrap_or_else(|error| panic!("takt's {GATES} with {PROJECT} loads: {error:#}"));
    let mut bound = Vec::new();
    for gate in &prepared.document.gates {
        for assertion in &gate.assertions {
            let binding = assertion
                .strip_prefix("process.succeeded(\"")
                .and_then(|rest| rest.strip_suffix("\")"))
                .unwrap_or_else(|| {
                    panic!(
                        "gate {}: {assertion} is not one command succeeding",
                        gate.id
                    )
                });
            bound.push(binding.to_owned());
        }
    }
    let gate_exec = root.join(GATE_EXEC);
    let mut commands = Vec::new();
    for binding in &bound {
        let command = &prepared.context.commands[binding];
        assert_eq!(
            command.executable, gate_exec,
            "{binding} runs through {GATE_EXEC}"
        );
        assert!(
            command.env.is_empty(),
            "{binding} sets its own environment, so the gate's CARGO_HOME may not reach {GATE_EXEC}: {:?}",
            command.env
        );
        commands.push(command.args.join(" "));
    }
    assert_eq!(
        prepared.context.commands.len(),
        bound.len(),
        "every admitted command is one a gate runs"
    );
    assert_eq!(commands, gate_commands());
    commands.push(TEST_COMMAND.to_owned());
    assert_eq!(commands, check_task());
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

fn scratch(name: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("takt-gates-document-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&outer).expect("a scratch directory");
    let outer = outer.canonicalize().expect("the scratch directory");
    (Scratch(outer.clone()), outer)
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

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// Writes `name` into `dir`, executable, with `text`.
fn executable(dir: &Path, name: &str, text: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    fs::create_dir_all(dir).expect("a directory");
    let path = dir.join(name);
    fs::write(&path, text).expect("the stub is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    path
}

/// A stub tool that appends its name and arguments to `log` and exits `code`.
fn stub(dir: &Path, name: &str, log: &Path, code: i32) {
    executable(
        dir,
        name,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$(basename \"$0\") $*\" >> '{}'\nexit {code}\n",
            log.display()
        ),
    );
}

/// One scratch world: a repository with the fixture store's one story `proposed`, copies of
/// takt's gates document, project file and `gate-exec` (those takt has), and an `origin` on
/// github.com; the forge's and the harness's doubles in `bin`, their state in `state`; the gate's
/// tools as stubs in `cargo-home/bin`, logging to `gate-log`; a `cargo` on the program path, the
/// test command's, logging to `test-log`; a home, a state home and a temporary directory of its
/// own.
struct World {
    _scratch: Scratch,
    outer: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// The world `name`; the gate's `cargo` stub exits 1 when `clippy_fails`.
fn world(name: &str, clippy_fails: bool) -> World {
    let (scratch, outer) = scratch(name);
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    // Copied when takt has them: on a base without them the delivery shows what it does then.
    for file in [GATES, PROJECT, GATE_EXEC] {
        let from = root().join(file);
        if from.exists() {
            let to = repo.join(file);
            fs::create_dir_all(to.parent().expect("a parent")).expect("a directory");
            fs::copy(&from, &to).expect("a copied file");
        }
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let state = outer.join("forge");
    fs::create_dir_all(&state).expect("a state directory");
    git(
        &outer,
        &["init", "-q", "-b", "main", "--bare", "remote.git"],
    );
    let bin = outer.join("stub-bin");
    doubles(&bin, &state, &outer.join("remote.git"));
    stub(&bin, "cargo", &outer.join("test-log"), 0);
    let tools = outer.join("cargo-home/bin");
    let log = outer.join("gate-log");
    stub(&tools, "ess", &log, 0);
    stub(&tools, "python3", &log, 0);
    executable(
        &tools,
        "cargo",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$(basename \"$0\") $*\" >> '{}'\n[ \"$1\" = clippy ] && exit {}\nexit 0\n",
            log.display(),
            i32::from(clippy_fails)
        ),
    );
    for dir in ["home", "xdg-state", "tmp"] {
        fs::create_dir_all(outer.join(dir)).expect("a directory");
    }
    World {
        _scratch: scratch,
        outer,
        repo,
        bin,
        state,
    }
}

/// The forge's doubles and the harness's in `bin`. `b10x-gates bot -- push … <remote> <spec>`
/// keeps the commit pushed per branch (`pushed-<branch, / as _>`) and pushes into the bare
/// `remote`; `scan-text` passes; `api --method POST` keeps the request as `pr-<k>.json` and
/// answers pull request `k`. `gh pr view` reports it open at the commit last pushed for its
/// head; `gh pr checks` one check, passing. `claude` appends a line to `work.txt` and answers
/// `implementation.edit`, takes `change.handover` once `work.txt` exists, and answers
/// `repository.merge` once an approval is offered.
fn doubles(bin: &Path, state: &Path, remote: &Path) {
    let state = state.display().to_string();
    let remote = remote.display().to_string();
    executable(
        bin,
        "b10x-gates",
        &format!(
            r#"#!/bin/sh
state='{state}'
remote='{remote}'
echo "$*" >> "$state/calls"
case "$1" in
  bot)
    shift 2
    verb="$1"; shift
    options=''
    while [ $# -gt 0 ]; do
      case "$1" in
        -*) options="$options $1"; shift ;;
        *) break ;;
      esac
    done
    spec="$2"
    case "$verb" in
      push)
        branch="${{spec#*:refs/heads/}}"; revision="${{spec%%:*}}"
        if [ "$branch" = "$spec" ]; then revision="refs/heads/$spec"; fi
        git rev-parse "$revision" > "$state/pushed-$(printf '%s' "$branch" | tr / _)" || exit 1
        env -u GIT_ALLOW_PROTOCOL git push $options "$remote" "$spec" 2>/dev/null; exit $? ;;
    esac ;;
  scan-text) exit 0 ;;
  api)
    method=''; input=''; output=''
    while [ $# -gt 0 ]; do
      case "$1" in
        --method) method="$2"; shift ;;
        --input) input="$2"; shift ;;
        --output) output="$2"; shift ;;
      esac
      shift
    done
    if [ "$method" = POST ]; then
      k=$(( $(ls "$state" | grep -c '^pr-.*\.json$') + 1 ))
      cp "$input" "$state/pr-$k.json"
      sed -n 's/.*"head": "\(.*\)".*/\1/p' "$input" > "$state/pr-$k.head"
      printf '{{"number": %s}}' "$k" > "$output"
      exit 0
    fi ;;
esac
echo "b10x-gates double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        bin,
        "gh",
        &format!(
            r#"#!/bin/sh
state='{state}'
case "$2" in
  view)
    head=$(cat "$state/pr-$3.head")
    oid=$(tr -d '\n' < "$state/pushed-$(printf '%s' "$head" | tr / _)")
    printf '{{"state":"OPEN","mergedAt":null,"headRefName":"%s","headRefOid":"%s","baseRefName":"main"}}' "$head" "$oid"
    exit 0 ;;
  checks) printf '[{{"bucket":"pass"}}]'; exit 0 ;;
esac
echo "gh double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        bin,
        "claude",
        r#"#!/bin/sh
case "$2" in
  *"These need an approval"*) action=repository.merge ;;
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else echo "the work" >> work.txt; action=implementation.edit; fi ;;
  *) echo "the work" >> work.txt; action=implementation.edit ;;
esac
printf '{"result":"{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}}' "$action"
"#,
    );
}

/// The protocol sources cache `aep` reads in this process's own environment: with the scratch
/// home alone, `aep` would fetch the fixture store's protocol source again.
fn aep_cache() -> PathBuf {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    var("AEP_CACHE_DIR").map_or_else(
        || {
            var("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| var("HOME").map(|home| Path::new(&home).join(".cache")))
                .expect("a cache directory")
                .join("aep")
        },
        PathBuf::from,
    )
}

impl World {
    /// `takt run` on the repository's one story with the claude double and `extra`, the test
    /// command takt's: the doubles first on the program path, the scratch Cargo home, home,
    /// state home and temporary directory, no `b10x-gates` policy. The exit code, stdout and
    /// stderr.
    fn deliver(&self, extra: &[&str]) -> (i32, String, String) {
        let mut args = vec![
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--harness",
            "claude",
            "--test-cmd",
            TEST_COMMAND,
        ];
        args.extend_from_slice(extra);
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(&args)
            .env_remove("B10X_GATES_POLICY")
            .env_remove("CARGO_TARGET_DIR")
            .env("HOME", self.outer.join("home"))
            .env("AEP_CACHE_DIR", aep_cache())
            .env("CARGO_HOME", self.outer.join("cargo-home"))
            .env("XDG_STATE_HOME", self.outer.join("xdg-state"))
            .env("TMPDIR", self.outer.join("tmp"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The lines of the log `name` in the scratch world.
    fn log(&self, name: &str) -> Vec<String> {
        fs::read_to_string(self.outer.join(name))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The pull requests the forge double was asked to open.
    fn pull_requests(&self) -> usize {
        fs::read_dir(&self.state)
            .expect("the forge state")
            .filter(|entry| {
                let name = entry.as_ref().expect("an entry").file_name();
                let name = name.to_string_lossy();
                name.starts_with("pr-") && name.ends_with(".json")
            })
            .count()
    }
}

/// The steps of a run's stdout, `step <n>: <action> …`, each with the evidence lines it wrote.
fn steps(out: &str) -> Vec<(String, Vec<String>)> {
    let mut steps: Vec<(String, Vec<String>)> = Vec::new();
    for line in out.lines() {
        if let Some(evidence) = line.strip_prefix("  evidence: ") {
            if let Some(last) = steps.last_mut() {
                last.1.push(evidence.to_owned());
            }
        } else if let Some((n, rest)) = line
            .strip_prefix("step ")
            .and_then(|rest| rest.split_once(": "))
            && !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
        {
            let action = rest.split_whitespace().next().unwrap_or_default();
            steps.push((action.to_owned(), Vec::new()));
        }
    }
    steps
}

/// The story's acceptance: a delivery on a repository carrying takt's gate offers `gate.run`
/// after the implementation, the gate passes with each of the four commands run once a gate run
/// through `gate-exec` from the Cargo bin directory, the tests run as takt's test command, and
/// the run reaches `pr.open`.
#[test]
fn a_delivery_on_takts_gate_runs_it_after_the_implementation_and_opens_the_pull_request() {
    let world = world("passes", false);
    let (code, out, err) = world.deliver(&[]);
    let steps = steps(&out);
    let actions: Vec<&str> = steps.iter().map(|(action, _)| action.as_str()).collect();
    let edit = actions
        .iter()
        .rposition(|action| *action == "implementation.edit")
        .unwrap_or_else(|| panic!("the implementation: exit {code} stdout {out} stderr {err}"));
    let open = actions
        .iter()
        .position(|action| *action == "pr.open")
        .unwrap_or_else(|| panic!("pr.open is reached: exit {code} stdout {out} stderr {err}"));
    assert!(
        edit < open && actions[edit..open].contains(&"gate.run"),
        "gate.run is offered after the implementation and before the pull request: {actions:?}"
    );
    let gates: Vec<&Vec<String>> = steps
        .iter()
        .filter(|(action, _)| action == "gate.run")
        .map(|(_, evidence)| evidence)
        .collect();
    assert!(
        gates
            .iter()
            .all(|evidence| *evidence == &["gate_result pass"]),
        "every gate run passes: stdout {out} stderr {err}"
    );
    assert_eq!(
        steps[open].1,
        ["pull_request open"],
        "stdout {out} stderr {err}"
    );
    assert_eq!(world.pull_requests(), 1, "stdout {out} stderr {err}");
    // The library runs a document's commands in its plan's order, not the document's; each
    // gate run runs every one of the four once.
    let mut ran = world.log("gate-log");
    ran.sort();
    let mut expected: Vec<String> = gates.iter().flat_map(|_| gate_commands()).collect();
    expected.sort();
    assert_eq!(ran, expected, "stdout {out}");
    let tests = steps
        .iter()
        .filter(|(action, _)| action == "tests.run")
        .count();
    assert_eq!(
        world.log("test-log"),
        vec![TEST_COMMAND.to_owned(); tests],
        "stdout {out}"
    );
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
}

/// One gate command failing: the gate's record is `fail`, `gate.green` does not hold, and no
/// pull request is opened within the run's budget.
#[test]
fn a_failing_gate_command_holds_the_gate_red_and_opens_no_pull_request() {
    let world = world("fails", true);
    let (code, out, err) = world.deliver(&["--iterations", "8"]);
    let steps = steps(&out);
    let gates: Vec<&Vec<String>> = steps
        .iter()
        .filter(|(action, _)| action == "gate.run")
        .map(|(_, evidence)| evidence)
        .collect();
    assert!(
        !gates.is_empty()
            && gates
                .iter()
                .all(|evidence| *evidence == &["gate_result fail"]),
        "every gate run is a failure: exit {code} stdout {out} stderr {err}"
    );
    assert!(
        steps.iter().all(|(action, _)| action != "pr.open"),
        "no pull request is opened: stdout {out} stderr {err}"
    );
    assert_eq!(world.pull_requests(), 0, "stdout {out} stderr {err}");
    let mut ran = world.log("gate-log");
    ran.sort();
    let mut expected: Vec<String> = gates.iter().flat_map(|_| gate_commands()).collect();
    expected.sort();
    assert_eq!(
        ran, expected,
        "each gate run ran its four commands: stdout {out}"
    );
    assert!(
        !out.contains("stopped: Completed") && !out.contains("ApprovalRequired"),
        "stdout {out} stderr {err}"
    );
}

/// Runs a copy of `gate-exec` at `<repo>/scripts/gate-exec` with `args`, the environment
/// cleared as the gate clears it and `cargo_home` its Cargo home. The exit code and stderr.
fn gate_exec(repo: &Path, cargo_home: &Path, args: &[&str]) -> (i32, String) {
    let script = repo.join(GATE_EXEC);
    fs::create_dir_all(script.parent().expect("a parent")).expect("a directory");
    fs::copy(root().join(GATE_EXEC), &script).expect("takt's gate-exec");
    let output = Command::new(&script)
        .args(args)
        .env_clear()
        .env("CARGO_HOME", cargo_home)
        .current_dir(repo)
        .output()
        .expect("gate-exec runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `gate-exec` runs the tool the Cargo bin directory has with its arguments and its exit code,
/// and refuses one that resolves inside the repository, directly or through a symlink, naming
/// the tool and the path, without running it.
#[test]
fn gate_exec_refuses_a_tool_that_resolves_inside_the_repository() {
    let (_scratch, outer) = scratch("gate-exec");
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    let log = outer.join("ran");

    let outside = outer.join("cargo-home");
    stub(&outside.join("bin"), "cargo", &log, 3);
    let (code, err) = gate_exec(&repo, &outside, &["cargo", "fmt", "--all", "--check"]);
    assert_eq!(code, 3, "the tool's exit code: stderr {err}");
    assert_eq!(
        fs::read_to_string(&log).expect("the tool ran"),
        "cargo fmt --all --check\n"
    );
    fs::remove_file(&log).expect("the log is cleared");

    let planted = repo.join("planted-home");
    stub(&planted.join("bin"), "cargo", &log, 0);
    let (code, err) = gate_exec(&repo, &planted, &["cargo", "fmt", "--all", "--check"]);
    let resolved = planted.join("bin/cargo");
    assert_ne!(code, 0, "a planted cargo is refused: stderr {err}");
    assert!(
        err.contains("cargo") && err.contains(&resolved.display().to_string()),
        "the refusal names the tool and where it resolves: {err}"
    );
    assert!(!log.exists(), "the planted cargo ran");

    let linked = outer.join("linked-home");
    fs::create_dir_all(linked.join("bin")).expect("a directory");
    std::os::unix::fs::symlink(&resolved, linked.join("bin/cargo")).expect("a symlink");
    let (code, err) = gate_exec(&repo, &linked, &["cargo", "fmt", "--all", "--check"]);
    assert_ne!(
        code, 0,
        "a cargo linked into the repository is refused: stderr {err}"
    );
    assert!(
        err.contains("cargo") && err.contains(&resolved.display().to_string()),
        "the refusal names the tool and where it resolves: {err}"
    );
    assert!(!log.exists(), "the linked cargo ran");
}
