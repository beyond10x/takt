//! Test doubles for story:an-unqualified-harness-stops-before-the-first-session: a vendor program
//! that answers `--version` and, asked for a session, sends one request to a model endpoint the
//! test counts, and the endpoint. The program is Rust, compiled once per test binary from
//! [`FAKE_SOURCE`] with `rustc`, and copied under the name of the vendor program it stands for
//! (`claude` or `codex`). Its version, and the endpoint it calls, are read at each start from
//! files beside it, so a test changes what the next start reports without rebuilding.

#![allow(dead_code)]

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

/// What the double fixes when its session is asked for: the broken function of the test
/// repository, made right.
pub const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

const FAKE_SOURCE: &str = r#"
use std::io::{Read, Write};

const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

fn main() {
    let me = std::env::current_exe().expect("its own path");
    let name = me.file_name().expect("a name").to_string_lossy().into_owned();
    let beside = |extension: &str| std::fs::read_to_string(me.with_extension(extension)).ok();
    let version = beside("version").unwrap_or_default().trim().to_owned();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        if name == "claude" {
            println!("{version} (Claude Code)");
        } else {
            println!("codex-cli {version}");
        }
        return;
    }
    if let Some(address) = beside("endpoint")
        && let Ok(mut stream) = std::net::TcpStream::connect(address.trim())
    {
        let _ = stream.write_all(b"POST /v1/messages HTTP/1.1\r\nHost: fake\r\nContent-Length: 0\r\n\r\n");
        let mut answer = [0_u8; 64];
        let _ = stream.read(&mut answer);
    }
    if name != "claude" {
        std::process::exit(1);
    }
    let cwd = std::env::current_dir().expect("a working directory");
    std::fs::write(cwd.join("src/lib.rs"), FIXED).expect("the fix");
    let reply = "Done.\n{\"action\": \"implementation.edit\", \"summary\": \"fixed add\"}";
    println!(
        "{}",
        format!(
            "{{\"type\":\"system\",\"subtype\":\"init\",\"claude_code_version\":\"{version}\",\"cwd\":\"{}\",\"tools\":[\"Read\",\"Edit\",\"Write\",\"Glob\",\"Grep\"],\"mcp_servers\":[],\"plugins\":[],\"output_style\":\"default\",\"apiKeySource\":\"none\",\"model\":\"fake\",\"permissionMode\":\"default\",\"session_id\":\"s-1\"}}",
            cwd.display()
        )
    );
    println!(
        "{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":{}}}]}}}}",
        quoted(reply)
    );
    println!(
        "{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"duration_ms\":5,\"duration_api_ms\":4,\"result\":{},\"modelUsage\":{{\"fake\":{{}}}},\"usage\":{{\"input_tokens\":3,\"output_tokens\":4}}}}",
        quoted(reply)
    );
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
"#;

/// The compiled double, built once per test binary.
fn compiled() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("preflight-fake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory for the double");
        let source = dir.join("fake.rs");
        std::fs::write(&source, FAKE_SOURCE).expect("the double's source");
        let program = dir.join("fake");
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let status = Command::new(rustc)
            .args(["--edition", "2024", "-C", "debuginfo=0", "-o"])
            .arg(&program)
            .arg(&source)
            .status()
            .expect("rustc runs");
        assert!(status.success(), "the double compiles");
        program
    })
}

/// A vendor program double named `name` (`claude` or `codex`) in `dir`, reporting `version`.
/// Returns its path.
pub fn install(dir: &Path, name: &str, version: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("the program's directory");
    let program = dir.join(name);
    let _ = std::fs::remove_file(&program);
    std::fs::copy(compiled(), &program).expect("the double is copied");
    set_version(&program, version);
    program
}

/// What the double at `program` reports as its version from its next start on.
pub fn set_version(program: &Path, version: &str) {
    std::fs::write(program.with_extension("version"), format!("{version}\n")).expect("a version");
}

/// A model endpoint that counts the requests it is sent. The double at `program` calls it when
/// it is asked for a session, never when it is asked for its version.
pub struct Endpoint {
    requests: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
    address: SocketAddr,
}

impl Endpoint {
    /// Starts the endpoint and points the double at `program` at it.
    pub fn serving(program: &Path) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        listener.set_nonblocking(true).expect("non-blocking");
        let address = listener.local_addr().expect("an address");
        let requests = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let (counted, finished) = (Arc::clone(&requests), Arc::clone(&done));
        std::thread::spawn(move || {
            while !finished.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ =
                            stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                        let mut head = [0_u8; 1024];
                        if stream.read(&mut head).is_ok_and(|read| read > 0) {
                            counted.fetch_add(1, Ordering::SeqCst);
                        }
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        );
                    }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
                }
            }
        });
        std::fs::write(program.with_extension("endpoint"), address.to_string())
            .expect("the endpoint's address");
        Self {
            requests,
            done,
            address,
        }
    }

    /// The requests the endpoint has been sent.
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    /// Where it listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
    }
}
