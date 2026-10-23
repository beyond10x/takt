//! Adversary pass 1 on the trial fetch that needs no user credential: what reaches the
//! `b10x-gates bot -- fetch` child, and what that child's git does with the HTTPS URL takt names.
//! The `b10x-gates` double resolves and fetches the URL as the bot route's git does (no global or
//! system configuration, its own credential helper for `https://github.com`), so the
//! repository's own configuration is the only configuration it reads. `ssh` is a double on the
//! program path that records how it was started and refuses; nothing reaches a network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// Where the scratch repository declares its scenarios.
const SCENARIOS: &str = ".engineering/scenarios.conf";

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

fn executable(bin: &Path, name: &str, text: &str) {
    let path = bin.join(name);
    fs::write(&path, text).expect("the stub is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
}

struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("trial-fetch-adv1-{}-{name}", std::process::id()));
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
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(SCENARIOS),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&repo, &["add", SCENARIOS]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let bin = outer.join("stub-bin");
    let state = outer.join("forge");
    fs::create_dir_all(&state).expect("a state directory");
    let remote = outer.join("remote.git");
    git(
        &outer,
        &["init", "-q", "-b", "main", "--bare", "remote.git"],
    );
    doubles(&bin, &state, &remote);
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        state,
    }
}

/// The doubles. `b10x-gates bot -- fetch <url> <refspec>` records `GIT_ASKPASS` and
/// `SSH_ASKPASS` in `state/fetch-env`; with `state/orphan-fetch` it starts a `sleep` in a
/// session of its own (its pid in `state/orphan.pid`) and hangs; with `state/refuse-fetch` it
/// fails at once; otherwise it resolves `<url>` as the bot route's git does (`GIT_CONFIG_GLOBAL`
/// `/dev/null`, no system configuration) into `state/resolved` and, for an
/// `https://github.com/` URL, fetches the refspec from the bare `remote` (standing in for the
/// HTTPS server, so without the HTTPS-only transport takt sets); for an SSH URL it runs
/// that git's real fetch, which starts the `ssh` double; anything else it refuses. `ssh`
/// appends its arguments to `state/ssh-calls` and refuses. `push`, `scan-text`, `api`, `gh` and
/// `claude` answer as in the scenario tests.
fn doubles(bin: &Path, state: &Path, remote: &Path) {
    fs::create_dir_all(bin).expect("a stub directory");
    let state = state.display().to_string();
    let remote = remote.display().to_string();
    executable(
        bin,
        "b10x-gates",
        &format!(
            r#"#!/bin/sh
state='{state}'
remote='{remote}'
echo "$*" >> "$state/calls"
case "$1" in
  bot)
    shift 2
    verb="$1"; shift
    options=''
    while [ $# -gt 0 ]; do
      case "$1" in
        -*) options="$options $1"; shift ;;
        *) break ;;
      esac
    done
    url="$1"
    spec="$2"
    case "$verb" in
      fetch)
        {{
          printf 'GIT_ASKPASS=%s\n' "${{GIT_ASKPASS-unset}}"
          printf 'SSH_ASKPASS=%s\n' "${{SSH_ASKPASS-unset}}"
        }} > "$state/fetch-env"
        if [ -e "$state/orphan-fetch" ]; then
          setsid sleep 50 &
          echo $! > "$state/orphan.pid"
          sleep 45; exit 1
        fi
        if [ -e "$state/refuse-fetch" ]; then
          echo "Please make sure you have the correct access rights" >&2
          exit 1
        fi
        resolved=$(GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git ls-remote --get-url "$url")
        echo "$resolved" > "$state/resolved"
        case "$resolved" in
          https://github.com/*) env -u GIT_ALLOW_PROTOCOL git fetch -q "$remote" "$spec"; exit $? ;;
          ssh://*|*@*:*)
            GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git \
              -c 'credential.https://github.com.helper=' \
              -c 'credential.https://github.com.helper=!f() {{ echo username=x-access-token; echo password=bot; }}; f' \
              fetch -q "$url" "$spec"
            exit $? ;;
          *) echo "the double fetches from no network: $resolved" >&2; exit 1 ;;
        esac ;;
      push)
        branch="${{spec#*:refs/heads/}}"; revision="${{spec%%:*}}"
        if [ "$branch" = "$spec" ]; then revision="refs/heads/$spec"; fi
        git rev-parse "$revision" > "$state/pushed-$(printf '%s' "$branch" | tr / _)" || exit 1
        git push $options "$remote" "$spec" 2>/dev/null; exit $? ;;
    esac ;;
  scan-text) exit 0 ;;
  api)
    method=''; input=''; output=''
    while [ $# -gt 0 ]; do
      case "$1" in
        --method) method="$2"; shift ;;
        --input) input="$2"; shift ;;
        --output) output="$2"; shift ;;
      esac
      shift
    done
    if [ "$method" = POST ]; then
      k=$(( $(ls "$state" | grep -c '^pr-.*\.json$') + 1 ))
      cp "$input" "$state/pr-$k.json"
      sed -n 's/.*"head": "\(.*\)".*/\1/p' "$input" > "$state/pr-$k.head"
      printf '{{"number": %s}}' "$k" > "$output"
      exit 0
    fi ;;
