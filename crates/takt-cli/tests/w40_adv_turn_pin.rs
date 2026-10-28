//! Adversarial cases for story:a-turn-pins-the-repository-of-its-forge (wave 40, U3). The helpers
//! below are copied from `self_improvement_cli.rs`, whose functions are private to that test
//! crate; the cases are at the end of the file.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The self-improvement protocol, the engineering protocols' built-in.
const PROTOCOL: &str = "self.improvement/1";

/// The finding scripts of this repository, committed into the scratch repository's base.
const SCRIPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");

/// The finding the turns are opened for.
const FINDING: &str = "story:probe-shows-the-defect";

/// The output the defect prints, which the finding's predicate names.
const DEFECT: &str = "the defect is still here";

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

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// One turn's world: the repository, its baseline and candidate, the scenario checkout, and the
/// state and cache directories the run is given.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    baseline: String,
    candidate: String,
    scenarios: PathBuf,
    state: PathBuf,
    cache: PathBuf,
    /// The install root `takt promote` installs into and `binary.promote` reads the installed
    /// binary from (`CARGO_INSTALL_ROOT`), never the operator's.
    install: PathBuf,
}

/// A binary planted in `check-finding`'s cache in place of a build: a script that shows the
/// defect, or that does not.
fn build(defect: bool) -> String {
    if defect {
        format!("#!/bin/sh\necho '{DEFECT}'\nexit 1\n")
    } else {
        "#!/bin/sh\necho 'all is well here'\nexit 0\n".to_owned()
    }
}

/// The `main.rs` of a scratch takt revision: `probe` prints the defect and exits 1 when
/// `defect`, else prints that all is well and exits 0. `check-finding` builds it with cargo,
/// confined, in seconds: the workspace has no dependency.
fn takt_main(defect: bool) -> String {
    if defect {
        format!("fn main() {{\n    println!(\"{DEFECT}\");\n    std::process::exit(1);\n}}\n")
    } else {
        "fn main() {\n    println!(\"all is well here\");\n}\n".to_owned()
    }
}

/// The scratch takt's main source, relative to its repository.
const TAKT_MAIN: &str = "crates/takt-cli/src/main.rs";

/// The finding's file in the store.
const FINDING_FILE: &str = ".engineering/planning/story/probe-shows-the-defect.md";

/// The reviewer's rigged `scripts/check-finding`: `not_reproduced` for the revision `HEAD` is
/// at, `reproduced` for any other, whatever the predicate shows.
const RIGGED_FOR_HEAD: &str = "#!/bin/sh\nrev=\"\"\nfor a in \"$@\"; do rev=$a; done\nif [ \"$rev\" = \"$(git rev-parse HEAD)\" ]; then echo not_reproduced; else echo reproduced; fi\n";

/// A rigged `scripts/check-finding` that says `not_reproduced` for every revision.
const RIGGED_ALWAYS: &str = "#!/bin/sh\necho not_reproduced\n";

/// Where the operator's `HEAD` is when the turn opens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    /// `main`, the baseline.
    Main,
    /// The candidate's branch.
    Fix,
    /// A branch off the baseline whose one commit writes `(path, text)`.
    Side(&'static str, &'static str),
}

/// The promotion store's baseline record: one the operator stated for the binary with this
/// sha256 (the test binary's when none is given), or one that names the baseline's revision as
/// the binary's own, with no digest or with the digest given.
#[derive(Clone, PartialEq, Eq)]
enum Promoted {
    Stated(Option<String>),
    Named,
    NamedWith(String),
}

/// How a world differs from the plain one.
struct Setup {
    at_baseline: bool,
    at_candidate: bool,
    head: Head,
    /// What the candidate's commit writes besides `fix.txt`.
    touches: Option<(&'static str, &'static str)>,
    /// Text appended to the baseline's `scripts/check-finding`, making it another script than
    /// the confined one.
    unconfined: bool,
    /// The baseline's `.gitattributes`, when it has one.
    attributes: Option<&'static str>,
    promoted: Promoted,
    /// The finding filed on `main` after the baseline, in a commit of its own, rather than in
    /// the baseline.
    filed_after: bool,
}

