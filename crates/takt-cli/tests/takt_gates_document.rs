//! story:takt-has-a-gates-document: takt's own repository names its gate. The gates document
//! binds the five fast commands of `Taskfile.yml`'s `check` task, each through
//! `scripts/gate-exec`, which finds the tool on the system directories, then the Cargo bin
//! directory, and refuses one that resolves inside the repository; the fifth, the workspace's
//! tests, is a delivery's test command. A delivery on a scratch repository carrying copies of
//! those three files offers `gate.run` after the implementation and reaches `pr.open`; a gate
//! command that fails holds `gate.green` false and opens no pull request. The tools are stubs
//! (see [`World`]), the forge and the harness doubles on the program path; nothing reaches a
//! network or a model, and takt's own tests are not run from here.
//!
//! story:gate-exec-is-readable-by-the-gate: `gate-exec` is a Python program, not a shell
//! script, so the command reader the step guard and the gate share takes it whole and compared
//! at the base instead of failing closed on its branches: a delivery changing a Python file, a
//! shell file or a program no gate command runs is judged by the gate, and one changing a script
//! a gate command runs still stops it, naming the script.

#[path = "../../takt/tests/support/stand_in_policy.rs"]
mod stand_in_policy;

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

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

/// The log the test command's `cargo` stub appends to, in the scratch world: beside the
/// repository, and on macOS in its git directory, which Seatbelt lets the test command write
/// besides its tree and its home, and which leaves the work tree clean.
const TEST_LOG: &str = if cfg!(target_os = "macos") {
    "repo/.git/test-log"
} else {
    "test-log"
};

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

/// The five commands the gate runs: the check task's but its test command.
fn gate_commands() -> Vec<String> {
    let mut commands = check_task();
    assert_eq!(
        commands.pop().as_deref(),
        Some(TEST_COMMAND),
        "the check task ends with the delivery's test command"
    );
    commands
}

/// The commands of `Taskfile.yml`'s task `name`, read now, in order, each a line.
fn task_commands(name: &str) -> Vec<String> {
    let text = fs::read_to_string(root().join("Taskfile.yml")).expect("Taskfile.yml");
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("Taskfile.yml parses");
    doc["tasks"][name]["cmds"]
        .as_sequence()
        .unwrap_or_else(|| panic!("the task {name} has commands"))
        .iter()
        .map(|cmd| {
            cmd.as_str()
                .unwrap_or_else(|| panic!("each command of {name} is a line"))
                .to_owned()
        })
        .collect()
}

/// story:the-cli-suite-fits-its-ci-budget: CI runs `task check` as three jobs of one matrix,
/// one per task, and the three tasks run the check task's commands: `check-static` the five
/// before the tests, word for word, and `test-cli` and `test-library` its test command split by
/// package, `b10x-takt-cli` and every other package of the workspace, which together are the
/// workspace's.
#[test]
fn ci_runs_the_check_tasks_commands_as_three_jobs() {
    let check = check_task();
    let (test, before) = check.split_last().expect("the check task has commands");
    assert_eq!(test, TEST_COMMAND);
    assert_eq!(task_commands("check-static"), before);
    assert_eq!(
        task_commands("test-cli"),
        ["cargo test --locked -p b10x-takt-cli"]
    );
    assert_eq!(
        task_commands("test-library"),
        ["cargo test --workspace --locked --exclude b10x-takt-cli"]
    );
    let text = fs::read_to_string(root().join(".github/workflows/check.yml")).expect("check.yml");
    let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("check.yml parses");
    let job = &workflow["jobs"]["check"];
    let parts: Vec<&str> = job["strategy"]["matrix"]["part"]
        .as_sequence()
        .expect("the check job's parts")
        .iter()
        .map(|part| part.as_str().expect("each part is a task name"))
        .collect();
    assert_eq!(parts, ["check-static", "test-cli", "test-library"]);
    assert_eq!(job["timeout-minutes"].as_u64(), Some(30));
    let last = job["steps"]
        .as_sequence()
        .and_then(|steps| steps.last())
        .expect("the check job's steps");
    assert_eq!(last["env"]["PART"].as_str(), Some("${{ matrix.part }}"));
    // The step may first close what its shell inherited from the runner; its last command
    // is the task.
    let run = last["run"].as_str().expect("the last step runs a script");
    assert_eq!(
        run.lines().map(str::trim).rfind(|line| !line.is_empty()),
        Some("task \"$PART\""),
        "{run}"
    );
}

