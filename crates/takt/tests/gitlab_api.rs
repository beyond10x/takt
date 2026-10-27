//! story:the-gitlab-forge-can-call-the-api-with-a-named-credential: `transport: api` of the
//! `gitlab` forge calls the GitLab REST API over HTTPS under `credential: {env: NAME}` or
//! `{file: PATH}`. The host here is a fake GitLab on a loopback port of this process, where the
//! `api` transport speaks plain HTTP (it never does to another host); lines the forge runs go
//! through a double that records the argument list and the environment of each. Nothing here
//! reaches a real host.

use std::cell::RefCell;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::Result;
use serde_json::{Value, json};
use takt::forge::credential::CredentialSource;
use takt::forge::gitlab::{Glab, Transport};
use takt::forge::{Checks, Exit, Forge, PullRequest, PullRequestState, Runner};
use takt::run_config::RunConfig;
use takt::scan::Scan;

const TOKEN: &str = "tokenvalue-for-the-fake-host";
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const OLDER: &str = "89abcdef0123456789abcdef0123456789abcdef";

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("gitlab-api-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }

    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn artifacts(&self) -> PathBuf {
        self.0.join("case").join("artifacts")
    }

    fn state(&self) -> PathBuf {
        self.0.join("state")
    }

    fn scan(&self) -> Scan {
        Scan::None {
            declared_in: self.0.join("run.yaml"),
        }
    }

    /// A credential file outside the tree holding `text`, of the mode `mode`.
    fn credential(&self, text: &str, mode: u32) -> PathBuf {
        let path = self.0.join("token");
        fs::write(&path, text).expect("a credential file");
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("its mode");
        path
    }

    /// A repository whose `origin` is the fake host's project `group/sub/repo`.
    fn repository(&self, host: &str) {
        let tree = self.tree();
        fs::create_dir_all(&tree).expect("a tree");
        for args in [
            vec!["init", "-q"],
            vec![
                "remote",
                "add",
                "origin",
                &format!("https://{host}/group/sub/repo.git"),
            ],
        ] {
            let status = Command::new("git")
                .arg("-C")
                .arg(&tree)
                .args(args)
                .status()
                .expect("git runs");
            assert!(status.success());
        }
        fs::create_dir_all(self.state()).expect("a state directory");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One request the fake host received.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    token: Option<String>,
    body: String,
}

#[derive(Default)]
struct Host {
    seen: Vec<Seen>,
    pipeline: Value,
    merge_request: Value,
    echo_token: bool,
}

/// A fake GitLab on a loopback port: opens merge request 7, reads it back with the head pipeline
/// the test sets, and merges it when the head it is given is the one it holds.
struct FakeGitlab {
    address: String,
    host: Arc<Mutex<Host>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeGitlab {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let address = listener.local_addr().expect("its address").to_string();
        let host = Arc::new(Mutex::new(Host {
            pipeline: Value::Null,
            ..Host::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (host, stop) = (host.clone(), stop.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        serve(stream, &host);
                    }
                }
            })
        };
        Self {
            address,
            host,
            stop,
            thread: Some(thread),
        }
    }

    fn seen(&self) -> Vec<Seen> {
        self.host.lock().unwrap().seen.clone()
    }

    fn pipeline(&self, sha: &str, status: &str) {
        self.host.lock().unwrap().pipeline = json!({ "sha": sha, "status": status });
    }

    fn echo_token(&self) {
        self.host.lock().unwrap().echo_token = true;
    }
}

impl Drop for FakeGitlab {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(mut stream: TcpStream, host: &Mutex<Host>) {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head, mut body) = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            return;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(at) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&bytes[..at]).into_owned();
            break (head, bytes[at + 4..].to_vec());
        }
    };
    let mut lines = head.lines();
    let mut request = lines.next().unwrap_or_default().split_whitespace();
    let method = request.next().unwrap_or_default().to_owned();
    let path = request.next().unwrap_or_default().to_owned();
    let mut token = None;
    let mut length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "private-token" => token = Some(value.trim().to_owned()),
                "content-length" => length = value.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    while body.len() < length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    let body = String::from_utf8_lossy(&body).into_owned();
    let (status, answer) = {
        let mut host = host.lock().unwrap();
        host.seen.push(Seen {
            method: method.clone(),
            path: path.clone(),
            token: token.clone(),
            body: body.clone(),
        });
        answer(&mut host, &method, &path, token.as_deref(), &body)
    };
    let reply = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
        answer.len()
    );
    let _ = stream.write_all(reply.as_bytes());
}

