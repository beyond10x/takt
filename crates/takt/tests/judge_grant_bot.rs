//! `takt approve`'s grant of an improvement turn's judge-surface merge under the bot forge
//! (story:a-judge-surface-candidate-merges-after-its-grant): the tip of `main` the grant records
//! is the one the bot route reports for the repository on its host, read through a stand-in
//! `b10x-gates` first on `PATH`, never the tree's own `main`; a read that fails refuses the grant
//! with nothing written. One case in this binary, as it sets `PATH` for the process.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{Authority, TurnOpening};
use takt::evidence::Record;
use takt::improvement::{FindingFile, ScenarioReproduce, ScenarioVerify};
use takt::{CaseDir, LoadedProtocol};

const SELF_IMPROVEMENT: &str = "self.improvement/1";

const JUDGE_SURFACE: &str = "repository.merge.judge_surface";

/// The stand-in for the bot route: `STUB_MODE=tip` reports `STUB_TIP` as the repository's `main`
/// to the dry-run fetch and lets the object fetch succeed; `STUB_MODE=netfail` fails every line
/// as a network error does. Every line is appended to `STUB_LOG`.
const STUB: &str = r#"#!/bin/sh
echo "$*" >> "$STUB_LOG"
case "$*" in
  *--dry-run*)
    if [ "$STUB_MODE" = netfail ]; then
      echo "fatal: unable to access the repository: Could not resolve host" >&2
      exit 128
    fi
    echo "* 0000000000000000000000000000000000000000 $STUB_TIP refs/remotes/origin/main"
    exit 0 ;;
  *--no-write-fetch-head*)
    if [ "$STUB_MODE" = netfail ]; then
      echo "fatal: unable to access the repository: Could not resolve host" >&2
      exit 128
    fi
    exit 0 ;;
esac
echo "stand-in: unexpected line $*" >&2
exit 2
"#;

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

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

/// Under the bot forge, the judge-surface grant records the tip the bot route reports for the
/// repository's `main` (the candidate, merged there after the grant would have been asked for),
/// not the tree's own `main` (the baseline, which a stale local checkout still shows); and a
/// read that fails refuses the grant with nothing written.
#[test]
fn a_bot_forge_turns_grant_records_the_tip_the_bot_route_reports() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("judge-grant-bot-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let _scratch = Scratch(dir.clone());
    let tree = dir.join("takt");
    fs::create_dir_all(&tree).expect("the tree");
    fs::write(tree.join("README.md"), "takt\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "base"]);
    let baseline = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "-b", "fix"]);
    fs::create_dir_all(tree.join("tools")).expect("a directory");
    fs::write(tree.join("tools/judge.sh"), "#!/bin/sh\nexit 0\n").expect("a file");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "fix"]);
    let candidate = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/takt.git",
        ],
    );

    let protocol = LoadedProtocol::builtin(SELF_IMPROVEMENT).expect("loads");
    let mut case = CaseDir::open(
        &tree,
        protocol,
        "turn",
        "one improvement turn",
        &[("surface".to_owned(), "judge".to_owned())],
    )
    .expect("the case opens");
    case.open_turn(&TurnOpening {
        finding: "story:probe-shows-the-defect".to_owned(),
        candidate: candidate.clone(),
        baseline: baseline.clone(),
        surface: "judge".to_owned(),
        title: "The probe shows the defect".to_owned(),
        judge_sha256: "0".repeat(64),
        install_root: None,
        forge: Some("bot".to_owned()),
    })
    .expect("the turn opens");
    for (n, kind, result, subject, producer) in [
        (1, "finding_record", "filed", "finding", FindingFile::NAME),
        (
            2,
            "scenario_result",
            "reproduced",
            "baseline",
            ScenarioReproduce::NAME,
        ),
        (
            3,
            "scenario_result",
            "not_reproduced",
            "candidate",
            ScenarioVerify::NAME,
        ),
    ] {
        let revision = case.revision(subject).expect("the subject has a revision");
        case.add_evidence(&Record::new(
            format!("e{n}"),
            kind,
            Some(result.to_owned()),
            subject,
            revision,
            producer,
        ))
        .expect("the record is written");
    }
    let decision = case.evaluate(0).expect("evaluates");
    assert_eq!(
        decision
            .actions
            .get("candidate.merge_judge_surface")
            .map(String::as_str),
        Some("approval-required"),
        "{}",
        decision.json
    );

    let bin = dir.join("bin");
    let stub = bin.join("b10x-gates");
    fs::create_dir_all(&bin).expect("the stand-in's directory");
    fs::write(&stub, STUB).expect("the stand-in");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("executable");
    let log = dir.join("stub.log");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    // SAFETY: this binary holds this one case, so no other thread reads the environment.
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("STUB_LOG", &log);
        std::env::set_var("STUB_MODE", "netfail");
        std::env::set_var("STUB_TIP", &candidate);
        std::env::remove_var("B10X_GATES_POLICY");
    }
    let authority = case.dir.join("authority.yaml");
    let refused = takt::case::grant(&tree, "turn", JUDGE_SURFACE, "human:owner")
        .expect_err("a tip the bot route cannot read grants nothing");
    assert!(
        format!("{refused:#}").contains("could not be read"),
        "{refused:#}"
    );
    assert!(!authority.exists(), "nothing written");

    // SAFETY: as above.
    unsafe {
        std::env::set_var("STUB_MODE", "tip");
    }
    assert_eq!(git(&tree, &["rev-parse", "main"]), baseline);
    takt::case::grant(&tree, "turn", JUDGE_SURFACE, "human:owner").expect("granted");
    let decision = Authority::read(&case.dir)
        .expect("the authority reads")
        .expect("an authority")
        .decision(JUDGE_SURFACE)
        .cloned()
        .expect("the grant");
    assert_eq!(
        decision.mainline_tip.as_deref(),
        Some(candidate.as_str()),
        "the bot route's tip of main, not the tree's {baseline}"
    );
    let lines = fs::read_to_string(&log).expect("the stand-in ran");
    assert!(
        lines.contains("bot -- fetch https://github.com/example/takt.git refs/heads/main:refs/remotes/origin/main --porcelain --dry-run"),
        "{lines}"
    );
}