/// takt's own gates document, read through the loader the gate validator evaluates it with:
/// it binds, in order, exactly the check task's commands but the tests, each as `gate-exec`
/// with the tool and its arguments, and those five and the test command are the check task.
#[test]
fn takts_gates_document_binds_the_check_tasks_commands_through_gate_exec() {
    let root = root();
    let prepared = canon_engineering_assertions::prepare(
        &root,
        Path::new(GATES),
        Some(Path::new(PROJECT)),
        Some(Path::new(".takt/cases")),
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
/// github.com; the forge's and the harness's doubles in `bin`, their state in `state`; a `cargo`
/// on the program path, the test command's, logging to `test-log`; a home, a state home and a
/// temporary directory of its own.
///
/// The gate's tools log what they were run with to `gate-log`. `gate-exec` looks in the system
/// directories first, which a test cannot change, so each tool is a stub wherever the system
/// directories hand it on: `ess` in the scratch Cargo bin (`cargo-home/bin`); `python3` is the
/// system's, running the repository's `scripts/check-citations` and `scripts/release-notes`,
/// stubs that also log the interpreter to `python-log`; `cargo` is the scratch Cargo bin's stub on a host with no cargo
/// in the system directories and the stub toolchain of the scratch rustup home (`rustup-home`)
/// where the system's cargo is rustup's proxy. A host whose system directories hold `ess` or a
/// cargo that is not rustup's runs the real one here. A `python3` planted in the scratch Cargo
/// bin logs to `planted-log` and must never run.
struct World {
    _scratch: Scratch,
    outer: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// The world `name`; the gate's `cargo` stub exits 1 when `clippy_fails`.
fn world(name: &str, clippy_fails: bool) -> World {
    world_editing(name, clippy_fails, "")
}

/// The world `name`, whose claude double runs the shell lines `edit` in the repository with
/// each implementation step, beside its line in `work.txt`.
fn world_editing(name: &str, clippy_fails: bool, edit: &str) -> World {
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
    let log = outer.join("gate-log");
    fs::create_dir_all(repo.join("scripts")).expect("a directory");
    for script in ["scripts/check-citations", "scripts/release-notes"] {
        fs::write(
            repo.join(script),
            format!(
                "import sys\n\
                 with open({log:?}, 'a') as log:\n    \
                 log.write(' '.join(['python3'] + (['-I'] if sys.flags.isolated else []) + sys.argv) + '\\n')\n\
                 with open({python:?}, 'a') as log:\n    \
                 log.write(sys.executable + '\\n')\n",
                log = log.display().to_string(),
                python = outer.join("python-log").display().to_string(),
            ),
        )
        .unwrap_or_else(|error| panic!("the stub {script} is written: {error}"));
    }
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
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
    doubles(&bin, &state, &outer.join("remote.git"), edit);
    stub(&bin, "cargo", &outer.join(TEST_LOG), 0);
    let tools = outer.join("cargo-home/bin");
    stub(&tools, "ess", &log, 0);
    stub(&tools, "python3", &outer.join("planted-log"), 0);
    let cargo = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$(basename \"$0\") $*\" >> '{}'\n[ \"$1\" = clippy ] && exit {}\nexit 0\n",
        log.display(),
        i32::from(clippy_fails)
    );
    executable(&tools, "cargo", &cargo);
    let rustup = outer.join("rustup-home");
    executable(&rustup.join("toolchains/stub/bin"), "cargo", &cargo);
    fs::write(
        rustup.join("settings.toml"),
        "version = \"12\"\ndefault_toolchain = \"stub\"\n",
    )
    .expect("the rustup settings are written");
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
/// answers pull request `k`; `api --method GET` reports it open at the commit last pushed for
/// its head, and one check run, passing. `claude` appends a line to `work.txt` and answers
/// `implementation.edit`, takes `change.handover` once `work.txt` exists, and answers
/// `repository.merge` once an approval is offered; with each line it adds, it runs `edit`.
fn doubles(bin: &Path, state: &Path, remote: &Path, edit: &str) {
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
    method=''; path=''; input=''; output=''
    while [ $# -gt 0 ]; do
      case "$1" in
        --method) method="$2"; shift ;;
        --path) path="$2"; shift ;;
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
    fi
    if [ "$method" = GET ]; then
      case "$path" in
        */pulls\?*) printf '[]' > "$output" ;;
        */check-runs*) printf '{{"total_count":1,"check_runs":[{{"status":"completed","conclusion":"success"}}]}}' > "$output" ;;
        */status*) printf '{{"state":"pending","total_count":0,"statuses":[]}}' > "$output" ;;
        *)
          k="${{path##*/}}"
          head=$(cat "$state/pr-$k.head")
          oid=$(tr -d '\n' < "$state/pushed-$(printf '%s' "$head" | tr / _)")
          printf '{{"state":"open","merged":false,"head":{{"ref":"%s","sha":"%s"}},"base":{{"ref":"main"}}}}' "$head" "$oid" > "$output" ;;
      esac
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
        "claude",
        &format!(
            r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
work() {{
  echo "the work" >> work.txt
  {edit}
}}
case "$2" in
  *"These need an approval"*) action=repository.merge ;;
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else work; action=implementation.edit; fi ;;
  *) work; action=implementation.edit ;;
