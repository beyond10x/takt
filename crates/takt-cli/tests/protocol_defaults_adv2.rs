//! Adversary pass 2 on correction 1 of the `--protocol` and `--input` refusals for a resumed
//! case: `case_to_resume` in `main.rs` finds the case to check by listing the store once, and
//! `Run::open_next` finds the case it resumes by listing it again; and `takt run --help`, which
//! documents the `--protocol` refusal for a resumed case, against the `--input` one.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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

fn repository(fixture: &str, name: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "protocol-defaults-adv2-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join(fixture), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

fn takt(path: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
    command.args(args);
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

/// The `aep` the program path gives, by absolute path.
fn real_aep() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("a program path"))
        .map(|dir| dir.join("aep"))
        .find(|candidate| candidate.is_file())
        .expect("aep on the program path")
}

/// An `aep` written into `<dir>/stub-bin` that answers the first `plan artifact list` as the
/// store stood a moment before its story was taken (every `active` listed `proposed`), and every
/// later call as the real `aep` does: a store that changed between two reads of one run.
fn store_changing_between_lists(dir: &Path) -> PathBuf {
    let bin = dir.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let marker = bin.join("listed-once");
    let script = format!(
        "#!/bin/sh\nif [ \"$3\" = list ] && [ ! -e '{marker}' ]; then\n  : > '{marker}'\n  '{real}' \"$@\" | sed 's/\"status\": \"active\"/\"status\": \"proposed\"/g'\n  exit 0\nfi\nexec '{real}' \"$@\"\n",
        marker = marker.display(),
        real = real_aep().display(),
    );
    let path = bin.join("aep");
    fs::write(&path, script).expect("the stub is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    bin
}

/// The one case directory under `repo`, by name.
fn the_case(repo: &Path) -> String {
    let names: Vec<String> = fs::read_dir(repo.join(".takt/cases"))
        .expect("a cases directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    names[0].clone()
}

/// The open case of an active story runs `software.change/1`; a plain run gives
/// `--protocol story.delivery/1`. `case_to_resume` lists the store, and `Run::open_next` lists it
/// a second time to choose what it resumes. When the story becomes active between the two
/// lists, the check finds no case to resume and passes, `open_next` resumes the
/// `software.change/1` case, and the `--protocol` that names another protocol goes through
/// without a word: the check and the resume disagree about which case runs. The case is found
/// by the take committed on its branch, which no list of the store can hide, and checked before
/// anything is resumed: the refusal names it.
#[test]
fn adv2_a_store_that_changes_between_the_check_and_the_resume_lets_another_protocol_through() {
    let (outer, repo) = repository("one-ready", "toctou");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--protocol", "software.change/1"]);
    let (code, out, err) = takt(None, &args);
    assert_eq!(code, 3, "the setup run: stdout {out} stderr {err}");
    assert!(out.contains("on software.change/1"), "{out}");
    let case = the_case(&repo);
    git(&repo, &["checkout", "-q", "main"]);
    let bin = store_changing_between_lists(&outer.0);
    let mut args = no_steps(path);
    args.extend(["--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(Some(&bin), &args);
    assert!(
        err.contains(&format!(
            "the case {case} runs on software.change/1, not story.delivery/1"
        )),
        "the check is on the case the run resumes: stdout {out} stderr {err}"
    );
    assert_eq!(
        (code, out.contains("frontier: resumed")),
        (1, false),
        "a --protocol other than the resumed case's must be refused for the case that is \
         resumed, not for the one a separate list found: stdout {out} stderr {err}"
    );
}

/// The same store, changing between two lists, for a case found only through the store's list:
/// its take is on `main`, as `git cherry-pick` and `git branch -f` leave it, and not on the
/// case's branch, a state takt does not support (coordinator decision B); what is pinned here is
/// only that a case no check saw never runs. The run lists the store once to choose what it
/// resumes: the first list shows no
/// story active, so nothing is resumed, and the later lists, which do show it, resume nothing
/// either. A case the check did not see never runs.
#[test]
fn adv2_a_case_found_only_through_the_store_is_resumed_only_from_the_list_that_was_checked() {
    let (outer, repo) = repository("one-ready", "toctou-list");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--protocol", "software.change/1"]);
    let (code, out, err) = takt(None, &args);
    assert_eq!(code, 3, "the setup run: stdout {out} stderr {err}");
    let case = the_case(&repo);
    let branch = format!("takt/{case}");
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["cherry-pick", &branch]);
    git(&repo, &["branch", "-f", &branch, "main~1"]);
    let bin = store_changing_between_lists(&outer.0);
    let mut args = no_steps(path);
    args.extend(["--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(Some(&bin), &args);
    assert!(
        bin.join("listed-once").exists(),
        "the stub answered the first list: stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("frontier: resumed") && !out.contains("stopped:"),
        "a case no check saw was resumed: exit {code} stdout {out} stderr {err}"
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(out.contains("frontier: nothing ready"), "{out}");
    assert_eq!(the_case(&repo), case, "no second case was opened");
}

/// `takt run --help` documents that a `--protocol` other than a resumed case's is refused; the
/// correction refuses every `--input` given for a resumed case as well. The `--input` paragraph
/// of the help says only that an input the opening protocol does not declare is refused, so an
/// operator reading it is not told that a resumed case refuses every input.
#[test]
fn adv2_run_help_says_an_input_for_a_resumed_case_is_refused() {
    let (code, out, err) = takt(None, &["run", "--help"]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let start = out.find("--input").expect("the --input paragraph");
    let rest = &out[start..];
    let end = rest[2..]
        .find("\n      --")
        .or_else(|| rest[2..].find("\n  -"))
        .map_or(rest.len(), |at| at + 2);
    let paragraph = &rest[..end];
    assert!(
        paragraph.contains("resumed"),
        "the --input help says nothing of a resumed case: {paragraph}"
    );
}