impl Setup {
    fn plain(at_baseline: bool, at_candidate: bool) -> Self {
        Self {
            at_baseline,
            at_candidate,
            head: Head::Main,
            touches: None,
            unconfined: false,
            attributes: None,
            promoted: Promoted::Stated(None),
            filed_after: false,
        }
    }
}

/// The baseline's judge allow-list: the candidate's own paths (`fix.txt`, the scratch takt's
/// sources) are no judge surface; every other path is one.
const JUDGE_ALLOW: &str = "paths:\n  - fix.txt\n  - crates/**\n";

/// The finding's text in the store.
fn finding_text() -> String {
    format!(
        "# The probe shows the defect\n\nThe probe prints the defect.\n\n```expected_before\nscenario: 1\ncommand: \"$TAKT\" probe\nexit: 1\noutput: {DEFECT}\nsource: docs/runs/trial-1.md\n```\n"
    )
}

/// A scorecard of `revision` as `takt scorecard --save` writes it: scenario 1 run five times,
/// each accepted with its held-out check passed, made with the binary of `binary`.
fn scorecard(revision: &str, binary: &str) -> String {
    let metrics: Vec<String> = takt::scorecard::METRICS
        .iter()
        .map(|(metric, version)| format!("\"{metric}\":{version}"))
        .collect();
    let rows: Vec<String> = (1..=5)
        .map(|n| {
            format!(
                "{{\"case\":\"scenario-1-trial-{n}\",\"scenario\":\"1\",\"outcome\":\"accepted\",\"stop\":\"accepted\",\"held_out\":{{\"passed\":true,\"result\":\"PASS\"}},\"steps\":4,\"tokens\":1000,\"refusals\":0,\"no_progress\":0,\"errors\":0,\"wall_ms\":1000,\"saturated\":false,\"binary\":null,\"opened_binary\":\"{binary}\"}}"
            )
        })
        .collect();
    format!(
        "{{\"format\":\"{}\",\"revision\":\"{revision}\",\"named_by\":\"build\",\"metrics\":{{{}}},\"rule\":{{\"trials\":5,\"margin\":1}},\"rows\":[{}],\"no_record\":[],\"unstated\":{{}}}}",
        takt::scorecard::FORMAT,
        metrics.join(","),
        rows.join(",")
    )
}

/// [`world_with`] with HEAD on `main`, a candidate that touches `fix.txt` alone and a baseline
/// record the operator stated for the test binary.
fn world(name: &str, at_baseline: bool, at_candidate: bool) -> World {
    world_with(name, &Setup::plain(at_baseline, at_candidate))
}