esac
printf '{{"result":"{{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}}","usage":{{"input_tokens":1}}}}' "$action"
"#
        ),
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
        if !extra.contains(&"--protocol") {
            args.extend(["--protocol", "story.delivery/1"]);
        }
        args.extend_from_slice(extra);
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(&args)
            .env("B10X_GATES_POLICY", stand_in_policy::path())
            .env_remove("CARGO_TARGET_DIR")
            .env("HOME", self.outer.join("home"))
            .env("AEP_CACHE_DIR", aep_cache())
            .env("CARGO_HOME", self.outer.join("cargo-home"))
            .env("RUSTUP_HOME", self.outer.join("rustup-home"))
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

    /// The `python3` planted in the scratch Cargo bin never ran, and every interpreter the
    /// citations stub ran under is outside the scratch world: the system's.
    fn assert_no_planted_python(&self) {
        assert!(
            !self.outer.join("planted-log").exists(),
            "the python3 planted in the Cargo bin ran: {:?}",
            self.log("planted-log")
        );
        let interpreters = self.log("python-log");
        assert!(
            !interpreters.is_empty()
                && interpreters
                    .iter()
                    .all(|python| !Path::new(python).starts_with(&self.outer)),
            "the citations stub ran under the system's python3: {interpreters:?}"
        );
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
/// after the implementation, the gate passes with each of the five commands run once a gate run
/// through `gate-exec`, the tests run as takt's test command, and the run reaches `pr.open`
/// within 20 steps, so a gate that never passes ends the run, red, instead of looping.
#[test]
fn a_delivery_on_takts_gate_runs_it_after_the_implementation_and_opens_the_pull_request() {
    let world = world("passes", false);
    let (code, out, err) = world.deliver(&["--iterations", "20"]);
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
    // gate run runs every one of the five once.
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
        world.log(TEST_LOG),
        vec![TEST_COMMAND.to_owned(); tests],
        "stdout {out}"
    );
    world.assert_no_planted_python();
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
    // A failed `process.succeeded` command keeps only that it failed, so the gate runs it once
    // more to read what it printed: the failing clippy command runs twice a gate run.
    let mut expected: Vec<String> = gates
        .iter()
        .flat_map(|_| {
            let commands = gate_commands();
            let failed: Vec<String> = commands
                .iter()
                .filter(|command| command.starts_with("cargo clippy"))
                .cloned()
                .collect();
            commands.into_iter().chain(failed)
        })
        .collect();
    expected.sort();
    assert_eq!(
        ran, expected,
        "each gate run ran its five commands, and its failing one once more: stdout {out}"
    );
    world.assert_no_planted_python();
    assert!(
        !out.contains("stopped: Completed") && !out.contains("ApprovalRequired"),
        "stdout {out} stderr {err}"
    );
}

/// The steps of a run's stdout, `step <n>: <action> …`, each with the effect line it wrote
/// (`  effect: …`), in order.
fn step_effects(out: &str) -> Vec<(String, Option<String>)> {
    let mut steps: Vec<(String, Option<String>)> = Vec::new();
    for line in out.lines() {
        if let Some(effect) = line.strip_prefix("  effect: ") {
            if let Some(last) = steps.last_mut() {
                last.1 = Some(effect.to_owned());
            }
        } else if let Some((n, rest)) = line
            .strip_prefix("step ")
            .and_then(|rest| rest.split_once(": "))
            && !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
        {
            let action = rest.split_whitespace().next().unwrap_or_default();
            steps.push((action.to_owned(), None));
        }
    }
    steps
}

/// The effect lines of a run's stdout for the steps of `action`, in order.
fn effects(out: &str, action: &str) -> Vec<String> {
    step_effects(out)
        .into_iter()
        .filter(|(step, _)| step == action)
        .filter_map(|(_, effect)| effect)
        .collect()
}

