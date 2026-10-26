//! `takt run` on `loop.improvement/1` (story:loop-case-opener, folded into story:loop-scorecard):
//! a case opens with `loop_baseline` at the commit `--input loop_baseline=<full commit id>`
//! names and the baseline's metric definitions snapshotted before any step, so `loop.measure`
//! records over the recorded turns through the binary alone; a missing value, or one that is no
//! full commit id of a commit of the repository, is refused before anything is written.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The loop-improvement protocol, the engineering protocols' built-in.
const LOOP_PROTOCOL: &str = "loop.improvement/1";

/// Definitions with one target: accepted share at least three quarters.
const DEFINED: &str = "format: takt-loop-metrics/1
metrics:
  turns:
    version: 1
    definition: the improvement turns recorded
  accepted_share:
    version: 1
    definition: accepted rows over all rows, open turns included
    target:
      at_least: 0.75
";

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

/// A scratch repository under this crate's build directory, removed on drop.
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

/// A committed repository holding the loop's metric definitions and three recorded turns
/// (one accepted, two declined) under its cases directory; its `HEAD`.
fn repository(name: &str) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-case-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let metrics = dir.join("protocols/loop-improvement/metrics.yaml");
    fs::create_dir_all(metrics.parent().expect("a parent")).expect("the directory");
    fs::write(&metrics, DEFINED).expect("the definitions");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    for (id, verdict) in [
        ("turn-a", "accepted"),
        ("turn-b", "declined"),
        ("turn-c", "declined"),
    ] {
        let case = dir.join(".engineering/cases").join(id);
        fs::create_dir_all(case.join("decisions")).expect("the case directory");
        fs::write(
            case.join("case.yaml"),
            format!(
                "format: canon-case/1\nid: {id}\nprotocol: self.improvement\nartifacts:\n  finding:\n    revision: {head}\n  baseline:\n    revision: {head}\n  candidate:\n    revision: {head}\n  promotion:\n    revision: {head}\ninputs:\n  surface: non_judge\n"
            ),
        )
        .expect("case.yaml");
        fs::write(
            case.join("protocol.yaml"),
            "format: protocol/1\nprotocol:\n  id: self.improvement\n  revision: 1\n",
        )
        .expect("protocol.yaml");
        fs::write(
            case.join("decisions/0000.json"),
            format!(
                r#"{{"claims":{{"finding.cited":{{"value":"true"}}}},"outcomes":{{"{verdict}":{{"status":"legitimate"}}}},"explanation":{{"computed_from":{{"at":"2026-10-18T10:00:00Z"}}}}}}"#
            ),
        )
        .expect("a decision");
    }
    (Scratch(dir), head)
}

