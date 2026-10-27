//! story:a-command-forge-runs-the-configured-commands: `forge: command` runs the exact argument
//! lists the run file's `forge_commands` section names, without a shell, through the forge
//! `Runner`. A double of the command records the argument list it is given and answers from a
//! script; one case runs the real process runner on `echo`, to show that a title holding shell
//! syntax is one argument and nothing else runs. Nothing here reaches a host.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use takt::forge::command::{
    CommandForge, ForgeCommands, ForgeEntryMissing, ForgeFailed, ForgeOutput, ForgeRefused,
};
use takt::forge::{Checks, Exit, Forge, PullRequest, PullRequestState, Runner};
use takt::protocol::LoadedProtocol;
use takt::run_config::RunConfig;
use takt::scan::Scan;

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("command-forge-{}-{name}", std::process::id()));
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

fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

/// The command double: records every argument list it is run with and answers the next line of
/// its script.
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

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl Runner for Fake {
    fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
        self.calls.borrow_mut().push(argv.to_vec());
        Ok(self
            .script
            .borrow_mut()
            .pop_front()
            .expect("the script has a reply for each command"))
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
        self.run(tree, argv)
    }
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// The commands the story's example names, with `tool` for the program.
fn commands() -> ForgeCommands {
    ForgeCommands {
        open: Some(argv(&[
            "tool",
            "open",
            "--source-branch",
            "{branch}",
            "--target-branch={base}",
            "--title",
            "{title}",
            "--description-file",
            "{body_file}",
        ])),
        read: Some(argv(&["tool", "read", "{number}"])),
        checks: Some(argv(&["tool", "checks", "{number}", "{head}"])),
        merge: Some(argv(&["tool", "merge", "{number}", "{head}"])),
        id_pattern: Some(r"/merge_requests/(\d+)".to_owned()),
        refused_exit: Some(vec![3]),
    }
}

const ACTIONS: [&str; 3] = ["pr.open", "ci.read", "repository.merge"];

fn actions() -> Vec<String> {
    ACTIONS.iter().map(|action| (*action).to_owned()).collect()
}

fn forge_over(
    scratch: &Scratch,
    commands: ForgeCommands,
    runner: Box<dyn Runner>,
) -> Result<CommandForge> {
    CommandForge::for_tree(
        &scratch.0,
        commands,
        &actions(),
        scratch.0.join("state"),
        Scan::None {
            declared_in: scratch.0.join("run.yaml"),
        },
        runner,
    )
}

