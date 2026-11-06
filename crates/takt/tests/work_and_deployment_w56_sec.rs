//! Wave 56 security review: integrity checks of the `command` work source, the work item key
//! and the run file's `deployment_paths` against the contracts the wave states for them
//! (story:the-command-work-source, story:work-flag-and-the-aep-source,
//! story:git-excluded-files-are-not-step-changes, story:git-reads-no-user-excludes). Every
//! repository is a scratch one under the crate's build directory; nothing is written outside it,
//! and no process but `git` is started.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::forge::{Exit, Runner};
use takt::frontier::Key;
use takt::git::{Checkpoint, DeploymentPaths};
use takt::work::command::{CommandSource, WorkCommands};

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

/// A scratch directory under the crate's build directory, removed when dropped.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A repository holding `README.md` and `src/lib.rs`, committed on `main`.
fn repository(name: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w56-sec-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("src")).expect("a directory");
    fs::write(repo.join("README.md"), "a repository\n").expect("a file");
    fs::write(repo.join("src/lib.rs"), "pub fn f() {}\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

fn append(path: &Path, text: &str) {
    use std::io::Write as _;
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .expect("an append");
}

/// A runner that records every argument list and prints the item of the key it was built
/// with; it starts no process.
struct Recording {
    calls: Rc<RefCell<Vec<Vec<String>>>>,
    item: String,
}

impl Runner for Recording {
    fn run(&self, _tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.calls.borrow_mut().push(argv.to_vec());
        Ok(Exit {
            code: Some(0),
            stdout: self.item.clone(),
            stderr: String::new(),
        })
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }
}

/// Invariant 3: `{id}` is substituted inside its own element and never split, and no shell is
/// involved; an id that starts with `-` must be impossible or harmless. With `read` written as
/// the run file reference's own tests write it (`["tracker", "show", "{id}"]`), the id is a
/// whole argument of its own, so one starting with `-` reaches the team's command line as an
/// option.
#[test]
fn an_id_starting_with_a_dash_never_reaches_the_work_command_as_an_option() {
    let scratch = repository("dash-id");
    let id = "--output=/etc/takt-w56-sec";
    let Ok(key) = Key::parse(&format!("command:{id}")) else {
        // Refused at the key: impossible, and so harmless.
        return;
    };
    let calls = Rc::new(RefCell::new(Vec::new()));
    let item = format!(
        r#"{{"format": "takt.work-item/1", "key": "command:{id}", "title": "T", "body": "B", "revision": "1"}}"#
    );
    let commands = WorkCommands {
        read: Some(["tracker", "show", "{id}"].map(str::to_owned).to_vec()),
        select: None,
        refused_exit: None,
    };
    let source = CommandSource::new(
        commands,
        scratch.root.join("state"),
        Box::new(Recording {
            calls: Rc::clone(&calls),
            item,
        }),
    )
    .expect("a source");
    let read = source.read(&scratch.repo, &key);
    let calls = calls.borrow();
    let options: Vec<&String> = calls
        .iter()
        .flat_map(|argv| argv.iter().skip(1))
        .filter(|argument| argument.as_str() == id)
        .collect();
    assert!(
        options.is_empty(),
        "the key `command:{id}` was taken (Key::parse) and its id reached `work_commands.read` as an argument of its own, which the team's command line reads as an option: argv {calls:?}; the read gave {:?}",
        read.map(|item| item.key.to_string())
    );
}

/// Invariant 4 (a commit trailer and a message): a key's id is refused when it holds a
/// character "which would not show as itself in a trailer or a message"
/// (`crates/takt/src/frontier.rs`, `Key::parse`). A bidirectional override or a zero-width
/// space does not show as itself either; the case id rule (`check_id`) refuses both, the key
/// takes them, and the key is the step commit's `Refs:` trailer for a `command` item.
#[test]
fn a_key_whose_id_does_not_show_as_itself_is_refused() {
    let mut taken = Vec::new();
    for (name, id) in [
        ("U+202E RIGHT-TO-LEFT OVERRIDE", "X-1\u{202E}2-Y"),
        ("U+200B ZERO WIDTH SPACE", "X-1\u{200B}"),
        ("U+2066 LEFT-TO-RIGHT ISOLATE", "X\u{2066}-1"),
    ] {
        if let Ok(key) = Key::parse(&format!("command:{id}")) {
            taken.push(format!(
                "{name}: taken, trailer `Refs: {}` ({:?})",
                takt::commit_style::reference(&key),
                takt::commit_style::reference(&key)
            ));
        }
    }
    assert!(
        taken.is_empty(),
        "Key::parse takes ids that do not show as themselves:\n{}",
        taken.join("\n")
    );
}

/// Invariant 4 (a path), cleared: a case id minted from any key `Key::parse` takes passes the
/// case id rule, holds no `..` and no separator, its slug at most 48 characters, and the
/// trailer the key gives holds no line break. A fixed-seed generator over ids of separators,
/// dots, pattern characters, non-ASCII and format characters.
#[test]
fn every_minted_case_id_passes_the_case_id_rule() {
    const PALETTE: [char; 26] = [
        'a', 'Z', '0', '9', '-', '.', '/', '\\', '#', ':', '_', '@', '?', '*', '[', ']', '{', '}',
        '~', '^', 'é', 'ß', '\u{202E}', '\u{200B}', '\u{FEFF}', '%',
    ];
    const SOURCES: [&str; 4] = ["jira", "github", "gitlab", "command"];
    let mut seed: u64 = 0x5eed_0056;
    let mut next = |bound: usize| -> usize {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) as usize) % bound
    };
    let mut checked = 0;
    let mut wrong = Vec::new();
    for _ in 0..4000 {
        let source = SOURCES[next(SOURCES.len())];
        let connection = match next(3) {
            0 => format!("@{}", ["team-a", "a.b", "x_1"][next(3)]),
            _ => String::new(),
        };
        let length = 1 + next(80);
        let id: String = (0..length).map(|_| PALETTE[next(PALETTE.len())]).collect();
        let text = format!("{source}{connection}:{id}");
        let Ok(key) = Key::parse(&text) else {
            continue;
        };
        checked += 1;
        let case = key.case_id_at("2026-11-06T12:00:00Z");
        if let Err(error) = takt::case::check_id(&case) {
            wrong.push(format!("{text:?}: {case:?}: {error:#}"));
        }
        if case.contains("..") || case.contains('/') || case.contains('\\') {
            wrong.push(format!("{text:?}: {case:?} holds a path part"));
        }
        let slug = case
            .strip_prefix(&format!("{source}-"))
            .and_then(|rest| rest.rsplitn(3, '-').nth(2))
            .unwrap_or_default();
        if slug.len() > takt::frontier::SLUG_MAX {
            wrong.push(format!("{text:?}: slug {slug:?} longer than 48"));
        }
        let reference = takt::commit_style::reference(&key);
        if reference.contains(['\n', '\r']) {
            wrong.push(format!(
                "{text:?}: trailer {reference:?} holds a line break"
            ));
        }
    }
    assert!(checked > 200, "only {checked} keys were taken");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Invariant 5 (a symlink): with `.autodev/` the deployment's, a step that turns `.autodev`