/// `takt run --protocol <loop-improvement/1> --intent ... --input loop_baseline=<value>` (no
/// input when `value` is none) in `repo`, one step at most; its exit code and standard error.
fn run(repo: &Path, value: Option<&str>) -> (i32, String, String) {
    let input = value.map(|value| format!("loop_baseline={value}"));
    let mut args = vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--protocol",
        LOOP_PROTOCOL,
        "--intent",
        "raise the loop's accepted share",
        "--case-id",
        "loop-1",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "1",
    ];
    if let Some(input) = &input {
        args.extend(["--input", input.as_str()]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The case directories under `repo`, by name.
fn cases(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
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

/// Acceptance of story:loop-case-opener: the case opens with `loop_baseline` at the commit given
/// and its snapshot taken, and `loop.measure` records over the three recorded turns without a
/// hand-written `case.yaml`.
#[test]
fn takt_run_opens_a_loop_case_whose_measure_records_over_the_turns() {
    let (repo, head) = repository("opens");
    let (code, out, err) = run(&repo, Some(&head));
    let case = repo.join(".engineering/cases/loop-1");
    assert!(
        case.join("case.yaml").is_file(),
        "exit {code}\n{out}\n{err}"
    );
    let opened = takt::CaseDir::resume(&repo, "loop-1").expect("the case reads");
    assert_eq!(
        opened.revision("loop_baseline").as_deref(),
        Some(head.as_str()),
        "{out}\n{err}"
    );
    let taken = fs::read_to_string(case.join("loop-baseline.json"))
        .unwrap_or_else(|error| panic!("the snapshot ({error})\n{out}\n{err}"));
    assert!(
        taken.contains(&format!("\"revision\": \"{head}\"")),
        "{taken}"
    );
    assert!(taken.contains("takt-loop-metrics/1"), "{taken}");
    let measured: Vec<String> = fs::read_dir(case.join("evidence"))
        .expect("the evidence directory")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("a record"))
        .filter(|record| record.contains("\"kind\": \"loop_measurement\""))
        .collect();
    assert_eq!(measured.len(), 1, "exit {code}\n{out}\n{err}");
    let record = &measured[0];
    assert!(record.contains("\"result\": \"below_target\""), "{record}");
    assert!(
        record.contains("\"subject\": \"loop_baseline\""),
        "{record}"
    );
    assert!(
        record.contains(&format!("\"subject_revision\": \"{head}\"")),
        "{record}"
    );
}

/// A loop case without `--input loop_baseline`, or with a value that is no full commit id of a
/// commit of the repository (a short id, an id the repository does not hold, a tree's id, a
/// branch name), is refused before anything is written: no case directory, no case branch.
#[test]
fn a_loop_case_without_a_commit_for_its_baseline_is_refused_before_anything_is_written() {
    let (repo, head) = repository("refused");
    let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
    let short = head[..12].to_owned();
    let unknown = "0".repeat(40);
    for value in [
        None,
        Some(short.as_str()),
        Some(unknown.as_str()),
        Some(tree.as_str()),
        Some("main"),
    ] {
        let (code, out, err) = run(&repo, value);
        assert_eq!(code, 1, "{value:?}: {out}\n{err}");
        assert!(err.contains("loop_baseline"), "{value:?}: {err}");
        assert_eq!(
            cases(&repo),
            ["turn-a", "turn-b", "turn-c"],
            "{value:?}: nothing was written"
        );
        assert_eq!(
            git(&repo, &["branch", "--list", "takt/*"]),
            "",
            "{value:?}: no case branch"
        );
        assert_eq!(git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}

/// The reviewer's planter: writes a loose object under an id with other bytes than the ones the
/// id names, as any process with write access to the object directory can.
const PLANT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/plant_loose_object.py"
);

/// The definitions of [`DEFINED`] with the accepted share's target lowered to 0.25.
fn forged() -> String {
    DEFINED.replace("at_least: 0.75", "at_least: 0.25")
}

/// Plants, under the id of the definitions blob at `HEAD` of `repo`, a loose object holding the
/// forged definitions; the blob's id.
fn plant(repo: &Path) -> String {
    let blob = git(
        repo,
        &["rev-parse", "HEAD:protocols/loop-improvement/metrics.yaml"],
    );
    let forged_file = repo.join(".git/forged.yaml");
    fs::write(&forged_file, forged()).expect("the forged definitions");
    let status = Command::new("python3")
        .arg("-I")
        .arg(PLANT)
        .arg(repo.join(".git"))
        .arg(&blob)
        .arg(&forged_file)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("python3 runs");
    assert!(status.success(), "the object is planted");
    assert!(
        git(repo, &["show", &blob]).contains("at_least: 0.25"),
        "plain git reads the planted bytes under the blob's id"
    );
    blob
}

/// That a refusal of the loop case wrote nothing: only the recorded turns, no case branch.
fn nothing_written(repo: &Path, what: &str) {
    assert_eq!(
        cases(repo),
        ["turn-a", "turn-b", "turn-c"],
        "{what}: nothing was written"
    );
    assert_eq!(
        git(repo, &["branch", "--list", "takt/*"]),
        "",
        "{what}: no case branch"
    );
}

/// The reviewer's probe 5 (plant): a loose object planted over the base's definitions blob, the
/// real object loose or packed, as a step of an earlier case could leave it. The loop case is
/// refused before anything is written, naming the object, instead of snapshotting the forged
/// target.
#[test]
fn a_definitions_object_planted_before_the_loop_case_opens_is_refused() {
    for packed in [false, true] {
        let (repo, head) = repository(if packed {
            "plant-packed"
        } else {
            "plant-loose"
        });
        if packed {
            git(&repo, &["gc", "-q", "--prune=now"]);
        }
        let blob = plant(&repo);
        let (code, out, err) = run(&repo, Some(&head));
        assert_eq!(code, 1, "packed {packed}: {out}\n{err}");
        assert!(
            err.contains(&blob),
            "packed {packed}: the object is named: {err}"
        );
        nothing_written(&repo, &format!("packed {packed}"));
    }
}

/// The reviewer's probe 5 (test command): an earlier case's test command, code from the tree
/// run by `tests.run`, plants the forged object; the loop case opened afterwards is refused.
#[test]
fn a_definitions_object_an_earlier_test_command_planted_is_refused() {
    let (repo, head) = repository("test-command");
    fs::write(repo.join(".git/forged.yaml"), forged()).expect("the forged definitions");
    fs::write(
        repo.join("test.sh"),
        format!(
            "#!/bin/sh\nblob=$(git rev-parse HEAD:protocols/loop-improvement/metrics.yaml)\npython3 -I '{PLANT}' .git \"$blob\" .git/forged.yaml\nexit 0\n"
        ),
    )
    .expect("test.sh");
    git(&repo, &["add", "test.sh"]);
    git(&repo, &["commit", "-q", "-m", "the test command"]);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "an earlier case",
            "--case-id",
            "earlier",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "sh test.sh",
            "--iterations",
            "3",
        ])
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let earlier = String::from_utf8_lossy(&output.stdout).into_owned();
    git(&repo, &["checkout", "-q", "main"]);
    let blob = git(
        &repo,
        &["rev-parse", "HEAD:protocols/loop-improvement/metrics.yaml"],
    );
    assert!(
        git(&repo, &["show", &blob]).contains("at_least: 0.25"),
        "the earlier case's test command planted the object: {earlier}"
    );
    let (code, out, err) = run(&repo, Some(&head));
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains(&blob), "the object is named: {err}");
    assert!(
        !repo.join(".engineering/cases/loop-1").exists(),
        "no loop case was written"
    );
}

