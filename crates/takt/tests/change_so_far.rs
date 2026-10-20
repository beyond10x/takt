//! story:step-prompt-omits-the-change-so-far: each judgement step's prompt carries the change
//! the case has made so far, fenced as data after the obligations and before the offered
//! actions: the diff from the case's `base_commit` to `HEAD` over the paths its accepted steps
//! committed, read with no configured code; a line that says no change has been made yet; a
//! line that says the change is not known for a case with no `base_commit`; or, past the bound
//! or when the scan does not pass, a line that says it is not shown. No live `claude` runs: the
//! harness program is a script that keeps each prompt it is given.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::Branch;
use takt::harness::ClaudeCode;
use takt::run::ChangeScan;
use takt::validator::Validator;
use takt::{Run, RunOptions};

/// The label every block of the change so far opens with.
const LABEL: &str = "The change this case has made so far";

/// A scratch directory under this crate's build directory, removed when dropped.
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

/// An empty scratch directory named for this process and `name`.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("change-so-far-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A committed repository on `main` with a README and `files`.
fn repository(name: &str, files: &[(&str, &[u8])]) -> Scratch {
    let tree = directory(name);
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    for (path, bytes) in files {
        let path = tree.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("its directory");
        fs::write(path, bytes).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "show the change so far".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "standard".to_owned()),
            ("change_kind".to_owned(), "bugfix".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// A fake `claude` in `dir`. Each call counts itself in `dir/calls`, keeps the prompt it was
/// given (the argument after `-p`) as `dir/prompt-<n>`, runs `edit` (shell text, `$n` the call)
/// in the tree, and reports `tests.write`.
fn fake_claude(dir: &Path, edit: &str) -> ClaudeCode {
    fake_claude_acting(dir, edit, "tests.write")
}

/// [`fake_claude`] reporting `action`.
fn fake_claude_acting(dir: &Path, edit: &str, action: &str) -> ClaudeCode {
    let script = r#"#!/bin/sh
d='@DIR@'
n=$(cat "$d/calls" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$d/calls"
printf '%s' "$2" > "$d/prompt-$n"
@EDIT@
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"@ACTION@\", \"summary\": \"made a change\"}"}'
"#
    .replace("@DIR@", &dir.display().to_string())
    .replace("@EDIT@", edit)
    .replace("@ACTION@", action);
    let program = dir.join("claude");
    fs::write(&program, script).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    }
}

/// An executable script `name` in `dir` holding `body` after its `#!/bin/sh` line.
fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("exec bit");
    path
}

/// The edit each call makes: one line naming the call appended to `tests/work.txt`.
const APPEND: &str = r#"mkdir -p tests
echo "committed-by-call-$n" >> tests/work.txt"#;

/// Runs a case `id` opened (`resume` false) or resumed on `tree` for `iterations` steps with
/// `harness`, the run's scan of the change so far `scan`; the run's output.
fn run_case(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &ClaudeCode,
    resume: bool,
    scan: Option<ChangeScan>,
) -> String {
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = if resume {
        Run::resume(tree, options(id, iterations), id, harness, &validators)
            .expect("the case resumes")
    } else {
        Run::open(tree, options(id, iterations), harness, &validators).expect("the case opens")
    };
    run.scan_change_with(scan);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    text
}

/// The prompt the fake was given at its call `n`.
fn prompt(scripts: &Path, n: usize) -> String {
    fs::read_to_string(scripts.join(format!("prompt-{n}")))
        .unwrap_or_else(|error| panic!("the prompt of call {n}: {error}"))
}

/// The block of the change so far in `prompt`: from its label to the offered actions, which it
/// stands between the obligations and.
fn block(prompt: &str) -> &str {
    let obligations = prompt
        .find("\nOpen obligations:\n")
        .unwrap_or_else(|| panic!("the obligations: {prompt}"));
    let start = prompt
        .find(LABEL)
        .unwrap_or_else(|| panic!("the change so far in the prompt: {prompt}"));
    let offered = prompt
        .find("\nYou may perform exactly ONE of these actions now")
        .unwrap_or_else(|| panic!("the offered actions: {prompt}"));
    assert!(
        obligations < start && start < offered,
        "the change so far stands after the obligations and before the offered actions: {prompt}"
    );
    assert_eq!(
        prompt.matches(LABEL).count(),
        1,
        "one block of the change so far: {prompt}"
    );
    &prompt[start..offered]
}

