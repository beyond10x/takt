//! story:gitlab-forge at the binary's edge: a repository whose origin is on a GitLab host runs
//! the delivery through `glab`. `glab`, `b10x-gates` and the HTTPS push are doubles on the
//! program path (a `git` shim answers every line naming an `https://` URL and hands every other
//! line to the real git), so nothing in these tests reaches a host.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The delivery protocol, the engineering protocols' built-in, which declares `pr.open` and `ci.read`.
const DELIVERY_PROTOCOL: &str = "story.delivery/1";

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The origin of every repository below: a project in a nested group on a GitLab host.
const ORIGIN: &str = "https://gitlab.example/group/sub/repo.git";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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

/// Under `<scratch>/repo`, a committed copy of the fixture store `one-ready` with a manifest, a
/// gates document whose one assertion is true, a commit identity and `origin` at [`ORIGIN`];
/// the doubles go under `<scratch>/forge`.
fn repository(name: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gitlab-forge-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&repo, &["remote", "add", "origin", ORIGIN]);
    (Scratch(outer), repo)
}

/// Writes `name` into `bin`, executable, with `text`.
fn executable(bin: &Path, name: &str, text: &str) {
    let path = bin.join(name);
    fs::write(&path, text).expect("the stub is written");
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

/// The real git's absolute path, found on the program path: the shim hands it every line that
/// names no `https://` URL.
fn real_git() -> PathBuf {
    path_without_glab()
        .into_iter()
        .map(|dir| dir.join("git"))
        .find(|path| path.is_file())
        .expect("git is on the program path")
}

/// The doubles in `<dir>/stub-bin`, their state in `<dir>/forge`. `glab auth status` exits
/// with the code in `<state>/auth` (0 when there is none); `glab api ... --method POST ...
/// merge_requests --input <file>` keeps the request and prints `{"iid": 7}`; `glab api ...
/// merge_requests/7` prints the merge request with the pushed head and its head pipeline
/// passing; `glab api ... repository/branches/main` prints the local `main`; `--method PUT`
/// merges and counts the merge. `b10x-gates scan-text` passes. The `git` shim records, for a
/// push to an `https://` URL, the URL, the branch and the revision, exits 0 for a fetch from
/// one, and runs the real git for every other line. With `glab` false, no `glab` is written.
fn doubles(dir: &Path, glab: bool) -> (PathBuf, PathBuf) {
    let bin = dir.join("stub-bin");
    let state = dir.join("forge");
    fs::create_dir_all(&bin).expect("a stub directory");
    fs::create_dir_all(&state).expect("a state directory");
    let state_text = state.display().to_string();
    if glab {
        executable(
            &bin,
            "glab",
            &format!(
                r#"#!/bin/sh
state='{state_text}'
echo "$*" >> "$state/glab.lines"
if [ "$1" = auth ] && [ "$2" = status ]; then
  code=0
  if [ -f "$state/auth" ]; then code=$(cat "$state/auth"); fi
  if [ "$code" != 0 ]; then echo "x $4 has not been authenticated with glab" >&2; fi
  exit "$code"
fi
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
    cp "$input" "$state/request.json"; printf opened > "$state/state"
    printf '{{"iid": 7, "id": 1234}}'; exit 0 ;;
  "GET projects/group%2Fsub%2Frepo/merge_requests/7")
    sha=$(tr -d '\n' < "$state/pushed")
    printf '{{"iid":7,"state":"%s","source_branch":"%s","target_branch":"main","sha":"%s","head_pipeline":{{"sha":"%s","status":"success"}}}}' "$(cat "$state/state")" "$(cat "$state/branch")" "$sha" "$sha"
    exit 0 ;;
  "GET projects/group%2Fsub%2Frepo/repository/branches/main")
    printf '{{"name":"main","commit":{{"id":"%s"}}}}' "$(git rev-parse refs/heads/main)"; exit 0 ;;
  "PUT projects/group%2Fsub%2Frepo/merge_requests/7/merge")
    printf merged > "$state/state"; echo merged >> "$state/merges"
    printf '{{"iid": 7, "state": "merged"}}'; exit 0 ;;
esac
echo "glab double: $method $path" >&2
exit 2
"#
            ),
        );
    }
    executable(
        &bin,
        "b10x-gates",
        r#"#!/bin/sh
if [ "$1" = scan-text ]; then exit 0; fi
echo "b10x-gates double: $*" >&2
exit 2
"#,
    );
    executable(
        &bin,
        "git",
        &format!(
            r#"#!/bin/sh
state='{state_text}'
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
    printf '%s' "$url" > "$state/url"
    printf '%s' "${{refspec#*:refs/heads/}}" > "$state/branch"
    printf '%s' "${{refspec%%:*}}" > "$state/pushed"
  fi
  exit 0
fi
exec '{}' "$@"
"#,
            real_git().display()
        ),
    );
    (bin, state)
}

