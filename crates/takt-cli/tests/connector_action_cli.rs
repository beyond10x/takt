//! story:a-connector-read-action through the binary: a protocol action with `effect: read` the run
//! file's `connector_actions` binds to an operation of the served instance `connectors` names is
//! performed by one generic validator: one describe before the case opens, one invoke; the answer
//! is kept as `artifacts/connector/<action>.json` and the record is `canon-evidence/1` of the bound
//! kind, `pass` on success, `fail` on a refusal. No credential reaches the instance.
//!
//! The instance is a fake served on 127.0.0.1 by this test, which keeps every request it read.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// A protocol whose read action `thread.read` produces `thread_read`, a pass of which makes the
/// claim `thread.found` true and a fail the claim `thread.refused`; and a write action
/// `status.publish`, which produces `status_published`.
const PROTOCOL: &str = "format: protocol/1

protocol:
  id: connector.probe
  revision: 1
  description: reads a thread through a connector action

artifacts:
  implementation:
    description: a revision of the repository

evidence_kinds:
  thread_read:
    description: what reading the thread came to; pass or fail
  status_published:
    description: what publishing the status came to

claims:
  thread.found:
    description: the thread was read
    true_when:
      evidence:
        kind: thread_read
        result: pass
        subject: implementation
  thread.refused:
    description: the instance refused to read the thread
    true_when:
      evidence:
        kind: thread_read
        result: fail
        subject: implementation
  status.published:
    description: the status was published
    true_when:
      evidence:
        kind: status_published
        result: pass
        subject: implementation

actions:
  thread.read:
    description: read the thread
    effect: read
    may_produce:
      - evidence: thread_read
  status.publish:
    description: publish the status
    effect: write
    may_produce:
      - evidence: status_published

outcomes:
  found:
    description: the thread was read
    requires:
      claim: thread.found
  refused:
    description: the instance refused the read
    requires:
      claim: thread.refused
";

/// The operation the read binds.
const OPERATION: &str = "conversations.replies";

/// What the fake instance's invoke answers with on success.
fn messages() -> Value {
    json!({"messages": [{"ts": "1700000000.000100", "text": "hello"}]})
}

/// One request the fake instance read: its request line, its header lines and its body.
#[derive(Debug, Clone)]
struct Seen {
    line: String,
    headers: Vec<String>,
    body: String,
}

