//! The `takt` binary's `--gates` flag at its edges: an absolute path to the document inside
//! the repository, a document outside the repository (absolute, and relative through `..`),
//! and a path that names a directory. Adversary pass 1 over story:gate-validator.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture protocol of the library crate that declares `gate_result` and `gate.green`.
const GATE_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gate-only/1.yaml"
);

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

/// A committed repository with a manifest and the gates document at `<scratch>/repo`; the
/// scratch directory around it holds what is outside the repository.
fn repository(name: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
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

/// `takt run` with no harness on the fixture protocol over `repo`, the gates document at
/// `gates`.
fn run_args<'a>(repo: &'a str, case: &'a str, gates: &'a str) -> Vec<&'a str> {
    vec![
        "run",
        "--repo",
        repo,
        "--intent",
        "pass the gate",
        "--case-id",
        case,
        "--harness",
        "none",
        "--protocol",
        GATE_PROTOCOL,
        "--test-cmd",
        "true",
        "--gates",
        gates,
    ]
}

#[test]
fn an_absolute_gates_path_inside_the_repository_evaluates_the_gate() {
    let (_outer, repo) = repository("absolute");
    let gates = repo.join(".engineering/gates.yaml");
    let (code, out, err) = takt(&run_args(
        repo.to_str().expect("utf-8"),
        "absolute",
        gates.to_str().expect("utf-8"),
    ));
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("evidence: gate_result pass"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
}

#[test]
fn a_gates_document_outside_the_repository_is_refused_and_never_passes() {
    let (outer, repo) = repository("outside");
    let outside = outer.join("gates.yaml");
    fs::write(&outside, GATES).expect("a file");
    let absolute = outside.to_str().expect("utf-8").to_owned();
    for (case, gates) in [
        ("outside-absolute", absolute.as_str()),
        ("outside-relative", "../gates.yaml"),
    ] {
        let (code, out, err) = takt(&run_args(repo.to_str().expect("utf-8"), case, gates));
        assert_ne!(code, 0, "--gates {gates}: stdout {out} stderr {err}");
        assert!(
            !out.contains("gate_result pass"),
            "--gates {gates}: stdout {out} stderr {err}"
        );
        assert!(
            err.contains("gate"),
            "--gates {gates}: stdout {out} stderr {err}"
        );
    }
}

/// An explicit `--gates` that is not a file, a directory or a missing path, relative or
/// absolute, is refused in one line before any case is opened, not silently left out.
#[test]
fn an_explicit_gates_path_that_is_not_a_file_is_refused_before_the_run() {
    let (_outer, repo) = repository("gates-not-a-file");
    let missing_absolute = repo.join("gates/nope.yaml");
    let missing_absolute = missing_absolute.to_str().expect("utf-8").to_owned();
    for (case, gates) in [
        ("gates-directory", ".engineering"),
        ("gates-missing", "gates/nope.yaml"),
        ("gates-missing-absolute", missing_absolute.as_str()),
    ] {
        let (code, out, err) = takt(&run_args(repo.to_str().expect("utf-8"), case, gates));
        assert_ne!(code, 0, "--gates {gates}: stdout {out} stderr {err}");
        assert!(
            err.contains("--gates") && err.contains("not a file"),
            "--gates {gates}: stdout {out} stderr {err}"
        );
        assert!(
            !out.contains("case "),
            "--gates {gates}: a case was opened: stdout {out} stderr {err}"
        );
        assert!(
            !repo.join(".takt/cases").exists(),
            "--gates {gates}: a case directory exists"
        );
    }
}

/// A committed repository at `<scratch>/repo` whose gate is a command, `check.sh` exiting 0,
/// admitted by an `engineering-assertions-project/1` file at `project`, relative to the
/// repository.
fn command_repository(name: &str, project: &str) -> (Scratch, PathBuf) {
    use std::os::unix::fs::PermissionsExt as _;
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(
        repo.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check passes.\n    assertions:\n      - 'process.succeeded(\"check\")'\n",
    )
    .expect("a file");
    fs::write(repo.join("check.sh"), "#!/bin/sh\nexit 0\n").expect("a file");
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o755))
        .expect("exec bit");
    let project = repo.join(project);
    fs::create_dir_all(project.parent().expect("a parent")).expect("a directory");
    fs::write(
        project,
        "format: engineering-assertions-project/1\ncommands:\n  check:\n    executable: check.sh\n",
    )
    .expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// `takt run` with no harness on the fixture protocol over `repo`, the project file at