/// Runs the built `takt` binary with `args`, `bin` first on a program path that holds no other
/// `glab`, and no `b10x-gates` policy: the exit code, stdout and stderr.
fn takt_through(bin: &Path, args: &[&str]) -> (i32, String, String) {
    let mut path = vec![bin.to_path_buf()];
    path.extend(path_without_glab());
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env_remove("B10X_GATES_POLICY")
        .env("PATH", std::env::join_paths(path).expect("a program path"))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `takt run --forge gitlab` on the delivery protocol over `repo`, no harness.
fn gitlab_run<'a>(repo: &'a str, case: &'a str) -> Vec<&'a str> {
    vec![
        "run",
        "--repo",
        repo,
        "--intent",
        "deliver the story",
        "--case-id",
        case,
        "--harness",
        "none",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--test-cmd",
        "true",
        "--forge",
        "gitlab",
    ]
}

/// No `glab` on the program path: `--forge gitlab` is refused before any case opens, exit 1,
/// naming `glab` and the host.
#[test]
fn without_glab_the_run_is_refused_before_its_first_step() {
    let (outer, repo) = repository("no-glab");
    let (bin, _) = doubles(&outer.0.join("forge"), false);
    let (code, out, err) = takt_through(&bin, &gitlab_run(repo.to_str().expect("utf-8"), "c1"));
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("glab") && err.contains("gitlab.example"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("step 1"), "stdout {out} stderr {err}");
    assert!(
        !repo.join(".takt/cases").exists(),
        "no case opened: stdout {out} stderr {err}"
    );
}

/// `glab auth status --hostname gitlab.example` exits 1: refused before any case opens, exit 1,
/// naming `glab`, the host and what `glab` said; by default, the run goes on with no forge and
/// says why in one line.
#[test]
fn without_a_glab_login_the_run_is_refused_before_its_first_step() {
    let (outer, repo) = repository("no-login");
    let (bin, state) = doubles(&outer.0.join("forge"), true);
    fs::write(state.join("auth"), "1").expect("the login fails");
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt_through(&bin, &gitlab_run(path, "c1"));
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    for named in ["glab", "gitlab.example", "has not been authenticated"] {
        assert!(err.contains(named), "{named}: stdout {out} stderr {err}");
    }
    assert!(!out.contains("step 1"), "stdout {out} stderr {err}");
    assert!(
        !repo.join(".takt/cases").exists(),
        "no case opened: stdout {out} stderr {err}"
    );
    assert_eq!(
        fs::read_to_string(state.join("glab.lines")).expect("glab ran"),
        "auth status --hostname gitlab.example\n"
    );
    let mut default = gitlab_run(path, "c2");
    default.truncate(default.len() - 2);
    default.extend(["--iterations", "1"]);
    let (_, out, err) = takt_through(&bin, &default);
    assert!(
        out.contains("forge: none (origin gitlab.example is not github.com, and glab is not logged in to it: ")
            && out.contains("has not been authenticated"),
        "stdout {out} stderr {err}"
    );
}

/// The delivery through the binary with `gitlab` the default forge for an origin on a GitLab
/// host that `glab` is logged in to: a plain run takes the ready story; the operator commits
/// the work on the case's branch; the next run resumes it, runs the tests and the gate, pushes
/// the case's commit to the project's HTTPS URL, opens one merge request, reads its head
/// pipeline passing and stops at the merge's grant, exit 4, nothing merged.
#[test]
fn a_gitlab_origin_delivers_through_glab_up_to_the_merge_grant() {
    let (outer, repo) = repository("deliver");
    let (bin, state) = doubles(&outer.0.join("forge"), true);
    let path = repo.to_str().expect("utf-8");
    let run = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
    ];
    let mut first = run.to_vec();
    first.extend(["--iterations", "0"]);
    let (code, out, err) = takt_through(&bin, &first);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains(
            "forge: gitlab (gitlab.example group/sub/repo); pr.open, ci.read and repository.merge go through glab"
        ),
        "stdout {out} stderr {err}"
    );
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();

    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);

    let (code, out, err) = takt_through(&bin, &run);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    for line in [
        "evidence: test_result pass",
        "evidence: gate_result pass",
        "evidence: pull_request open",
        "evidence: ci_result pass",
        "stopped: ApprovalRequired (repository.merge)",
    ] {
        assert!(out.contains(line), "{line}: stdout {out} stderr {err}");
    }
    let pushed = fs::read_to_string(state.join("pushed")).expect("a push");
    assert_eq!(
        fs::read_to_string(state.join("url")).expect("a push"),
        ORIGIN
    );
    assert_eq!(
        fs::read_to_string(state.join("branch")).expect("a push"),
        format!("takt/{id}")
    );
    assert_eq!(
        git_out(&repo, &["rev-parse", &format!("{pushed}^{{commit}}")]),
        pushed,
        "the push names a commit by its id"
    );
    let request: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(state.join("request.json")).expect("a merge request"),
    )
    .expect("json");
    assert_eq!(request["source_branch"], format!("takt/{id}"), "{request}");
    assert_eq!(request["target_branch"], "main", "{request}");
    assert!(request["title"].is_string(), "{request}");
    assert!(request["description"].is_string(), "{request}");
    let glab = fs::read_to_string(state.join("glab.lines")).expect("glab ran");
    assert_eq!(
        glab.lines()
            .filter(|line| line.contains("--method POST"))
            .count(),
        1,
        "one merge request: {glab}"
    );
    assert!(
        glab.lines()
            .all(|line| line.contains("--hostname gitlab.example")),
        "{glab}"
    );
    assert!(
        !state.join("merges").exists(),
        "nothing merged before the grant"
    );
}