/// The fenced part of `block`: the opening fence, the lines between, the closing fence; the
/// fence a run of backticks the text inside holds no run as long as.
fn fenced(block: &str) -> (String, Vec<&str>) {
    let lines: Vec<&str> = block.lines().collect();
    let open = lines
        .iter()
        .position(|line| line.starts_with("```"))
        .unwrap_or_else(|| panic!("a fence: {block}"));
    let fence: String = lines[open].chars().take_while(|c| *c == '`').collect();
    let close = lines
        .iter()
        .rposition(|line| *line == fence)
        .unwrap_or_else(|| panic!("the closing fence {fence}: {block}"));
    assert!(open < close, "the fence closes after it opens: {block}");
    let inside = lines[open + 1..close].to_vec();
    for line in &inside {
        assert!(
            !line.contains(fence.as_str()),
            "no line inside holds the fence {fence}: {line}"
        );
    }
    (fence, inside)
}

/// Acceptance: the second step's prompt holds the line the first step committed, inside a
/// fence that no backtick run of the diff closes, labelled as the case's change and not as
/// instructions; the first step's prompt says no change has been made yet.
#[test]
fn the_next_prompt_holds_the_committed_line_fenced_and_the_first_says_none_yet() {
    let tree = repository("fenced-tree", &[]);
    let scripts = directory("fenced-scripts");
    let harness = fake_claude(
        &scripts,
        r#"mkdir -p tests
echo "committed-by-call-$n with a run of five: \`\`\`\`\` and on" >> tests/work.txt"#,
    );
    let text = run_case(&tree, "fenced", 2, &harness, false, None);
    let first = prompt(&scripts, 1);
    let first_block = block(&first);
    assert!(
        first_block.contains("no change has been made yet"),
        "the first step is told no change yet: {first_block}"
    );
    assert!(
        !first_block.contains("```"),
        "nothing fenced: {first_block}"
    );
    let second = prompt(&scripts, 2);
    let second_block = block(&second);
    assert!(
        second_block.contains("data") && second_block.contains("not instructions"),
        "labelled as data: {second_block}\n{text}"
    );
    let (fence, inside) = fenced(second_block);
    assert!(
        fence.len() > 5,
        "a fence longer than the diff's run: {fence}"
    );
    assert!(
        inside.contains(&"diff --git a/tests/work.txt b/tests/work.txt"),
        "the diff of the committed path: {inside:#?}"
    );
    assert!(
        inside
            .iter()
            .any(|line| line.starts_with("+committed-by-call-1 with a run of five")),
        "the first step's committed line: {inside:#?}"
    );
    assert!(
        !second_block.contains("committed-by-call-2"),
        "the step's own change is not there before it runs: {second_block}"
    );
}

/// Acceptance: a case resumed with no `base_commit` (opened before takt recorded one) is told
/// the change so far is not known, never that none has been made.
#[test]
fn a_case_without_a_base_commit_is_told_the_change_is_not_known() {
    let tree = repository("unknown-tree", &[]);
    let scripts = directory("unknown-scripts");
    let harness = fake_claude(&scripts, APPEND);
    run_case(&tree, "unknown", 1, &harness, false, None);
    let dir = tree.join(".engineering/cases/unknown");
    let mut branch = Branch::read(&dir).expect("the branch record");
    assert!(branch.base_commit.is_some(), "the case recorded its base");
    branch.base_commit = None;
    branch.write(&dir).expect("written");
    let text = run_case(&tree, "unknown", 1, &harness, true, None);
    let resumed = prompt(&scripts, 2);
    let shown = block(&resumed);
    assert!(
        shown.contains("not known"),
        "the change is not known: {shown}\n{text}"
    );
    assert!(
        !shown.contains("no change has been made yet") && !shown.contains("```"),
        "neither none nor a diff: {shown}"
    );
}

