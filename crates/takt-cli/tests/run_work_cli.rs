//! `takt run --work <key>` and the run file's `work` list through the binary, over a copy of a
//! fixture planning store, as `run_story_cli.rs` builds it (story:work-flag-and-the-aep-source):
//! `--work aep:<id>` takes, resumes and refuses a story exactly as `--story <id>` does; a key of
//! any other source but `command` exits 1 before any case opens, naming the sources this release
//! serves;
//! `work = ["aep"]` and no `work` key select the next ready story as before, and an unknown
//! source, a filter or a source this release does not serve in `work` is refused; the case
//! records the item's key and the store's revision of the story beside its id.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::Frontier as _;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const STORIES: &str = ".engineering/planning/story";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A scratch git repository under this crate's build directory, removed on drop.
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-work-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A committed copy of the `one-ready` fixture store, with `story:second-ready`, ready and after
/// `story:ready-one` in id order, and `story:worked`, listed `active` with no case: the store
/// `run_story_cli.rs` runs `--story` against.
fn repository(name: &str) -> Scratch {
    let dir = scratch(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    let ready = fs::read_to_string(dir.join(STORIES).join("ready-one.md")).expect("ready-one.md");
    let second = ready
        .replace("ready-one", "second-ready")
        .replace("Make the check pass", "Make the second check pass")
        .replace("The one ready story", "A second ready story");
    fs::write(dir.join(STORIES).join("second-ready.md"), second).expect("a story");
    let worked = ready
        .replace("ready-one", "worked")
        .replace("Make the check pass", "A story being worked")
        .replace("The one ready story", "Active, with no case");
    fs::write(dir.join(STORIES).join("worked.md"), worked).expect("a story");
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    match takt::frontier::Aep::default()
        .take(&dir, "story:worked")
        .expect("the store moves story:worked")
    {
        takt::frontier::Take::Taken => {}
        takt::frontier::Take::Refused(refusal) => panic!("story:worked not moved: {refusal}"),
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// Runs the built `takt run` on `repo` with no harness, one iteration, a passing test command
/// and `story.delivery/1` unless `extra` names a protocol, plus `extra`: the exit code, stdout
/// and stderr.
fn run(repo: &Path, extra: &[&str]) -> (i32, String, String) {
    let pinned: &[&str] = if extra.contains(&"--protocol") {
        &[]
    } else {
        &["--protocol", "story.delivery/1"]
    };
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
        .args(pinned)
        .args(extra)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The status the store in `repo` lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    takt::frontier::Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
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

/// The `revision` the store holds `story` at in the take commit of the case `case_id`.
fn revision_at_take(repo: &Path, case_id: &str, story: &str) -> String {
    let log = git_out(repo, &["log", &format!("takt/{case_id}"), "--format=%H %s"]);
    let take = log
        .lines()
        .find_map(|line| line.strip_suffix(&format!(" takt: take {story}")))
        .unwrap_or_else(|| panic!("the take of {story} on takt/{case_id}: {log}"));
    let name = story.strip_prefix("story:").expect("a story id");
    let text = git_out(repo, &["show", &format!("{take}:{STORIES}/{name}.md")]);
    text.lines()
        .find_map(|line| line.strip_prefix("revision: "))
        .unwrap_or_else(|| panic!("the story states its revision: {text}"))
        .to_owned()
}

/// A run configuration file holding `text`, beside the scratch tree `repo` and out of it, so
/// the tree stays clean; named after it.
fn config_beside(repo: &Path, text: &str) -> PathBuf {
    let mut name = repo.file_name().expect("a name").to_owned();
    name.push(".config.toml");
    let path = repo.with_file_name(name);
    fs::write(&path, text).expect("a run configuration file");
    path
}

/// Acceptance 1, take: `--work aep:<id>` opens a case for the story it names, not the
/// frontier's next ready one, and the store records it taken, as `--story <id>` does; the case
/// records the key and the store's revision of the story in its take.
#[test]
fn an_aep_key_takes_the_story_as_story_does() {
    let repo = repository("taken");
    let (code, out, err) = run(&repo, &["--work", "aep:story:second-ready"]);
    assert_eq!(
        code, 7,
        "the gate with no gates document stops the run: stdout {out} stderr {err}"
    );
    assert!(
        out.starts_with("frontier: took story:second-ready: Make the second check pass\n"),
        "{out}"
    );
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "{opened:?}");
    let case = &opened[0];
    let id = case
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    assert!(id.starts_with("story-second-ready-"), "{id}");
    assert!(
        git_out(&repo, &["log", "--format=%s"])
            .lines()
            .any(|subject| subject == "takt: take story:second-ready"),
        "the take is committed on the case's branch"
    );
    assert_eq!(status(&repo, "story:second-ready"), "active");
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
    assert_eq!(
        case_member(case, "item_key").as_deref(),
        Some("aep:story:second-ready"),
        "the case records the key"
    );
    assert_eq!(
        case_member(case, "item_revision"),
        Some(revision_at_take(&repo, &id, "story:second-ready")),
        "the case records the store's revision of the story"
    );
}

/// A case `--story` opens records the same key: the story is the `aep` item either way.
#[test]
fn a_story_case_records_the_aep_key() {
    let repo = repository("story-key");
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(
        case_member(&opened[0], "item_key").as_deref(),
        Some("aep:story:second-ready")
    );
}

/// Acceptance 1, resume: a story with an open case named by `--work aep:<id>` resumes that
/// case, as `--story` does: no second case, nothing taken again. The case `--story` opened is
/// resumed by `--work`, and the other way round.
#[test]
fn an_aep_key_resumes_the_open_case_as_story_does() {
    for (first, second) in [
        (
            ["--story", "story:second-ready"],
            ["--work", "aep:story:second-ready"],
        ),
        (
            ["--work", "aep:story:second-ready"],
            ["--story", "story:second-ready"],
        ),
        (
            ["--work", "aep:story:second-ready"],
            ["--work", "aep:story:second-ready"],
        ),
    ] {
        let repo = repository(&format!("resume{}{}", first[0], second[0]));
        let (code, out, err) = run(&repo, &first);
        assert_eq!(code, 7, "{first:?}: stdout {out} stderr {err}");
        let opened = cases(&repo);
        assert_eq!(opened.len(), 1, "{opened:?}");
        let id = opened[0]
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        let (code, out, err) = run(&repo, &second);
        assert_eq!(code, 7, "{second:?}: stdout {out} stderr {err}");
        assert!(
            out.starts_with(&format!(
                "frontier: resumed story:second-ready: Make the second check pass (case {id})\n"
            )),
            "{first:?} then {second:?}: {out}"
        );
        assert!(!out.contains("frontier: took"), "{out}");
        assert_eq!(cases(&repo), opened, "no second case is opened");
    }
}

/// Acceptance 1, refuse: a story `--work aep:<id>` names that the store does not list or the
/// frontier would not take is refused before any case opens, exit 1, naming the key and the
/// same reason `--story` gives; the store is left as it was.
#[test]
fn an_aep_key_not_ready_is_refused_as_story_is() {
    for (story, why) in [
        ("story:no-such", "the store lists no story:no-such"),
        (
            "decision-blocker:open-question",
            "is a decision-blocker, not a story",
        ),
        ("story:draft-dep", "is draft, not proposed"),
        (
            "story:blocked-one",
            "blocked by decision-blocker:open-question",
        ),
        ("story:waits-one", "depends on story:draft-dep, draft"),
        (
            "story:worked",
            "is active, and no open case of it is under the tree to resume",
        ),
    ] {
        let repo = repository(&format!("refused-{}", story.replace(':', "-")));
        let key = format!("aep:{story}");
        let (code, out, err) = run(&repo, &["--work", &key]);
        assert_eq!(code, 1, "{key}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!("--work {key}")) && err.contains(why),
            "{key}: the refusal names the key and why ({why}): {err}"
        );
        assert!(!err.contains("--story"), "{key}: {err}");
        assert!(err.contains("no case was opened"), "{key}: {err}");
        assert!(!out.contains("frontier: took"), "{key}: {out}");
        assert!(cases(&repo).is_empty(), "{key}: no case is opened");
        assert_eq!(status(&repo, "story:ready-one"), "proposed", "{key}");
        assert_eq!(status(&repo, "story:second-ready"), "proposed", "{key}");
        assert_eq!(status(&repo, "story:draft-dep"), "draft", "{key}");
        assert_eq!(status(&repo, "story:worked"), "active", "{key}");
    }
}

/// Acceptance 2: a key of any source but `aep` and `command` (story:the-command-work-source)
/// exits 1 before any case opens, naming the sources this release serves; so does a key that
/// names no source takt knows, or that is no key, each naming why. Nothing is taken. A
/// `command` key is `run_work_command_cli.rs`'s.
#[test]
fn a_key_of_another_source_is_refused_naming_the_served_sources() {
    let repo = repository("other-sources");
    for key in [
        "jira:ABC-12",
        "jira@team-a:ABC-12",
        "github:owner/repo#42",
        "gitlab:group/project#7",
    ] {
        let (code, out, err) = run(&repo, &["--work", key]);
        assert_eq!(code, 1, "{key}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!("--work {key}"))
                && err.contains("this release serves the work sources aep and command"),
            "{key}: the refusal names the key and the sources served: {err}"
        );
        assert_eq!(out, "", "{key}");
        assert!(cases(&repo).is_empty(), "{key}: no case is opened");
    }
    for (key, why) in [
        ("linear:X-1", "`linear` is no work source"),
        ("story:ready-one", "`story` is no work source"),
        ("aep", "a key is `<source>[@<connection>]:<id>`"),
        ("aep:", "names no id"),
        ("jira@:ABC-12", "connection"),
        ("jira@-a:ABC-12", "connection"),
        (
            "aep@main:story:ready-one",
            "the source aep takes no connection",
        ),
    ] {
        let (code, out, err) = run(&repo, &["--work", key]);
        assert_eq!(code, 1, "{key}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!("--work {key}")) && err.contains(why),
            "{key}: the refusal names the key and why ({why}): {err}"
        );
        assert_eq!(out, "", "{key}");
        assert!(cases(&repo).is_empty(), "{key}: no case is opened");
    }
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
    assert_eq!(status(&repo, "story:second-ready"), "proposed");
}

/// `--work` names the item, as `--story`, `--intent` and `--case` do: given with any of them it
/// is refused by the command line, exit 2, nothing read or written.
#[test]
fn work_with_story_intent_or_case_is_refused_by_the_command_line() {
    let repo = repository("conflicts");
    for other in [
        ["--story", "story:second-ready"],
        ["--intent", "do something"],
        ["--case", "some-case"],
    ] {
        let (code, out, err) = run(
            &repo,
            &["--work", "aep:story:second-ready", other[0], other[1]],
        );
        assert_eq!(code, 2, "{other:?}: stdout {out} stderr {err}");
        assert!(
            err.contains("--work") && err.contains(other[0]) && err.contains("cannot be used"),
            "{other:?}: {err}"
        );
        assert_eq!(out, "", "{other:?}");
        assert!(cases(&repo).is_empty(), "{other:?}");
        assert_eq!(status(&repo, "story:second-ready"), "proposed", "{other:?}");
    }
}

/// A `self.improvement/1` turn opens on its finding: `--work aep:<id>` with it is refused as
/// `--story` is, naming the key, exit 1, nothing written.
#[test]
fn an_aep_key_with_an_improvement_turn_is_refused() {
    let repo = repository("turn");
    let (code, out, err) = run(
        &repo,
        &[
            "--work",
            "aep:story:second-ready",
            "--protocol",
            "self.improvement/1",
        ],
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains(
            "--work aep:story:second-ready: a self.improvement/1 turn opens on its finding"
        ),
        "{err}"
    );
    assert!(cases(&repo).is_empty());
    assert_eq!(status(&repo, "story:second-ready"), "proposed");
}

/// Acceptance 3: `work = ["aep"]` in the run file selects the next ready story as a run with no
/// `work` key does: the first ready one in id order, taken, recorded with its key.
#[test]
fn work_aep_and_no_work_key_take_the_next_ready_story() {
    for (name, text) in [
        ("work-aep", "work = [\"aep\"]\n"),
        ("no-work", "iterations = 1\n"),
    ] {
        let repo = repository(name);
        let config = config_beside(&repo, text);
        let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
        let _ = fs::remove_file(&config);
        assert_eq!(code, 7, "{name}: stdout {out} stderr {err}");
        assert!(
            out.starts_with("frontier: took story:ready-one: Make the check pass\n"),
            "{name}: {out}"
        );
        assert_eq!(status(&repo, "story:ready-one"), "active", "{name}");
        assert_eq!(status(&repo, "story:second-ready"), "proposed", "{name}");
        let opened = cases(&repo);
        assert_eq!(opened.len(), 1, "{name}: {opened:?}");
        assert_eq!(
            case_member(&opened[0], "item_key").as_deref(),
            Some("aep:story:ready-one"),
            "{name}"
        );
    }
}

/// Acceptance 3: a `work` entry that names no source takt knows is refused, naming it; a
/// filter after `?` is refused (decision 6); a known source this release does not serve is
/// refused naming the sources it serves. Each before any case opens, exit 1, the file named.
#[test]
fn an_unknown_filtered_or_unserved_work_source_is_refused() {
    let repo = repository("work-refused");
    for (text, why) in [
        ("work = [\"linear\"]\n", "`linear` is no work source"),
        ("work = [\"aep\", \"Jira\"]\n", "`Jira` is no work source"),
        (
            "work = [\"jira@team-a?label=takt-ready\"]\n",
            "a filter after `?` is refused in this release",
        ),
        (
            "work = [\"jira\"]\n",
            "this release serves the work sources aep and command",
        ),
        (
            "work = [\"aep\", \"github\"]\n",
            "this release serves the work sources aep and command",
        ),
        ("work = [\"jira@\"]\n", "connection"),
        ("work = []\n", "lists no source"),
    ] {
        let config = config_beside(&repo, text);
        let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
        let _ = fs::remove_file(&config);
        assert_eq!(code, 1, "{text}: stdout {out} stderr {err}");
        assert!(
            err.contains(&config.display().to_string()) && err.contains("`work`"),
            "{text}: the refusal names the file and the key: {err}"
        );
        assert!(err.contains(why), "{text}: ({why}) {err}");
        assert_eq!(out, "", "{text}");
        assert!(cases(&repo).is_empty(), "{text}: no case is opened");
    }
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
}

/// A file's `story` names the work only when no flag does: `--work` wins over it, as `--story`
/// does.
#[test]
fn work_wins_over_the_configurations_story() {
    let repo = repository("config-work-wins");
    let config = config_beside(&repo, "story = \"story:draft-dep\"\n");
    let (code, out, err) = run(
        &repo,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--work",
            "aep:story:second-ready",
        ],
    );
    let _ = fs::remove_file(&config);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with("frontier: took story:second-ready"),
        "{out}"
    );
    assert_eq!(status(&repo, "story:draft-dep"), "draft");
}