/// A repository whose base holds the finding scripts, the finding and the run record it cites,
/// and a candidate commit on a side branch; the scenario repository the predicate runs on; a
/// promotion store whose baseline is the base, stated by the operator for the test binary (its
/// sha256), as a turn needs the binary that judges to be the promoted one; and
/// `check-finding`'s cache holding a build of each revision, the baseline's showing the defect
/// when `at_baseline` and the candidate's when `at_candidate`. `setup` says where `HEAD` is,
/// what else the candidate writes, and how the baseline differs.
fn world_with(name: &str, setup: &Setup) -> World {
    let Setup {
        at_baseline,
        at_candidate,
        ..
    } = *setup;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "self-improvement-cli-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("takt");
    for script in ["check-finding", "file-finding"] {
        let mut text = fs::read_to_string(Path::new(SCRIPTS).join(script)).expect("the script");
        if setup.unconfined && script == "check-finding" {
            text.push_str("# an older check-finding than the confined one\n");
        }
        executable(&repo.join("scripts").join(script), &text);
    }
    write(
        &repo.join("docs/runs/trial-1.md"),
        "# Trial 1\n\nThe probe showed the defect.\n",
    );
    write(
        &repo.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    );
    write(
        &repo.join("crates/takt-cli/Cargo.toml"),
        "[package]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"takt\"\npath = \"src/main.rs\"\n",
    );
    write(
        &repo.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\n",
    );
    write(&repo.join(TAKT_MAIN), &takt_main(at_baseline));
    if let Some(attributes) = setup.attributes {
        write(&repo.join(".gitattributes"), attributes);
    }
    write(&repo.join(".engineering/judge-allow.yaml"), JUDGE_ALLOW);
    if !setup.filed_after {
        write(&repo.join(FINDING_FILE), &finding_text());
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    let baseline = git(&repo, &["rev-parse", "HEAD"]);
    if setup.filed_after {
        write(&repo.join(FINDING_FILE), &finding_text());
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "file the finding"]);
    }
    git(&repo, &["checkout", "-q", "-b", "fix", &baseline]);
    write(&repo.join("fix.txt"), "the fix\n");
    write(&repo.join(TAKT_MAIN), &takt_main(at_candidate));
    if let Some((path, text)) = setup.touches {
        executable(&repo.join(path), text);
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "fix"]);
    let candidate = git(&repo, &["rev-parse", "HEAD"]);
    match setup.head {
        Head::Fix => {}
        Head::Main => {
            git(&repo, &["checkout", "-q", "main"]);
        }
        Head::Side(path, text) => {
            git(&repo, &["checkout", "-q", "-b", "side", &baseline]);
            executable(&repo.join(path), text);
            git(&repo, &["add", "-A"]);
            git(&repo, &["commit", "-q", "-m", "side"]);
        }
    }

    let scenarios = dir.join("scenarios");
    write(&scenarios.join("README.md"), "the scenario\n");
    git(&scenarios, &["init", "-q", "-b", "main"]);
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "seed"]);
    let seed = git(&scenarios, &["rev-parse", "HEAD"]);
    write(
        &scenarios.join(".engineering/scenarios.conf"),
        &format!("[scenario \"1\"]\n\tstory = {FINDING}\n\tseed = {seed}\n"),
    );
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "declare"]);

    let state = dir.join("state");
    let store = state.join("takt/promotions");
    write(
        &store.join(format!("{baseline}.scorecard.json")),
        &scorecard(&baseline, &baseline),
    );
    let digest = sha256(&store.join(format!("{baseline}.scorecard.json")));
    let tree = git(&repo, &["rev-parse", &format!("{baseline}^{{tree}}")]);
    let stated = match &setup.promoted {
        Promoted::Stated(digest) => format!(
            "\"binary\":null,\"binary_sha256\":\"{}\",\"revision_stated_by\":\"operator\",",
            digest
                .clone()
                .unwrap_or_else(|| sha256(Path::new(env!("CARGO_BIN_EXE_takt"))))
        ),
        Promoted::Named => "\"binary\":null,\"binary_sha256\":null,".to_owned(),
        Promoted::NamedWith(digest) => {
            format!("\"binary\":null,\"binary_sha256\":\"{digest}\",")
        }
    };
    write(
        &store.join(format!("{baseline}.json")),
        &format!(
            "{{\"format\":\"takt-promotion/1\",\"revision\":\"{baseline}\",\"tree\":\"{tree}\",\"promoted_at\":\"2026-01-01T00:00:00Z\",\"kind\":\"baseline\",\"scorecard\":\"{baseline}\",\"scorecard_file\":\"/srv/scorecards/scorecard-{baseline}.json\",\"scorecard_sha256\":\"{digest}\",\"against\":null,\"rule\":{{\"trials\":5,\"margin\":1}},\"worse\":0,{stated}\"previous_sha256\":null}}"
        ),
    );

    let cache = dir.join("cache");
    let install = dir.join("install");
    // A turn opens only on an install root that exists: the one it is given, and the decoy the
    // environment names.
    fs::create_dir_all(&install).expect("the install root");
    fs::create_dir_all(dir.join("decoy-install")).expect("the decoy install root");
    World {
        _scratch: Scratch(dir),
        repo,
        baseline,
        candidate,
        scenarios,
        state,
        cache,
        install,
    }
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

/// `takt run` on the protocol in `world.repo` opening the case `turn-1` with `inputs`; its exit
/// code, standard output and standard error.
fn run(world: &World, inputs: &[String]) -> (i32, String, String) {
    run_with_gate(world, inputs, "repository.merge")
}

/// [`run`], with `gate` the approval the harness may request.
fn run_with_gate(world: &World, inputs: &[String], gate: &str) -> (i32, String, String) {
    run_with_env(world, inputs, gate, &[])
}

