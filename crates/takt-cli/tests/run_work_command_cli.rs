//! story:the-command-work-source through the binary: the run file's `work_commands` names an
//! argument list for `read` (one item by its id, `{id}`) and for `select` (the next ready item),
//! each printing one `takt.work-item/1` document. `--work command:<id>` runs `read` and opens a
//! case on the item as an `--intent` case opens, its `intent.md` the item's title and body, its
//! record the key and the revision; `"command"` in the `work` list runs `select`, the first
//! source in list order that yields an item wins, and a `select` that prints nothing is no ready
//! work. Output that is no item, or an item naming another key, is refused before a case opens,
//! naming the command; the command's child holds no credential of takt's environment.
//!
//! The commands are programs every Unix has, `cat` and `printenv`, each given its arguments as
//! the run file writes them: no shell script, no program of the test's own.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The item `read` prints for `X-1`.
const ITEM: &str = r#"{"format": "takt.work-item/1", "key": "command:X-1", "title": "Fix the login", "body": "The login fails on a long name.\nMake it pass.", "url": "https://tracker.example/X-1", "revision": "2026-11-06T10:00:00Z", "labels": ["takt-ready"]}"#;

/// What the case's `intent.md` holds for [`ITEM`]: its title, a blank line and its body.
const INTENT: &str = "Fix the login\n\nThe login fails on a long name.\nMake it pass.";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop: `tree` the
/// repository, `items` the files the fake commands print, the run file beside them.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn items(&self) -> PathBuf {
        self.0.join("items")
    }

    /// Writes the item file `name` under `items`.
    fn item(&self, name: &str, text: &str) {
        fs::write(self.items().join(name), text).expect("an item file");
    }

    /// A run file holding `text`, `{items}` replaced by the items directory.
    fn config(&self, text: &str) -> PathBuf {
        let path = self.0.join("run.toml");
        let items = self.items().display().to_string();
        fs::write(&path, text.replace("{items}", &items)).expect("a run configuration file");
        path
    }
}

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

/// A committed repository, holding a copy of the fixture store `store` when given, else no
/// planning store at all.
fn scratch(name: &str, store: Option<&str>) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-work-command-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(scratch.items()).expect("the items");
    if let Some(store) = store {
        copy_dir(&Path::new(FIXTURES).join(store), &tree);
    }
    fs::write(tree.join("check.txt"), "broken\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
}

