//! story:a-delivery-hands-its-candidate-to-a-turn through the binary: a `--forge none` delivery
//! with a gates document works up to `change.read` ahead and stops `Unperformable` at
//! `pr.opened`, exit 7, printing `candidate: <full id>`: its code commits replayed on `main`'s
//! tip, the take left out. A `self.improvement/1` turn opened on that id passes its opening
//! checks with no operator step between. With no identity configured the stop and the exit stay,
//! and the line names the setting instead of a candidate. A resume that can no longer make one
//! says so and leaves no ref naming the earlier candidate.
//!
//! The harness is a `claude` double on the program path; `b10x-gates` and `gh` are doubles that
//! log each call and fail. The promotion store, the state and cache homes and the install root
//! are the test's own; nothing reaches a network, a model or the bot route.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The finding scripts of this repository, committed into the scratch repository's base, as a
/// turn needs a confined `scripts/check-finding` at its baseline.
const SCRIPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The story the delivery takes, which the turn verifies as its finding.
const STORY: &str = "story:ready-one";
const STORY_FILE: &str = ".engineering/planning/story/ready-one.md";

/// The identity the repository configures.
const IDENTITY: &str = "The Bot <bot@example.invalid>";

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

/// A scratch directory under this crate's build directory, removed on drop.
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

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

fn sha256(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum runs");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("a digest")
        .to_owned()
}

/// The scratch world: `repo`, a committed copy of the `one-ready` store (its story `proposed`)
/// with a manifest, a gates document, the finding scripts and, when `identity`,
/// `.engineering/commit-identity.conf`, its one commit on `main` the promoted baseline; the
/// promotion store under `state`, its baseline that commit, stated by the operator for the test
/// binary; an install root; and `bin`, a `claude` double that edits `work.txt` once and hands the
/// change over when offered, and `b10x-gates` and `gh` doubles that log each call to `route` and
/// fail.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    baseline: String,
    bin: PathBuf,
    state: PathBuf,
    cache: PathBuf,
    install: PathBuf,
    route: PathBuf,
}

fn world(name: &str, identity: bool) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "delivery-candidate-cli-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    // The store's project file without its pinned protocol source, which `aep` would fetch from
    // a remote into a cache this test does not share: the store reads with the protocols `aep`
    // ships.
    let project = repo.join(".engineering/project.yaml");
    let text = fs::read_to_string(&project).expect("the project file");
    let unpinned: String = text
        .lines()
        .filter(|line| !line.starts_with("protocols:"))
        .map(|line| format!("{line}\n"))
        .collect();
    write(&project, &unpinned);
    write(&repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n");
    write(&repo.join(".engineering/gates.yaml"), GATES);
    if identity {
        write(
            &repo.join(".engineering/commit-identity.conf"),
            "[identity]\n\tname = The Bot\n\temail = bot@example.invalid\n",
        );
    }
    for script in ["check-finding", "file-finding"] {
        let text = fs::read_to_string(Path::new(SCRIPTS).join(script)).expect("the script");
        executable(&repo.join("scripts").join(script), &text);
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let baseline = git(&repo, &["rev-parse", "HEAD"]);

    let state = outer.join("state");
    let store = state.join("takt/promotions");
    let scorecard = store.join(format!("{baseline}.scorecard.json"));
    write(
        &scorecard,
        &format!(
            "{{\"format\":\"{}\",\"revision\":\"{baseline}\",\"named_by\":\"build\",\"metrics\":{{}},\"rule\":{{\"trials\":5,\"margin\":1}},\"rows\":[],\"no_record\":[],\"unstated\":{{}}}}",
            takt::scorecard::FORMAT
        ),
    );
    let tree = git(&repo, &["rev-parse", &format!("{baseline}^{{tree}}")]);
    write(
        &store.join(format!("{baseline}.json")),
        &format!(
            "{{\"format\":\"takt-promotion/1\",\"revision\":\"{baseline}\",\"tree\":\"{tree}\",\"promoted_at\":\"2026-01-01T00:00:00Z\",\"kind\":\"baseline\",\"scorecard\":\"{baseline}\",\"scorecard_file\":\"/srv/scorecards/scorecard-{baseline}.json\",\"scorecard_sha256\":\"{}\",\"against\":null,\"rule\":{{\"trials\":5,\"margin\":1}},\"worse\":0,\"binary\":null,\"binary_sha256\":\"{}\",\"revision_stated_by\":\"operator\",\"previous_sha256\":null}}",
            sha256(&scorecard),
            sha256(Path::new(env!("CARGO_BIN_EXE_takt")))
        ),
    );
    let cache = outer.join("cache");
    let install = outer.join("install");
    fs::create_dir_all(&install).expect("the install root");

    let bin = outer.join("stub-bin");
    executable(
        &bin.join("claude"),
        r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
case "$2" in
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else echo "the work" >> work.txt; action=implementation.edit; fi ;;
  *) echo "the work" >> work.txt; action=implementation.edit ;;
esac
printf '{"result":"{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}}' "$action"
"#,
    );
    let route = outer.join("route-calls");
    let refusing = format!(
        "#!/bin/sh\necho \"$0 $*\" >> '{}'\necho 'the route double refuses' >&2\nexit 1\n",
        route.display()
    );
    for name in ["b10x-gates", "gh"] {
        executable(&bin.join(name), &refusing);
    }
    World {
        _scratch: Scratch(outer),
        repo,
        baseline,
        bin,
        state,
        cache,
        install,
        route,
    }
}