/// [`run_with_gate`], with `env` added to the run's environment.
fn run_with_env(
    world: &World,
    inputs: &[String],
    gate: &str,
    env: &[(&str, &Path)],
) -> (i32, String, String) {
    run_as(
        world,
        inputs,
        Some(gate),
        Some("the probe no longer shows the defect"),
        env,
    )
}

/// `takt run` opening the turn `turn-1` on the protocol with `inputs`, `--gate` and `--intent`
/// given only when they are some, and `env` added to the run's environment.
fn run_as(
    world: &World,
    inputs: &[String],
    gate: Option<&str>,
    intent: Option<&str>,
    env: &[(&str, &Path)],
) -> (i32, String, String) {
    let mut args: Vec<String> = Vec::new();
    args.push("run".to_owned());
    if let Some(gate) = gate {
        args.extend(["--gate".to_owned(), gate.to_owned()]);
    }
    if let Some(intent) = intent {
        args.extend(["--intent".to_owned(), intent.to_owned()]);
    }
    args.extend(
        [
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--protocol",
            PROTOCOL,
            "--install-root",
            world.install.to_str().expect("utf-8"),
            "--case-id",
            "turn-1",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "6",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
        ]
        .iter()
        .map(|arg| (*arg).to_owned()),
    );
    for input in inputs {
        args.extend(["--input".to_owned(), input.clone()]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(world))
        .envs(env.iter().copied())
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The install root the environment names (`CARGO_INSTALL_ROOT`) to every run, which is not
/// the one the turn opens with (`--install-root`): `binary.promote` reads the recorded one.
fn decoy(world: &World) -> PathBuf {
    world.repo.parent().expect("a parent").join("decoy-install")
}

/// The inputs a turn opens with.
fn opening(world: &World) -> Vec<String> {
    vec![
        "surface=non_judge".to_owned(),
        format!("finding={FINDING}"),
        format!("candidate={}", world.candidate),
    ]
}

/// The case's evidence records, each file's text, in the order they were written.
fn records(repo: &Path) -> Vec<String> {
    let dir = repo.join(".takt/cases/turn-1/evidence");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .map(|entry| entry.expect("an entry").path())
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
        .iter()
        .map(|path| fs::read_to_string(path).expect("a record"))
        .collect()
}

/// The string value of `key` in a record's JSON text, as `canon-evidence/1` writes it pretty;
/// empty when it has none.
fn field(record: &str, key: &str) -> String {
    let opening = format!("\"{key}\": \"");
    record
        .split_once(&opening)
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(value, _)| value.to_owned())
        .unwrap_or_default()
}

/// Each record's kind, subject, result and producer.
fn shown(records: &[String]) -> Vec<[String; 4]> {
    records
        .iter()
        .map(|record| ["kind", "subject", "result", "producer"].map(|key| field(record, key)))
        .collect()
}

fn row(kind: &str, subject: &str, result: &str, producer: &str) -> [String; 4] {
    [kind, subject, result, producer].map(str::to_owned)
}

/// `takt` run in `world` with `args` and the run's state, cache and scenario checkout; its exit
/// code, standard output and standard error.
fn takt_in(world: &World, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .args([
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
        ])
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(world))
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `takt` run in `world` with `args` alone, the run's state and cache given, and `PATH` set to
/// `path` when some.
fn takt_on_path(world: &World, args: &[&str], path: Option<&Path>) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
    command
        .args(args)
        .env("XDG_STATE_HOME", &world.state)
        .env("XDG_CACHE_HOME", &world.cache)
        .env("CARGO_INSTALL_ROOT", decoy(world))
        .env_remove("B10X_GATES_POLICY");
    if let Some(path) = path {
        command.env("PATH", path);
    }
    let output = command.output().expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A stand-in for the bot route, `b10x-gates` on `PATH`: its dry-run fetch of `main` answers,
/// per repository (`acme/one`, `acme/two`), the commit named by `tip-<repo>` beside it, and every
/// other line (the object fetch, the identity's scan) succeeds, the commits being already local.
/// A `gh` that refuses beside it, so no line reaches a real forge. The `PATH` that finds them
/// first.
fn bot_stand_in(world: &World) -> PathBuf {
    let bin = world.repo.parent().expect("a parent").join("bot-bin");
    executable(
        &bin.join("b10x-gates"),
        "#!/bin/sh\nu=$(printf '%s\\n' \"$@\" | grep '^https://' | head -n 1)\ncase \"$*\" in *--dry-run*) printf '* %s %s refs/remotes/origin/main\\n' 0000000000000000000000000000000000000000 \"$(cat \"$(dirname \"$0\")/tip-$(basename \"$u\" .git)\")\";; esac\nexit 0\n",
    );
    executable(
        &bin.join("gh"),
        "#!/bin/sh\necho \"no gh in this test\" >&2\nexit 1\n",
    );
    write(&bin.join("tip-one"), &format!("{}\n", world.baseline));
    PathBuf::from(format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").expect("a PATH")
    ))
}