/// into a link to `src` is exempted as the deployment's path (the link is `?? .autodev`, which
/// the list covers), so the step guard counts nothing; the deployment's log written through it
/// during the next step lands in `src/`, which is that step's change and goes into its commit.
#[test]
fn a_link_the_step_puts_at_a_deployment_path_is_a_change() {
    let scratch = repository("link");
    let repo = &scratch.repo;
    append(
        &repo.join(".autodev/run-1.log"),
        "the wrapper, before the run\n",
    );
    let own = DeploymentPaths::new(vec![".autodev/".to_owned()]).expect("the list");
    let mut wrong = Vec::new();

    // Step 1: the deployment's directory becomes a link into the repository's sources.
    let before = Checkpoint::take_owning(repo, &[], &own).expect("a checkpoint");
    fs::remove_dir_all(repo.join(".autodev")).expect("remove");
    std::os::unix::fs::symlink("src", repo.join(".autodev")).expect("a link");
    let change = before.change(repo).expect("the change");
    if !change.paths.iter().any(|path| path == ".autodev") {
        wrong.push(format!(
            "step 1 put a link at .autodev (to src) and the guard counted none of it: paths {:?}, ignored_added {:?}",
            change.paths, change.ignored_added
        ));
    }
    // A link inside the deployment's directory to a path outside the tree.
    fs::remove_file(repo.join(".autodev")).expect("remove the link");
    fs::create_dir_all(repo.join(".autodev")).expect("the directory again");
    let before = Checkpoint::take_owning(repo, &[], &own).expect("a checkpoint");
    std::os::unix::fs::symlink(&scratch.root, repo.join(".autodev/out")).expect("a link");
    let change = before.change(repo).expect("the change");
    if !change.paths.iter().any(|path| path == ".autodev/out") {
        wrong.push(format!(
            "a step put a link at .autodev/out to a path outside the tree and the guard counted none of it: paths {:?}",
            change.paths
        ));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Invariant 5 (what an entry covers): an entry is a path, or a directory and everything under
/// it when it ends in `/` (`website/docs/reference/configuration.md`, `deployment_paths`;
/// `DeploymentPaths::covers`, "never a path that merely starts with the same characters").
/// `logs` names one path. A step that makes `logs/` a directory git ignores and writes into it
/// writes `logs/new.txt`, which is under no entry ending in `/`, and is still not counted.
#[test]
fn an_entry_without_a_slash_does_not_cover_what_a_step_writes_below_it() {
    let scratch = repository("file-entry");
    let repo = &scratch.repo;
    fs::write(repo.join(".gitignore"), "logs/\n").expect("the ignore file");
    git(repo, &["add", ".gitignore"]);
    git(repo, &["commit", "-q", "-m", "ignore logs"]);
    let own = DeploymentPaths::new(vec!["logs".to_owned()]).expect("the list");
    let before = Checkpoint::take_owning(repo, &[], &own).expect("a checkpoint");
    append(&repo.join("logs/new.txt"), "the step's\n");
    let change = before.change(repo).expect("the change");
    let counted = change
        .ignored_added
        .iter()
        .any(|path| path.to_string_lossy().starts_with("logs"))
        || change.paths.iter().any(|path| path.starts_with("logs"));
    assert!(
        counted,
        "the entry `logs` (no `/`) exempted everything under the ignored directory logs/: ignored_added {:?}, paths {:?}",
        change.ignored_added, change.paths
    );
}