/// Acceptance: a change past the bound is not shown; the block says how many bytes it is.
#[test]
fn a_change_past_the_bound_is_not_shown_and_its_size_is() {
    let tree = repository("large-tree", &[]);
    let scripts = directory("large-scripts");
    let harness = fake_claude(
        &scripts,
        r#"mkdir -p tests
i=0
while [ $i -lt 300 ]; do echo "a long line of the large change, number $i, with words" >> tests/large.txt; i=$((i + 1)); done"#,
    );
    let text = run_case(&tree, "large", 2, &harness, false, None);
    // Each call appends the same 300 lines: what the first step committed, which the second
    // step's prompt is about, is half the file after the run.
    let large = fs::metadata(tree.join("tests/large.txt"))
        .expect("the large file")
        .len()
        / 2;
    assert!(large > 9000, "the change is past the bound: {large}");
    let second = prompt(&scripts, 2);
    let shown = block(&second);
    assert!(
        shown.contains("not shown") && !shown.contains("```") && !shown.contains("number 7,"),
        "not shown: {shown}\n{text}"
    );
    let size: u64 = shown
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .max()
        .unwrap_or_else(|| panic!("a size: {shown}"));
    assert!(
        size > large,
        "the size is the diff's, the file's lines and its header: {size} against {large}: {shown}"
    );
}

/// Acceptance: the repository's own configuration (`diff.external`, `color.ui=always`, a wider
/// context, no prefix) changes nothing in the block: it is read with no configured code and
/// gives the same bytes as a repository without that configuration.
#[test]
fn a_configured_external_diff_and_color_change_nothing_in_the_block() {
    let mut blocks = Vec::new();
    for configured in [false, true] {
        let name = if configured { "configured" } else { "plain" };
        let tree = repository(&format!("{name}-tree"), &[]);
        let scripts = directory(&format!("{name}-scripts"));
        if configured {
            let external = script(
                &scripts,
                "external",
                &format!("echo external-ran >> '{}/external-ran'", scripts.display()),
            );
            for (key, value) in [
                ("diff.external", external.display().to_string()),
                ("color.ui", "always".to_owned()),
                ("color.diff", "always".to_owned()),
                ("diff.context", "9".to_owned()),
                ("diff.noprefix", "true".to_owned()),
                ("diff.algorithm", "patience".to_owned()),
            ] {
                git(&tree, &["config", key, &value]);
            }
        }
        let harness = fake_claude(&scripts, APPEND);
        let text = run_case(&tree, name, 2, &harness, false, None);
        assert!(
            !scripts.join("external-ran").exists(),
            "no external diff ran\n{text}"
        );
        let second = prompt(&scripts, 2);
        let shown = block(&second).to_owned();
        assert!(!shown.contains('\x1b'), "no color: {shown:?}");
        assert!(
            shown.contains("diff --git a/tests/work.txt b/tests/work.txt")
                && shown.contains("+committed-by-call-1"),
            "the patch: {shown}\n{text}"
        );
        blocks.push(shown);
    }
    assert_eq!(
        blocks[0], blocks[1],
        "the same block with and without the configuration"
    );
}

