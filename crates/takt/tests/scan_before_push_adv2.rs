//! Adversary pass 2 on the scan before a push (`BotRoute::scan_outgoing`): content a push
//! publishes that the scan does not read. Each case builds a tree whose push would publish a
//! literal the stand-in policy forbids, shows that the push publishes it where that is not
//! plain, and asserts what the story promises: the scan refuses, so nothing is pushed.
//!
//! A recording runner stands in for `b10x-gates scan-text`. It refuses a file holding
//! [`FORBIDDEN`] on one line, or once the line breaks the real scanner's wrapped rule spans
//! (`-?[ \t]*\r?\n[ \t>|*#]*`, between any two characters of the literal) are taken out.
//! Remotes are local bare repositories; nothing leaves the machine.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::forge::{BotRoute, Exit, Forge, Outgoing, Runner};

const FORBIDDEN: &str = "FORBIDDEN-7Q2X";

fn run_git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = run_git(dir, args);
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn bare(&self, name: &str) -> PathBuf {
        let bare = self.0.join(name);
        if !bare.exists() {
            fs::create_dir_all(&bare).expect("a bare dir");
            git(&bare, &["init", "-q", "-b", "main", "--bare"]);
        }
        bare
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("remove the scratch dir {}: {error}", self.0.display());
        }
    }
}

