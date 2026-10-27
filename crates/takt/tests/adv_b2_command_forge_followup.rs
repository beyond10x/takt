//! Follow-up cases of the adversarial pass over story:a-command-forge-runs-the-configured-commands,
//! parked for a later change: the command forge has no answer yet for the merge target's tip, and
//! a URL that names no request still gives a number. A double of the command records each
//! argument list and answers from a script; nothing reaches a host.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use takt::forge::command::{CommandForge, ForgeCommands, ForgeOutput};
use takt::forge::{Exit, Forge, PullRequest, Runner};
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

    /// A repository whose `origin` names a project on a fake host.
    fn repository(&self) -> PathBuf {
        let tree = self.0.join("tree");
        fs::create_dir_all(&tree).expect("a tree");
        for args in [
            vec!["init", "-q"],
            vec![
                "remote",
                "add",
                "origin",
                "https://forge.test/group/project.git",
            ],
        ] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&tree)
                .args(args)
                .status()
                .expect("git runs");
            assert!(status.success());
        }
        tree
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

/// The forge refuses at open a story.delivery/1 run file without a `merge` entry, so the entry
/// is required; but `repository.merge` reads the merge target's tip through
/// `Forge::target_tip` before it ever calls `merge` (validator.rs judge_guard), and the command
/// forge keeps the trait's default refusal. The `merge` the file must name can never run.
#[test]
fn adv_b2_the_required_merge_entry_is_reachable() {
    let scratch = Scratch::new("tip");
    let tree = scratch.repository();
    let fake = Fake::answering([exit(0, "0123456789abcdef0123456789abcdef01234567\n")]);
    let forge = CommandForge::for_tree(
        &tree,
        commands(),
        &delivery_actions(),
        scratch.0.join("state"),
        scan(&scratch),
        Box::new(fake),
    )
    .expect("every entry story.delivery/1 needs is named");
    let tip = forge.target_tip(&tree, "main");
    assert!(
        tip.is_ok(),
        "the forge requires `merge`, yet repository.merge stops before it: {:#}",
        tip.as_ref().expect_err("an error")
    );
}

/// Without `id_pattern`, a URL on standard output that names no request is no request number:
/// a digit of the project's path or of the host's port is not one.
#[test]
fn adv_b2_a_url_that_names_no_request_gives_no_number() {
    let scratch = Scratch::new("number");
    let mut read = Vec::new();
    for printed in [
        "https://forge.test/group/project-2/-/merge_requests/new\n",
        "https://forge.test:8443/group/project\n",
    ] {
        let fake = Fake::answering([exit(0, printed)]);
        let forge = CommandForge::for_tree(
            &scratch.0,
            commands(),
            &delivery_actions(),
            scratch.0.join("state"),
            scan(&scratch),
            Box::new(fake),
        )
        .expect("a forge");
        let opened = forge.open_pull_request(&scratch.0, &scratch.0.join("a"), &request("b", "x"));
        let typed = opened
            .as_ref()
            .err()
            .and_then(|error| error.downcast_ref::<ForgeOutput>())
            .map(|typed| typed.field.clone());
        read.push((printed.trim().to_owned(), opened.ok(), typed));
    }
    assert!(
        read.iter()
            .all(|(_, number, field)| number.is_none() && field.as_deref() == Some("id_pattern")),
        "a number was read from a URL that names no request: {read:?}"
    );
}