/// `project`, the gates document at its default path.
fn project_args<'a>(repo: &'a str, case: &'a str, project: &'a str) -> Vec<&'a str> {
    vec![
        "run",
        "--repo",
        repo,
        "--intent",
        "pass the gate",
        "--case-id",
        case,
        "--harness",
        "none",
        "--protocol",
        GATE_PROTOCOL,
        "--test-cmd",
        "true",
        "--project",
        project,
    ]
}

#[test]
fn the_project_flag_admits_the_gates_commands() {
    let (_outer, repo) = command_repository("project", "gates/project.yaml");
    let (code, out, err) = takt(&project_args(
        repo.to_str().expect("utf-8"),
        "project",
        "gates/project.yaml",
    ));
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("evidence: gate_result pass"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
}

#[test]
fn a_project_file_outside_the_repository_is_refused_before_the_run() {
    let (outer, repo) = command_repository("project-outside", "gates/project.yaml");
    let outside = outer.join("project.yaml");
    fs::copy(repo.join("gates/project.yaml"), &outside).expect("a copy");
    let absolute = outside.to_str().expect("utf-8").to_owned();
    for (case, project) in [
        ("project-absolute", absolute.as_str()),
        ("project-relative", "../project.yaml"),
    ] {
        let (code, out, err) = takt(&project_args(repo.to_str().expect("utf-8"), case, project));
        assert_ne!(code, 0, "--project {project}: stdout {out} stderr {err}");
        assert!(
            err.contains("--project") && err.contains("not under the repository"),
            "--project {project}: stdout {out} stderr {err}"
        );
        assert!(
            !out.contains("case "),
            "--project {project}: a case was opened: stdout {out} stderr {err}"
        );
        assert!(
            !repo.join(".takt/cases").exists(),
            "--project {project}: a case directory exists"
        );
    }
}

/// An explicit `--project` naming a directory is refused like a missing one: the admission of
/// the gate's commands is a file, or the run does not start.
#[test]
fn an_explicit_project_path_that_names_a_directory_is_refused_before_the_run() {
    let (_outer, repo) = command_repository("project-directory", "gates/project.yaml");
    let (code, out, err) = takt(&project_args(
        repo.to_str().expect("utf-8"),
        "project-directory",
        "gates",
    ));
    assert_ne!(code, 0, "stdout {out} stderr {err}");
    assert!(
        err.contains("--project gates: not a file"),
        "stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("case "),
        "a case was opened: stdout {out} stderr {err}"
    );
    assert!(
        !repo.join(".takt/cases").exists(),
        "a case directory exists"
    );
}

// Adversary pass 2 over story:gate-validator: `under_repository` at the edges the correction
// did not test, and the project file the validator falls back to. Every helper below is this
// pass's own; nothing above is changed.

#[test]
fn a_relative_gates_path_through_a_symlink_outside_the_repository_is_refused_before_the_run() {
    // The absolute form of this path is refused at parse (it canonicalises outside); the
    // relative form must be refused the same way, before any case is opened, not registered
    // and then aborted by the library at the first observation.
    let (outer, repo) = repository("symlink");
    let outside = outer.join("gates.yaml");
    fs::write(&outside, GATES).expect("a file");
    fs::create_dir_all(repo.join("gates")).expect("a directory");
    std::os::unix::fs::symlink(&outside, repo.join("gates/link.yaml")).expect("a symlink");
    let (code, out, err) = takt(&run_args(
        repo.to_str().expect("utf-8"),
        "symlink",
        "gates/link.yaml",
    ));
    assert_ne!(code, 0, "stdout {out} stderr {err}");
    assert!(
        err.contains("--gates") && err.contains("not under the repository"),
        "stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("case "),
        "a case was opened: stdout {out} stderr {err}"
    );
    assert!(
        !repo.join(".takt/cases").exists(),
        "a case directory exists: stdout {out} stderr {err}"
    );
}

#[test]
fn an_explicit_project_path_that_does_not_exist_is_refused_not_replaced_by_the_default() {
    // The project file is the admission of the commands the gate may run. A `--project` the
    // user named and that is not there must be refused, not silently replaced by whatever is
    // at the default path.
    let (_outer, repo) =
        command_repository("project-missing", ".engineering/assertions-project.yaml");
    let (code, out, err) = takt(&project_args(
        repo.to_str().expect("utf-8"),
        "project-missing",
        "gates/nope.yaml",
    ));
    assert_ne!(
        code, 0,
        "--project gates/nope.yaml does not exist and the run went on: stdout {out} stderr {err}"
    );
    assert!(
        err.contains("--project") && err.contains("gates/nope.yaml"),
        "stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("evidence: gate_result pass"),
        "the gate passed on a project file the flag did not name: stdout {out} stderr {err}"
    );
    assert!(
        !repo.join(".takt/cases").exists(),
        "a case directory exists: stdout {out} stderr {err}"
    );
}
