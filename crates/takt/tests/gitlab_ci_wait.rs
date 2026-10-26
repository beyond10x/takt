//! story:gitlab-ci-wait: `ci.read` on a GitLab host waits on the merge request's head pipeline
//! through `glab api` as it waits on GitHub's checks, under the same poll and the same hour
//! budget, and records `ci_result` `pass` or `fail` once the pipeline of the pushed head ends.
//!
//! The run goes through the forge `gitlab` ([`Glab`]) and the real process runner, with doubles
//! of `glab`, `b10x-gates` and git's HTTPS push first on every child's program path: the `glab`
//! double answers the merge request's reads from a script of head pipelines, one line a read.
//! A library test, not one of the binary: `takt run` waits [`Run::POLL`], thirty seconds,
//! between two reads and has no flag to shorten it, so only [`Run::poll`] makes a run of several
//! reads take milliseconds. Nothing here reaches a host.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::gitlab::Glab;
use takt::forge::{Exit, Forge, Memory, Process, Runner};
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// How long the runs here wait on the forge between two reads.
const POLL: Duration = Duration::from_millis(5);

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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
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

/// Writes `name` into `bin`, executable, with `text`.
fn executable(bin: &Path, name: &str, text: &str) {
    let path = bin.join(name);
    fs::write(&path, text).expect("the double is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
}

/// The program path without any directory that holds a `glab`: the real one is never run.
fn path_without_glab() -> Vec<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|dir| !dir.join("glab").exists())
        .collect()
}

/// The real git's absolute path: the shim hands it every line that names no `https://` URL.
fn real_git() -> PathBuf {
    path_without_glab()
        .into_iter()
        .map(|dir| dir.join("git"))
        .find(|path| path.is_file())
        .expect("git is on the program path")
}

/// The `glab` double. `api ... --method POST .../merge_requests` keeps the request and answers
/// iid 7. Each `api .../merge_requests/7` read takes the next line of `<state>/pipelines`, the
/// last one again once they run out, and answers the merge request opened at the pushed head
/// with that head pipeline: `null` for none, else `<status> head` for a pipeline of the pushed
/// head, `<status> behind` for one of the commit in `<state>/behind`; the line is appended to
/// `<state>/served`. `--method PUT` counts a merge in `<state>/merges`.
const GLAB: &str = r#"#!/bin/sh
state='@STATE@'
echo "$*" >> "$state/glab.lines"
if [ "$1" != api ]; then echo "glab double: $*" >&2; exit 2; fi
method=GET; path=''; input=''
shift
while [ $# -gt 0 ]; do
  case "$1" in
    --hostname|--header) shift ;;
    --method) method="$2"; shift ;;
    --input) input="$2"; shift ;;
    *) path="$1" ;;
  esac
  shift
done
case "$method $path" in
  "POST projects/group%2Fsub%2Frepo/merge_requests")
    cp "$input" "$state/request.json"
    printf '{"iid": 7, "id": 1234}'
    exit 0 ;;
  "GET projects/group%2Fsub%2Frepo/merge_requests/7")
    reads=0
    if [ -f "$state/reads" ]; then reads=$(cat "$state/reads"); fi
    reads=$((reads + 1))
    printf '%s' "$reads" > "$state/reads"
    total=$(wc -l < "$state/pipelines")
    n=$reads
    if [ "$n" -gt "$total" ]; then n=$total; fi
    line=$(sed -n "${n}p" "$state/pipelines")
    echo "$line" >> "$state/served"
    sha=$(cat "$state/pushed")
    set -- $line
    if [ "$1" = null ]; then
      pipeline=null
    else
      at=$sha
      if [ "$2" = behind ]; then at=$(cat "$state/behind"); fi
      pipeline=$(printf '{"id":%s,"sha":"%s","status":"%s"}' "$reads" "$at" "$1")
    fi
    printf '{"iid":7,"state":"opened","source_branch":"%s","target_branch":"main","sha":"%s","head_pipeline":%s}' "$(cat "$state/branch")" "$sha" "$pipeline"
    exit 0 ;;
  "PUT projects/group%2Fsub%2Frepo/merge_requests/7/merge")
    echo merged >> "$state/merges"
    printf '{"iid": 7, "state": "merged"}'
    exit 0 ;;