esac
echo "b10x-gates double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        bin,
        "ssh",
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> '{state}/ssh-calls'
echo "git@github.com: Permission denied (publickey)." >&2
exit 255
"#
        ),
    );
    executable(
        bin,
        "gh",
        &format!(
            r#"#!/bin/sh
state='{state}'
case "$2" in
  view)
    head=$(cat "$state/pr-$3.head")
    oid=$(tr -d '\n' < "$state/pushed-$(printf '%s' "$head" | tr / _)")
    printf '{{"state":"OPEN","mergedAt":null,"headRefName":"%s","headRefOid":"%s","baseRefName":"main"}}' "$head" "$oid"
    exit 0 ;;
  checks) printf '[{{"bucket":"pass"}}]'; exit 0 ;;
esac
echo "gh double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        bin,
        "claude",
        r#"#!/bin/sh
case "$2" in
  *"These need an approval"*) action=repository.merge ;;
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else echo "the work" >> work.txt; action=implementation.edit; fi ;;
  *) echo "the work" >> work.txt; action=implementation.edit ;;
esac
printf '{"result":"{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}}' "$action"
"#,
    );
}

impl World {
    fn takt(&self, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .envs(env.iter().copied())
            .env_remove("B10X_GATES_POLICY")
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

    fn scenario_run(&self, extra: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["scenario", "run", "--repo", repo, "1", "--test-cmd", "true"];
        args.extend_from_slice(extra);
        self.takt(&args, env)
    }

    fn read(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.state.join(name)).ok()
    }