fn answer(
    host: &mut Host,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: &str,
) -> (u16, String) {
    if token != Some(TOKEN) {
        return (401, json!({ "message": "401 Unauthorized" }).to_string());
    }
    if host.echo_token {
        let message = format!("500 the host choked on the token {TOKEN} it was given");
        return (500, json!({ "message": message }).to_string());
    }
    let base = "/api/v4/projects/group%2Fsub%2Frepo/merge_requests";
    let sent: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    match (method, path) {
        ("POST", p) if p == base => {
            host.merge_request = json!({
                "iid": 7,
                "state": "opened",
                "source_branch": sent["source_branch"],
                "target_branch": sent["target_branch"],
                "sha": HEAD,
            });
            (201, json!({ "iid": 7 }).to_string())
        }
        ("GET", p) if p == format!("{base}/7") => {
            let mut document = host.merge_request.clone();
            document["head_pipeline"] = host.pipeline.clone();
            (200, document.to_string())
        }
        ("PUT", p) if p == format!("{base}/7/merge") => {
            if sent["sha"] == HEAD {
                host.merge_request["state"] = json!("merged");
                (200, json!({ "state": "merged" }).to_string())
            } else {
                (
                    409,
                    json!({ "message": "SHA does not match HEAD of source branch" }).to_string(),
                )
            }
        }
        _ => (404, json!({ "message": "404 Not found" }).to_string()),
    }
}

/// A line run: its argument list and the environment it was run with.
type Call = (Vec<String>, Vec<(String, String)>);

/// The line double: records each argument list with the environment it was run with.
#[derive(Clone, Default)]
struct Recording {
    calls: Rc<RefCell<Vec<Call>>>,
}

impl Recording {
    fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }
}