/// A fake served instance on 127.0.0.1 describing the instance `team` with `operations`; its
/// invoke answers success with [`messages`], or a refusal `forbidden` when `refuse`. Its URL
/// and what it has read so far.
fn serve(operations: &[&str], refuse: bool) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    let operations: Vec<Value> = operations.iter().map(|id| json!({"id": id})).collect();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).is_err() {
                    break;
                }
                let header = header.trim_end().to_owned();
                if header.is_empty() {
                    break;
                }
                headers.push(header);
            }
            let length = headers
                .iter()
                .find_map(|header| {
                    let (name, value) = header.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            let mut bytes = vec![0; length];
            let _ = reader.read_exact(&mut bytes);
            let body = String::from_utf8_lossy(&bytes).into_owned();
            let sent: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let path = line.split_whitespace().nth(1).unwrap_or_default();
            let (status, reply) = if path == "/v1/describe" {
                (
                    200,
                    json!({"instance": "team", "revision": "r1", "operations": operations}),
                )
            } else if refuse {
                (
                    403,
                    json!({
                        "version": "v1alpha2",
                        "request_id": sent["request_id"],
                        "status": "error",
                        "error": {"code": "forbidden", "message": "the bot may not read it"},
                        "audit_ref": null,
                        "audit_status": "not_required",
                    }),
                )
            } else {
                (
                    200,
                    json!({
                        "version": "v1alpha2",
                        "request_id": sent["request_id"],
                        "status": "success",
                        "result": messages(),
                    }),
                )
            };
            log.lock().expect("the log").push(Seen {
                line: line.trim_end().to_owned(),
                headers,
                body,
            });
            let reply = reply.to_string();
            let mut stream = stream;
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Answer\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    (url, seen)
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

/// A scratch directory under this crate's build directory, removed on drop: `tree` the
/// repository, `state` the state directory, the protocol and the run file beside them.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn state(&self) -> PathBuf {
        self.0.join("state")
    }

    fn protocol(&self) -> PathBuf {
        self.0.join("probe.yaml")
    }

    fn config(&self) -> PathBuf {
        self.0.join("run.yaml")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository beside the protocol and the run file `config`.
fn scratch(name: &str, config: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("connector-action-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(scratch.state()).expect("the state directory");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    fs::write(scratch.protocol(), PROTOCOL).expect("the protocol");
    fs::write(scratch.config(), config).expect("the run file");
    scratch
}

/// A run file naming the instance at `url` and binding `action` to `operation`.
fn binding(url: &str, action: &str, operation: &str, evidence: &str) -> String {
    format!(
        "connectors:\n  endpoint: {url}\n  instance: chat/team\nconnector_actions:\n  {action}:\n    operation: {operation}\n    input: {{channel: C0000000, ts: \"1700000000.000100\"}}\n    evidence: {evidence}\n"
    )
}

/// The built `takt run` on an intent in the scratch tree, with no harness and no forge, on the
/// probe protocol and the run file, `env` set in its environment: exit code, stdout, stderr.
fn run(scratch: &Scratch, env: &[(&str, &str)]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(scratch.tree())
        .args([
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "3",
            "--test-cmd",
            "true",
            "--intent",
            "read the thread",
        ])
        .arg("--protocol")
        .arg(scratch.protocol())
        .arg("--config")
        .arg(scratch.config())
        .arg("--state-dir")
        .arg(scratch.state())
        .envs(env.iter().copied())
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The case directories under the tree's cases directory.
fn cases(tree: &Path) -> Vec<PathBuf> {
    fs::read_dir(tree.join(".takt/cases"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect()
}

/// The one case directory.
fn the_case(tree: &Path) -> PathBuf {
    let cases = cases(tree);
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases[0].clone()
}

/// The records of `kind` under the case's `evidence/`.
fn records(case: &Path, kind: &str) -> Vec<Value> {
    fs::read_dir(case.join("evidence"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|record| record["kind"] == kind)
        .collect()
}

/// The request lines the instance read.
fn lines(seen: &Arc<Mutex<Vec<Seen>>>) -> Vec<String> {
    seen.lock()
        .expect("the log")
        .iter()
        .map(|seen| seen.line.clone())
        .collect()
}

#[test]
fn connector_read_pass() {
    let (url, seen) = serve(&[OPERATION, "chat.postMessage"], false);
    let scratch = scratch(
        "pass",
        &binding(&url, "thread.read", OPERATION, "thread_read"),
    );
    let (code, out, err) = run(&scratch, &[]);
    assert_eq!(code, 0, "stdout {out}\nstderr {err}");
    assert!(
        out.contains("stopped: Completed (found)"),
        "the claim reading the pass is true: {out}"
    );
    let case = the_case(&scratch.tree());
    let read = records(&case, "thread_read");
    assert_eq!(read.len(), 1, "one record: {read:?}\n{out}");
    let record = &read[0];
    assert_eq!(record["result"], "pass", "{record}");
    assert_eq!(record["format"], "canon-evidence/1", "{record}");
    assert_eq!(record["subject"], "implementation", "{record}");
    assert!(
        record["subject_revision"]
            .as_str()
            .is_some_and(|revision| !revision.is_empty()),
        "{record}"
    );
    assert!(
        !record.to_string().contains("hello"),
        "no payload in the record: {record}"
    );
    let kept = fs::read_to_string(case.join("artifacts/connector/thread.read.json"))
        .expect("the answer is kept");
    let kept: Value = serde_json::from_str(&kept).expect("JSON");
    assert_eq!(kept["result"], messages(), "{kept}");
    assert_eq!(
        lines(&seen),
        [
            "GET /v1/describe HTTP/1.1",
            "POST /v1alpha2/invoke HTTP/1.1"
        ],
        "one describe, one invoke"
    );
    let sent: Value =
        serde_json::from_str(&seen.lock().expect("the log")[1].body).expect("a JSON body");
    assert_eq!(sent["operation"], OPERATION, "{sent}");
    assert_eq!(sent["revision"], "r1", "{sent}");
    assert_eq!(
        sent["input"],
        json!({"channel": "C0000000", "ts": "1700000000.000100"}),
        "{sent}"
    );
}

#[test]
fn connector_read_refused() {
    let (url, seen) = serve(&[OPERATION], true);
    let scratch = scratch(
        "refused",
        &binding(&url, "thread.read", OPERATION, "thread_read"),
    );
    let (code, out, err) = run(&scratch, &[]);
    let case = the_case(&scratch.tree());
    let read = records(&case, "thread_read");
    assert_eq!(read.len(), 1, "one record: {read:?}\n{out}\n{err}");
    assert_eq!(read[0]["result"], "fail", "{}", read[0]);
    assert!(
        !out.contains("stopped: Completed (found)"),
        "the claim reading a pass is false: {out}"
    );
    assert!(
        out.contains("stopped: Completed (refused)"),
        "stdout {out}\nstderr {err}"
    );
    assert_eq!(code, 0, "stdout {out}\nstderr {err}");
    assert_eq!(
        lines(&seen),
        [
            "GET /v1/describe HTTP/1.1",
            "POST /v1alpha2/invoke HTTP/1.1"
        ]
    );
}

#[test]
fn connector_read_missing_operation() {
    let (url, seen) = serve(&["chat.postMessage"], false);
    let scratch = scratch(
        "missing",
        &binding(&url, "thread.read", OPERATION, "thread_read"),
    );
    let (code, out, err) = run(&scratch, &[]);
    assert_eq!(code, 1, "stdout {out}\nstderr {err}");
    assert!(err.contains(OPERATION), "names the operation: {err}");
    assert!(cases(&scratch.tree()).is_empty(), "no case opened: {out}");
    assert_eq!(
        lines(&seen),
        ["GET /v1/describe HTTP/1.1"],
        "nothing invoked"
    );
}

#[test]
fn connector_read_write_binding_refused() {
    let (url, seen) = serve(&[OPERATION, "chat.postMessage"], false);
    let config = binding(
        &url,
        "status.publish",
        "chat.postMessage",
        "status_published",
    );
    let scratch = scratch("write", &config);
    let (code, out, err) = run(&scratch, &[]);
    assert_eq!(code, 2, "stdout {out}\nstderr {err}");
    assert!(
        err.contains(&format!(
            "the run configuration file {} is refused at line 5: `connector_actions`",
            scratch.config().display()
        )),
        "{err}"
    );
    assert!(err.contains("effect: write"), "{err}");
    assert!(
        !lines(&seen).iter().any(|line| line.starts_with("POST")),
        "nothing invoked: {:?}",
        lines(&seen)
    );
    for case in cases(&scratch.tree()) {
        assert!(records(&case, "status_published").is_empty());
    }
}

/// An action `connector_actions` binds and `command_actions` binds after it is refused at the
/// later binding, the `command_actions` one, naming the action and both members; nothing is
/// called.
#[test]
fn connector_read_action_bound_twice_refused_at_the_later_binding() {
    let (url, seen) = serve(&[OPERATION], false);
    let config = format!(
        "{}command_actions:\n  thread.read:\n    argv: [touch, \"{{artifacts}}/x.txt\"]\n    writes: x.txt\n    evidence: thread_read\n",
        binding(&url, "thread.read", OPERATION, "thread_read")
    );
    let scratch = scratch("twice", &config);
    let (code, out, err) = run(&scratch, &[]);
    assert_eq!(code, 2, "stdout {out}\nstderr {err}");
    assert!(
        err.contains(&format!(
            "the action thread.read: the run configuration file {} is refused at line 10: `command_actions` binds an action `connector_actions` binds too",
            scratch.config().display()
        )),
        "{err}"
    );
    assert!(
        lines(&seen).is_empty(),
        "nothing called: {:?}",
        lines(&seen)
    );
    assert!(cases(&scratch.tree()).is_empty(), "no case opened: {out}");
}

#[test]
fn connector_read_endpoint_rule() {
    let (_url, seen) = serve(&[OPERATION], false);
    let scratch = scratch(
        "endpoint",
        &binding(
            "http://example.com",
            "thread.read",
            OPERATION,
            "thread_read",
        ),
    );
    let (code, out, err) = run(&scratch, &[]);
    assert_eq!(code, 2, "stdout {out}\nstderr {err}");
    assert!(
        err.contains(&format!(
            "the run configuration file {} is refused at line 2: `connectors.endpoint`",
            scratch.config().display()
        )),
        "{err}"
    );
    assert!(!err.contains("example.com"), "{err}");
    assert!(cases(&scratch.tree()).is_empty(), "no case opened");
    let toml = scratch.0.join("run.toml");
    fs::write(
        &toml,
        "[connectors]\nendpoint = \"http://example.com\"\ninstance = \"chat/team\"\n",
    )
    .expect("a file");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(scratch.tree())
        .args(["--harness", "none", "--forge", "none", "--intent", "read"])
        .arg("--config")
        .arg(&toml)
        .output()
        .expect("takt runs");
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{err}");
    assert!(
        err.contains(&format!(
            "{} is refused at line 2: `connectors.endpoint`",
            toml.display()
        )),
        "{err}"
    );
    assert!(lines(&seen).is_empty());
}

#[test]
fn connector_read_no_credential() {
    let (url, seen) = serve(&[OPERATION], false);
    let scratch = scratch(
        "no-credential",
        &binding(&url, "thread.read", OPERATION, "thread_read"),
    );
    let secrets = [
        ("CONNECTORS_TOKEN", "connectors-secret-value"),
        ("SLACK_BOT_TOKEN", "slack-secret-value"),
        ("GITHUB_TOKEN", "github-secret-value"),
    ];
    let (code, out, err) = run(&scratch, &secrets);
    assert_eq!(code, 0, "stdout {out}\nstderr {err}");
    let seen = seen.lock().expect("the log").clone();
    assert_eq!(seen.len(), 2, "{seen:?}");
    for request in &seen {
        assert!(
            !request.headers.iter().any(|header| header
                .split_once(':')
                .is_some_and(|(name, _)| name.eq_ignore_ascii_case("authorization"))),
            "{}: {:?}",
            request.line,
            request.headers
        );
        for (name, value) in secrets {
            let sent = format!("{}\n{}", request.headers.join("\n"), request.body);
            assert!(!sent.contains(value), "{name}'s value reached the instance");
        }
    }
}