    fn calls(&self) -> Vec<String> {
        self.read("calls")
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

/// The acceptance, the route half, with an `insteadOf` rule in the repository's own
/// configuration rather than the user's global one: the trial fetch names the HTTPS URL, the
/// bot route's git rewrites it to SSH, and SSH starts with the user's `~/.ssh` (its config, its
/// default keys) under `HOME`. The fetch must go over HTTPS, or refuse SSH outright, and never
/// start `ssh`.
#[test]
fn a_repository_insteadof_rule_does_not_send_the_trial_fetch_to_the_users_ssh() {
    let world = world("local-insteadof");
    git(
        &world.repo,
        &[
            "config",
            "--local",
            "url.ssh://git@github.com/.insteadOf",
            "https://github.com/",
        ],
    );
    let (code, out, err) = world.scenario_run(&[], &[]);
    let resolved = world.read("resolved").map(|text| text.trim().to_owned());
    let ssh = world.read("ssh-calls");
    let over_https = resolved.as_deref() == Some("https://github.com/owner/repo.git");
    let ssh_refused = err.contains("transport 'ssh' not allowed");
    assert!(
        ssh.is_none() && (over_https || ssh_refused),
        "the URL the bot route's git resolved {resolved:?}, every ssh it started {ssh:?}: code {code} stdout {out} stderr {err}"
    );
}

/// `GIT_TERMINAL_PROMPT=0` does not stop git asking a person: git runs `GIT_ASKPASS`, else
/// `core.askPass`, else `SSH_ASKPASS` first, whatever `GIT_TERMINAL_PROMPT` says. A user's askpass
/// (an editor's credential bridge, a desktop dialog) must not be what the bot route's git can
/// reach: either `GIT_ASKPASS` is set empty, which ends git's lookup, or neither variable is
/// passed on.
#[test]
fn the_trial_fetch_child_reaches_no_user_askpass() {
    let world = world("askpass");
    fs::write(world.state.join("refuse-fetch"), "").expect("the double refuses the fetch");
    let (code, out, err) = world.scenario_run(
        &[],
        &[
            ("GIT_ASKPASS", "/user/editor-askpass.sh"),
            ("SSH_ASKPASS", "/user/desktop-askpass"),
        ],
    );
    let seen = world.read("fetch-env").expect("the fetch ran");
    let lines: Vec<&str> = seen.lines().collect();
    let closed = matches!(
        lines.as_slice(),
        ["GIT_ASKPASS=", _] | ["GIT_ASKPASS=unset", "SSH_ASKPASS=unset"]
    );
    assert!(
        closed,
        "the askpass the fetch child's git can reach: {lines:?}; code {code} stdout {out} stderr {err}"
    );
}

/// The 30 s bound leaves nothing behind: a process the fetch started in a session of its own,
/// outside its process group, is gone when takt has answered.
#[test]
fn a_timed_out_trial_fetch_leaves_no_process_behind() {
    let world = world("orphan");
    fs::write(world.state.join("orphan-fetch"), "").expect("the double hangs");
    let started = Instant::now();
    let (code, out, err) = world.scenario_run(&[], &[]);
    let took = started.elapsed();
    let pid = world
        .read("orphan.pid")
        .expect("the double started its orphan")
        .trim()
        .to_owned();
    // Listed by `ps`, not a zombie, and still the double's `sleep`.
    let alive = Command::new("ps")
        .args(["-o", "stat=,args=", "-p", &pid])
        .output()
        .is_ok_and(|output| {
            let listed = String::from_utf8_lossy(&output.stdout);
            let listed = listed.trim();
            output.status.success()
                && !listed.starts_with('Z')
                && listed
                    .split_once(char::is_whitespace)
                    .is_some_and(|(_, args)| args.trim_start().starts_with("sleep"))
        });
    if alive {
        let _ = Command::new("kill").args(["-9", &pid]).status();
    }
    assert!(!alive, "sleep {pid} outlived takt: stderr {err}");
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(took < Duration::from_secs(35), "took {took:?}");
    assert!(err.contains("timed out after 30.0 s"), "stderr {err}");
}

/// Each origin form on github.com, with a port, a trailing slash, an upper-case host, dots and
/// dashes in the names, or a credential in its userinfo, fetches from the one HTTPS URL, and the
/// credential appears in nothing takt runs or prints. Another host, or no origin, fetches
/// nothing.
#[test]
fn origin_forms_map_to_one_https_url_and_no_userinfo_travels() {
    let world = world("origin-edges");
    fs::write(world.state.join("refuse-fetch"), "").expect("the double refuses the fetch");
    let line = "bot -- fetch https://github.com/my-org/my.repo-x.git +refs/heads/trial/*:refs/remotes/origin/trial/*";
    for origin in [
        "ssh://git@github.com:22/my-org/my.repo-x.git",
        "https://github.com/my-org/my.repo-x.git/",
        "git@GitHub.com:my-org/my.repo-x.git",
        "https://user:s3cr3t-tok@github.com/my-org/my.repo-x",
    ] {
        git(&world.repo, &["remote", "set-url", "origin", origin]);
        let _ = fs::remove_file(world.state.join("calls"));
        let (code, out, err) = world.scenario_run(&[], &[]);
        assert_eq!(code, 1, "origin {origin}: stdout {out} stderr {err}");
        assert_eq!(world.calls(), [line], "origin {origin}");
        assert!(
            !format!("{out}{err}").contains("s3cr3t"),
            "origin {origin}: stdout {out} stderr {err}"
        );
    }
    for origin in ["git@github.example.com:my-org/my.repo-x.git", ""] {
        if origin.is_empty() {
            git(&world.repo, &["remote", "remove", "origin"]);
        } else {
            git(&world.repo, &["remote", "set-url", "origin", origin]);
        }
        let _ = fs::remove_file(world.state.join("calls"));
        let (code, out, err) = world.scenario_run(&["--forge", "bot"], &[]);
        assert_eq!(code, 1, "origin {origin:?}: stdout {out} stderr {err}");
        assert!(world.calls().is_empty(), "origin {origin:?}");
        assert!(err.contains("give --forge none") || err.contains("needs an origin remote"));
    }
}
