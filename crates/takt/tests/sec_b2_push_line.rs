//! Security review of story:the-gitlab-forge-can-call-the-api-with-a-named-credential.
//!
//! The push of the `api` transport is the one line that carries the token, in the environment
//! variable its credential helper reads (`api::HELPER_VARIABLE`). The contract
//! (`ess/domains/takt.yaml`, `takt.work.GitlabTransport` and `takt.work.CredentialSource`):
//! the token is held in takt's memory only, and the token goes to the host named and no other.
//! These cases run the forge's own push line, with only its URL replaced by a local stand-in,
//! since the fakes here speak no TLS. Nothing reaches a real host.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use takt::forge::credential::CredentialSource;
use takt::forge::gitlab::{Glab, api};
use takt::forge::{Process, Runner};
use takt::scan::Scan;

const TOKEN: &str = "tokenvalue-for-the-push-line";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("sec-b2-push-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree")).expect("a scratch tree");
        Self(dir)
    }

    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A repository with one commit whose `origin` is a project on `gitlab.example.com`, and the
/// `api` forge over it under a mode-600 credential file outside the tree. Returns the forge and
/// the commit.
fn forge(scratch: &Scratch) -> (Glab, String) {
    let tree = scratch.tree();
    git(&tree, &["init", "-q", "-b", "main"]);
    fs::write(tree.join("a.txt"), "a\n").expect("a file");
    git(&tree, &["add", "a.txt"]);
    git(&tree, &["commit", "-q", "-m", "a"]);
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://gitlab.example.com/group/sub/repo.git",
        ],
    );
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let token = scratch.0.join("token");
    fs::write(&token, format!("{TOKEN}\n")).expect("a credential file");
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).expect("its mode");
    let glab = Glab::for_tree_api_through(
        &tree,
        &Scan::None {
            declared_in: scratch.0.join("run.yaml"),
        },
        &CredentialSource::File(token),
        Box::new(Process::default()),
    )
    .expect("the api forge opens");
    (glab, head)
}

/// The forge's push line with its URL replaced by `url`.
fn push_line(glab: &Glab, head: &str, url: &str) -> Vec<String> {
    let argv = glab.push_revision_argv("takt/case-1", head);
    let forge_url = glab.push_url();
    assert!(argv.contains(&forge_url), "{argv:?}");
    argv.into_iter()
        .map(|arg| {
            if arg == forge_url {
                url.to_owned()
            } else {
                arg
            }
        })
        .collect()
}

/// The token's line runs no code the repository's configuration names: a `pre-push` hook of the
/// tree (as a hook manager installs one, running a script the tree tracks) does not see the
/// token the line's credential helper reads.
#[test]
fn sec_b2_the_token_line_runs_no_repository_hook() {
    let scratch = Scratch::new("hook");
    let (glab, head) = forge(&scratch);
    let remote = scratch.0.join("remote.git");
    fs::create_dir_all(&remote).expect("a remote");
    git(&remote, &["init", "-q", "--bare"]);
    let saw = scratch.0.join("hook-saw");
    let hook = scratch.tree().join(".git/hooks/pre-push");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\nprintf %s \"${}\" > '{}'\n",
            api::HELPER_VARIABLE,
            saw.display()
        ),
    )
    .expect("a hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("its mode");

    let argv = push_line(&glab, &head, &remote.display().to_string());
    // The forge's own environment of this line, but for `GIT_ALLOW_PROTOCOL=https`, which the
    // local stand-in for the host cannot satisfy.
    let exit = Process::default()
        .run_env(&scratch.tree(), &argv, &[(api::HELPER_VARIABLE, TOKEN)])
        .expect("the push runs");
    assert!(exit.success(), "{exit:?}");
    let seen = fs::read_to_string(&saw).unwrap_or_default();
    assert!(
        !seen.contains(TOKEN),
        "a pre-push hook of the tree read the forge token from the push line's environment"
    );
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The request line and headers of one HTTP request.
fn request_of(stream: &TcpStream) -> (String, Vec<String>) {
    let mut reader = BufReader::new(stream.try_clone().expect("a stream"));
    let mut first = String::new();
    let _ = reader.read_line(&mut first);
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        headers.push(line.trim().to_owned());
    }
    (first.trim().to_owned(), headers)
}

/// Serves `listener` forever, each request answered by `answer`.
fn serve(listener: TcpListener, answer: impl Fn(&str, &[String]) -> String + Send + 'static) {
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let (line, headers) = request_of(&stream);
            let reply = answer(&line, &headers);
            let _ = stream.write_all(reply.as_bytes());
        }
    });
}

/// The forge host answers the push's first request with a redirect to another host. The token
/// goes to the host named and no other: the other host is never given it.
#[test]
fn sec_b2_the_push_gives_the_token_to_no_other_host() {
    let scratch = Scratch::new("redirect");
    let (glab, head) = forge(&scratch);

    let other = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let other_port = other.local_addr().expect("its address").port();
    let authorizations = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let authorizations = authorizations.clone();
        serve(other, move |_, headers| {
            let auth: Vec<String> = headers
                .iter()
                .filter(|h| h.to_ascii_lowercase().starts_with("authorization:"))
                .cloned()
                .collect();
            if auth.is_empty() {
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
            } else {
                authorizations.lock().unwrap().extend(auth);
                "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_owned()
            }
        });
    }
    let named = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let named_port = named.local_addr().expect("its address").port();
    serve(named, move |line, _| {
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
        format!(
            "HTTP/1.1 302 Found\r\nLocation: http://localhost:{other_port}{path}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
    });

    let argv = push_line(
        &glab,
        &head,
        &format!("http://127.0.0.1:{named_port}/group/sub/repo.git"),
    );
    // The forge's own environment of this line, but for `GIT_ALLOW_PROTOCOL=https`: the fakes
    // speak plain HTTP. The redirect logic of git is the same over HTTPS.
    let exit = Process::default()
        .run_env(&scratch.tree(), &argv, &[(api::HELPER_VARIABLE, TOKEN)])
        .expect("the push runs");
    assert!(
        !exit.success(),
        "the stand-in hosts accept nothing: {exit:?}"
    );
    let basic = base64(format!("oauth2:{TOKEN}").as_bytes());
    let given = authorizations.lock().unwrap().clone();
    assert!(
        !given.iter().any(|header| header.contains(&basic)),
        "the push's credential helper gave the forge token to the host a redirect named: {given:?}"
    );
}
