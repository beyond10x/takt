//! Adversarial cases for story:a-command-forge-runs-the-configured-commands. A double of the
//! command records each argument list and answers from a script; nothing reaches a host.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use takt::forge::command::{CommandForge, ForgeCommands};
use takt::forge::{Exit, Forge, PullRequest, PullRequestState, Runner};
use takt::protocol::LoadedProtocol;
use takt::scan::Scan;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("adv-b2-command-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn exit(code: i32, stdout: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

#[derive(Clone, Default)]
struct Fake {
    calls: Rc<RefCell<Vec<Vec<String>>>>,
    script: Rc<RefCell<VecDeque<Exit>>>,
}

impl Fake {
    fn answering(replies: impl IntoIterator<Item = Exit>) -> Self {
        let fake = Self::default();
        fake.script.borrow_mut().extend(replies);
        fake
    }
}

impl Runner for Fake {
    fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
        self.calls.borrow_mut().push(argv.to_vec());
        Ok(self
            .script
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| exit(0, "")))
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
        self.run(tree, argv)
    }
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn commands() -> ForgeCommands {
    ForgeCommands {
        open: Some(argv(&[
            "tool",
            "open",
            "{branch}",
            "--description-file",
            "{body_file}",
        ])),
        read: Some(argv(&["tool", "read", "{number}"])),
        checks: Some(argv(&["tool", "checks", "{number}", "{head}"])),
        merge: Some(argv(&["tool", "merge", "{number}", "{head}"])),
        id_pattern: None,
        refused_exit: Some(vec![3]),
    }
}

fn scan(scratch: &Scratch) -> Scan {
    Scan::None {
        declared_in: scratch.0.join("run.yaml"),
    }
}

fn delivery_actions() -> Vec<String> {
    LoadedProtocol::builtin("story.delivery/1")
        .expect("a built-in")
        .actions
}

fn request(branch: &str, body: &str) -> PullRequest {
    PullRequest {
        branch: branch.to_owned(),
        base: "main".to_owned(),
        title: "T".to_owned(),
        body: body.to_owned(),
    }
}

/// What `read` prints for an open request whose head is `branch`.
fn opened_view(branch: &str) -> String {
    format!(
        r#"{{"state": "opened", "head": "0123456789abcdef0123456789abcdef01234567", "branch": "{branch}", "base": "main"}}"#
    )
}

/// The runner of case A: while A's `open` runs, another run's command forge on the same state
/// directory (the CLI gives every run on the machine `<state home>/forge-command` without
/// `--state-dir`) opens its own request; then A's command reads the body file it was handed.
struct Interleaved {
    other: Rc<RefCell<Option<CommandForge>>>,
    other_dir: PathBuf,
    seen: Rc<RefCell<Option<String>>>,
}

impl Runner for Interleaved {
    fn run(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        if argv.get(1).map(String::as_str) == Some("open") {
            if let Some(other) = self.other.borrow().as_ref() {
                other
                    .open_pull_request(tree, &self.other_dir, &request("case-b", "body of case B"))
                    .expect("the other case opens");
            }
            let at = argv
                .iter()
                .position(|word| word == "--description-file")
                .expect("the body file flag");
            *self.seen.borrow_mut() = Some(fs::read_to_string(&argv[at + 1]).expect("a body"));
        }
        if argv.get(1).map(String::as_str) == Some("read") {
            return Ok(exit(0, &opened_view("case-a")));
        }
        Ok(exit(0, "https://forge.test/g/p/-/merge_requests/1\n"))
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
        self.run(tree, argv)
    }
}

/// The body `{body_file}` names is the case's own: another case opening meanwhile on the same
/// state directory does not change what this case's `open` publishes.
#[test]
fn adv_b2_the_body_file_is_the_cases_own() {
    let scratch = Scratch::new("body");
    let state = scratch.0.join("forge-command");
    let other = Rc::new(RefCell::new(None));
    let seen = Rc::new(RefCell::new(None));
    let forge_a = CommandForge::for_tree(
        &scratch.0,
        commands(),
        &delivery_actions(),
        &state,
        scan(&scratch),
        Box::new(Interleaved {
            other: other.clone(),
            other_dir: scratch.0.join("case-b"),
            seen: seen.clone(),
        }),
    )
    .expect("a forge");
    let forge_b = CommandForge::for_tree(
        &scratch.0,
        commands(),
        &delivery_actions(),
        &state,
        scan(&scratch),
        Box::new(Fake::answering([
            exit(0, "https://forge.test/g/p/-/merge_requests/2\n"),
            exit(0, &opened_view("case-b")),
        ])),
    )
    .expect("a forge");
    *other.borrow_mut() = Some(forge_b);
    forge_a
        .open_pull_request(
            &scratch.0,
            &scratch.0.join("case-a"),
            &request("case-a", "body of case A"),
        )
        .expect("opens");
    assert_eq!(
        seen.borrow().as_deref(),
        Some("body of case A"),
        "case A's open command published another case's body"
    );
}

/// Kills two mutants the suite keeps alive: `read`'s `merged` and `closed` swapped or folded
/// into open (no case reads either), and `id_pattern` taking the first match instead of the
/// last (no case prints two).
#[test]
fn adv_b2_read_states_and_the_last_match() {
    let scratch = Scratch::new("mutants");
    let view = |state: &str| {
        format!(
            r#"{{"state": "{state}", "head": "0123456789abcdef0123456789abcdef01234567", "branch": "b", "base": "main"}}"#
        )
    };
    let fake = Fake::answering([
        exit(0, &view("merged")),
        exit(0, &view("closed")),
        exit(0, &view("locked")),
    ]);
    let forge = CommandForge::for_tree(
        &scratch.0,
        commands(),
        &delivery_actions(),
        scratch.0.join("state"),
        scan(&scratch),
        Box::new(fake),
    )
    .expect("a forge");
    let states: Vec<PullRequestState> = (0..3)
        .map(|_| forge.pull_request(&scratch.0, 1).expect("reads").state)
        .collect();
    assert_eq!(
        states,
        [
            PullRequestState::Merged,
            PullRequestState::Closed,
            PullRequestState::Open
        ]
    );

    let fake = Fake::answering([
        exit(
            0,
            "see https://forge.test/g/p/-/merge_requests/1\nopened https://forge.test/g/p/-/merge_requests/2\n",
        ),
        exit(0, &opened_view("b")),
    ]);
    let forge = CommandForge::for_tree(
        &scratch.0,
        ForgeCommands {
            id_pattern: Some(r"/merge_requests/(\d+)".to_owned()),
            ..commands()
        },
        &delivery_actions(),
        scratch.0.join("state"),
        scan(&scratch),
        Box::new(fake),
    )
    .expect("a forge");
    assert_eq!(
        forge
            .open_pull_request(&scratch.0, &scratch.0.join("a"), &request("b", "x"))
            .expect("opens"),
        2
    );
}
