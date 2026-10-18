//! The `takt` binary's defaults at its edges: the protocol a case opens on when `--protocol` is
//! not given, a `--protocol` that names another protocol than a resumed case runs, a forge that
//! refuses the pull request, the exit codes that tell the stops apart, and what `takt run
//! --help` says about them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::CaseDir;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The story-delivery protocol of this repository, the one the binary embeds.
const DELIVERY_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

/// A committed repository at `<scratch>/repo`: a copy of the fixture store `fixture` when one
/// is given, a manifest and a gates document whose one assertion is true. `name` keeps the
/// cases of this process apart.
fn repository(fixture: Option<&str>, name: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "protocol-defaults-cli-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    match fixture {
        Some(fixture) => copy_dir(&Path::new(FIXTURES).join(fixture), &repo),
        None => fs::create_dir_all(&repo).expect("a directory"),
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// Runs the built `takt` binary with `args`, `path` first on the program path when given: the
/// exit code, stdout and stderr.
fn takt_with_path(path: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
    command.args(args).env_remove("B10X_GATES_POLICY");
    if let Some(path) = path {
        command.env(
            "PATH",
            format!(
                "{}:{}",
                path.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
    }
    let output = command.output().expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn takt(args: &[&str]) -> (i32, String, String) {
    takt_with_path(None, args)
}

/// The status the store lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    takt::frontier::Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

/// The one case directory under `repo`, by name.
fn the_case(repo: &Path) -> String {
    let mut cases: Vec<String> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("a cases directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases.remove(0)
}

/// Every file under `dir` but `.git`, with its bytes, and the git state of the tree: what a
/// refused run must leave exactly as it was.
fn everything(dir: &Path) -> (BTreeMap<PathBuf, Vec<u8>>, String, String, String) {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).expect("under the root").to_owned();
                files.insert(relative, fs::read(&path).expect("a readable file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(dir, dir, &mut files);
    (
        files,
        git(dir, &["rev-parse", "HEAD"]),
        git(dir, &["symbolic-ref", "-q", "HEAD"]),
        git(dir, &["status", "--porcelain", "--untracked-files=all"]),
    )
}

/// The step budget of zero: the case is opened, its first line printed, nothing run.
fn no_steps(repo: &str) -> Vec<&str> {
    vec![
        "run",
        "--repo",
        repo,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "0",
    ]
}

/// A story the planning store gives, with no `--protocol`: its case opens on
/// `story.delivery/1`, the protocol that opens a pull request, embedded in the binary so the
/// target repository need not carry it.
#[test]
fn a_store_story_without_a_protocol_opens_on_story_delivery() {
    let (_outer, repo) = repository(Some("one-ready"), "store-default");
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&no_steps(path));
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    let case = the_case(&repo);
    assert!(
        out.contains(&format!("case {case} on story.delivery/1 in ")),
        "stdout {out} stderr {err}"
    );
    assert!(
        !repo.join("protocols").exists(),
        "the repository carries no protocol file"
    );
}

/// An input the protocol a case would open on does not declare: `story.delivery/1` declares
/// none, so `--input risk=…` on a store story, kept by nothing, is refused before the story is
/// taken, no case opened; on an intent, `software.change/1` declares `risk` and takes it.
#[test]
fn an_input_the_opening_protocol_does_not_declare_is_refused_before_anything_is_opened() {
    let (_outer, repo) = repository(Some("one-ready"), "undeclared-input");
    let path = repo.to_str().expect("utf-8");
    let before = everything(&repo);
    let mut args = no_steps(path);
    args.extend(["--input", "risk=high"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("story.delivery/1 declares no input risk"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(out, "", "{out}");
    assert_eq!(
        everything(&repo),
        before,
        "the refused run changed the tree"
    );
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
    let mut args = no_steps(path);
    args.extend([
        "--intent",
        "a change",
        "--case-id",
        "risky",
        "--input",
        "risk=elevated",
    ]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("case risky on software.change/1") && out.contains("risk=elevated"),
        "stdout {out} stderr {err}"
    );
}

/// An input given for a case that is resumed, not opened, is one the resume would drop: the
/// open case of an active story on `software.change/1`, which declares `risk`, given
/// `--input risk=elevated` by a run with no intent (with and without its own `--protocol`) or
/// with `--case`. Refused, naming the resumed case and not the protocol a new case would open
/// on, saying a resumed case keeps its inputs; the tree, back on `main`, left as it was.
#[test]
fn an_input_given_for_a_resumed_case_is_refused_touching_nothing() {
    let (_outer, repo) = repository(Some("one-ready"), "resumed-input");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--protocol", "software.change/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let case = the_case(&repo);
    git(&repo, &["checkout", "-q", "main"]);
    let before = everything(&repo);
    for extra in [
        vec![],
        vec!["--protocol", "software.change/1"],
        vec!["--case", case.as_str()],
    ] {
        let mut args = no_steps(path);
        args.extend(extra.iter().copied());
        args.extend(["--input", "risk=elevated"]);
        let (code, out, err) = takt(&args);
        assert_eq!(code, 1, "{extra:?}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!("the case {case} is resumed"))
                && err.contains("keeps the inputs it was opened with"),
            "{extra:?}: stdout {out} stderr {err}"
        );
        assert!(!err.contains("story.delivery/1"), "{extra:?}: {err}");
        assert!(!out.contains("stopped:"), "{extra:?}: {out}");
        assert_eq!(
            everything(&repo),
            before,
            "{extra:?}: the refused run changed the tree"
        );
    }
}

/// An intent, with no `--protocol`: the case opens on `software.change/1`, as before.
#[test]
fn an_intent_without_a_protocol_opens_on_software_change() {
    let (_outer, repo) = repository(Some("one-ready"), "intent-default");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--intent", "make the check pass", "--case-id", "given"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("case given on software.change/1 in "),
        "stdout {out} stderr {err}"
    );
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
}

/// `--case` with a `--protocol` other than the one the case runs: refused, exit non-zero, both
/// protocols named, and nothing in the tree or its git state changed.
#[test]
fn a_protocol_other_than_the_resumed_case_runs_is_refused_touching_nothing() {
    let (_outer, repo) = repository(None, "case-mismatch");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--intent", "a change", "--case-id", "change"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(out.contains("case change on software.change/1"), "{out}");
    let before = everything(&repo);
    for given in ["story.delivery/1", DELIVERY_PROTOCOL] {
        let mut args = no_steps(path);
        args.extend(["--case", "change", "--protocol", given]);
        let (code, out, err) = takt(&args);
        assert_ne!(code, 0, "--protocol {given}: stdout {out} stderr {err}");
        assert_ne!(code, 3, "--protocol {given}: stdout {out} stderr {err}");
        assert!(
            err.contains("software.change/1") && err.contains("story.delivery/1"),
            "--protocol {given}: the refusal names both protocols: stdout {out} stderr {err}"
        );
        assert!(
            !out.contains("stopped:"),
            "--protocol {given}: the run did not start: {out}"
        );
        assert_eq!(
            everything(&repo),
            before,
            "--protocol {given}: the refused run changed the tree"
        );
    }
}

/// The open case of an active story, resumed by a run with no intent: a `--protocol` other than
/// the case's is refused the same way, the story still active with its one case; with no
/// `--protocol`, or with the case's own in either spelling, the case resumes and says which
/// protocol it runs.
#[test]
fn the_store_resumed_case_keeps_its_protocol_and_refuses_another() {
    let (_outer, repo) = repository(Some("one-ready"), "store-resume");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--protocol", "software.change/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let case = the_case(&repo);
    assert!(
        out.contains(&format!("case {case} on software.change/1")),
        "{out}"
    );
    let before = everything(&repo);
    let mut args = no_steps(path);
    args.extend(["--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(&args);
    assert_ne!(code, 0, "stdout {out} stderr {err}");
    assert_ne!(code, 3, "stdout {out} stderr {err}");
    assert!(
        err.contains("software.change/1") && err.contains("story.delivery/1"),
        "the refusal names both protocols: stdout {out} stderr {err}"
    );
    assert!(!out.contains("stopped:"), "{out}");
    assert_eq!(
        everything(&repo),
        before,
        "the refused run changed the tree"
    );
    assert_eq!(status(&repo, "story:ready-one"), "active");
    for given in [None, Some("software.change/1"), Some("software-change/1")] {
        let mut args = no_steps(path);
        if let Some(given) = given {
            args.extend(["--protocol", given]);
        }
        let (code, out, err) = takt(&args);
        assert_eq!(code, 3, "--protocol {given:?}: stdout {out} stderr {err}");
        assert!(
            out.contains("frontier: resumed story:ready-one"),
            "--protocol {given:?}: {out}"
        );
        assert!(
            out.contains(&format!("case {case} on software.change/1")),
            "--protocol {given:?}: {out}"
        );
        assert_eq!(the_case(&repo), case, "no second case was opened");
    }
}

/// A `--protocol` file under the case's own id whose rules differ (here `pr.open` without the
/// gate) is another protocol: refused for the resumed case, nothing changed; the embedded one,
/// the same document the case was opened from, resumes it.
#[test]
fn another_document_under_the_case_protocol_id_is_refused() {
    let (outer, repo) = repository(None, "same-id");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend([
        "--intent",
        "deliver",
        "--case-id",
        "delivery",
        "--protocol",
        DELIVERY_PROTOCOL,
    ]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let original = fs::read_to_string(DELIVERY_PROTOCOL).expect("the protocol");
    assert_eq!(original.matches("        - claim: gate.green\n").count(), 1);
    let altered = outer.join("altered.yaml");
    fs::write(
        &altered,
        original.replace("        - claim: gate.green\n", ""),
    )
    .expect("a file");
    let before = everything(&repo);
    let mut args = no_steps(path);
    args.extend([
        "--case",
        "delivery",
        "--protocol",
        altered.to_str().expect("utf-8"),
    ]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("story.delivery/1") && err.contains("another document"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        everything(&repo),
        before,
        "the refused run changed the tree"
    );
    let mut args = no_steps(path);
    args.extend(["--case", "delivery", "--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(out.contains("case delivery on story.delivery/1"), "{out}");
}

/// A `b10x-gates` on the program path that pushes and scans without a word and refuses every
/// API write the way the forge refused the trial's pull request; every line it is given is
/// appended to `calls` beside it.
fn refusing_gates(dir: &Path) -> PathBuf {
    let bin = dir.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let script = r#"#!/bin/sh
printf '%s\n' "$*" >> "$(dirname "$0")/calls"
case "$1" in
  bot|scan-text) exit 0 ;;
  api)
    printf '%s\n' 'b10x-gates: GitHub operation failed (422 Unprocessable Entity): Validation Failed' >&2
    exit 1
    ;;
  *)
    printf 'unexpected: %s\n' "$*" >&2
    exit 2
    ;;
esac
"#;
    let path = bin.join("b10x-gates");
    fs::write(&path, script).expect("the stub is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    bin
}

/// A forge that refuses `open_pull_request`: the run ends with a `stopped: error:` line that
/// names the refusal, on stdout like every other stop, and exits with the code `takt run
/// --help` documents for it, which is none of the other stops' codes.
#[test]
fn a_forge_refusing_the_pull_request_stops_the_run_with_a_documented_code() {
    let (outer, repo) = repository(None, "forge-refusal");
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/repo.git",
        ],
    );
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--intent",
        "deliver the story",
        "--case-id",
        "refused",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    // The commit a harness would make on the case's branch, recorded as the implementation.
    fs::write(repo.join("feature.txt"), "a feature\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the feature"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let mut case = CaseDir::resume(&repo, "refused").expect("the case reads back");
    case.set_revision("implementation", &head)
        .expect("the revision is recorded");
    // The work handed over at its revision, the record takt writes for a `change.handover`
    // step: a `--harness none` run takes no judgement step, so the test writes it.
    takt::CaseDir::resume(&repo, "refused")
        .expect("the case reads back")
        .add_evidence(&takt::evidence::Record::new(
            "handover",
            "handover",
            None,
            "implementation",
            &head,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("the record is written");

    let bin = refusing_gates(&outer);
    let (code, out, err) = takt_with_path(
        Some(&bin),
        &[
            "run",
            "--repo",
            path,
            "--case",
            "refused",
            "--harness",
            "none",
            "--test-cmd",
            "true",
        ],
    );
    let calls = fs::read_to_string(bin.join("calls")).unwrap_or_default();
    assert!(
        out.contains("forge: bot (owner/repo)"),
        "stdout {out} stderr {err} calls {calls}"
    );
    assert!(
        calls
            .lines()
            .any(|line| line.starts_with("api --method POST")),
        "the pull request was asked for: stdout {out} stderr {err} calls {calls}"
    );
    let last = out.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("stopped: error: ") && last.contains("422 Unprocessable Entity"),
        "the last line is the stop naming the refusal: stdout {out} stderr {err}"
    );
    assert_eq!(out.matches("stopped:").count(), 1, "{out}");
    for other in [0, 1, 2, 3, 4] {
        assert_ne!(code, other, "stdout {out} stderr {err}");
    }
    let (_, help, _) = takt(&["run", "--help"]);
    assert!(
        help.lines()
            .any(|line| line.trim_start().starts_with(&format!("{code} "))
                && line.contains("stopped: error:")),
        "takt run --help documents exit {code}: {help}"
    );
}

/// `ApprovalRequired` and `Completed` exit with different codes, each documented in `takt run
/// --help`: a case with everything but the grant stops for the approval, one through the gate
/// completes.
#[test]
fn approval_required_and_completed_exit_with_different_documented_codes() {
    let (_outer, repo) = repository(None, "approval-code");
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--intent",
        "deliver the story",
        "--case-id",
        "gated",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let mut case = CaseDir::resume(&repo, "gated").expect("the case reads back");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result)) in [
        ("test_result", "pass"),
        ("gate_result", "pass"),
        ("pull_request", "open"),
        ("ci_result", "pass"),
    ]
    .iter()
    .enumerate()
    {
        case.add_evidence(&takt::evidence::Record::new(
            format!("e{n}"),
            *kind,
            Some((*result).to_owned()),
            "implementation",
            &revision,
            "protocol-defaults-cli-test",
        ))
        .expect("the record is written");
    }
    // The change handed over at the revision, as takt records a `change.handover` step.
    case.add_evidence(&takt::evidence::Record::new(
        "handover",
        "handover",
        None,
        "implementation",
        &revision,
        takt::run::CHOICE_PRODUCER,
    ))
    .expect("the record is written");
    let (approval, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--case",
        "gated",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
    ]);
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
    let (_outer2, gated) = repository(None, "completed-code");
    let (completed, out, err) = takt(&[
        "run",
        "--repo",
        gated.to_str().expect("utf-8"),
        "--intent",
        "pass the gate",
        "--harness",
        "none",
        "--protocol",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../takt/tests/fixtures/gate-only/1.yaml"
        ),
        "--test-cmd",
        "true",
    ]);
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(completed, 0, "stdout {out} stderr {err}");
    assert_ne!(approval, completed, "ApprovalRequired exits as Completed");
    assert_ne!(approval, 3, "ApprovalRequired exits as a budget stop");
    let (_, help, _) = takt(&["run", "--help"]);
    for (code, stop) in [(completed, "Completed"), (approval, "ApprovalRequired")] {
        assert!(
            help.lines()
                .any(|line| line.trim_start().starts_with(&format!("{code} "))
                    && line.contains(stop)),
            "takt run --help documents exit {code} for {stop}: {help}"
        );
    }
}

/// `takt run --help` says what the code does for `--forge` when it is not given: the bot route
/// only for an `origin` on github.com, else none.
#[test]
fn run_help_describes_the_forge_default_as_the_code_chooses_it() {
    let (code, help, err) = takt(&["run", "--help"]);
    assert_eq!(code, 0, "{err}");
    let forge: String = help
        .split("--forge <FORGE>")
        .nth(1)
        .expect("--forge is documented")
        .split("\n  -")
        .next()
        .expect("its text")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        forge.contains(
            "Not given, `bot` when the repository's `origin` is a repository on github.com"
        ),
        "{forge}"
    );
    assert!(
        !forge.contains("when the repository has an `origin` remote"),
        "{forge}"
    );
}

/// `takt protocols` lists the embedded `story.delivery/1` beside Canon's built-ins.
#[test]
fn protocols_lists_the_embedded_story_delivery() {
    let (code, out, err) = takt(&["protocols"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.lines().any(|line| line == "story.delivery/1"), "{out}");
    assert!(
        out.lines()
            .any(|line| line == "software-change/1" || line == "software.change/1"),
        "{out}"
    );
}