/// The reviewer's probe 5 (branch): a baseline that is a commit of the repository but not on
/// the base branch the case opens from, a side branch's with a looser target or an unreachable
/// one, is refused before anything is written.
#[test]
fn a_baseline_off_the_base_branch_is_refused_before_anything_is_written() {
    let (repo, _) = repository("off-base");
    git(&repo, &["checkout", "-q", "-b", "side"]);
    fs::write(
        repo.join("protocols/loop-improvement/metrics.yaml"),
        DEFINED.replace("at_least: 0.75", "at_least: 0.1"),
    )
    .expect("the looser definitions");
    git(&repo, &["commit", "-q", "-am", "loosen"]);
    let side = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", "main"]);
    let tree = git(&repo, &["rev-parse", &format!("{side}^{{tree}}")]);
    let dangling = git(
        &repo,
        &["commit-tree", &tree, "-p", &side, "-m", "dangling"],
    );
    for (what, value) in [("side", side.as_str()), ("unreachable", dangling.as_str())] {
        let (code, out, err) = run(&repo, Some(value));
        assert_eq!(code, 1, "{what}: {out}\n{err}");
        assert!(err.contains("loop_baseline"), "{what}: {err}");
        assert!(
            err.contains("main"),
            "{what}: the base branch is named: {err}"
        );
        nothing_written(&repo, what);
        assert_eq!(git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}

/// `data` as a zlib stream of stored deflate blocks, as git reads a loose object.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(65535).collect()
    };
    for (n, chunk) in chunks.iter().enumerate() {
        out.push(u8::from(n + 1 == chunks.len()));
        let len = u16::try_from(chunk.len()).expect("a stored block");
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend(*chunk);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}

/// The reviewer's probe 6 (side-made-ancestor): main is the definitions' commit, C1 and the
/// tip; a side branch off C1 loosens the target to 0.1. C1 is rewritten under its id with the
/// side commit as its parent, a cycle: plain git then takes the side commit for a commit of
/// main, and the ancestry path between them is empty. The loop case with the side commit as its
/// baseline is refused before anything is written, naming the forged commit.
#[test]
fn a_side_commit_a_forged_commit_makes_an_ancestor_of_the_base_is_refused() {
    let (repo, _) = repository("side-made-ancestor");
    fs::write(repo.join("README.md"), "c1\n").expect("a file");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-q", "-m", "c1"]);
    let c1 = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    fs::write(
        repo.join("protocols/loop-improvement/metrics.yaml"),
        DEFINED.replace("at_least: 0.75", "at_least: 0.1"),
    )
    .expect("the looser definitions");
    git(&repo, &["commit", "-q", "-am", "loosen"]);
    let side = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", "main"]);
    fs::write(repo.join("README2.md"), "tip\n").expect("a file");
    git(&repo, &["add", "README2.md"]);
    git(&repo, &["commit", "-q", "-m", "tip"]);
    let real = git(&repo, &["cat-file", "commit", &c1]);
    let parent = real
        .lines()
        .find_map(|line| line.strip_prefix("parent "))
        .expect("a parent")
        .to_owned();
    let forged = format!("{}\n", real.replace(&parent, &side));
    // Plain git must read the forged object, so nothing may stand in for it: no commit-graph
    // (which keeps C1's true parent), no automatic gc or maintenance that could pack it.
    git(&repo, &["config", "gc.auto", "0"]);
    git(&repo, &["config", "maintenance.auto", "false"]);
    git(&repo, &["config", "core.commitGraph", "false"]);
    let info = repo.join(".git/objects/info");
    for graph in [info.join("commit-graph"), info.join("commit-graphs")] {
        if graph.is_dir() {
            fs::remove_dir_all(&graph).expect("the commit-graph chain is removed");
        } else if graph.exists() {
            fs::remove_file(&graph).expect("the commit-graph is removed");
        }
    }
    let object = repo.join(".git/objects").join(&c1[..2]).join(&c1[2..]);
    fs::remove_file(&object).expect("the loose commit is removed");
    let mut raw = format!("commit {}\0", forged.len()).into_bytes();
    raw.extend_from_slice(forged.as_bytes());
    fs::write(&object, zlib_stored(&raw)).expect("the forged commit is written");
    let plain = Command::new("git")
        .args([
            "-c",
            "core.commitGraph=false",
            "merge-base",
            "--is-ancestor",
            &side,
            "main",
        ])
        .current_dir(&*repo)
        .output()
        .expect("git runs");
    assert!(
        plain.status.success(),
        "plain git takes the side commit for a commit of main ({}): {}",
        git(&repo, &["version"]),
        String::from_utf8_lossy(&plain.stderr)
    );
    let (code, out, err) = run(&repo, Some(&side));
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("loop_baseline"), "{err}");
    assert!(err.contains(&c1), "the forged commit is named: {err}");
    nothing_written(&repo, "side made an ancestor");
}