/// Points the stand-in's `acme/two` at `tip`.
fn tip_of_two(path: &Path, tip: &str) {
    let path = path.to_str().expect("utf-8");
    let bin = Path::new(path.split(':').next().expect("the stand-in's directory"));
    write(&bin.join("tip-two"), &format!("{tip}\n"));
}

/// A turn whose candidate touches a judge surface, opened by `takt run` through the bot route on
/// `origin` `https://github.com/acme/one.git`, at the gate of its judge-surface merge; and the
/// `PATH` of the bot route's stand-in ([`bot_stand_in`]), `acme/one`'s `main` at the baseline.
fn bot_turn_at_the_gate(name: &str) -> (World, PathBuf) {
    let world = world_with(
        name,
        &Setup {
            touches: Some(("tools/judge.sh", "#!/bin/sh\nexit 0\n")),
            ..Setup::plain(true, false)
        },
    );
    git(
        &world.repo,
        &["remote", "add", "origin", "https://github.com/acme/one.git"],
    );
    let path = bot_stand_in(&world);
    let finding = format!("finding={FINDING}");
    let candidate = format!("candidate={}", world.candidate);
    let (code, out, err) = takt_on_path(
        &world,
        &[
            "run",
            "--intent",
            "the probe no longer shows the defect",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--protocol",
            PROTOCOL,
            "--install-root",
            world.install.to_str().expect("utf-8"),
            "--case-id",
            "turn-1",
            "--harness",
            "none",
            "--forge",
            "bot",
            "--commit-identity",
            "Probe <probe@example.com>",
            "--test-cmd",
            "true",
            "--iterations",
            "6",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
            "--input",
            &finding,
            "--input",
            &candidate,
        ],
        Some(&path),
    );
    assert_eq!(code, 4, "{out}\n{err}");
    assert!(
        out.contains("candidate.merge_judge_surface"),
        "{out}\n{err}"
    );
    (world, path)
}

/// `takt run --case turn-1` through the bot route, on `PATH` `path`.
fn bot_resume(world: &World, path: &Path) -> (i32, String, String) {
    takt_on_path(
        world,
        &[
            "run",
            "--case",
            "turn-1",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "bot",
            "--test-cmd",
            "true",
            "--scenarios",
            world.scenarios.to_str().expect("utf-8"),
            "--hours",
            "0.001",
        ],
        Some(path),
    )
}

/// The owner's `takt approve` of the turn's judge-surface merge, on `PATH` `path`.
fn bot_approve(world: &World, path: &Path) -> (i32, String, String) {
    takt_on_path(
        world,
        &[
            "approve",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "turn-1",
            "repository.merge.judge_surface",
            "--by",
            "human:owner",
        ],
        Some(path),
    )
}

/// The names of the files in the turn's evidence directory, sorted.
fn evidence_files(world: &World) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(world.repo.join(".takt/cases/turn-1/evidence"))
        .expect("the evidence directory reads")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Turns `turn-1`'s case into one written before this change: its `forge` file names `bot`
/// alone, and its judge-surface grant records the tip and no `mainline_repository`.
fn as_before_the_change(world: &World) {
    let case = world.repo.join(".takt/cases/turn-1");
    fs::write(case.join("forge"), "bot\n").expect("written");
    let authority = fs::read_to_string(case.join("authority.yaml")).expect("the authority");
    assert!(
        authority.contains("mainline_repository: acme/one"),
        "{authority}"
    );
    let legacy: String = authority
        .lines()
        .filter(|line| !line.trim_start().starts_with("mainline_repository:"))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(case.join("authority.yaml"), legacy).expect("written");
}