/// Runs the built `takt run` on `repo` with no harness, one iteration and a passing test
/// command, plus `extra`, `env` set for it: the exit code, stdout and stderr.
fn run_with(repo: &Path, extra: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .args(extra)
        .envs(env.iter().copied())
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run(repo: &Path, extra: &[&str]) -> (i32, String, String) {
    run_with(repo, extra, &[])
}

/// The case directories under `repo`, sorted; empty when there is no cases directory.
fn cases(repo: &Path) -> Vec<PathBuf> {
    let dir = repo.join(".takt/cases");
    if !dir.exists() {
        return Vec::new();
    }
    let mut cases: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("a readable cases directory")
        .map(|e| e.expect("an entry").path())
        .collect();
    cases.sort();
    cases
}

/// The string `case.yaml` of `case` holds under `key`.
fn case_member(case: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(case.join("case.yaml")).expect("case.yaml");
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("case.yaml reads");
    doc[key].as_str().map(str::to_owned)
}

fn name_of(case: &Path) -> String {
    case.file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned()
}

/// Whether `id` is a case id of the tracker scheme for a key whose slug is `slug`:
/// `command-<slug>-<8 hex digits>-<instant>` (approval-record:wave-56-work-model-points, 9).
fn tracker_case_id(id: &str, slug: &str) -> bool {
    let Some(rest) = id.strip_prefix(&format!("command-{slug}-")) else {
        return false;
    };
    let (h8, instant) = rest.split_at(8.min(rest.len()));
    h8.len() == 8
        && h8
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && instant.starts_with('-')
        && instant.ends_with('Z')
}

/// Acceptance 1: a fake `read` printing one item: `--work command:X-1` opens a case whose
/// `intent.md` holds its title and body and whose record holds its key and revision, under the
/// tracker scheme's case id. It opens as an `--intent` case with that text does: the same exit
/// and the same protocol. A second `--work command:X-1` resumes that open case, running no
/// `read`: an item that no longer reads changes nothing in it.
#[test]
fn a_read_item_opens_a_case_with_its_text_key_and_revision() {
    let scratch = scratch("read", None);
    scratch.item("X-1.json", ITEM);
    let config = scratch.config("work_commands = { read = [\"cat\", \"{items}/{id}.json\"] }\n");
    let config = config.to_str().expect("utf-8");
    let repo = scratch.tree();
    let (code, out, err) = run(&repo, &["--config", config, "--work", "command:X-1"]);
    let opened = cases(&repo);
    assert_eq!(
        opened.len(),
        1,
        "one case: {opened:?}\nstdout {out}\nstderr {err}"
    );
    let case = &opened[0];
    let id = name_of(case);
    assert!(tracker_case_id(&id, "x-1"), "the tracker scheme's id: {id}");
    assert_eq!(
        fs::read_to_string(case.join("intent.md")).expect("intent.md"),
        INTENT
    );
    assert_eq!(
        case_member(case, "item_key").as_deref(),
        Some("command:X-1")
    );
    assert_eq!(
        case_member(case, "item_revision").as_deref(),
        Some("2026-11-06T10:00:00Z")
    );
    assert!(
        !case.join("story.yaml").exists(),
        "an item of a tracker is no story of the planning store"
    );
    assert!(
        out.starts_with("work: read command:X-1: Fix the login\n"),
        "{out}"
    );

    // The same text given as --intent opens on the same protocol and ends the same way.
    let twin = self::scratch("read-twin", None);
    let (twin_code, twin_out, twin_err) = run(&twin.tree(), &["--intent", INTENT]);
    assert_eq!(
        code, twin_code,
        "as an --intent case: stdout {out} stderr {err}\n--intent: stdout {twin_out} stderr {twin_err}"
    );
    let twin_case = &cases(&twin.tree())[0];
    assert_eq!(
        fs::read_to_string(case.join("protocol.yaml")).expect("protocol.yaml"),
        fs::read_to_string(twin_case.join("protocol.yaml")).expect("protocol.yaml"),
        "the protocol an --intent case opens on"
    );

    // Resumed, not opened again, and `read` is not run for it.
    scratch.item("X-1.json", "the tracker is down\n");
    // The resumed case goes on from its first step, so it stops where that leads, not where a
    // fresh case stops: what decides is that it is the same case.
    let (_, out, err) = run(&repo, &["--config", config, "--work", "command:X-1"]);
    assert!(
        !err.contains("work_commands.read"),
        "read is not run: {err}"
    );
    assert!(
        out.starts_with(&format!(
            "frontier: resumed command:X-1: Fix the login (case {id})\n"
        )),
        "{out}"
    );
    assert_eq!(cases(&repo), opened, "no second case is opened");
}

/// Acceptance 2: a `select` printing nothing: the run says no ready work and exits as a run
/// with no story does, `frontier: nothing ready`, exit 0, no case.
#[test]
fn a_select_printing_nothing_is_no_ready_work() {
    let scratch = scratch("select-none", None);
    scratch.item("none.json", "");
    let config = scratch.config(
        "work = [\"command\"]\nwork_commands = { select = [\"cat\", \"{items}/none.json\"] }\n",
    );
    let repo = scratch.tree();
    let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(out, "frontier: nothing ready\n", "stderr {err}");
    assert!(cases(&repo).is_empty());

    // As a run with no story says it, on a store with none ready.
    let empty = self::scratch("select-none-store", Some("none-ready"));
    let (store_code, store_out, _) = run(&empty.tree(), &[]);
    assert_eq!((store_code, store_out.as_str()), (code, out.as_str()));
}

/// A `select` printing an item opens a case on it, its key and revision recorded.
#[test]
fn a_select_printing_an_item_opens_a_case_on_it() {
    let scratch = scratch("select-one", None);
    scratch.item("next.json", ITEM);
    let config = scratch.config(
        "work = [\"command\"]\nwork_commands = { select = [\"cat\", \"{items}/next.json\"] }\n",
    );
    let repo = scratch.tree();
    let (_, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "stdout {out} stderr {err}");
    assert!(tracker_case_id(&name_of(&opened[0]), "x-1"));
    assert_eq!(
        fs::read_to_string(opened[0].join("intent.md")).expect("intent.md"),
        INTENT
    );
    assert_eq!(
        case_member(&opened[0], "item_key").as_deref(),
        Some("command:X-1")
    );
    assert!(
        out.starts_with("work: selected command:X-1: Fix the login\n"),
        "{out}"
    );
}

/// The first source in list order that yields an item wins: `["command", "aep"]` with a
/// `select` printing nothing takes the store's ready story; `["aep", "command"]` on a store with
/// none ready opens the item `select` prints.
#[test]
fn the_first_source_in_list_order_that_yields_wins() {
    let scratch = scratch("order-aep", Some("one-ready"));
    scratch.item("none.json", "");
    let config = scratch.config(
        "work = [\"command\", \"aep\"]\nwork_commands = { select = [\"cat\", \"{items}/none.json\"] }\n",
    );
    let repo = scratch.tree();
    let (_, out, err) = run(
        &repo,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--protocol",
            "story.delivery/1",
        ],
    );
    assert!(
        out.starts_with("frontier: took story:ready-one: Make the check pass\n"),
        "stdout {out} stderr {err}"
    );

    let scratch = self::scratch("order-command", Some("none-ready"));
    scratch.item("next.json", ITEM);
    let config = scratch.config(
        "work = [\"aep\", \"command\"]\nwork_commands = { select = [\"cat\", \"{items}/next.json\"] }\n",
    );
    let repo = scratch.tree();
    let (_, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
    assert!(
        out.starts_with("work: selected command:X-1: Fix the login\n"),
        "stdout {out} stderr {err}"
    );
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(
        case_member(&opened[0], "item_key").as_deref(),
        Some("command:X-1")
    );
}

/// Acceptance 3: output that is not a `takt.work-item/1` document (not JSON, another format,
/// a field missing or of another type, a member the format does not have), or an item naming
/// another key than the one asked for, is refused before a case opens, exit 1, naming the
/// command and never repeating what it printed.
#[test]
fn output_that_is_no_item_or_names_another_key_is_refused_naming_the_command() {
    let scratch = scratch("refused", None);
    let config = scratch.config(
        "work = [\"command\"]\nwork_commands = { read = [\"cat\", \"{items}/{id}.json\"], select = [\"cat\", \"{items}/next.json\"] }\n",
    );
    let config = config.to_str().expect("utf-8");
    let repo = scratch.tree();
    let printed_marker = "PRINTED-W56";
    for (text, why) in [
        (
            format!("{printed_marker} is no JSON\n"),
            "not a takt.work-item/1 document",
        ),
        (
            ITEM.replace("takt.work-item/1", "takt.work-item/2")
                .replace("Fix the login", printed_marker),
            "its `format` is not `takt.work-item/1`",
        ),
        (
            ITEM.replace(r#""revision": "2026-11-06T10:00:00Z", "#, ""),
            "not a takt.work-item/1 document",
        ),
        (
            ITEM.replace(r#""labels": ["takt-ready"]"#, r#""labels": "takt-ready""#),
            "not a takt.work-item/1 document",
        ),
        (
            ITEM.replace(r#""url""#, r#""link""#),
            "not a takt.work-item/1 document",
        ),
        (
            ITEM.replace("command:X-1", "command:X-2")
                .replace("Fix the login", printed_marker),
            "names another key than command:X-1",
        ),
        (
            ITEM.replace("command:X-1", "jira:X-1"),
            "names another key than command:X-1",
        ),
    ] {
        scratch.item("X-1.json", &text);
        let (code, out, err) = run(&repo, &["--config", config, "--work", "command:X-1"]);
        assert_eq!(code, 1, "{text}: stdout {out} stderr {err}");
        assert!(
            err.contains("--work command:X-1") && err.contains("`work_commands.read`"),
            "{text}: the refusal names the key and the command: {err}"
        );
        assert!(err.contains(why), "{text}: ({why}) {err}");
        assert!(
            !err.contains(printed_marker) && !out.contains(printed_marker),
            "{text}: what the command printed is not repeated: {err}"
        );
        assert!(cases(&repo).is_empty(), "{text}: no case is opened");
    }

    // `select`'s item is any item of the source `command`; one of another source, or no item,
    // is refused naming `select`.
    for (text, why) in [
        (
            ITEM.replace("command:X-1", "jira:X-1"),
            "names no item of the source command",
        ),
        (
            format!("{printed_marker}\n"),
            "not a takt.work-item/1 document",
        ),
    ] {
        scratch.item("next.json", &text);
        let (code, out, err) = run(&repo, &["--config", config]);
        assert_eq!(code, 1, "{text}: stdout {out} stderr {err}");
        assert!(
            err.contains("`work_commands.select`") && err.contains(why),
            "{text}: ({why}) {err}"
        );
        assert!(!err.contains(printed_marker), "{err}");
        assert!(cases(&repo).is_empty(), "{text}: no case is opened");
    }
}

/// A `--work` key or a `work` entry that needs a command the run file's `work_commands` lacks is
/// refused before a case opens, naming the entry and the command.
#[test]
fn a_key_or_entry_whose_command_is_missing_is_refused_naming_it() {
    let scratch = scratch("missing", None);
    let repo = scratch.tree();
    let config = scratch.config("work_commands = { select = [\"cat\", \"{items}/next.json\"] }\n");
    for extra in [
        vec!["--work", "command:X-1"],
        vec![
            "--config",
            config.to_str().expect("utf-8"),
            "--work",
            "command:X-1",
        ],
    ] {
        let (code, out, err) = run(&repo, &extra);
        assert_eq!(code, 1, "{extra:?}: stdout {out} stderr {err}");
        assert!(
            err.contains("--work command:X-1") && err.contains("`work_commands.read`"),
            "{extra:?}: {err}"
        );
        assert_eq!(out, "", "{extra:?}");
        assert!(cases(&repo).is_empty());
    }
    for text in [
        "work = [\"command\"]\n",
        "work = [\"aep\", \"command\"]\nwork_commands = { read = [\"cat\", \"{items}/{id}.json\"] }\n",
    ] {
        let config = scratch.config(text);
        let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
        assert_eq!(code, 1, "{text}: stdout {out} stderr {err}");
        assert!(
            err.contains(&config.display().to_string())
                && err.contains("`work`")
                && err.contains("`command`")
                && err.contains("`work_commands.select`"),
            "{text}: {err}"
        );
        assert_eq!(out, "", "{text}");
        assert!(cases(&repo).is_empty());
    }
    // `command` takes no connection: the run file has one `work_commands` section.
    let (code, _, err) = run(&repo, &["--work", "command@team-a:X-1"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("the source command takes no connection"),
        "{err}"
    );
}

/// Acceptance 4: the command's child has no credential in its environment: a variable that
/// carries one in takt's environment does not reach `read`, which `printenv` shows by failing
/// for it, while a plain variable does reach it (so the probe sees the child's environment).
/// Neither the credential nor what the command printed is repeated.
#[test]
fn the_command_runs_with_no_credential_of_takts_environment() {
    const SECRET: &str = "w56-secret-value-of-the-tracker";
    let scratch = scratch("credential", None);
    let repo = scratch.tree();
    let env = [
        ("W56_TRACKER_TOKEN", SECRET),
        ("W56_TRACKER_PLAIN", "w56-plain-value"),
    ];
    let config =
        scratch.config("work_commands = { read = [\"printenv\", \"W56_TRACKER_TOKEN\"] }\n");
    let (code, out, err) = run_with(
        &repo,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--work",
            "command:X-1",
        ],
        &env,
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("the work command `work_commands.read` failed, exit 1"),
        "printenv found no credential to print: {err}"
    );
    assert!(!err.contains(SECRET) && !out.contains(SECRET), "{err}");
    assert!(cases(&repo).is_empty());

    let config =
        scratch.config("work_commands = { read = [\"printenv\", \"W56_TRACKER_PLAIN\"] }\n");
    let (code, out, err) = run_with(
        &repo,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--work",
            "command:X-1",
        ],
        &env,
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("not a takt.work-item/1 document"),
        "printenv printed the plain variable, no item: {err}"
    );
    assert!(!err.contains("w56-plain-value"), "{err}");
    assert!(cases(&repo).is_empty());
}

/// An item `select` offers again at the revision a finished case recorded opens no second case:
/// the run names the key and the finished case, takes the source as having no ready item and
/// goes on, here to no ready work. The same item at another revision is new work and opens a
/// case.
#[test]
fn select_offering_a_finished_item_again_opens_no_second_case() {
    let scratch = scratch("finished", None);
    scratch.item("next.json", ITEM);
    let config = scratch.config(
        "work = [\"command\"]\nwork_commands = { select = [\"cat\", \"{items}/next.json\"] }\n",
    );
    let config = config.to_str().expect("utf-8");
    let repo = scratch.tree();
    let (_, out, err) = run(&repo, &["--config", config]);
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "stdout {out} stderr {err}");
    let id = name_of(&opened[0]);
    // The case finishes: its latest decision has a legitimate outcome.
    let decisions = opened[0].join("decisions");
    fs::create_dir_all(&decisions).expect("the decisions directory");
    fs::write(
        decisions.join("9999.json"),
        r#"{"claims":{},"outcomes":{"completed":{"status":"legitimate"}},"explanation":{"computed_from":{"at":"2026-11-06T12:00:00Z"}}}"#,
    )
    .expect("a finished decision");

    let (code, out, err) = run(&repo, &["--config", config]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(
        out,
        format!(
            "work: command:X-1 has the finished case {id} at its revision; the source command has no ready item\nfrontier: nothing ready\n"
        ),
        "stderr {err}"
    );
    assert_eq!(cases(&repo), opened, "no second case is opened");

    scratch.item(
        "next.json",
        &ITEM.replace("2026-11-06T10:00:00Z", "2026-11-06T11:00:00Z"),
    );
    let (_, out, err) = run(&repo, &["--config", config, "--case-id", "x-1-again"]);
    assert!(
        out.starts_with("work: selected command:X-1: Fix the login\n"),
        "stdout {out} stderr {err}"
    );
    let again = cases(&repo);
    assert_eq!(again.len(), 2, "{again:?}");
    let second = again
        .iter()
        .find(|case| name_of(case) == "x-1-again")
        .expect("the new case");
    assert_eq!(
        case_member(second, "item_revision").as_deref(),
        Some("2026-11-06T11:00:00Z")
    );
}
