//! Adversarial cases for story:a-connector-read-action (wave 59) through the binary.
//!
//! The record of a connector read action is `pass` on `status: success` and `fail` on a refusal
//! (design § 4 Read action, the story's Outcome, `takt.work.ConnectorAction`). These cases drive
//! the validator with answers that are no refusal of the read by the instance, with a call in
//! flight at the end of the run's hour budget, with the same action bound by two members of the
//! run file, and with an answer whose kept artifact outgrows the bound the client reads it to.
//!
//! The instance is a fake served on 127.0.0.1 by this test; each connection is answered on a
//! thread of its own.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// A protocol whose read action `thread.read` produces `thread_read`; a pass makes the outcome
/// `found`, a fail the outcome `refused`.
const PROTOCOL: &str = "format: protocol/1

protocol:
  id: connector.adversary
  revision: 1
  description: reads a thread through a connector action

artifacts:
  implementation:
    description: a revision of the repository

evidence_kinds:
  thread_read:
    description: what reading the thread came to; pass or fail

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

actions:
  thread.read:
    description: read the thread
    effect: read
    may_produce:
      - evidence: thread_read

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

const OPERATION: &str = "conversations.replies";

/// What the fake answers the invoke with.
#[derive(Clone)]
enum Invoke {
    /// The served host's answer to an invocation at a revision it no longer describes
    /// (connectors-host `ErrorCode::StaleDescription`): "refresh the descriptor before
    /// resubmitting".
    Stale,
    /// A gateway in front of the instance: HTTP 502 with an HTML page, no envelope.
    Gateway,
    /// A redirect to another host, which takt never follows.
    Redirect,
    /// Success, after `delay`.
    Slow(Duration),
    /// Success with a result of `depth` nested arrays around `zeros` zeros.
    Nested { depth: usize, zeros: usize },
}

/// A fake served instance describing `team` at `r1` with [`OPERATION`]: its URL and the request
/// lines it has read.
fn serve(invoke: Invoke) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let log = Arc::clone(&log);
            let invoke = invoke.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() {
                        break;
                    }
                    let header = header.trim_end().to_owned();
                    if header.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut bytes = vec![0; length];
                let _ = reader.read_exact(&mut bytes);
                let sent: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                let path = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                log.lock()
                    .expect("the log")
                    .push(line.trim_end().to_owned());
                let success = |result: String| {
                    format!(
                        "{{\"version\":\"v1alpha2\",\"request_id\":{},\"status\":\"success\",\"result\":{result}}}",
                        sent["request_id"]
                    )
                };
                let (status, kind, extra, body) = if path == "/v1/describe" {
                    (
                        200,
                        "application/json",
                        String::new(),
                        json!({"instance": "team", "revision": "r1", "operations": [{"id": OPERATION}]})
                            .to_string(),
                    )
                } else {
                    match invoke {
                        Invoke::Stale => (
                            409,
                            "application/json",
                            String::new(),
                            json!({
                                "version": "v1alpha2",
                                "request_id": sent["request_id"],
                                "status": "error",
                                "error": {"code": "stale_description", "message": "refresh the descriptor before resubmitting"},
                                "audit_ref": null,
                                "audit_status": "not_required",
                            })
                            .to_string(),
                        ),
                        Invoke::Gateway => (
                            502,
                            "text/html",
                            String::new(),
                            "<html><body>502 Bad Gateway</body></html>".to_owned(),
                        ),
                        Invoke::Redirect => (
                            307,
                            "text/plain",
                            "location: https://elsewhere.invalid/v1alpha2/invoke\r\n".to_owned(),
                            String::new(),
                        ),
                        Invoke::Slow(delay) => {
                            std::thread::sleep(delay);
                            (
                                200,
                                "application/json",
                                String::new(),
                                success(json!({"messages": []}).to_string()),
                            )
                        }
                        Invoke::Nested { depth, zeros } => {
                            let mut result = "[".repeat(depth);
                            result.push_str(&vec!["0"; zeros].join(","));
                            result.push_str(&"]".repeat(depth));
                            (200, "application/json", String::new(), success(result))
                        }
                    }
                };
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} Answer\r\ncontent-type: {kind}\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str, config: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w59-adv-connector-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(scratch.0.join("state")).expect("the state directory");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    fs::write(scratch.0.join("probe.yaml"), PROTOCOL).expect("the protocol");
    fs::write(scratch.0.join("run.yaml"), config).expect("the run file");
    scratch
}

fn binding(url: &str) -> String {
    format!(
        "connectors:\n  endpoint: {url}\n  instance: chat/team\nconnector_actions:\n  thread.read:\n    operation: {OPERATION}\n    input: {{channel: C0000000, ts: \"1700000000.000100\"}}\n    evidence: thread_read\n"
    )
}