/// A merge of the candidate onto the baseline, made now (after the grant).
fn merge_after_the_grant(world: &World) -> String {
    let tree = git(
        &world.repo,
        &["rev-parse", &format!("{}^{{tree}}", world.candidate)],
    );
    git(
        &world.repo,
        &[
            "commit-tree",
            &tree,
            "-p",
            &world.baseline,
            "-p",
            &world.candidate,
            "-m",
            "the candidate, merged after the grant",
        ],
    )
}

/// The story's Decision: "a grant given before grants recorded a repository authorises no
/// judge-surface merge through the bot route (a new grant records it)", and the refusal tells
/// the operator to "grant it again". A turn opened and granted before the change (its `forge`
/// file `bot`, its grant without `mainline_repository`), resumed on the same `acme/one`, whose
/// `main` holds the candidate by a merge made after the grant: the merge is refused, as the
/// Decision says. The new grant the refusal asks for must then be possible: `takt approve`
/// exits 0 and records `acme/one`.
#[test]
fn w40_adv_a_grant_refused_for_recording_no_repository_can_be_given_again() {
    let (world, path) = bot_turn_at_the_gate("adv-legacy-regrant");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    as_before_the_change(&world);
    let bin = Path::new(
        path.to_str()
            .expect("utf-8")
            .split(':')
            .next()
            .expect("a dir"),
    );
    write(
        &bin.join("tip-one"),
        &format!("{}\n", merge_after_the_grant(&world)),
    );
    let (_, out, err) = bot_resume(&world, &path);
    assert!(
        out.contains("records no repository") && out.contains("grant it again"),
        "the refusal for a grant that records no repository was not reached: {out}\n{err}"
    );
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "{out}\n{err}"
    );
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(
        code, 0,
        "the refusal says `grant it again`, and `takt approve` refuses the grant:\n{out}\n{err}"
    );
    let authority = fs::read_to_string(world.repo.join(".takt/cases/turn-1/authority.yaml"))
        .expect("the authority");
    assert!(
        authority.contains("mainline_repository: acme/one"),
        "{authority}"
    );
}

/// A turn opened before the change (`forge` file `bot` alone) resumes wherever `origin` names,
/// as the story decides; its new grant records `acme/one`, and with `origin` then moved to
/// `acme/two`, whose `main` holds the candidate by a merge made after the grant, the merge
/// is still refused: the grant holds the turn to `acme/one`.
#[test]
fn w40_adv_a_bare_bot_turn_granted_on_one_records_no_merge_read_from_two() {
    let (world, path) = bot_turn_at_the_gate("adv-bare-bot");
    let case = world.repo.join(".takt/cases/turn-1");
    fs::write(case.join("forge"), "bot\n").expect("written");
    let (code, out, err) = bot_approve(&world, &path);
    assert_eq!(code, 0, "{out}\n{err}");
    let authority = fs::read_to_string(case.join("authority.yaml")).expect("the authority");
    assert!(
        authority.contains("mainline_repository: acme/one"),
        "{authority}"
    );
    git(
        &world.repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/two.git",
        ],
    );
    tip_of_two(&path, &merge_after_the_grant(&world));
    let (_, out, err) = bot_resume(&world, &path);
    assert!(
        records(&world.repo)
            .iter()
            .all(|record| field(record, "kind") != "merge_observation"),
        "merged recorded from acme/two under a grant read from acme/one: {out}\n{err}"
    );
    assert!(
        out.contains("acme/one") && out.contains("acme/two"),
        "{out}\n{err}"
    );
}

/// `origin` spelled another way for the same repository (ssh, a trailing slash, no `.git`): the
/// turn resumes, since the bot route reads one `<owner>/<repo>` from each.
#[test]
fn w40_adv_origin_spellings_of_the_same_repository_resume() {
    let (world, path) = bot_turn_at_the_gate("adv-spellings");
    for url in [
        "git@github.com:acme/one.git",
        "https://github.com/acme/one/",
        "https://github.com/acme/one",
    ] {
        git(&world.repo, &["remote", "set-url", "origin", url]);
        let (code, out, err) = bot_resume(&world, &path);
        assert!(
            !err.contains("recorded the forge `bot` on"),
            "{url}: {code}\n{out}\n{err}"
        );
    }
}