/// [`run`], killed when it has not ended within `bound`: its exit code (none when killed),
/// standard output and standard error, and how long it ran.
fn run_bounded(
    repo: &Path,
    value: &str,
    bound: std::time::Duration,
) -> (Option<i32>, String, String, std::time::Duration) {
    let input = format!("loop_baseline={value}");
    let started = std::time::Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--protocol",
            LOOP_PROTOCOL,
            "--intent",
            "raise the loop's accepted share",
            "--case-id",
            "loop-1",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "1",
            "--input",
            &input,
        ])
        .env_remove("B10X_GATES_POLICY")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("takt runs");
    let code = loop {
        if let Some(status) = child.try_wait().expect("takt is waited for") {
            break status.code();
        }
        if started.elapsed() > bound {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let took = started.elapsed();
    let output = child.wait_with_output().expect("takt's output");
    (
        code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        took,
    )
}

/// The reviewer's probe 7 (`header[blob 100000]`): a loose object planted over the definitions
/// blob whose header claims 100,000 bytes over a body of a few hundred. The read is bounded: the
/// run ends within 30 s (the reader kills `git cat-file` 10 s into a read), refused, naming the
/// object, with nothing written.
#[test]
fn a_planted_object_whose_header_overstates_its_size_is_refused_within_a_bound() {
    let (repo, head) = repository("overstated-size");
    let blob = git(
        &repo,
        &["rev-parse", "HEAD:protocols/loop-improvement/metrics.yaml"],
    );
    let body = forged();
    let mut raw = b"blob 100000\0".to_vec();
    raw.extend_from_slice(body.as_bytes());
    let object = repo.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
    fs::remove_file(&object).expect("the loose blob is removed");
    fs::write(&object, zlib_stored(&raw)).expect("the overstated object is written");
    let bound = std::time::Duration::from_secs(30);
    let (code, out, err, took) = run_bounded(&repo, &head, bound);
    assert!(
        took < bound,
        "the run ended within {bound:?}: it took {took:?}\n{out}\n{err}"
    );
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(err.contains(&blob), "the object is named: {err}");
    nothing_written(&repo, "an overstated size");
}

/// Definitions above 256 KiB are refused when the case opens, naming their size: they would be
/// copied whole into the case's snapshot.
#[test]
fn definitions_above_their_cap_are_refused_naming_the_size() {
    let (repo, _) = repository("big-definitions");
    let mut big = DEFINED.to_owned();
    big.push_str("  padding:\n    version: 1\n    definition: \"");
    while big.len() < 300 * 1024 {
        big.push_str("padding the definitions past their cap ");
    }
    big.push_str("\"\n");
    fs::write(repo.join("protocols/loop-improvement/metrics.yaml"), &big).expect("the definitions");
    git(&repo, &["commit", "-q", "-am", "big definitions"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let (code, out, err) = run(&repo, Some(&head));
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(
        err.contains(&big.len().to_string()),
        "the size {} is named: {err}",
        big.len()
    );
    nothing_written(&repo, "big definitions");
}

/// A partial clone (`--filter=blob:none`) of a repository whose second commit holds [`DEFINED`]
/// and whose third changes it, so the second commit's definitions blob is missing from the
/// clone: the clone, its second commit (the baseline) and that blob's id.
fn partial_clone(name: &str) -> (Scratch, PathBuf, String, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-case-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let src = dir.join("src");
    fs::create_dir_all(src.join("protocols/loop-improvement")).expect("the directory");
    git(&src, &["init", "-q", "-b", "main"]);
    fs::write(src.join("README.md"), "a\n").expect("a file");
    git(&src, &["add", "-A"]);
    git(&src, &["commit", "-q", "-m", "c0"]);
    let metrics = src.join("protocols/loop-improvement/metrics.yaml");
    fs::write(&metrics, DEFINED).expect("the definitions");
    git(&src, &["add", "-A"]);
    git(&src, &["commit", "-q", "-m", "c1"]);
    fs::write(&metrics, DEFINED.replace("0.75", "0.8")).expect("the definitions");
    git(&src, &["commit", "-q", "-am", "c2"]);
    git(&src, &["config", "uploadpack.allowFilter", "true"]);
    git(&src, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    let clone = dir.join("clone");
    let url = format!("file://{}", src.display());
    git(
        &dir,
        &[
            "clone",
            "-q",
            "--filter=blob:none",
            &url,
            clone.to_str().expect("utf-8"),
        ],
    );
    let baseline = git(&clone, &["rev-parse", "main~1"]);
    let blob = git(
        &clone,
        &[
            "rev-parse",
            &format!("{baseline}:protocols/loop-improvement/metrics.yaml"),
        ],
    );
    let missing = Command::new("git")
        .args(["cat-file", "-e", &blob])
        .env("GIT_NO_LAZY_FETCH", "1")
        .current_dir(&clone)
        .status()
        .expect("git runs");
    assert!(
        !missing.success(),
        "the baseline's definitions blob is missing locally"
    );
    (Scratch(dir), clone, baseline, blob)
}

/// High 1 of the planner's review: in a partial clone whose promisor remote never answers, the
/// baseline's definitions blob missing locally, reading it would start a lazy fetch that
/// outlives the reader. No git takt runs fetches lazily: the run ends well within 30 s,
/// refused, naming the blob, and leaves no fetch running.
#[test]
fn a_blob_missing_from_a_partial_clone_is_refused_without_fetching() {
    let (_scratch, clone, baseline, blob) = partial_clone("partial-stalled");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        // Accepts and never answers: a remote that stalls.
        let mut held = Vec::new();
        for stream in listener.incoming() {
            held.push(stream);
        }
    });
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            &format!("http://127.0.0.1:{port}/x.git"),
        ],
    );
    let bound = std::time::Duration::from_secs(30);
    let (code, out, err, took) = run_bounded(&clone, &baseline, bound);
    assert!(
        took < bound,
        "the run ended within {bound:?}: it took {took:?}\n{out}\n{err}"
    );
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(err.contains(&blob), "the blob is named: {err}");
    assert!(
        !clone.join(".engineering/cases/loop-1").exists(),
        "no loop case was written"
    );
    let marker = format!("127.0.0.1:{port}");
    for cmdline in command_lines() {
        assert!(!cmdline.contains(&marker), "left running: {cmdline}");
    }
}