impl World {
    /// The built `takt` with `args`: the doubles first on the program path, the state and cache
    /// homes and the install root the test's own. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
            .env("XDG_STATE_HOME", &self.state)
            .env("XDG_CACHE_HOME", &self.cache)
            .env("CARGO_INSTALL_ROOT", &self.install)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The delivery: `takt run` taking the next ready story under `--forge none`.
    fn deliver(&self) -> (i32, String, String) {
        self.takt(&[
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--test-cmd",
            "true",
            "--forge",
            "none",
            "--iterations",
            "20",
            "--min-free-gib",
            "0",
        ])
    }

    /// The turn `case` opened on `candidate` for the delivered story, no step taken after it
    /// opens.
    fn turn(&self, case: &str, candidate: &str) -> (i32, String, String) {
        self.takt(&[
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--protocol",
            "self.improvement/1",
            "--input",
            &format!("finding={STORY}"),
            "--input",
            &format!("candidate={candidate}"),
            "--case-id",
            case,
            "--install-root",
            self.install.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "0",
            "--min-free-gib",
            "0",
        ])
    }

    /// The delivery's case `case` resumed under `--forge none`.
    fn resume(&self, case: &str) -> (i32, String, String) {
        self.takt(&[
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--case",
            case,
            "--test-cmd",
            "true",
            "--forge",
            "none",
            "--iterations",
            "20",
            "--min-free-gib",
            "0",
        ])
    }

    /// The calls the route's doubles logged.
    fn route_calls(&self) -> String {
        fs::read_to_string(&self.route).unwrap_or_default()
    }
}

/// The full commit id a `candidate: <id>` line of `text` names, if one does.
fn candidate_of(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("candidate: "))
        .find(|id| id.len() == 40 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned)
}