/// A scratch directory holding `tree`, a repository on `main` with one commit holding
/// `files`, and `origin.git`, a bare repository added as `origin` and holding nothing yet.
fn scratch(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scan-push-adv2-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch dir is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("scratch tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        fs::write(tree.join(path), text).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    let origin = scratch.bare("origin.git");
    git(
        &tree,
        &["remote", "add", "origin", origin.to_str().expect("utf-8")],
    );
    scratch
}

fn commit(tree: &Path, path: &str, text: &str, message: &str) -> String {
    fs::write(tree.join(path), text).expect("a file");
    git(tree, &["add", "-A", "--", path]);
    git(tree, &["commit", "-q", "-m", message]);
    git(tree, &["rev-parse", "HEAD"])
}

/// `text` with every line break the real scanner's wrapped rule spans taken out: blanks and
/// `\r` before it, blanks and `>|*#` after it; with `hyphen` a `-` before those blanks too.
fn unwrapped(text: &str, hyphen: bool) -> String {
    let mut out = String::new();
    for (n, line) in text.split('\n').enumerate() {
        let mut line = line;
        if n > 0 {
            line = line.trim_start_matches([' ', '\t', '>', '|', '*', '#']);
        }
        out.push_str(line);
        let trimmed = out.trim_end_matches([' ', '\t', '\r']).len();
        out.truncate(trimmed);
        if hyphen && out.ends_with('-') {
            out.pop();
        }
    }
    out
}

/// Whether the stand-in policy refuses `text`.
fn breaks(text: &str) -> bool {
    text.contains(FORBIDDEN)
        || unwrapped(text, false).contains(FORBIDDEN)
        || unwrapped(text, true).contains(FORBIDDEN)
}

/// Stands in for `b10x-gates`: `scan-text` refuses what [`breaks`]; every line is recorded
/// with the text of each file it scanned.
#[derive(Default)]
struct Gates {
    scanned: RefCell<Vec<String>>,
}

impl Runner for Gates {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        // The bot route's read of the repository's base: the same dry-run fetch, from the bare
        // repository `origin` names, which stands in for the repository on the forge.
        if let ["b10x-gates", "bot", "--", "fetch", _url, rest @ ..] = words.as_slice() {
            let origin = git(tree, &["remote", "get-url", "origin"]);
            let out = Command::new("git")
                .arg("fetch")
                .arg(&origin)
                .args(rest)
                .current_dir(tree)
                .output()?;
            return Ok(Exit {
                code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        let ["b10x-gates", "scan-text", rest @ ..] = words.as_slice() else {
            return Ok(Exit {
                code: Some(2),
                stdout: String::new(),
                stderr: format!("the double runs only scan-text, not {}", argv.join(" ")),
            });
        };
        let mut refused = false;
        let mut rest = rest.iter();
        while let Some(word) = rest.next() {
            if word.starts_with("--") {
                rest.next();
                continue;
            }
            let text = fs::read_to_string(word)
                .unwrap_or_else(|error| panic!("scan-text names {word}: {error}"));
            refused |= breaks(&text);
            self.scanned.borrow_mut().push(text);
        }
        Ok(Exit {
            code: Some(i32::from(refused)),
            stdout: String::new(),
            stderr: if refused {
                "1 private rule(s); private details remain local".to_owned()
            } else {
                String::new()
            },
        })
    }
}

struct Shared(Rc<Gates>);

impl Runner for Shared {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.run(tree, argv)
    }
}

/// What the bot route's scan before pushing `head` (forked from `base`) to `origin` answers,
/// and the text it handed to `scan-text`.
fn scan(scratch: &Scratch, base: &str, head: &str) -> (Result<(), String>, String) {
    let gates = Rc::new(Gates::default());
    let answer = {
        let route = BotRoute::new(
            "owner/repo",
            Some(PathBuf::from("/policies/gates.json")),
            Box::new(Shared(gates.clone())),
        );
        route
            .scan_outgoing(
                &scratch.tree(),
                &scratch.0.join("artifacts"),
                &Outgoing {
                    branch: "takt/case",
                    base,
                    head,
                    request: None,
                },
            )
            .map_err(|error| format!("{error:#}"))
    };
    let scanned = gates.scanned.borrow().join("\n=====\n");
    (answer, scanned)
}

/// The scan refused what the push would publish.
fn refused(answer: &Result<(), String>, scanned: &str, published: &str) {
    assert!(
        answer.is_err(),
        "the scan passed, so the push would publish the forbidden literal ({published}); \
         what the scan read:\n{scanned}"
    );
}

/// Pushes `head` of the tree to a fresh copy of `origin.git` and answers what that copy then
/// holds, `git log --patch` with messages, read without any replacement: what a push of `head`
/// to the remote publishes.
fn published_by_a_push(scratch: &Scratch, head: &str) -> String {
    let probe = scratch.0.join("probe.git");
    let origin = scratch.bare("origin.git");
    git(
        &scratch.0,
        &[
            "clone",
            "-q",
            "--bare",
            origin.to_str().expect("utf-8"),
            probe.to_str().expect("utf-8"),
        ],
    );
    git(
        &scratch.tree(),
        &[
            "push",
            "-q",
            probe.to_str().expect("utf-8"),
            &format!("{head}:refs/heads/takt/case"),
        ],
    );
    git(
        &probe,
        &[
            "--no-replace-objects",
            "log",
            "--patch",
            "--format=commit %H%n%B",
            "takt/case",
        ],
    )
}

/// `git replace` swaps a commit for a clean one in every read git makes, `git log` included,
/// but a push sends the objects as they are. A step that commits the literal, removes it and
/// replaces the first commit with a clean one leaves a scan that reads nothing forbidden and a
/// push that publishes the literal in the replaced commit's message and file.
/// `refs/replace/` is not among the refs a step's checkpoint guards.
#[test]
fn a_replace_ref_hides_a_pushed_commit_from_the_scan() {
    let scratch = scratch("replace", &[]);
    let tree = scratch.tree();
    git(&tree, &["push", "-q", "origin", "main"]);
    let start = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let leaked = commit(
        &tree,
        "secret.txt",
        &format!("token {FORBIDDEN}\n"),
        &format!("implementation.edit: {FORBIDDEN}"),
    );
    git(&tree, &["rm", "-q", "secret.txt"]);
    git(&tree, &["commit", "-q", "-m", "implementation.edit: tidy"]);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let clean = git(
        &tree,
        &[
            "commit-tree",
            "-p",
            &start,
            "-m",
            "implementation.edit: clean",
            &format!("{start}^{{tree}}"),
        ],
    );
    git(&tree, &["replace", &leaked, &clean]);
    let published = published_by_a_push(&scratch, &head);
    assert!(
        published.contains(FORBIDDEN),
        "precondition: the push publishes the literal: {published}"
    );
    let (answer, scanned) = scan(&scratch, "main", &head);
    refused(&answer, &scanned, "in the commit a replace ref hides");
}

/// The scan trusts `refs/remotes/origin/*` as what the remote holds. A step that writes one
/// there (`git update-ref refs/remotes/origin/x HEAD`, or a fetch from another URL into that
/// namespace) makes every commit of `HEAD` count as published, though the remote holds none of
/// them. `refs/remotes/` is not among the refs a step's checkpoint guards.
#[test]
fn a_tracking_ref_the_remote_does_not_hold_hides_commits_from_the_scan() {
    let scratch = scratch("tracking", &[]);
    let tree = scratch.tree();
    git(&tree, &["push", "-q", "origin", "main"]);
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let head = commit(
        &tree,
        "secret.txt",
        &format!("token {FORBIDDEN}\n"),
        "implementation.edit: notes",
    );
    git(&tree, &["update-ref", "refs/remotes/origin/scratch", &head]);
    let origin = scratch.bare("origin.git");
    assert!(
        !run_git(&origin, &["cat-file", "-e", &format!("{head}^{{commit}}")])
            .status
            .success(),
        "precondition: the remote does not hold {head}"
    );
    let published = published_by_a_push(&scratch, &head);
    assert!(published.contains(FORBIDDEN), "precondition: {published}");
    let (answer, scanned) = scan(&scratch, "main", &head);
    refused(
        &answer,
        &scanned,
        "in a commit only a local tracking ref holds",
    );
}

/// With no tracking ref of `origin` (a new, empty repository on the forge), the base's commits
/// are scanned only when no remote-tracking ref holds them, any remote's. A commit another
/// remote holds (the repository's previous, private host) is not on `origin`, and the push
/// publishes it unscanned.
#[test]
fn with_no_tracking_ref_a_commit_another_remote_holds_is_pushed_unscanned() {
    let scratch = scratch("other-remote", &[]);
    let tree = scratch.tree();
    commit(
        &tree,
        "internal.txt",
        &format!("token {FORBIDDEN}\n"),
        "internal notes",
    );
    let internal = scratch.bare("internal.git");
    git(
        &tree,
        &[
            "remote",
            "add",
            "internal",
            internal.to_str().expect("utf-8"),
        ],
    );
    git(&tree, &["push", "-q", "internal", "main"]);
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let head = commit(&tree, "work.txt", "clean\n", "implementation.edit: clean");
    assert!(
        git(&tree, &["for-each-ref", "refs/remotes/origin/"]).is_empty(),
        "precondition: origin has no tracking ref"
    );
    let published = published_by_a_push(&scratch, &head);
    assert!(published.contains(FORBIDDEN), "precondition: {published}");
    let (answer, scanned) = scan(&scratch, "main", &head);
    refused(
        &answer,
        &scanned,
        "in a base commit only another remote holds",
    );
}

/// With `log.showRoot=false` (the repository's or the user's configuration; `log.*` is not a
/// key a step's checkpoint guards), `git log --patch` prints a root commit without its diff.
/// A push to a new, empty repository publishes the root commit's files unscanned.
#[test]
fn log_show_root_false_hides_the_root_commit_from_the_scan() {
    let scratch = scratch(
        "show-root",
        &[("secret.txt", &format!("token {FORBIDDEN}\n"))],
    );
    let tree = scratch.tree();
    git(&tree, &["config", "log.showRoot", "false"]);
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let head = commit(&tree, "work.txt", "clean\n", "implementation.edit: clean");
    let published = published_by_a_push(&scratch, &head);
    assert!(published.contains(FORBIDDEN), "precondition: {published}");
    let (answer, scanned) = scan(&scratch, "main", &head);
    refused(&answer, &scanned, "in the root commit's file");
}

/// With `diff.context=0` (the user's or the repository's configuration) a hunk holds the added
/// lines alone. A line the remote holds, `about FORBID`, and the line the push adds after it,
/// `DEN-7Q2X`, are next to each other in the file published, which the policy refuses as a
/// wrapped literal; the scanned diff holds the added line alone.
#[test]
fn diff_context_zero_hides_the_line_a_wrapped_literal_continues() {
    let (first, rest) = FORBIDDEN.split_at(6);
    let before = format!("intro\n  about {first}\n");
    let after = format!("{before}{rest} wrapped\n");
    assert!(!breaks(&before), "precondition: the remote's file passes");
    assert!(breaks(&after), "precondition: the file pushed is refused");
    let scratch = scratch("context", &[("notes.md", &before)]);
    let tree = scratch.tree();
    git(&tree, &["push", "-q", "origin", "main"]);
    git(&tree, &["config", "diff.context", "0"]);
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let head = commit(&tree, "notes.md", &after, "implementation.edit: notes");
    let (answer, scanned) = scan(&scratch, "main", &head);
    refused(&answer, &scanned, "across a remote line and an added one");
}

/// The scanned diff holds three lines of context around a change. A literal wrapped over more
/// lines than that, one character a line, which one commit completes by fixing its middle
/// character, is in the file published, and the policy refuses that file; the hunk holds seven
/// of its fourteen lines, and the scan passes. No configuration is involved.
#[test]
fn a_literal_wrapped_past_the_hunk_context_passes_the_scan() {
    let column = |literal: &str| {
        literal
            .chars()
            .map(|c| format!("{c}\n"))
            .collect::<String>()
    };
    let mut broken: Vec<char> = FORBIDDEN.chars().collect();
    broken[6] = 'x';
    let before = column(&broken.iter().collect::<String>());
    let after = column(FORBIDDEN);
    assert!(!breaks(&before), "precondition: the remote's file passes");
    assert!(breaks(&after), "precondition: the file pushed is refused");
    let scratch = scratch("column", &[("column.txt", &before)]);
    let tree = scratch.tree();
    git(&tree, &["push", "-q", "origin", "main"]);
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let head = commit(&tree, "column.txt", &after, "implementation.edit: fix");
    let (answer, scanned) = scan(&scratch, "main", &head);
    refused(&answer, &scanned, "wrapped over fourteen lines");
}