/// `takt run` on an intent in the scratch tree, no harness, no forge, `extra` arguments added:
/// exit code, stdout, stderr.
fn run(scratch: &Scratch, extra: &[&str]) -> (i32, String, String) {
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
        .args(extra)
        .arg("--protocol")
        .arg(scratch.0.join("probe.yaml"))
        .arg("--config")
        .arg(scratch.0.join("run.yaml"))
        .arg("--state-dir")
        .arg(scratch.0.join("state"))
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn cases(tree: &Path) -> Vec<PathBuf> {
    fs::read_dir(tree.join(".takt/cases"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect()
}

/// The `thread_read` records of every case.
fn records(tree: &Path) -> Vec<Value> {
    cases(tree)
        .iter()
        .flat_map(|case| fs::read_dir(case.join("evidence")).into_iter().flatten())
        .map(|entry| entry.expect("an entry").path())
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|record| record["kind"] == "thread_read")
        .collect()
}

/// An answer that is no refusal by the instance of the read is recorded as no `fail`: the claim
/// "the instance refused to read the thread" does not become true, and the case does not reach
/// the outcome `refused`.
fn no_refusal_recorded(name: &str, invoke: Invoke) {
    let (url, seen) = serve(invoke);
    let scratch = scratch(name, &binding(&url));
    let (code, out, err) = run(&scratch, &[]);
    assert!(
        seen.lock()
            .expect("the log")
            .iter()
            .any(|line| line.starts_with("POST /v1alpha2/invoke")),
        "the invoke was sent: {out}\n{err}"
    );
    let failed: Vec<Value> = records(&scratch.tree())
        .into_iter()
        .filter(|record| record["result"] == "fail")
        .collect();
    assert!(
        failed.is_empty() && !out.contains("stopped: Completed (refused)"),
        "{name}: an answer that is no refusal of the read was recorded as the instance's refusal (exit {code}): {failed:?}\nstdout {out}\nstderr {err}"
    );
}

/// The served host answers `stale_description` when the descriptor moved after takt's one
/// describe: "refresh the descriptor before resubmitting". That is no refusal of the read.
#[test]
fn w59_adv_stale_description_is_no_refusal_of_the_read() {
    no_refusal_recorded("stale", Invoke::Stale);
}

/// A gateway's 502 page in front of the instance is no answer of the instance at all.
#[test]
fn w59_adv_gateway_page_is_no_refusal_of_the_read() {
    no_refusal_recorded("gateway", Invoke::Gateway);
}

/// A redirect, which takt never follows, is no refusal of the read.
#[test]
fn w59_adv_redirect_is_no_refusal_of_the_read() {
    no_refusal_recorded("redirect", Invoke::Redirect);
}

/// A command action is held to what is left of the run's `hours` (wave 58; configuration.md:
/// "held to five minutes and to the run's `hours`: killed when they end"). The connector action's
/// call in flight is held to the client's fixed 120 s instead, so a run with 7.2 s of budget
/// waits on a slow instance until it answers, 45 s later.
#[test]
fn w59_adv_call_in_flight_is_held_to_the_hour_budget() {
    let (url, seen) = serve(Invoke::Slow(Duration::from_secs(45)));
    let scratch = scratch("deadline", &binding(&url));
    let started = Instant::now();
    let (code, out, err) = run(&scratch, &["--hours", "0.002"]);
    let took = started.elapsed();
    assert!(
        seen.lock()
            .expect("the log")
            .iter()
            .any(|line| line.starts_with("POST /v1alpha2/invoke")),
        "the invoke was sent before the budget ended: {out}\n{err}"
    );
    assert!(
        took < Duration::from_secs(25),
        "a run with 7.2 s of hours waited {took:?} on a call in flight (exit {code})\nstdout {out}\nstderr {err}"
    );
}

/// The same action bound by `command_actions` and by `connector_actions`: the run takes the
/// first validator registered for the action (`run.rs` finds by action id), so one of the two
/// bindings is silently not what performs it. Expected: refused at its line, exit 2.
#[test]
fn w59_adv_an_action_bound_twice_is_refused() {
    let (url, seen) = serve(Invoke::Slow(Duration::ZERO));
    let config = format!(
        "command_actions:\n  thread.read:\n    argv: [touch, \"{{artifacts}}/x.txt\"]\n    writes: x.txt\n    evidence: thread_read\n{}",
        binding(&url)
    );
    let scratch = scratch("twice", &config);
    let (code, out, err) = run(&scratch, &[]);
    let invoked = seen
        .lock()
        .expect("the log")
        .iter()
        .any(|line| line.starts_with("POST"));
    assert_eq!(
        code, 2,
        "an action bound by both members is not refused (invoked: {invoked})\nstdout {out}\nstderr {err}"
    );
    assert!(err.contains("thread.read") || err.contains("line"), "{err}");
}

/// The client reads at most 16 MiB of an answer (`ANSWER_LIMIT`); the kept artifact is that
/// answer pretty-printed, each value indented by its depth, so a 0.2 MB answer of nested arrays
/// is kept as tens of megabytes: the bound on what is read is no bound on what is written.
#[test]
fn w59_adv_kept_answer_is_bounded_like_the_answer_read() {
    let (url, _seen) = serve(Invoke::Nested {
        depth: 100,
        zeros: 100_000,
    });
    let scratch = scratch("nested", &binding(&url));
    let (code, out, err) = run(&scratch, &[]);
    let kept: Vec<u64> = cases(&scratch.tree())
        .iter()
        .filter_map(|case| fs::metadata(case.join("artifacts/connector/thread.read.json")).ok())
        .map(|meta| meta.len())
        .collect();
    assert_eq!(kept.len(), 1, "exit {code}\nstdout {out}\nstderr {err}");
    assert!(
        kept[0] <= 16 << 20,
        "a {} byte answer is kept as {} bytes, beyond the 16 MiB the client reads",
        200 + 2 * 100_000,
        kept[0]
    );
}