impl Runner for Recording {
    fn run(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _tree: &Path, argv: &[String], env: &[(&str, &str)]) -> Result<Exit> {
        self.calls.borrow_mut().push((
            argv.to_vec(),
            env.iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        ));
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

fn request() -> PullRequest {
    PullRequest {
        branch: "takt/case-1".to_owned(),
        base: "main".to_owned(),
        title: "A title".to_owned(),
        body: "the body".to_owned(),
    }
}

/// The forge of the scratch repository under the credential file, every line run through
/// `runner`.
fn forge_over(scratch: &Scratch, host: &FakeGitlab, runner: &Recording) -> Result<Glab> {
    scratch.repository(&host.address);
    let source = CredentialSource::File(scratch.credential(&format!("{TOKEN}\n"), 0o600));
    Glab::for_tree_api_through(
        &scratch.tree(),
        &scratch.scan(),
        &source,
        Box::new(runner.clone()),
    )
}

/// Every file under `dir`, with its contents as text.
fn files_under(dir: &Path, found: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, found);
        } else if let Ok(bytes) = fs::read(&path) {
            found.push((path, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
}

/// The merge request is opened, read and merged through the API, each call carrying the token
/// in its header, and no `glab` line is run for any of it.
#[test]
fn opens_reads_and_merges_through_the_api() {
    let scratch = Scratch::new("opens");
    let host = FakeGitlab::start();
    let runner = Recording::default();
    let forge = forge_over(&scratch, &host, &runner).expect("a forge");
    assert_eq!(forge.transport(), Transport::Api);
    host.pipeline(HEAD, "success");

    let number = forge
        .open_pull_request(&scratch.tree(), &scratch.artifacts(), &request())
        .expect("opens");
    assert_eq!(number, 7);
    let view = forge.pull_request(&scratch.tree(), number).expect("reads");
    assert_eq!(view.state, PullRequestState::Open);
    assert_eq!(view.head.sha, HEAD);
    assert_eq!(view.head.branch, "takt/case-1");
    assert_eq!(view.base, "main");
    assert_eq!(
        forge.checks(&scratch.tree(), number).expect("checks"),
        Checks::Pass
    );
    forge
        .merge(&scratch.tree(), &scratch.artifacts(), number, HEAD)
        .expect("merges");

    let seen = host.seen();
    let lines: Vec<String> = seen
        .iter()
        .map(|call| format!("{} {}", call.method, call.path))
        .collect();
    let base = "/api/v4/projects/group%2Fsub%2Frepo/merge_requests";
    assert_eq!(
        lines,
        [
            format!("POST {base}"),
            format!("GET {base}/7"),
            format!("GET {base}/7"),
            format!("PUT {base}/7/merge"),
        ]
    );
    assert!(seen.iter().all(|call| call.token.as_deref() == Some(TOKEN)));
    let opened: Value = serde_json::from_str(&seen[0].body).expect("a JSON body");
    assert_eq!(opened["source_branch"], "takt/case-1");
    assert_eq!(opened["target_branch"], "main");
    let merged: Value = serde_json::from_str(&seen[3].body).expect("a JSON body");
    assert_eq!(merged["sha"], HEAD);
    assert!(
        runner.calls().is_empty(),
        "no line is run for an API call: {:?}",
        runner.calls()
    );
}

/// The token is in no argument line, no file under the tree, the state directory or the case,
/// no error and no debug text, whatever the host answers; the one line that carries it is the
/// push, in that line's environment.
#[test]
fn the_token_is_never_written() {
    let scratch = Scratch::new("never-written");
    let host = FakeGitlab::start();
    let runner = Recording::default();
    let forge = forge_over(&scratch, &host, &runner).expect("a forge");
    host.pipeline(HEAD, "success");

    let mut said = Vec::new();
    let number = forge
        .open_pull_request(&scratch.tree(), &scratch.artifacts(), &request())
        .expect("opens");
    forge.pull_request(&scratch.tree(), number).expect("reads");
    forge.checks(&scratch.tree(), number).expect("checks");
    said.push(format!(
        "{:#}",
        forge
            .merge(&scratch.tree(), &scratch.artifacts(), number, OLDER)
            .expect_err("a head that is not the request's is refused by the host")
    ));
    forge
        .push_revision(&scratch.tree(), "takt/case-1", HEAD)
        .expect("the push line runs");
    host.echo_token();
    said.push(format!(
        "{:#}",
        forge
            .pull_request(&scratch.tree(), number)
            .expect_err("a host that errors")
    ));
    said.push(format!(
        "{:#}",
        forge
            .open_pull_request(&scratch.tree(), &scratch.artifacts(), &request())
            .expect_err("a host that errors")
    ));
    said.push(format!("{forge:?}"));
    for text in &said {
        assert!(!text.contains(TOKEN), "the token is in `{text}`");
    }
    assert!(
        said.iter().any(|text| text.contains("<redacted>")),
        "the host's echo of the token is replaced: {said:?}"
    );

    let calls = runner.calls();
    assert_eq!(calls.len(), 1, "only the push is a line: {calls:?}");
    let (argv, env) = &calls[0];
    assert!(argv.iter().all(|word| !word.contains(TOKEN)), "{argv:?}");
    assert!(
        argv.iter()
            .any(|word| word.starts_with("credential.helper=!f()"))
    );
    assert_eq!(
        env.iter()
            .find(|(name, _)| name == "TAKT_FORGE_SECRET")
            .map(|(_, value)| value.as_str()),
        Some(TOKEN),
        "the helper of that one line reads it from its environment"
    );

    let mut found = Vec::new();
    files_under(&scratch.0, &mut found);
    found.retain(|(path, _)| path != &scratch.0.join("token"));
    assert!(
        found
            .iter()
            .any(|(path, _)| path.starts_with(scratch.artifacts())),
        "the case directory holds what the forge wrote"
    );
    for (path, text) in &found {
        assert!(!text.contains(TOKEN), "the token is in {}", path.display());
    }
}

/// A credential file open to group or others, or inside the tree, is refused before anything is
/// asked of the host, naming the source and not its value.
#[test]
fn an_open_credential_file_is_refused() {
    let scratch = Scratch::new("open-file");
    let host = FakeGitlab::start();
    scratch.repository(&host.address);

    let open = CredentialSource::File(scratch.credential(&format!("{TOKEN}\n"), 0o644));
    let error = Glab::for_tree_api_through(
        &scratch.tree(),
        &scratch.scan(),
        &open,
        Box::new(Recording::default()),
    )
    .expect_err("a file open to others is refused");
    let text = format!("{error:#}");
    assert!(text.contains("644"), "{text}");
    assert!(
        text.contains(&scratch.0.join("token").display().to_string()),
        "{text}"
    );
    assert!(!text.contains(TOKEN), "{text}");

    let inside = scratch.tree().join("token");
    fs::write(&inside, format!("{TOKEN}\n")).expect("a file");
    fs::set_permissions(&inside, fs::Permissions::from_mode(0o600)).expect("its mode");
    let error = Glab::for_tree_api_through(
        &scratch.tree(),
        &scratch.scan(),
        &CredentialSource::File(inside),
        Box::new(Recording::default()),
    )
    .expect_err("a file inside the tree is refused");
    let text = format!("{error:#}");
    assert!(text.contains("inside"), "{text}");
    assert!(!text.contains(TOKEN), "{text}");
    assert!(host.seen().is_empty(), "nothing was asked of the host");
}

/// An environment variable that is not set is refused by its name.
#[test]
fn a_missing_env_is_refused_by_name() {
    let scratch = Scratch::new("missing-env");
    let host = FakeGitlab::start();
    scratch.repository(&host.address);
    let name = "TAKT_GITLAB_API_TEST_NO_SUCH_TOKEN";
    let error = Glab::for_tree_api_through(
        &scratch.tree(),
        &scratch.scan(),
        &CredentialSource::Env(name.to_owned()),
        Box::new(Recording::default()),
    )
    .expect_err("an unset variable is refused");
    let text = format!("{error:#}");
    assert!(text.contains(name), "{text}");
    assert!(text.contains("not set"), "{text}");
    assert!(host.seen().is_empty());
}

/// The head pipeline counts only when it is the head's own: one of another commit is pending,
/// whatever its status.
#[test]
fn checks_are_bound_to_the_head() {
    let scratch = Scratch::new("checks");
    let host = FakeGitlab::start();
    let runner = Recording::default();
    let forge = forge_over(&scratch, &host, &runner).expect("a forge");
    let number = forge
        .open_pull_request(&scratch.tree(), &scratch.artifacts(), &request())
        .expect("opens");
    for (sha, status, expected) in [
        (OLDER, "success", Checks::Pending),
        (OLDER, "failed", Checks::Pending),
        (HEAD, "success", Checks::Pass),
        (HEAD, "failed", Checks::Fail),
        (HEAD, "running", Checks::Pending),
    ] {
        host.pipeline(sha, status);
        assert_eq!(
            forge.checks(&scratch.tree(), number).expect("checks"),
            expected,
            "pipeline of {sha} {status}"
        );
    }
    host.host.lock().unwrap().pipeline = Value::Null;
    assert_eq!(
        forge.checks(&scratch.tree(), number).expect("checks"),
        Checks::Pending
    );
}

/// Without `transport: api` nothing changes: the forge runs `glab` lines, no call reaches the
/// host, a run file without the keys reads as before, and the keys are refused where they do not
/// fit.
#[test]
fn glab_stays_the_default() {
    let scratch = Scratch::new("default");
    let host = FakeGitlab::start();
    scratch.repository(&host.address);
    let runner = Recording::default();
    let forge = Glab::for_tree_through(&scratch.tree(), &scratch.scan(), Box::new(runner.clone()))
        .expect("a forge under glab's login");
    assert_eq!(forge.transport(), Transport::Glab);
    let calls = runner.calls();
    assert_eq!(calls.len(), 1, "the login check: {calls:?}");
    assert_eq!(&calls[0].0[..2], ["glab", "auth"]);
    assert!(
        forge
            .open_pull_request(&scratch.tree(), &scratch.artifacts(), &request())
            .is_err(),
        "the double answers no merge request"
    );
    let calls = runner.calls();
    assert!(
        calls[1].0.iter().any(|word| word == "api") && calls[1].0[0] == "glab",
        "{calls:?}"
    );
    assert!(host.seen().is_empty(), "no call reached the host");
    let push = forge.push_revision_argv("takt/case-1", HEAD);
    assert!(
        push.contains(&Glab::CREDENTIAL_HELPER.to_owned()),
        "{push:?}"
    );

    let path = scratch.0.join("run.toml");
    fs::write(&path, "forge = \"gitlab\"\n").expect("a file");
    let settings = RunConfig::read(&path).expect("reads").forge_settings();
    assert_eq!(settings.transport, None);
    settings
        .check_for(Some("gitlab"))
        .expect("the default fits");
    fs::write(&path, "forge = \"gitlab\"\ntransport = \"glab\"\n").expect("a file");
    let settings = RunConfig::read(&path).expect("reads").forge_settings();
    assert_eq!(settings.transport, Some(Transport::Glab));
    settings.check_for(Some("gitlab")).expect("glab fits");

    fs::write(
        &path,
        "forge = \"gitlab\"\ntransport = \"api\"\ncredential = { file = \"token\" }\n",
    )
    .expect("a file");
    let settings = RunConfig::read(&path).expect("reads").forge_settings();
    assert_eq!(settings.transport, Some(Transport::Api));
    assert_eq!(
        settings.credential,
        Some(CredentialSource::File(scratch.0.join("token"))),
        "a relative file is read beside the run file"
    );
    settings
        .check_for(Some("gitlab"))
        .expect("api with a credential fits");
    assert!(settings.check_for(Some("bot")).is_err());
    assert!(settings.check_for(None).is_err());

    for (text, why) in [
        ("forge = \"gitlab\"\ntransport = \"api\"\n", "no credential"),
        (
            "forge = \"gitlab\"\ncredential = { env = \"X\" }\n",
            "a credential without the api transport",
        ),
        (
            "forge = \"gitlab\"\ntransport = \"ftp\"\n",
            "an unknown transport",
        ),
        (
            "forge = \"gitlab\"\ntransport = \"api\"\ncredential = \"inline\"\n",
            "a credential that is no reference",
        ),
    ] {
        fs::write(&path, text).expect("a file");
        let refused = RunConfig::read(&path)
            .and_then(|config| config.forge_settings().check_for(Some("gitlab")));
        assert!(refused.is_err(), "{why}");
    }
}
