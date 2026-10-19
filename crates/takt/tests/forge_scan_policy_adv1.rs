//! Adversary cases for `pr.open`'s scan through the bot route: what `b10x-gates scan-text`
//! reads is the pull request as it will be published. The scanner matches line by line and
//! re-checks a long literal across a real line break; a body it reads only as one JSON-escaped
//! line (`\n` as two bytes) hides every line start and every wrapped term from it, while the
//! forge publishes the decoded text. A recording runner stands in for `b10x-gates`; nothing
//! leaves the machine.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use takt::forge::{BotRoute, Exit, Forge, PullRequest, Runner};

/// Records what the file `scan-text` names held when it ran; `api` answers pull request #7.
struct Recording {
    scanned: std::rc::Rc<RefCell<Vec<String>>>,
}

impl Runner for Recording {
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

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        match argv.get(1).map(String::as_str) {
            Some("scan-text") => {
                let path = argv.last().expect("a path");
                self.scanned
                    .borrow_mut()
                    .push(fs::read_to_string(path).expect("the scanned file reads"));
            }
            Some("api") => {
                let output = argv
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| &argv[i + 1])
                    .expect("--output");
                fs::write(output, r#"{"number": 7}"#)?;
            }
            _ => {}
        }
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("forge-scan-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch dir is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// A body shaped as `pr.open` writes one (the intent, a rule, takt's line), carrying a
/// machine-local path at the start of a line. Every line of it is what the forge publishes;
/// the scan has to read those lines, not an escaped copy of them.
#[test]
fn the_scan_reads_the_body_as_it_is_published() {
    let dir = scratch("body");
    let path_at_line_start = format!("/{}/someone/notes/todo.txt", "home");
    let body = format!(
        "Remove the TODO\n\nThe list came from\n{path_at_line_start}\n\n---\n\nOpened by takt for the case `c1` at `abc`. Claims that hold: none yet.\n"
    );
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "Remove the TODO".to_owned(),
        body: body.clone(),
    };
    let scanned = std::rc::Rc::new(RefCell::new(Vec::new()));
    let forge = BotRoute::new(
        "owner/repo",
        Some(PathBuf::from("/policies/gates.json")),
        Box::new(Recording {
            scanned: scanned.clone(),
        }),
    );
    let number = forge
        .open_pull_request(&dir, &dir.join("artifacts"), &request)
        .expect("the pull request opens");
    assert_eq!(number, 7);
    let scanned = scanned.borrow().clone();
    fs::remove_dir_all(&dir).expect("the scratch dir is removed");
    assert_eq!(scanned.len(), 1, "one scan-text line ran");
    let text = &scanned[0];
    assert!(
        text.lines().any(|line| line == path_at_line_start),
        "scan-text never sees the line the forge publishes as `{path_at_line_start}`; it read:\n{text}"
    );
    assert!(
        text.contains(&body),
        "scan-text read an encoding of the body, not the body; it read:\n{text}"
    );
}