fn request(title: &str) -> PullRequest {
    PullRequest {
        branch: "takt/case-1".to_owned(),
        base: "main".to_owned(),
        title: title.to_owned(),
        body: "the body\nof two lines".to_owned(),
    }
}

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn view(state: &str) -> String {
    format!(r#"{{"state": "{state}", "head": "{HEAD}", "branch": "takt/case-1", "base": "main"}}"#)
}

/// The fake receives exactly the configured argument list with each placeholder substituted
/// inside its element, and the number parsed from what `open` printed is the request takt reads
/// next.
#[test]
fn opens_with_the_configured_argv() {
    let scratch = Scratch::new("opens");
    let fake = Fake::answering([
        exit(
            0,
            "created\nhttps://forge.test/g/p/-/merge_requests/42\n",
            "",
        ),
        exit(0, &view("opened"), ""),
    ]);
    let forge = forge_over(&scratch, commands(), Box::new(fake.clone())).expect("a forge");
    let artifacts = scratch.0.join("artifacts");
    let number = forge
        .open_pull_request(&scratch.0, &artifacts, &request("A title"))
        .expect("opens");
    assert_eq!(number, 42);
    let body = artifacts.join(CommandForge::BODY);
    assert_eq!(
        fs::read_to_string(&body).expect("the body file"),
        "the body\nof two lines"
    );
    let body = body.display().to_string();
    assert_eq!(
        fake.calls()[0],
        argv(&[
            "tool",
            "open",
            "--source-branch",
            "takt/case-1",
            "--target-branch=main",
            "--title",
            "A title",
            "--description-file",
            &body,
        ])
    );
    let read = forge.pull_request(&scratch.0, number).expect("reads");
    assert_eq!(fake.calls()[1], argv(&["tool", "read", "42"]));
    assert_eq!(read.state, PullRequestState::Open);
    assert_eq!(read.head.sha, HEAD);
    assert_eq!(read.head.branch, "takt/case-1");
    assert_eq!(read.base, "main");
}

/// A title holding `; rm -rf x` and `$(id)` arrives as one argument of the double, and the real
/// process runner, given `echo`, prints it back whole and runs nothing else.
#[test]
fn a_title_is_one_argument() {
    let title = "fix; rm -rf x && echo $(id) `id`";
    let scratch = Scratch::new("title");
    let fake = Fake::answering([exit(0, "https://forge.test/merge_requests/1", "")]);
    let forge = forge_over(&scratch, commands(), Box::new(fake.clone())).expect("a forge");
    forge
        .open_pull_request(&scratch.0, &scratch.0.join("a"), &request(title))
        .expect("opens");
    let call = &fake.calls()[0];
    assert_eq!(call.len(), 9, "no word was split: {call:?}");
    assert_eq!(call[6], title);

    let marker = scratch.0.join("marker");
    let touched = format!("; touch {}", marker.display());
    let echoing = ForgeCommands {
        open: Some(argv(&[
            "echo",
            "https://forge.test/merge_requests/9",
            "{title}",
        ])),
        ..commands()
    };
    let forge =
        forge_over(&scratch, echoing, Box::new(takt::forge::Process::default())).expect("a forge");
    let dir = scratch.0.join("real");
    let number = forge
        .open_pull_request(&scratch.0, &dir, &request(&format!("{title}{touched}")))
        .expect("opens");
    assert_eq!(number, 9);
    let printed = fs::read_to_string(dir.join("pull-request.response.json")).expect("a response");
    assert!(
        printed.contains(&format!("{title}{touched}")),
        "echo was handed the title whole: {printed}"
    );
    assert!(!marker.exists(), "nothing else ran");
}

/// `open` printing no URL gives a typed error naming `open` and `id_pattern`.
#[test]
fn unparseable_output_is_named() {
    let scratch = Scratch::new("unparseable");
    let fake = Fake::answering([exit(0, "created it, no link\n", "")]);
    let forge = forge_over(&scratch, commands(), Box::new(fake)).expect("a forge");
    let error = forge
        .open_pull_request(&scratch.0, &scratch.0.join("a"), &request("T"))
        .expect_err("no number");
    let typed = error.downcast_ref::<ForgeOutput>().expect("a typed error");
    assert_eq!(typed.command, "open");
    assert_eq!(typed.field, "id_pattern");
    let text = format!("{error:#}");
    assert!(
        text.contains("open") && text.contains("id_pattern"),
        "{text}"
    );

    let fake = Fake::answering([
        exit(0, "not json", ""),
        exit(0, "{\"state\": \"open\"}", ""),
    ]);
    let forge = forge_over(&scratch, commands(), Box::new(fake)).expect("a forge");
    let error = forge.pull_request(&scratch.0, 1).expect_err("no JSON");
    assert_eq!(
        error.downcast_ref::<ForgeOutput>().expect("typed").field,
        "output"
    );
    let error = forge.pull_request(&scratch.0, 1).expect_err("no head");
    let typed = error.downcast_ref::<ForgeOutput>().expect("typed");
    assert_eq!(
        (typed.command.as_str(), typed.field.as_str()),
        ("read", "head")
    );
}

/// An exit in `refused_exit` is `ForgeRefused`, another non-zero exit `ForgeFailed`; standard
/// error is kept under the state directory.
#[test]
fn a_refused_exit_is_typed() {
    let scratch = Scratch::new("exits");
    let fake = Fake::answering([
        exit(3, "", "not allowed: needs approval"),
        exit(4, "", "boom"),
    ]);
    let forge = forge_over(&scratch, commands(), Box::new(fake)).expect("a forge");
    let dir = scratch.0.join("a");
    let error = forge.merge(&scratch.0, &dir, 7, HEAD).expect_err("refused");
    let refused = error.downcast_ref::<ForgeRefused>().expect("refused");
    assert_eq!((refused.command.as_str(), refused.exit), ("merge", 3));
    assert_eq!(
        fs::read_to_string(&refused.stderr).expect("stderr kept"),
        "not allowed: needs approval"
    );
    assert!(refused.stderr.starts_with(scratch.0.join("state")));
    let error = forge.merge(&scratch.0, &dir, 7, HEAD).expect_err("failed");
    assert!(error.downcast_ref::<ForgeRefused>().is_none());
    let failed = error.downcast_ref::<ForgeFailed>().expect("failed");
    assert_eq!((failed.command.as_str(), failed.exit), ("merge", Some(4)));
}

/// No `merge` entry under a protocol that merges is refused before the case opens, naming it.
#[test]
fn a_missing_entry_is_refused_at_open() {
    let scratch = Scratch::new("missing");
    let protocol = LoadedProtocol::builtin("story.delivery/1").expect("a built-in");
    let without = ForgeCommands {
        merge: None,
        ..commands()
    };
    let error = CommandForge::for_tree(
        &scratch.0,
        without,
        &protocol.actions,
        scratch.0.join("state"),
        Scan::None {
            declared_in: scratch.0.join("run.yaml"),
        },
        Box::new(Fake::default()),
    )
    .expect_err("refused");
    let missing = error.downcast_ref::<ForgeEntryMissing>().expect("typed");
    assert_eq!(missing.entry, "merge");
    assert!(format!("{error:#}").contains("`merge`"));
    assert!(!scratch.0.join("state").exists(), "nothing was written");
}

/// `merge` receives the head the checks were read on: `checks` reads the request first and runs
/// the checks command with the head it reported, and the merge is given the head it is called
/// with.
#[test]
fn merge_carries_the_head() {
    let scratch = Scratch::new("head");
    let fake = Fake::answering([
        exit(0, &view("open"), ""),
        exit(
            0,
            r#"[{"name": "build", "state": "success"}, {"name": "lint", "state": "passed"}]"#,
            "",
        ),
        exit(0, "merged", ""),
    ]);
    let forge = forge_over(&scratch, commands(), Box::new(fake.clone())).expect("a forge");
    assert_eq!(forge.checks(&scratch.0, 7).expect("checks"), Checks::Pass);
    forge
        .merge(&scratch.0, &scratch.0.join("a"), 7, HEAD)
        .expect("merges");
    let calls = fake.calls();
    assert_eq!(calls[0], argv(&["tool", "read", "7"]));
    assert_eq!(calls[1], argv(&["tool", "checks", "7", HEAD]));
    assert_eq!(calls[2], argv(&["tool", "merge", "7", HEAD]));
}

/// The checks of a head, as one: a failed check fails, a pending one or none yet leaves it
/// pending, an unknown state is a typed error.
#[test]
fn checks_read_as_one_result() {
    let scratch = Scratch::new("checks");
    let one = |text: &str| {
        let fake = Fake::answering([exit(0, &view("open"), ""), exit(0, text, "")]);
        let forge = forge_over(&scratch, commands(), Box::new(fake)).expect("a forge");
        forge.checks(&scratch.0, 1)
    };
    let state = |states: &[&str]| {
        let checks: Vec<String> = states
            .iter()
            .map(|state| format!(r#"{{"name": "c", "state": "{state}"}}"#))
            .collect();
        format!(r#"{{"checks": [{}]}}"#, checks.join(","))
    };
    assert_eq!(
        one(&state(&["success", "running"])).expect("pending"),
        Checks::Pending
    );
    assert_eq!(
        one(&state(&["success", "failure", "queued"])).expect("fail"),
        Checks::Fail
    );
    assert_eq!(one("[]").expect("none yet"), Checks::Pending);
    let error = one(&state(&["strange"])).expect_err("unknown");
    let typed = error.downcast_ref::<ForgeOutput>().expect("typed");
    assert_eq!(
        (typed.command.as_str(), typed.field.as_str()),
        ("checks", "state")
    );
}

/// Without `id_pattern` the number is the last integer of the last URL on standard output, or a
/// lone number; a pattern's first group is read from the last match.
#[test]
fn the_request_number_is_read_from_its_url() {
    let scratch = Scratch::new("number");
    let without = ForgeCommands {
        id_pattern: None,
        ..commands()
    };
    for (printed, number) in [
        (
            "see https://a.test/x/1 then\nhttps://a.test/g/p/-/merge_requests/17?x=3#note\n",
            17,
        ),
        ("#23\n", 23),
        ("5", 5),
    ] {
        let fake = Fake::answering([exit(0, printed, "")]);
        let forge = forge_over(&scratch, without.clone(), Box::new(fake)).expect("a forge");
        let opened = forge
            .open_pull_request(&scratch.0, &scratch.0.join("a"), &request("T"))
            .expect("opens");
        assert_eq!(opened, number, "{printed}");
    }
    let fake = Fake::answering([exit(0, "words without a link", "")]);
    let forge = forge_over(&scratch, without, Box::new(fake)).expect("a forge");
    assert!(
        forge
            .open_pull_request(&scratch.0, &scratch.0.join("a"), &request("T"))
            .is_err()
    );
}

/// A `forge_commands` section is read from a run file: argument lists, never a shell line.
#[test]
fn the_run_file_reads_argument_lists_only() {
    let scratch = Scratch::new("file");
    let path = scratch.0.join("run.yaml");
    fs::write(
        &path,
        "forge: command\nforge_commands:\n  open: [tool, open, \"{title}\"]\n  read: [tool, read, \"{number}\"]\n  id_pattern: '/merge_requests/(\\d+)'\n  refused_exit: [3]\n",
    )
    .expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    let commands = config.forge_commands.expect("the section");
    assert_eq!(commands.open, Some(argv(&["tool", "open", "{title}"])));
    assert_eq!(commands.refused_exit, Some(vec![3]));

    for text in [
        "forge_commands:\n  open: tool open {title}\n",
        "forge_commands:\n  open: []\n",
        "forge_commands:\n  open: [\"\"]\n",
        "forge_commands:\n  id_pattern: '(' \n  open: [tool]\n",
        "forge_commands:\n  id_pattern: 'no group'\n  open: [tool]\n",
        "forge_commands:\n  refused_exit: [0]\n",
        "forge_commands:\n  other: [tool]\n",
    ] {
        fs::write(&path, text).expect("a file");
        assert!(RunConfig::read(&path).is_err(), "{text}");
    }
}

/// Settings that do not fit the forge are refused, naming keys.
#[test]
fn settings_must_fit_the_forge() {
    let some = takt::run_config::ForgeSettings {
        commands: Some(commands()),
        ..Default::default()
    };
    assert!(some.check_for(Some("command")).is_ok());
    let error = some.check_for(Some("bot")).expect_err("not for bot");
    assert!(format!("{error:#}").contains("forge_commands"));
    let none = takt::run_config::ForgeSettings::default();
    let error = none.check_for(Some("command")).expect_err("needs commands");
    assert!(format!("{error:#}").contains("forge_commands"));
    assert!(none.check_for(None).is_ok());
}