/// Acceptance: a committed file whose NUL lies past byte 8000, which git shows as text, does
/// not stop the next step: the prompt is one argument, which cannot hold a NUL, and the block
/// shows the control replaced.
#[test]
fn a_nul_past_byte_8000_does_not_stop_the_next_step() {
    let mut data = Vec::new();
    while data.len() < 8100 {
        data.extend_from_slice(b"a line of filler before the control byte\n");
    }
    data.extend_from_slice(b"nul\0here and a bell\x07 too\n");
    data.extend_from_slice(b"the last line\n");
    let tree = repository("nul-tree", &[("tests/data.txt", &data)]);
    let scripts = directory("nul-scripts");
    let harness = fake_claude(&scripts, r#"echo "after-the-nul-$n" >> tests/data.txt"#);
    let text = run_case(&tree, "nul", 2, &harness, false, None);
    let calls = fs::read_to_string(scripts.join("calls")).expect("the calls");
    assert_eq!(calls.trim(), "2", "the second step ran\n{text}");
    let second = prompt(&scripts, 2);
    let shown = block(&second);
    let (_, inside) = fenced(shown);
    assert!(
        inside
            .iter()
            .any(|line| line.starts_with("+after-the-nul-1")),
        "the committed line: {inside:#?}\n{text}"
    );
    assert!(
        inside
            .iter()
            .any(|line| line.contains("nul\u{fffd}here and a bell\u{fffd} too")),
        "the controls replaced in the context line: {inside:#?}"
    );
    assert!(
        !second
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t'),
        "no control but a line end and a tab: {second:?}"
    );
}

/// Acceptance: a commit on the case's branch that no accepted step made stays out of the block:
/// the diff is read over the paths the steps' `effect` lines name.
#[test]
fn a_commit_no_accepted_step_made_stays_out_of_the_block() {
    let tree = repository("other-tree", &[]);
    let scripts = directory("other-scripts");
    let harness = fake_claude(&scripts, APPEND);
    run_case(&tree, "other", 1, &harness, false, None);
    fs::write(tree.join("notes.txt"), "a-line-no-step-committed\n").expect("a file");
    git(&tree, &["add", "notes.txt"]);
    git(&tree, &["commit", "-q", "-m", "by hand"]);
    let text = run_case(&tree, "other", 1, &harness, true, None);
    let resumed = prompt(&scripts, 2);
    let shown = block(&resumed);
    assert!(
        shown.contains("+committed-by-call-1"),
        "the step's committed line: {shown}\n{text}"
    );
    assert!(
        !shown.contains("a-line-no-step-committed") && !shown.contains("notes.txt"),
        "the commit by hand stays out: {shown}"
    );
}

/// Acceptance: with a policy, the diff passes the scan before a prompt shows it. A scan that
/// refuses and a scan that cannot run (the program not there) both give "not shown", never the
/// diff; a scan that passes shows it, and was given the policy and the diff.
#[test]
fn with_a_policy_a_scan_that_refuses_or_cannot_run_gives_not_shown() {
    let scripts = directory("scan-scripts");
    let policy = scripts.join("policy.toml");
    fs::write(&policy, "a policy\n").expect("a policy");
    let passes = script(
        &scripts,
        "passes",
        &format!(
            "printf '%s\\n' \"$@\" > '{0}/passes-args'\nfor last; do :; done\ncat \"$last\" > '{0}/passes-scanned'\nexit 0",
            scripts.display()
        ),
    );
    let refuses = script(&scripts, "refuses", "echo refused >&2\nexit 1");
    let absent = scripts.join("no-such-program");
    for (name, program, shown) in [
        ("passes", passes, true),
        ("refuses", refuses, false),
        ("absent", absent, false),
    ] {
        let tree = repository(&format!("scan-{name}-tree"), &[]);
        let calls = directory(&format!("scan-{name}-calls"));
        let harness = fake_claude(&calls, APPEND);
        let scan = ChangeScan {
            program: program.clone(),
            policy: policy.clone(),
        };
        let text = run_case(&tree, name, 2, &harness, false, Some(scan));
        let second = prompt(&calls, 2);
        let block = block(&second);
        if shown {
            assert!(
                block.contains("+committed-by-call-1"),
                "{name}: shown: {block}\n{text}"
            );
            let args = fs::read_to_string(scripts.join("passes-args")).expect("the scan's args");
            let args: Vec<&str> = args.lines().collect();
            assert!(
                args.first() == Some(&"scan-text")
                    && args
                        .windows(2)
                        .any(|pair| pair == ["--policy", &policy.display().to_string()]),
                "{name}: scan-text under the policy: {args:?}"
            );
            let scanned =
                fs::read_to_string(scripts.join("passes-scanned")).expect("what was scanned");
            assert!(
                scanned.contains("+committed-by-call-1"),
                "{name}: the diff was scanned: {scanned}"
            );
        } else {
            assert!(
                block.contains("not shown")
                    && !block.contains("committed-by-call-1")
                    && !block.contains("```"),
                "{name}: not shown: {block}\n{text}"
            );
        }
    }
}

/// `count` numbered lines, `<word> line <n>`.
fn numbered(word: &str, count: usize) -> Vec<u8> {
    (1..=count)
        .map(|n| format!("{word} line {n}\n"))
        .collect::<String>()
        .into_bytes()
}

/// A protocol whose judgement step edits the implementation from the first step on.
const EDIT: &str = "format: protocol/1

protocol:
  id: edit.only
  revision: 1
  description: >-
    Edit the implementation; accepted once the tests pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// Opens a case `id` on `tree` under [`EDIT`] (written to `scripts`) and runs 2 steps with
/// `harness`, the change so far unscanned; the run's output.
fn run_edits(tree: &Path, scripts: &Path, id: &str, harness: &ClaudeCode) -> String {
    let protocol = scripts.join("protocol.yaml");
    fs::write(&protocol, EDIT).expect("the protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        inputs: Vec::new(),
        gate: None,
        ..options(id, 2)
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(tree, options, harness, &validators).expect("the case opens");
    run.scan_change_with(None);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    text
}

/// The edit of the step that changes an implementation file and a test file: one line appended
/// to each.
const BOTH: &str = r#"echo "app-change-$n" >> app.py
echo "test-change-$n" >> tests/test_app.py"#;

/// Acceptance (re-trial 1): the step that hands over needs no file read. Each file that is not
/// a test path is shown whole, as it is now, every line of it; each test file shows its changed
/// lines with 3 around each; and the block says it is the complete change and that every line
/// and file not shown is as at the commit the case forked from.
#[test]
fn each_file_that_is_not_a_test_is_shown_whole_and_the_block_says_it_is_complete() {
    let app = numbered("app", 30);
    let tests = numbered("test", 30);
    let tree = repository(
        "whole-tree",
        &[("app.py", &app), ("tests/test_app.py", &tests)],
    );
    let scripts = directory("whole-scripts");
    let harness = fake_claude_acting(&scripts, BOTH, "implementation.edit");
    let text = run_edits(&tree, &scripts, "whole", &harness);
    let second = prompt(&scripts, 2);
    let shown = block(&second);
    let (_, inside) = fenced(shown);
    for n in 1..=30 {
        assert!(
            inside.contains(&format!(" app line {n}").as_str()),
            "app.py is shown whole, line {n}: {inside:#?}\n{text}"
        );
    }
    assert!(
        inside.contains(&"+app-change-1") && inside.contains(&"+test-change-1"),
        "both changes: {inside:#?}"
    );
    assert!(
        inside.contains(&" test line 28") && !inside.contains(&" test line 26"),
        "the test file shows its change with 3 lines around it: {inside:#?}"
    );
    let label = &shown[..shown.find("```").expect("a fence")];
    assert!(
        label.contains("complete")
            && label.contains("shown whole")
            && label.contains("as at the commit"),
        "the block says it is the whole change: {label}"
    );
}

/// Acceptance (re-trial 1): a file that is not a test and whose whole would put the diff past
/// the bound is shown as its changed lines with 3 around each, like every other file then, and
/// the block does not say it is shown whole.
#[test]
fn a_file_too_large_to_show_whole_shows_its_changed_lines() {
    let app = numbered("a longer application", 400);
    assert!(app.len() > 8192, "past the bound whole: {}", app.len());
    let tree = repository(
        "hunks-tree",
        &[
            ("app.py", &app),
            ("tests/test_app.py", &numbered("test", 30)),
        ],
    );
    let scripts = directory("hunks-scripts");
    let harness = fake_claude_acting(&scripts, BOTH, "implementation.edit");
    let text = run_edits(&tree, &scripts, "hunks", &harness);
    let second = prompt(&scripts, 2);
    let shown = block(&second);
    let (_, inside) = fenced(shown);
    assert!(
        inside.contains(&"+app-change-1")
            && inside.contains(&" a longer application line 398")
            && !inside.contains(&" a longer application line 1"),
        "app.py's changed lines with 3 around them: {inside:#?}\n{text}"
    );
    assert!(
        inside.contains(&"+test-change-1"),
        "the test file's change: {inside:#?}"
    );
    let label = &shown[..shown.find("```").expect("a fence")];
    assert!(
        label.contains("complete") && !label.contains("shown whole"),
        "complete, not whole: {label}"
    );
}