/// The paths the delivery's commits changed, on any branch of the scratch repository.
fn delivered(world: &World) -> Vec<String> {
    git(
        &world.repo,
        &["log", "--all", "--format=", "--name-only", "main.."],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

/// story:gate-exec-is-readable-by-the-gate: the command reader the step guard and the gate share
/// reads every command of takt's gates document with certainty, so a delivery that changes a
/// Python file, a shell file and a program no gate command runs is judged by its gate: each gate
/// run's summary names no fail-closed reason, the gate runs its five commands and passes, and the
/// run reaches `pr.open`.
#[test]
fn a_delivery_changing_an_unrelated_python_file_is_judged_by_takts_gate() {
    let world = world_editing(
        "unrelated-python",
        false,
        "mkdir -p tools && printf 'X = 1\\n' >> tools/unrelated.py && printf 'echo x\\n' >> tools/unrelated.sh && printf '#!/bin/sh\\n' > tools/run && chmod +x tools/run",
    );
    let (code, out, err) = world.deliver(&["--iterations", "20"]);
    let changed = delivered(&world);
    for path in ["tools/unrelated.py", "tools/unrelated.sh", "tools/run"] {
        assert!(
            changed.iter().any(|changed| changed == path),
            "the delivery changes {path}: {changed:?}; stdout {out} stderr {err}"
        );
    }
    let gate_effects = effects(&out, "gate.run");
    assert!(
        !gate_effects.is_empty(),
        "the gate runs: exit {code} stdout {out} stderr {err}"
    );
    for effect in &gate_effects {
        assert!(
            !effect.contains("fail closed") && !effect.contains("not run"),
            "no gate line of takt's fails closed, and the gate runs: {effect}"
        );
    }
    let steps = steps(&out);
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
    let mut ran = world.log("gate-log");
    ran.sort();
    let mut expected: Vec<String> = gates.iter().flat_map(|_| gate_commands()).collect();
    expected.sort();
    assert_eq!(ran, expected, "stdout {out}");
    assert!(
        steps.iter().any(|(action, _)| action == "pr.open"),
        "pr.open is reached: stdout {out} stderr {err}"
    );
    world.assert_no_planted_python();
}

/// A delivery that changes a script a gate command runs, `gate-exec` itself or a check it hands
/// to `python3` (the citations or the changelog check), still stops the gate: from the
/// implementation on, nothing is run, and each gate run's summary names the script and no
/// fail-closed reason.
#[test]
fn a_delivery_changing_a_script_a_gate_runs_stops_the_gate_naming_it() {
    for (name, script) in [
        ("gate-exec-changed", GATE_EXEC),
        ("citations-changed", "scripts/check-citations"),
        ("release-notes-changed", "scripts/release-notes"),
    ] {
        let world = world_editing(name, false, &format!("printf '# the step\\n' >> {script}"));
        let (code, out, err) = world.deliver(&["--iterations", "8"]);
        assert!(
            delivered(&world).iter().any(|changed| changed == script),
            "the delivery changes {script}: exit {code} stdout {out} stderr {err}"
        );
        let steps = step_effects(&out);
        let edit = steps
            .iter()
            .position(|(action, _)| action == "implementation.edit")
            .unwrap_or_else(|| panic!("the implementation: stdout {out} stderr {err}"));
        let gate_runs = |steps: &[(String, Option<String>)]| -> Vec<String> {
            steps
                .iter()
                .filter(|(action, _)| action == "gate.run")
                .map(|(_, effect)| effect.clone().unwrap_or_default())
                .collect()
        };
        let after = gate_runs(&steps[edit..]);
        assert!(
            !after.is_empty()
                && after.iter().all(|effect| {
                    effect.contains("not run")
                        && effect.contains(script)
                        && !effect.contains("fail closed")
                }),
            "every gate run after the implementation stops, naming {script} and no fail-closed reason: stdout {out} stderr {err}"
        );
        // Only the gate runs before the implementation ran the gate's commands.
        let before = gate_runs(&steps[..edit]).len();
        assert_eq!(
            world.log("gate-log").len(),
            before * gate_commands().len(),
            "no gate command ran after the implementation: stdout {out}"
        );
        assert!(
            !out.lines().any(|line| line.contains(": pr.open")),
            "no pull request is opened: stdout {out}"
        );
        assert_eq!(world.pull_requests(), 0, "stdout {out} stderr {err}");
    }
}

/// Runs a copy of `gate-exec` at `<repo>/scripts/gate-exec` with `args`, the environment
/// cleared as the gate clears it and `cargo_home` its Cargo home. The exit code, stdout and
/// stderr.
fn gate_exec(repo: &Path, cargo_home: &Path, args: &[&str]) -> (i32, String, String) {
    let script = repo.join(GATE_EXEC);
    fs::create_dir_all(script.parent().expect("a parent")).expect("a directory");
    fs::copy(root().join(GATE_EXEC), &script).expect("takt's gate-exec");
    // A script just written can be busy for a moment: a child another test thread forked
    // holds its write descriptor until that child execs.
    let mut tries = 0;
    let output = loop {
        match Command::new(&script)
            .args(args)
            .env_clear()
            .env("CARGO_HOME", cargo_home)
            .current_dir(repo)
            .output()
        {
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < 100 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            output => break output.expect("gate-exec runs"),
        }
    };
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The directories `gate-exec` searches before the Cargo bin, in its order.
const SYSTEM: [&str; 3] = ["/usr/local/bin", "/usr/bin", "/bin"];

/// A tool name no system directory has, so the Cargo bin decides where it resolves.
const PROBE: &str = "takt-gate-exec-probe";

/// `gate-exec` runs a tool only the Cargo bin directory has with its arguments and its exit
/// code, and refuses one that resolves inside the repository, directly or through a symlink,
/// naming the tool and the path, without running it.
#[test]
fn gate_exec_refuses_a_tool_that_resolves_inside_the_repository() {
    assert!(
        SYSTEM
            .iter()
            .all(|dir| !Path::new(dir).join(PROBE).exists()),
        "a system directory has {PROBE}"
    );
    let (_scratch, outer) = scratch("gate-exec");
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    let log = outer.join("ran");

    let outside = outer.join("cargo-home");
    stub(&outside.join("bin"), PROBE, &log, 3);
    let (code, _, err) = gate_exec(&repo, &outside, &[PROBE, "fmt", "--all", "--check"]);
    assert_eq!(code, 3, "the tool's exit code: stderr {err}");
    assert_eq!(
        fs::read_to_string(&log).expect("the tool ran"),
        format!("{PROBE} fmt --all --check\n")
    );
    fs::remove_file(&log).expect("the log is cleared");

    let planted = repo.join("planted-home");
    stub(&planted.join("bin"), PROBE, &log, 0);
    let (code, _, err) = gate_exec(&repo, &planted, &[PROBE, "fmt", "--all", "--check"]);
    let resolved = planted.join("bin").join(PROBE);
    assert_ne!(code, 0, "a planted tool is refused: stderr {err}");
    assert!(
        err.contains(PROBE) && err.contains(&resolved.display().to_string()),
        "the refusal names the tool and where it resolves: {err}"
    );
    assert!(!log.exists(), "the planted tool ran");

    let linked = outer.join("linked-home");
    fs::create_dir_all(linked.join("bin")).expect("a directory");
    std::os::unix::fs::symlink(&resolved, linked.join("bin").join(PROBE)).expect("a symlink");
    let (code, _, err) = gate_exec(&repo, &linked, &[PROBE, "fmt", "--all", "--check"]);
    assert_ne!(
        code, 0,
        "a tool linked into the repository is refused: stderr {err}"
    );
    assert!(
        err.contains(PROBE) && err.contains(&resolved.display().to_string()),
        "the refusal names the tool and where it resolves: {err}"
    );
    assert!(!log.exists(), "the linked tool ran");
}

/// The system directories come before the Cargo bin: a `python3` planted in the user-writable
/// Cargo bin is not the one `gate-exec` runs; the system's is.
#[test]
fn gate_exec_runs_the_systems_python3_over_one_planted_in_the_cargo_bin() {
    assert!(
        SYSTEM
            .iter()
            .any(|dir| Path::new(dir).join("python3").exists()),
        "no system directory has python3"
    );
    let (_scratch, outer) = scratch("gate-exec-python");
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    let planted = outer.join("cargo-home");
    let log = outer.join("planted-ran");
    stub(&planted.join("bin"), "python3", &log, 0);
    let (code, out, err) = gate_exec(
        &repo,
        &planted,
        &["python3", "-I", "-c", "import sys; print(sys.executable)"],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        !log.exists(),
        "the planted python3 ran: {:?}",
        fs::read_to_string(&log)
    );
    let ran = Path::new(out.trim());
    assert!(
        SYSTEM.iter().any(|dir| ran.starts_with(dir)) && !ran.starts_with(&outer),
        "the system's python3 ran: {out}"
    );
}