/// The command line of every process on the host, one per line, from `ps`, which Linux and
/// macOS both have; macOS has no `/proc` to read them from.
fn command_lines() -> Vec<String> {
    let output = Command::new("ps")
        .args(["-A", "-ww", "-o", "args="])
        .output()
        .expect("ps runs");
    assert!(
        output.status.success(),
        "ps -A: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// High 2 of the planner's review: the same lazy fetch would run the repository's own
/// `remote.origin.uploadpack` through a shell. No git takt runs fetches lazily, so the command
/// configured there never runs: its marker is not written, and the run is refused naming the
/// blob.
#[test]
fn a_configured_upload_pack_never_runs_on_a_missing_blob() {
    let (scratch, clone, baseline, blob) = partial_clone("partial-uploadpack");
    let marker = scratch.join("MARKER");
    git(
        &clone,
        &[
            "config",
            "remote.origin.uploadpack",
            &format!("touch '{}'; git-upload-pack", marker.display()),
        ],
    );
    let bound = std::time::Duration::from_secs(30);
    let (code, out, err, took) = run_bounded(&clone, &baseline, bound);
    assert!(
        took < bound,
        "the run ended within {bound:?}: it took {took:?}\n{out}\n{err}"
    );
    assert!(
        !marker.exists(),
        "the configured upload-pack ran: exit {code:?}\n{out}\n{err}"
    );
    assert_eq!(code, Some(1), "{out}\n{err}");
    assert!(err.contains(&blob), "the blob is named: {err}");
}

/// The second review's low finding on the baseline: a baseline whose definitions are absent, or
/// do not read, would open a case that never records. It is refused when the case would open,
/// naming the reason, with nothing written.
#[test]
fn a_baseline_without_readable_definitions_is_refused_at_open() {
    let (repo, _) = repository("no-definitions");
    git(
        &repo,
        &["rm", "-q", "protocols/loop-improvement/metrics.yaml"],
    );
    git(&repo, &["commit", "-q", "-m", "no definitions"]);
    let absent = git(&repo, &["rev-parse", "HEAD"]);
    let (code, out, err) = run(&repo, Some(&absent));
    assert_eq!(code, 1, "absent: {out}\n{err}");
    assert!(err.contains("holds no"), "absent: {err}");
    nothing_written(&repo, "absent definitions");

    let metrics = repo.join("protocols/loop-improvement/metrics.yaml");
    fs::create_dir_all(metrics.parent().expect("a parent")).expect("the directory");
    fs::write(&metrics, "format: takt-loop-metrics/1\nmetrics: [\n").expect("the definitions");
    git(&repo, &["add", "protocols/loop-improvement/metrics.yaml"]);
    git(&repo, &["commit", "-q", "-m", "malformed definitions"]);
    let malformed = git(&repo, &["rev-parse", "HEAD"]);
    let (code, out, err) = run(&repo, Some(&malformed));
    assert_eq!(code, 1, "malformed: {out}\n{err}");
    assert!(err.contains("do not read"), "malformed: {err}");
    nothing_written(&repo, "malformed definitions");
}