/// [`repository`] with scenario `1` declared on `main`: the story `story:ready-one` at the
/// commit before the declaration.
fn scenario_repository(name: &str) -> (Scratch, PathBuf) {
    let (outer, repo) = repository(name);
    let seed = git_out(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&repo, &["add", ".engineering/scenarios.conf"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    (outer, repo)
}

/// `takt scenario run --repo <repo> 1 --test-cmd true --harness none --iterations 1` and
/// `extra`, through [`takt_through`]'s program path, its state under `<dir>/xdg-state`.
fn scenario_through(bin: &Path, dir: &Path, repo: &Path, extra: &[&str]) -> (i32, String, String) {
    let mut path = vec![bin.to_path_buf()];
    path.extend(path_without_glab());
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "scenario",
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "1",
            "--test-cmd",
            "true",
            "--harness",
            "none",
            "--iterations",
            "1",
        ])
        .args(extra)
        .env("XDG_STATE_HOME", dir.join("xdg-state"))
        .env_remove("B10X_GATES_POLICY")
        .env("PATH", std::env::join_paths(path).expect("a program path"))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The `trial/*` branches of `repo`.
fn trial_branches(repo: &Path) -> String {
    git_out(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/trial/",
        ],
    )
}

/// A scenario's trial base `trial/<n>` is created on the remote only through the bot route, so
/// `takt scenario run --forge gitlab` is refused before anything is cut or opened, exit 1,
/// naming why; `glab`, logged in, is never asked.
#[test]
fn a_scenario_run_refuses_the_gitlab_forge_before_anything_opens() {
    let (outer, repo) = scenario_repository("scenario-gitlab");
    let (bin, state) = doubles(&outer.0.join("forge"), true);
    let head = git_out(&repo, &["rev-parse", "HEAD"]);
    let (code, out, err) = scenario_through(&bin, &outer.0, &repo, &["--forge", "gitlab"]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("--forge gitlab is not for a scenario run")
            && err.contains("only through the bot route")
            && err.contains("nothing was changed"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(trial_branches(&repo), "", "stdout {out} stderr {err}");
    assert_eq!(git_out(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git_out(&repo, &["symbolic-ref", "--short", "HEAD"]), "main");
    assert!(
        !repo.join(".takt/cases").exists(),
        "no case opened: stdout {out} stderr {err}"
    );
    assert!(
        !state.join("glab.lines").exists(),
        "glab was asked: stdout {out} stderr {err}"
    );
}

/// Without `--forge`, a scenario run on an origin that is not on github.com gets no forge,
/// even with `glab` logged in to the host, and says so in one line: the trial branch is cut
/// locally and not pushed, and `glab` is never asked.
#[test]
fn a_scenario_run_on_a_gitlab_origin_defaults_to_no_forge() {
    let (outer, repo) = scenario_repository("scenario-default");
    let (bin, state) = doubles(&outer.0.join("forge"), true);
    let (_, out, err) = scenario_through(&bin, &outer.0, &repo, &[]);
    assert!(
        out.contains(
            "forge: none (origin gitlab.example is not github.com, and a scenario run creates its trial base only through the bot route)"
        ),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("not pushed: no forge"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("forge: gitlab"), "stdout {out} stderr {err}");
    assert!(
        trial_branches(&repo).starts_with("trial/"),
        "stdout {out} stderr {err}"
    );
    assert!(
        !state.join("glab.lines").exists(),
        "glab was asked: stdout {out} stderr {err}"
    );
    assert!(
        !state.join("https.lines").exists(),
        "nothing was pushed: stdout {out} stderr {err}"
    );
}