/// The story's acceptance, end to end: the delivery stops at ahead, exit 7, printing a candidate
/// that holds the code commit on `main`'s tip and not the take, under the configured identity;
/// the turn opened on it passes its opening checks (its case records the candidate) with no
/// operator step between; the route's doubles are never called.
#[test]
fn a_forge_none_delivery_hands_a_turn_its_candidate() {
    let world = world("handover", true);
    let (code, out, err) = world.deliver();
    assert!(
        out.contains("stopped: Unperformable (pr.opened; nothing performs it without the forge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let candidate =
        candidate_of(&out).unwrap_or_else(|| panic!("no candidate: stdout {out} stderr {err}"));

    let main = git(&world.repo, &["rev-parse", "main"]);
    assert_eq!(main, world.baseline);
    let subjects = git(
        &world.repo,
        &["log", "--format=%s", &format!("{main}..{candidate}")],
    );
    assert!(!subjects.is_empty(), "no code commit: {out}");
    assert!(
        !subjects.contains("takt: take"),
        "the take rode on the candidate: {subjects}"
    );
    assert_eq!(
        git(&world.repo, &["show", &format!("{candidate}:work.txt")]),
        "the work"
    );
    assert_eq!(
        git(&world.repo, &["show", &format!("{candidate}:{STORY_FILE}")]),
        git(&world.repo, &["show", &format!("{main}:{STORY_FILE}")]),
        "the story file is not main's"
    );
    let identities = git(
        &world.repo,
        &[
            "log",
            "--format=%an <%ae>|%cn <%ce>",
            &format!("{main}..{candidate}"),
        ],
    );
    for line in identities.lines() {
        assert_eq!(line, format!("{IDENTITY}|{IDENTITY}"));
    }

    let (code, out, err) = world.turn("turn-1", &candidate);
    assert!(!err.contains("takt:"), "the turn was refused: {err}");
    assert!(
        out.lines()
            .any(|line| line.starts_with("stopped: Iterations (0); steps 0,")),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let case = fs::read_to_string(world.repo.join(".takt/cases/turn-1/case.yaml"))
        .unwrap_or_else(|_| panic!("no turn opened: stdout {out} stderr {err}"));
    assert!(case.contains(&candidate), "{case}");
    assert_eq!(world.route_calls(), "", "the route was called");
}

/// The candidate is fetched by id into a copy, as a turn's judge reads it
/// (`takt::git::Mirror::of`), and `refs/takt/candidates/<case>` names it.
#[test]
fn the_candidate_is_named_by_a_ref_and_fetched_by_its_id() {
    let world = world("fetched", true);
    let (code, out, err) = world.deliver();
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let candidate =
        candidate_of(&out).unwrap_or_else(|| panic!("no candidate: stdout {out} stderr {err}"));
    let named = takt::git::refs(&world.repo, &["refs/takt/candidates/"]).expect("the refs read");
    assert_eq!(named.len(), 1, "{named:?}");
    assert_eq!(git(&world.repo, &["rev-parse", &named[0]]), candidate);
    let mirror = takt::git::Mirror::of(&world.repo, &[&candidate])
        .expect("the mirror is made")
        .unwrap_or_else(|why| panic!("the candidate did not arrive: {why}"));
    assert_eq!(
        mirror
            .run(&["cat-file", "-t", &candidate])
            .expect("the copy reads"),
        "commit"
    );
    // Over `file://` with git's first protocol, whose server sends only what a ref names, the
    // id arrives too: the ref advertises it.
    let copy = world.repo.parent().expect("a parent").join("copy.git");
    git(
        world.repo.parent().expect("a parent"),
        &["init", "-q", "--bare", copy.to_str().expect("utf-8")],
    );
    let url = format!("file://{}", world.repo.display());
    git(
        &copy,
        &[
            "-c",
            "protocol.version=0",
            "fetch",
            "-q",
            "--no-write-fetch-head",
            &url,
            &candidate,
        ],
    );
    assert_eq!(git(&copy, &["cat-file", "-t", &candidate]), "commit");
}

/// A resume that cannot make a candidate, because `main` gained a commit its code commit does
/// not apply on, stops as before, exit 7, and prints `candidate: none`: the line names the base,
/// its tip and the commit that did not apply, without blaming the commits left out of the
/// candidate, and `refs/takt/candidates/<case>` no longer names the earlier candidate on the
/// old `main`.
#[test]
fn a_resume_that_cannot_make_a_candidate_removes_the_earlier_one() {
    let world = world("withdrawn", true);
    let (code, out, err) = world.deliver();
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let earlier =
        candidate_of(&out).unwrap_or_else(|| panic!("no candidate: stdout {out} stderr {err}"));
    let named = takt::git::refs(&world.repo, &["refs/takt/candidates/"]).expect("the refs read");
    assert_eq!(named.len(), 1, "{named:?}");
    let case = named[0]
        .strip_prefix("refs/takt/candidates/")
        .expect("a candidate ref")
        .to_owned();
    assert_eq!(git(&world.repo, &["rev-parse", &named[0]]), earlier);
    let code_commits = git(
        &world.repo,
        &[
            "rev-list",
            "--branches",
            "--not",
            "refs/heads/main",
            "--",
            "work.txt",
        ],
    );

    // `main` gains its own work.txt, which the case's code commit adds too.
    let scratch = world.repo.parent().expect("a parent");
    let other = scratch.join("other-work.txt");
    write(&other, "other work\n");
    let blob = git(
        &world.repo,
        &["hash-object", "-w", other.to_str().expect("utf-8")],
    );
    let index = scratch.join("main-moves.index");
    let indexed = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .current_dir(&world.repo)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    indexed(&["read-tree", "main"]);
    indexed(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{blob},work.txt"),
    ]);
    let tree = indexed(&["write-tree"]);
    let moved = git(
        &world.repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            "main",
            "-m",
            "main moves: work.txt",
        ],
    );
    git(&world.repo, &["update-ref", "refs/heads/main", &moved]);

    let (code, out, err) = world.resume(&case);
    assert!(
        out.contains("stopped: Unperformable (pr.opened; nothing performs it without the forge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert_eq!(candidate_of(&out), None, "{out}");
    assert!(
        takt::git::refs(&world.repo, &["refs/takt/candidates/"])
            .expect("the refs read")
            .is_empty(),
        "the earlier candidate {earlier} is still named: {out}"
    );
    let line = out
        .lines()
        .find(|line| line.starts_with("candidate: none"))
        .unwrap_or_else(|| panic!("no refusal line: {out}"));
    assert!(line.contains("main"), "the base unnamed: {line}");
    assert!(line.contains(&moved[..7]), "the base's tip unnamed: {line}");
    assert!(line.contains("work.txt"), "the path unnamed: {line}");
    assert!(
        code_commits.lines().any(|commit| line.contains(commit)),
        "the commit that did not apply unnamed ({code_commits}): {line}"
    );
    assert!(
        !line.contains("left out"),
        "the refusal asserts a cause it does not know: {line}"
    );
    assert_eq!(world.route_calls(), "", "the route was called");
}

/// With no identity configured the delivery stops as before, exit 7, and prints no candidate:
/// the line names the file, its keys and the flag; no candidate ref is written.
#[test]
fn with_no_identity_the_delivery_names_the_setting_and_makes_no_candidate() {
    let world = world("anonymous", false);
    let (code, out, err) = world.deliver();
    assert!(
        out.contains("stopped: Unperformable (pr.opened; nothing performs it without the forge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert_eq!(candidate_of(&out), None, "{out}");
    let line = out
        .lines()
        .find(|line| line.starts_with("candidate: none"))
        .unwrap_or_else(|| panic!("no refusal line: {out}"));
    for named in [
        ".engineering/commit-identity.conf",
        "identity.name",
        "identity.email",
        "--commit-identity",
    ] {
        assert!(line.contains(named), "{named} unnamed: {line}");
    }
    assert!(
        takt::git::refs(&world.repo, &["refs/takt/candidates/"])
            .expect("the refs read")
            .is_empty()
    );
    assert_eq!(world.route_calls(), "", "the route was called");
}