esac
echo "glab double: $method $path" >&2
exit 2
"#;

/// The `git` shim: a push to an `https://` URL records the branch and the revision and exits 0,
/// a fetch from one exits 0, and every other line runs the real git at `@GIT@`.
const GIT: &str = r#"#!/bin/sh
state='@STATE@'
url=''; verb=''; refspec=''
for arg in "$@"; do
  case "$arg" in
    https://*) url="$arg" ;;
    push|fetch) if [ -z "$url" ]; then verb="$arg"; fi ;;
    *:refs/heads/*) refspec="$arg" ;;
  esac
done
if [ -n "$url" ]; then
  echo "$verb $url $refspec" >> "$state/https.lines"
  if [ "$verb" = push ]; then
    printf '%s' "${refspec#*:refs/heads/}" > "$state/branch"
    printf '%s' "${refspec%%:*}" > "$state/pushed"
  fi
  exit 0
fi
exec '@GIT@' "$@"
"#;

/// The real process runner ([`Process`]) with the doubles' directory first on each child's
/// program path: every line the forge runs is a real child process, and none reaches a host.
struct OnPath {
    path: String,
}

impl Runner for OnPath {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let mut env = env.to_vec();
        env.push(("PATH", &self.path));
        Process::default().run_with(tree, argv, &env)
    }
}

/// A repository with its doubles: the tree, the doubles' state directory and the forge over
/// `group/sub/repo` on `gitlab.example` through them.
struct Fixture {
    _scratch: Scratch,
    repo: PathBuf,
    state: PathBuf,
    forge: Rc<Glab>,
}

impl Fixture {
    /// The lines the `glab` double answered the merge request's reads with, in order.
    fn served(&self) -> Vec<String> {
        fs::read_to_string(self.state.join("served"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

/// Under `<scratch>/repo`, a committed copy of the fixture store `one-ready`; the doubles under
/// `<scratch>/forge`, the head pipelines `pipelines` one a read, and a pipeline `behind` at the
/// first commit of `main`.
fn fixture(name: &str, pipelines: &[&str]) -> Fixture {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gitlab-ci-wait-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("check.txt"), "broken\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let bin = outer.join("forge/bin");
    let state = outer.join("forge/state");
    fs::create_dir_all(&bin).expect("a directory");
    fs::create_dir_all(&state).expect("a directory");
    let state_text = state.to_str().expect("utf-8");
    executable(&bin, "glab", &GLAB.replace("@STATE@", state_text));
    executable(
        &bin,
        "git",
        &GIT.replace("@STATE@", state_text)
            .replace("@GIT@", real_git().to_str().expect("utf-8")),
    );
    executable(
        &bin,
        "b10x-gates",
        "#!/bin/sh\nif [ \"$1\" = scan-text ]; then exit 0; fi\necho \"b10x-gates double: $*\" >&2\nexit 2\n",
    );
    let mut script = pipelines.join("\n");
    script.push('\n');
    fs::write(state.join("pipelines"), script).expect("the script is written");
    fs::write(state.join("behind"), git(&repo, &["rev-parse", "main"])).expect("a file");
    let mut path = vec![bin];
    path.extend(path_without_glab());
    let path = std::env::join_paths(path)
        .expect("a program path")
        .into_string()
        .expect("utf-8");
    let forge = Rc::new(Glab::new(
        "gitlab.example",
        "group/sub/repo",
        None,
        Box::new(OnPath { path }),
    ));
    Fixture {
        _scratch: Scratch(outer),
        repo,
        state,
        forge,
    }
}

/// Requests the merge gate when it is offered; any other offer it counts and answers with an
/// inspection that changes nothing.
#[derive(Default)]
struct Gatekeeper {
    offers: Cell<usize>,
}

impl Harness for Gatekeeper {
    fn name(&self) -> &str {
        "gatekeeper"
    }

    fn keeps_state(&self) -> bool {
        true
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        self.offers.set(self.offers.get() + 1);
        Ok(StepReport {
            action: "repository.inspect".to_owned(),
            summary: "nothing to change".to_owned(),
            tokens: None,
        })
    }
}

struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

fn options(iterations: Option<u64>, hours: Option<f64>) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations,
        hours,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

fn record(id: String, kind: &str, result: Option<&str>, revision: &str, producer: &str) -> Record {
    Record::new(
        id,
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        producer,
    )
}

/// The story taken through `open_next`, its work committed on the case's branch, and the tests
/// and the gate recorded green at that revision, a change ahead and the handover chosen: the
/// case id and the revision. The case is ready for `pr.open`.
fn worked_case(repo: &Path) -> (String, String) {
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(forge.clone());
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        options(Some(1), None),
        &Aep::default(),
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(repo, &["commit", "-q", "-am", "the work"]);
    let worked = git(repo, &["rev-parse", "HEAD"]);
    let mut run = Run::resume(repo, options(Some(1), None), &id, &NoHarness, &validators)
        .expect("the case resumes");
    let short = &worked[..8];
    for (id, kind, result, producer) in [
        (
            format!("e1-{short}"),
            "test_result",
            Some("pass"),
            "gitlab-ci-wait",
        ),
        (
            format!("e2-{short}"),
            "gate_result",
            Some("pass"),
            "gitlab-ci-wait",
        ),
        (
            format!("handover-{short}"),
            "handover",
            None,
            takt::run::CHOICE_PRODUCER,
        ),
        (
            format!("change-record-{short}"),
            "change_record",
            Some("ahead"),
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&record(id, kind, result, &worked, producer))
            .expect("written");
    }
    (id, worked)
}

/// Resumes the case `id` through the fixture's forge with `harness` under `poll` and runs it:
/// the stop, or the error, and what it printed.
fn resumed_run(
    fixture: &Fixture,
    id: &str,
    options: RunOptions,
    poll: Duration,
    harness: &dyn Harness,
) -> (Result<Stop, String>, String) {
    let forge: Rc<dyn Forge> = fixture.forge.clone();
    let validators = validators(forge);
    let mut run =
        Run::resume(&fixture.repo, options, id, harness, &validators).expect("the case resumes");
    run.poll = poll;
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The `ci_result` records the case holds, each as its JSON.
fn ci_results(repo: &Path, id: &str) -> Vec<serde_json::Value> {
    let dir = repo.join(CASES_DIR).join(id).join("evidence");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .filter(|record: &serde_json::Value| record["kind"] == CiRead::KIND)
        .collect()
}

/// The merge request has no head pipeline yet, then one pending, then running, then passing:
/// `ci.read` reads it four times, two `glab api` reads each (the merge request's head, then its
/// checks), waits between them offering the harness nothing, and records one `ci_result` `pass`
/// for the pushed head on the fourth, then stops at the merge's grant with nothing merged.
#[test]
fn a_head_pipeline_from_none_through_running_to_success_records_ci_result_pass() {
    let fixture = fixture(
        "pass",
        &[
            "null",
            "null",
            "pending head",
            "pending head",
            "running head",
            "running head",
            "success head",
        ],
    );
    let (id, worked) = worked_case(&fixture.repo);
    let harness = Gatekeeper::default();

    let (stop, text) = resumed_run(&fixture, &id, options(Some(20), None), POLL, &harness);

    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert_eq!(
        fixture.served(),
        [
            "null",
            "null",
            "pending head",
            "pending head",
            "running head",
            "running head",
            "success head",
            "success head",
        ],
        "{text}"
    );
    assert_eq!(text.matches("  waiting: ").count(), 3, "{text}");
    assert_eq!(
        text.matches("through gitlab: pending; no record").count(),
        3,
        "{text}"
    );
    assert_eq!(harness.offers.get(), 0, "{text}");
    let ci = ci_results(&fixture.repo, &id);
    assert_eq!(ci.len(), 1, "{text}");
    assert_eq!(ci[0]["result"], "pass", "{text}");
    assert_eq!(ci[0]["subject_revision"], worked.as_str(), "{text}");
    assert_eq!(
        fs::read_to_string(fixture.state.join("pushed")).expect("a push"),
        worked,
        "the head the pipeline ran on is the one pushed"
    );
    assert!(!fixture.state.join("merges").exists(), "{text}");
}

/// The head pipeline runs, then fails: `ci.read` waits while it runs and records one
/// `ci_result` `fail` for the pushed head when it ends. The merge stays blocked, the harness is
/// offered a step again, and the run stops `NoProgress` when that step changes nothing; nothing
/// is merged.
#[test]
fn a_head_pipeline_that_fails_records_ci_result_fail() {
    let fixture = fixture(
        "fail",
        &[
            "null",
            "null",
            "running head",
            "running head",
            "failed head",
        ],
    );
    let (id, worked) = worked_case(&fixture.repo);
    let harness = Gatekeeper::default();

    let (stop, text) = resumed_run(&fixture, &id, options(Some(20), None), POLL, &harness);

    assert_eq!(stop, Ok(Stop::NoProgress), "{text}");
    assert_eq!(
        harness.offers.get(),
        1,
        "the harness is offered the failure, not the merge: {text}"
    );
    assert_eq!(
        fixture.served(),
        [
            "null",
            "null",
            "running head",
            "running head",
            "failed head",
            "failed head",
        ],
        "{text}"
    );
    assert_eq!(text.matches("  waiting: ").count(), 2, "{text}");
    let ci = ci_results(&fixture.repo, &id);
    assert_eq!(ci.len(), 1, "{text}");
    assert_eq!(ci[0]["result"], "fail", "{text}");
    assert_eq!(ci[0]["subject_revision"], worked.as_str(), "{text}");
    assert!(!fixture.state.join("merges").exists(), "{text}");
}

/// The head pipeline is of an older push: it passed, but at a commit behind the merge
/// request's head, so it says nothing of the head and is read as pending. The wait is the one
/// every forge's `ci.read` gets: the hour budget, 3 s, cuts the 120 s poll, and the run stops
/// `Waiting` on `ci.read` with `Hours` spent, well before the poll, nothing recorded. The run
/// ends within 60 s: a loaded host may take 57 s past the budget, and a wait the budget did not
/// cut takes the whole poll.
#[test]
fn a_head_pipeline_behind_the_head_is_pending_until_the_hour_budget_ends() {
    let fixture = fixture("behind", &["success behind"]);
    let (id, _) = worked_case(&fixture.repo);
    let hours = 3.0 / 3600.0;
    let started = Instant::now();

    let (stop, text) = resumed_run(
        &fixture,
        &id,
        options(None, Some(hours)),
        Duration::from_secs(120),
        &NoHarness,
    );

    assert!(
        started.elapsed() < Duration::from_secs(60),
        "waited {:?}: {text}",
        started.elapsed()
    );
    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Hours(hours))),
            local: Vec::new(),
        }),
        "{text}"
    );
    let served = fixture.served();
    assert!(served.len() >= 2, "{served:?}: {text}");
    assert!(
        served.iter().all(|line| line == "success behind"),
        "{served:?}"
    );
    assert!(
        text.contains("through gitlab: pending; no record"),
        "{text}"
    );
    assert!(ci_results(&fixture.repo, &id).is_empty(), "{text}");
    assert!(!fixture.state.join("merges").exists(), "{text}");
}
