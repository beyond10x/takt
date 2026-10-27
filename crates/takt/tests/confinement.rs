//! story:confinement-seam: every child process a run starts goes through one seam,
//! `crates/takt/src/confinement.rs`, and since
//! story:confinement-through-substrate-on-linux-and-macos the test command runs through a
//! Substrate adapter: chosen by facts under `--confinement substrate`, Substrate's `local`
//! adapter declared insecure under `none`.
//!
//! - No source under any crate's `src/` but the seam starts a process, or imports or reaches a
//!   way to (`std::process::Command`, `CommandExt`, `libc` and `nix` spawns, process crates);
//!   the CLI's one test module excepted, the rest of it read.
//! - An adapter that refuses every program: the run starts no process, writes no evidence
//!   record and stops with the refusal named; the harness's program is refused at its own spawn.
//! - Under Substrate the test command of a scratch crate writes the record it writes under
//!   `none`; a test that writes outside the build directory (M19) reaches no host path outside
//!   the tree, the tree being the adapter's workspace, and under a Linux adapter no path of the
//!   tree but its `target`; one that writes over the object store (M20) is refused the write,
//!   by `sealed_paths` on `macos` and by the scoped workspace on Linux; and a Python test
//!   command is refused by its toolchain's name with no child started unconfined.
//! - The first line a run prints and its `opened` event name the adapter, and under `none` the
//!   reason it is declared insecure.
//!
//! The Substrate cases run against the host's real adapters, each in a process of its own: the
//! test binary starts itself again and runs the case there. Where no adapter of the host serves
//! a test command, the case prints `skipped: missing prerequisite: …` and returns.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use takt::confinement::{self, Confinement, Substrate, TestRun, Unconfined};
use takt::forge::Finished;
use takt::harness::{ClaudeCode, Harness, Kind, SessionHarness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// One validator-owned action, `tests.run`; accepted once the tests pass at the current
/// revision.
const TEST_ONLY: &str = "format: protocol/1

protocol:
  id: test.only
  revision: 1
  description: >-
    Run the tests against the implementation; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// A judgement action, `implementation.edit`, before `tests.run`.
const EDIT_AND_TEST: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit the implementation and run the tests against it; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// The adapter is the process's: a case that installs one, or relies on the default, holds
/// this while it runs, and the default is put back when it lets go ([`Held`]).
static ADAPTER: Mutex<()> = Mutex::new(());

/// The process's adapter held for one case; dropping it installs [`Unconfined`] again.
struct Held(#[allow(dead_code)] MutexGuard<'static, ()>);

fn held() -> Held {
    let guard = ADAPTER.lock().unwrap_or_else(PoisonError::into_inner);
    confinement::install(Arc::new(Unconfined));
    Held(guard)
}

impl Held {
    fn install(&self, adapter: Arc<dyn Confinement>) {
        confinement::install(adapter);
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        confinement::install(Arc::new(Unconfined));
    }
}

/// Refuses every program, counting what it was asked.
#[derive(Default)]
struct RefusesAll {
    asked: AtomicUsize,
}

impl Confinement for RefusesAll {
    fn name(&self) -> &str {
        "refuses-all"
    }

    fn admit(&self, program: &OsStr) -> std::io::Result<()> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Err(confinement::refused(
            self.name(),
            program,
            "this test adapter refuses every program",
        ))
    }
}

/// Refuses the one program at `refused`, by path, and admits every other.
struct RefusesOne {
    refused: PathBuf,
    asked: AtomicUsize,
}

impl Confinement for RefusesOne {
    fn name(&self) -> &str {
        "refuses-one"
    }

    fn admit(&self, program: &OsStr) -> std::io::Result<()> {
        if Path::new(program) == self.refused {
            self.asked.fetch_add(1, Ordering::SeqCst);
            return Err(confinement::refused(
                self.name(),
                program,
                "this test adapter refuses the harness's program",
            ));
        }
        Ok(())
    }
}

/// Substrate, recording every program it admitted.
struct Watching {
    inner: Arc<dyn Confinement>,
    admitted: Mutex<Vec<String>>,
}

impl Watching {
    fn new(inner: Arc<dyn Confinement>) -> Self {
        Self {
            inner,
            admitted: Mutex::new(Vec::new()),
        }
    }

    fn admitted(&self) -> Vec<String> {
        self.admitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn noted(&self, program: &OsStr, admitted: std::io::Result<()>) -> std::io::Result<()> {
        if admitted.is_ok() {
            self.admitted
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(program.to_string_lossy().into_owned());
        }
        admitted
    }
}

impl Confinement for Watching {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn admit(&self, program: &OsStr) -> std::io::Result<()> {
        self.noted(program, self.inner.admit(program))
    }

    fn admit_delegated(&self, program: &OsStr) -> std::io::Result<()> {
        self.noted(program, self.inner.admit_delegated(program))
    }

    fn run_test(&self, test: &TestRun<'_>) -> anyhow::Result<Finished> {
        self.inner.run_test(test)
    }
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory: the protocol file at its root and
/// the repository in `repo/`. Dropping it removes both.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.repo
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Scratch {
    fn protocol(&self) -> String {
        self.root.join("protocol.yaml").display().to_string()
    }
}

/// A directory of this case's own under the build directory, empty.
fn directory(name: &str) -> PathBuf {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("conf-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a directory");
    dir
}

/// The probe crate's lock file as a cargo that sees no configuration above the crate writes
/// it, the one a confined cargo keeps.
const PROBE_LOCK: &str = "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"probe\"\nversion = \"0.1.0\"\n";

/// A repository on `main` holding `files` (path, text, executable) in its first commit, beside
/// a `.gitignore` that ignores `target/`, with `protocol` beside it; a crate given no lock file
/// gets one generated offline by the cargo running these tests, committed with it.
fn repository(name: &str, protocol: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let root = directory(name);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), protocol).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n").expect("a file");
    for (path, text, executable) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, text).expect("a file");
        if *executable {
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("a mode");
        }
    }
    if repo.join("Cargo.toml").is_file() && !repo.join("Cargo.lock").exists() {
        let cargo = std::env::var("CARGO").expect("cargo runs the tests");
        let locked = Command::new(cargo)
            .args(["generate-lockfile", "--offline", "--quiet"])
            .current_dir(&repo)
            .status()
            .expect("cargo runs");
        assert!(locked.success(), "the lock file is generated");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

/// A crate whose one test is `test`, a function body. The test reports into the crate's own
/// `target/`, which it makes first: where the build went elsewhere (`CARGO_TARGET_DIR` in the
/// environment the run inherits) there is none, and under Substrate the build made it. Its lock
/// file is the one the cargo running these tests writes, as a `none` run's cargo, on the host,
/// sees the same configuration.
fn scratch_crate(name: &str, protocol: &str, test: &str) -> Scratch {
    probe_crate(name, protocol, test, None)
}

/// [`scratch_crate`] for a confined run: its lock file is [`PROBE_LOCK`], since the confined
/// cargo sees no configuration above its workspace (a `[patch]` there would be dropped from it).
fn confined_crate(name: &str, protocol: &str, test: &str) -> Scratch {
    probe_crate(name, protocol, test, Some(PROBE_LOCK))
}

/// The probe crate of [`scratch_crate`] and [`confined_crate`], with `lock` as its lock file
/// when one is given.
fn probe_crate(name: &str, protocol: &str, test: &str, lock: Option<&str>) -> Scratch {
    let lib = format!(
        "#[test]\nfn the_test() {{\n    std::fs::create_dir_all(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/target\")).unwrap();\n{test}\n}}\n"
    );
    let mut files = vec![
        (
            "Cargo.toml",
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
            false,
        ),
        ("src/lib.rs", lib.as_str(), false),
    ];
    if let Some(lock) = lock {
        files.push(("Cargo.lock", lock, false));
    }
    repository(name, protocol, &files)
}

fn options(protocol: String, iterations: u64) -> RunOptions {
    RunOptions {
        protocol,
        intent: "make the check pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// A harness that is never stepped: reaching it is the failure.
struct Never;

impl Harness for Never {
    fn name(&self) -> &str {
        "never"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        anyhow::bail!("the harness was stepped")
    }
}

/// What one run left: its stop, what it printed, the case's evidence records, its record.
struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
    events: String,
}

impl Ran {
    /// The stop or the error, then what the run printed.
    fn said(&self) -> String {
        match &self.stop {
            Ok(stop) => format!("{stop:?}\n{}", self.text),
            Err(error) => format!("{error:#}\n{}", self.text),
        }
    }
}

/// Every evidence record under `tree`'s cases, whichever case wrote it.
fn evidence(tree: &Path) -> Vec<serde_json::Value> {
    let Ok(cases) = fs::read_dir(tree.join(".takt/cases")) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for case in cases {
        let Ok(entries) = fs::read_dir(case.expect("an entry").path().join("evidence")) else {
            continue;
        };
        paths.extend(
            entries
                .map(|e| e.expect("an entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .filter(|p| !p.to_string_lossy().ends_with(".bundle.json")),
        );
    }
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// Opens a case on `tree` with `harness` and the test command `command`, and runs it.
fn run(tree: &Scratch, command: &str, harness: &dyn Harness) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let mut run =
        Run::open(tree, options(tree.protocol(), 4), harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: evidence(tree),
        events: fs::read_to_string(run.case.dir.join("events.jsonl")).unwrap_or_default(),
    }
}

// --- Acceptance 1: the seam is the one place that starts a process. ---
//
// The scan reads every crate's `src/` but the seam's file. Comments and the contents of string
// and character literals are blanked first, so what it reads is code. It flags:
//
// - a `use` whose tree, in any form (a group, `self`, a glob, an alias, a leading `::`), names
//   a path of `WATCHED` or of `SPAWNS` under `SPAWNING_MODULES`, a module on the way to one
//   (`std`, `std::process`, `libc`, `nix::unistd`), or a path under one: so what name the
//   import binds never matters;
// - such a path written out in code, by its last two segments (`process::Command`,
//   `libc::fork`) or by its crate (`duct::`);
// - `extern crate` of a crate one of them is in, a foreign block (`extern "C" { … }`), inline
//   assembly, `include!` and a `#[path = …]` attribute, each a way of starting a process the
//   names above do not show.
//
// In an excepted file it skips exactly its one `#[cfg(test)] mod tests { … }`, brace-balanced,
// and reads the rest.

/// The paths that start a process or a child, or modules of nothing else.
const WATCHED: &[&str] = &[
    "std::process::Command",
    "std::os::unix::process::CommandExt",
    "std::os::linux::process::CommandExt",
    "std::os::unix::prelude::CommandExt",
    "tokio::process",
    "async_process",
    "async_std::process",
    "smol::process",
    "duct",
    "subprocess",
    "xshell",
    "cmd_lib",
    "nix::spawn",
    "nix::sched::clone",
    "rustix::runtime",
];

/// The C library's calls that start, replace or reach a process, in each of `SPAWNING_MODULES`.
const SPAWNS: &[&str] = &[
    "fork",
    "vfork",
    "clone",
    "clone3",
    "execv",
    "execve",
    "execvp",
    "execvpe",
    "execl",
    "execle",
    "execlp",
    "fexecve",
    "execveat",
    "posix_spawn",
    "posix_spawnp",
    "system",
    "popen",
    "daemon",
    "syscall",
    "dlsym",
];

/// The modules `SPAWNS` are called from.
const SPAWNING_MODULES: &[&str] = &["libc", "nix::unistd", "nix::libc"];

/// Every watched path, by its segments.
fn watched() -> Vec<Vec<String>> {
    let segments = |path: &str| path.split("::").map(str::to_owned).collect::<Vec<_>>();
    let mut paths: Vec<Vec<String>> = WATCHED.iter().map(|path| segments(path)).collect();
    for module in SPAWNING_MODULES {
        for call in SPAWNS {
            paths.push(segments(&format!("{module}::{call}")));
        }
    }
    paths
}

/// Whether `c` continues an identifier.
fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The character before byte `at` of `text`.
fn before(text: &str, at: usize) -> Option<char> {
    text[..at].chars().next_back()
}

/// `c` blanked: a line break kept, anything else a space.
fn blank(c: char) -> char {
    if c == '\n' { '\n' } else { ' ' }
}

/// Copies the string literal whose opening quote is at `quote` into `out`, its contents
/// blanked, and returns the index after it. A raw one, `raw` its count of `#`, ends at the
/// first quote followed by as many `#`; any other at the first quote no `\` escapes.
fn literal(chars: &[char], quote: usize, raw: Option<usize>, out: &mut String) -> usize {
    out.push('"');
    let mut i = quote + 1;
    while let Some(&c) = chars.get(i) {
        match raw {
            None if c == '\\' => {
                out.push(' ');
                if let Some(&escaped) = chars.get(i + 1) {
                    out.push(blank(escaped));
                }
                i += 2;
                continue;
            }
            None if c == '"' => {
                out.push('"');
                return i + 1;
            }
            Some(hashes) if c == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) => {
                out.push('"');
                out.extend(std::iter::repeat_n('#', hashes));
                return i + 1 + hashes;
            }
            _ => {}
        }
        out.push(blank(c));
        i += 1;
    }
    i
}

/// `text` with every comment and the contents of every string and character literal blanked,
/// its line breaks kept: what is left is code, on the lines it was on.
fn code(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(c) = at(i) {
        let previous = i.checked_sub(1).and_then(at);
        // A raw string's `r` starts a token, or follows a `b` or `c` that does.
        let starts_raw = c == 'r'
            && match previous {
                Some('b' | 'c') => !i.checked_sub(2).and_then(at).is_some_and(ident),
                Some(p) => !ident(p),
                None => true,
            };
        let hashes = (i + 1..chars.len())
            .take_while(|&k| chars[k] == '#')
            .count();
        if c == '/' && at(i + 1) == Some('/') {
            while let Some(c) = at(i) {
                if c == '\n' {
                    break;
                }
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && at(i + 1) == Some('*') {
            let mut depth = 0usize;
            while let Some(c) = at(i) {
                if c == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if c == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(c));
                    i += 1;
                }
            }
        } else if c == '"' {
            i = literal(&chars, i, None, &mut out);
        } else if starts_raw && at(i + 1 + hashes) == Some('"') {
            out.push('r');
            out.extend(std::iter::repeat_n('#', hashes));
            i = literal(&chars, i + 1 + hashes, Some(hashes), &mut out);
        } else if c == '\'' && at(i + 1) == Some('\\') {
            let mut end = i + 3;
            while at(end).is_some_and(|c| c != '\'') {
                end += 1;
            }
            out.push('\'');
            out.extend((i + 1..end).map(|k| blank(chars[k])));
            out.push('\'');
            i = end + 1;
        } else if c == '\'' && at(i + 2) == Some('\'') {
            out.push_str("' '");
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// `text` with its white space dropped where a token does not need it, and the line each byte
/// came from.
fn squeezed(text: &str) -> (String, Vec<usize>) {
    let punct = |c: char| ":{},;()<>[]*&=".contains(c);
    let mut flat = String::new();
    let mut lines = Vec::new();
    let mut line = 1;
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if c == '\n' {
                line += 1;
            }
            space = true;
            continue;
        }
        if space && !flat.is_empty() && !punct(c) && !flat.ends_with(punct) {
            flat.push(' ');
            lines.push(line);
        }
        space = false;
        flat.push(c);
        lines.extend(std::iter::repeat_n(line, c.len_utf8()));
    }
    (flat, lines)
}

/// The tokens of a squeezed `use` tree: words, `::` and single marks.
fn tokens(tree: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in tree.chars() {
        if ident(c) || c == '$' || c == '#' {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        match c {
            ' ' => {}
            ':' if out.last().is_some_and(|t| t == ":") => {
                out.pop();
                out.push("::".to_owned());
            }
            c => out.push(c.to_string()),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Every path the `use` tree at `tokens[*at]` imports, each from `prefix`: `self` in a group
/// is the group's own path, a glob ends in `*`, an alias is dropped.
fn imported(tokens: &[String], at: &mut usize, prefix: Vec<String>, into: &mut Vec<Vec<String>>) {
    let mut path = prefix;
    loop {
        let Some(token) = tokens.get(*at) else {
            into.push(path);
            return;
        };
        match token.as_str() {
            "::" | "self" => *at += 1,
            "{" => {
                *at += 1;
                while let Some(token) = tokens.get(*at) {
                    match token.as_str() {
                        "}" => {
                            *at += 1;
                            break;
                        }
                        "," => *at += 1,
                        _ => imported(tokens, at, path.clone(), into),
                    }
                }
                return;
            }
            "*" => {
                *at += 1;
                path.push("*".to_owned());
                into.push(path);
                return;
            }
            "as" => {
                *at += 2;
                into.push(path);
                return;
            }
            "," | "}" => {
                into.push(path);
                return;
            }
            segment => {
                path.push(segment.to_owned());
                *at += 1;
            }
        }
    }
}

/// Whether importing `path` imports a way to start a process: it is a watched path, a module
/// on the way to one, or a path under one. A glob imports its module.
fn imports_a_spawn(path: &[String], watched: &[Vec<String>]) -> bool {
    let path: Vec<String> = path.iter().filter(|s| *s != "*").cloned().collect();
    !path.is_empty()
        && watched
            .iter()
            .any(|w| w.starts_with(&path) || path.starts_with(w))
}

/// The byte range of the bracketed span whose opening mark is at `open` in `flat`.
fn span(flat: &str, open: usize, marks: (char, char)) -> Option<std::ops::Range<usize>> {
    let mut depth = 0usize;
    for (i, c) in flat[open..].char_indices() {
        if c == marks.0 {
            depth += 1;
        } else if c == marks.1 {
            depth -= 1;
            if depth == 0 {
                return Some(open..open + i + c.len_utf8());
            }
        }
    }
    None
}

/// The lines of the squeezed code `flat` that start a process, or import or reach a way to.
fn flagged(flat: &str, lines: &[usize]) -> Vec<usize> {
    let watched = watched();
    let mut found = Vec::new();
    // `use` items, whatever names they bind.
    for (at, _) in flat.match_indices("use") {
        if before(flat, at).is_some_and(|c| ident(c) || c == '$' || c == '#') {
            continue;
        }
        let rest = &flat[at + "use".len()..];
        if !rest.starts_with([' ', ':', '{']) {
            continue;
        }
        let tokens = tokens(&rest[..rest.find(';').unwrap_or(rest.len())]);
        let mut paths = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let start = i;
            imported(&tokens, &mut i, Vec::new(), &mut paths);
            if i == start {
                i += 1;
            }
        }
        if paths.iter().any(|path| imports_a_spawn(path, &watched)) {
            found.push(lines[at]);
        }
    }
    // Paths in code: a watched path by its last two segments, a crate by its name and `::`.
    let mut needles: Vec<(String, bool)> = WATCHED
        .iter()
        .map(|path| match path.rsplit_once("::") {
            Some((module, item)) => (
                format!("{}::{item}", module.rsplit("::").next().unwrap_or(module)),
                true,
            ),
            None => (format!("{path}::"), false),
        })
        .collect();
    for module in SPAWNING_MODULES {
        let last = module.rsplit("::").next().unwrap_or(module);
        needles.extend(SPAWNS.iter().map(|call| (format!("{last}::{call}"), true)));
    }
    for (needle, ends) in &needles {
        for (at, _) in flat.match_indices(needle.as_str()) {
            let after = flat[at + needle.len()..].chars().next();
            if !before(flat, at).is_some_and(ident) && !(*ends && after.is_some_and(ident)) {
                found.push(lines[at]);
            }
        }
    }
    // `extern crate` of a watched crate, and a foreign block.
    let crates: Vec<&str> = std::iter::once("std")
        .chain(
            WATCHED
                .iter()
                .chain(SPAWNING_MODULES)
                .map(|path| path.split("::").next().unwrap_or(path)),
        )
        .collect();
    for (at, _) in flat.match_indices("extern") {
        let rest = &flat[at + "extern".len()..];
        if before(flat, at).is_some_and(ident) || rest.starts_with(ident) {
            continue;
        }
        let rest = rest.trim_start();
        if let Some(name) = rest.strip_prefix("crate ") {
            let name: String = name.chars().take_while(|c| ident(*c)).collect();
            if crates.contains(&name.as_str()) {
                found.push(lines[at]);
            }
            continue;
        }
        let rest = match rest.strip_prefix('"') {
            Some(abi) => abi.find('"').map_or("", |end| &abi[end + 1..]),
            None => rest,
        };
        if rest.trim_start().starts_with('{') {
            found.push(lines[at]);
        }
    }
    // Inline assembly and an included source.
    for (at, _) in flat.match_indices('!') {
        let start = flat[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| ident(*c))
            .last()
            .map_or(at, |(i, _)| i);
        if ["asm", "global_asm", "naked_asm", "include"].contains(&&flat[start..at]) {
            found.push(lines[start]);
        }
    }
    // A module read from a path of its own.
    for (at, _) in flat.match_indices('#') {
        let rest = &flat[at + 1..];
        let open = at + 1 + usize::from(rest.starts_with('!'));
        if !flat[open..].starts_with('[') {
            continue;
        }
        let Some(range) = span(flat, open, ('[', ']')) else {
            continue;
        };
        let attribute = &flat[range.clone()];
        if attribute
            .match_indices("path=")
            .any(|(i, _)| !before(attribute, i).is_some_and(ident))
        {
            found.push(lines[at]);
        }
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// Takes out of `flat` and `lines` its one `#[cfg(test)] mod tests { … }`, brace-balanced; an
/// error when there is not exactly one `mod tests`, or it is not that.
fn without_the_test_module(flat: &mut String, lines: &mut Vec<usize>) -> Result<(), String> {
    let modules: Vec<usize> = flat
        .match_indices("mod tests{")
        .map(|(at, _)| at)
        .filter(|&at| !before(flat, at).is_some_and(ident))
        .collect();
    let [module] = modules[..] else {
        return Err(format!(
            "{} `mod tests` where the one excepted test module is",
            modules.len()
        ));
    };
    let attribute = "#[cfg(test)]";
    if !flat[..module].ends_with(attribute) {
        return Err("its `mod tests` is not `#[cfg(test)] mod tests {`".to_owned());
    }
    let start = module - attribute.len();
    let range = span(flat, module + "mod tests".len(), ('{', '}'))
        .ok_or_else(|| "its test module does not close".to_owned())?;
    let line = lines[start];
    flat.replace_range(start..range.end, " ");
    lines.splice(start..range.end, [line]);
    Ok(())
}

/// The lines of `text`, a source, that start a process, or import or reach a way to.
fn starts_a_process(text: &str) -> Vec<usize> {
    let (flat, lines) = squeezed(&code(text));
    flagged(&flat, &lines)
}

/// The lines the scan flags in `text`, a file's source; in an excepted file, everything but
/// its one test module.
fn scanned(text: &str, excepted: bool) -> Result<Vec<usize>, String> {
    let (mut flat, mut lines) = squeezed(&code(text));
    if excepted {
        without_the_test_module(&mut flat, &mut lines)?;
    }
    Ok(flagged(&flat, &lines))
}

/// Whether `file` is one whose test module the scan does not read. The CLI's fixtures start
/// `git` and `aep` to set up and read the repositories a test runs on; the credential tests
/// start `mkfifo` to make a pipe, and the GitLab API tests start `sh` to ask the credential
/// helper. None is a run's process. Everything else in the file is read.
fn excepted(file: &Path) -> bool {
    file.ends_with("takt-cli/src/main.rs")
        || file.ends_with("takt/src/forge/credential.rs")
        || file.ends_with("takt/src/forge/gitlab/api.rs")
}

/// Every source file the scan reads: each crate's `src/` under `crates`, the workspace's
/// crates directory.
fn scanned_sources(crates: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(crates).expect("the crates directory") {
        let src = entry.expect("an entry").path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    files.sort();
    files
}

/// Every `.rs` file under `dir`, at any depth.
fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a source directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|x| x == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn the_scan_finds_every_way_of_naming_the_process_command() {
    for line in [
        "use std::process::Command;",
        "use std::process::{Child, Command, Stdio};",
        "use std::process::{\n    Child,\n    Command,\n};",
        "let c = std::process::Command::new(\"x\");",
        "let c = std :: process :: Command :: new(\"x\");",
        "use std::process::*;",
        "use std::process as p;",
        "use std::process::{Command as C};",
        "use std::{process::{self, Command}};",
        "fn f(c: &mut process::Command) {}",
    ] {
        assert!(
            !starts_a_process(line).is_empty(),
            "the scan misses {line:?}"
        );
    }
    for line in [
        "use std::process::{Child, Stdio, ExitStatus};",
        "use std::process::ExitCode;",
        "let c = crate::confinement::Command::new(\"x\");",
        "myprocess::Command::new(\"x\")",
        "use metaharness::protocol::{Command, Decision};",
        "std::process::id()",
        "let c = std::process::Stdio::null();",
        "use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};",
        "let f = libc::O_NOFOLLOW | libc::O_CLOEXEC;",
        "unsafe { libc::kill(pid, libc::SIGKILL) };",
        "std::thread::spawn(|| ());",
        "fn execute(system: &str) {}",
        "// std::process::Command and libc::fork() in a comment",
        "/* use std::{process as p}; */",
        "let s = \"std::process::Command::new and libc::fork()\";",
        "let s = r#\"use std::process as p;\"#;",
        "let c = '\"'; let d = '{'; fn f<'a>(x: &'a str) {}",
        "pub extern \"C\" fn exported() {}",
        "let path = 1; format!(\"{path}\", path = path);",
    ] {
        assert!(starts_a_process(line).is_empty(), "the scan flags {line:?}");
    }
    assert_eq!(
        starts_a_process("// a\n\nuse std::process::{\n    Stdio,\n    Command,\n};\n"),
        vec![3],
        "the line a name is on is the line its path starts on"
    );
}

/// The planted sources in `cases` the scan does not flag, each with what it misses.
fn missed<'a>(cases: &[&'a str]) -> Vec<&'a str> {
    cases
        .iter()
        .copied()
        .filter(|case| starts_a_process(case).is_empty())
        .collect()
}

#[test]
fn the_scan_finds_a_grouped_or_aliased_import_of_the_process_module() {
    let missed = missed(&[
        "use std::{process as p};\nfn f() { p::Command::new(\"true\"); }",
        "use std::{process};",
        "use std::{fs, process as p, io};",
        "use std::process::{self, Stdio};",
        "use std::process::{self as p};",
        "use std::{process::{self as q}};",
        "use std::process;",
        "use ::std::process as p;",
        "use std::process::{Command as C};",
        "use std::process::{Stdio, Command as C, Child};",
        "use std as s;\nfn f() { s::process::Command::new(\"true\"); }",
        "use std::{self as s};",
        "use std::*;",
        "extern crate std as s;",
    ]);
    assert!(
        missed.is_empty(),
        "the scan misses {} of these imports:\n{}",
        missed.len(),
        missed.join("\n")
    );
}

#[test]
fn the_scan_finds_every_other_way_of_starting_a_process() {
    let missed = missed(&[
        "unsafe { libc::fork() };",
        "unsafe { libc::vfork() };",
        "unsafe { libc::execv(p, a) };",
        "unsafe { libc::execvp(p, a) };",
        "unsafe { libc::execve(p, a, e) };",
        "unsafe { libc::posix_spawn(&mut pid, p, f, a, v, e) };",
        "unsafe { libc::posix_spawnp(&mut pid, p, f, a, v, e) };",
        "unsafe { libc::system(c) };",
        "unsafe { libc::popen(c, m) };",
        "unsafe { libc::syscall(libc::SYS_clone) };",
        "use libc::{fork, c_int};",
        "use libc::*;",
        "use libc as l;",
        "use nix::unistd::{fork, ForkResult};",
        "nix::unistd::execv(p, a);",
        "use nix::spawn::posix_spawn;",
        "use nix::unistd::*;",
        "tokio::process::Command::new(\"x\");",
        "use tokio::process::Command;",
        "async_process::Command::new(\"x\");",
        "use async_process::Command;",
        "duct::cmd!(\"x\").run();",
        "subprocess::Exec::cmd(\"x\").join();",
        "use std::os::unix::process::CommandExt as _;",
        "use std::os::unix::process::CommandExt;",
        "std::os::unix::process::CommandExt::exec(&mut c);",
        "use std::os::unix::prelude::*;",
        "use std::os::unix::{prelude::CommandExt as _};",
        "unsafe extern \"C\" { fn fork() -> i32; }",
        "extern { fn system(c: *const u8) -> i32; }",
        "std::arch::asm!(\"syscall\");",
        "include!(\"elsewhere.txt\");",
        "#[path = \"../../elsewhere.rs\"]\nmod elsewhere;",
    ]);
    assert!(
        missed.is_empty(),
        "the scan misses {} of these ways of starting a process:\n{}",
        missed.len(),
        missed.join("\n")
    );
}

/// An excepted file whose test module is followed by library code that starts a process.
const AFTER_THE_TEST_MODULE: &str = "pub fn library() {}

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn fixture() {
        let _ = Command::new(\"git\").arg(\"}\").status();
    }
}

pub fn after_the_test_module() -> bool {
    std::process::Command::new(\"true\").status().is_ok()
}
";

/// An excepted file with a second test module, which starts a process.
const A_SECOND_TEST_MODULE: &str = "pub fn library() {}

#[cfg(test)]
mod tests {
    #[test]
    fn fixture() {}
}

#[cfg(test)]
mod more_tests {
    #[test]
    fn spawns() {
        let _ = std::process::Command::new(\"true\").status();
    }
}
";

/// An excepted file whose `mod tests` is not where its library ends: a nested one.
const TWO_TEST_MODULES: &str = "pub mod inner {
    #[cfg(test)]
    mod tests {
        fn spawns() { let _ = std::process::Command::new(\"true\"); }
    }
}

#[cfg(test)]
mod tests {
    fn fixture() {}
}
";

#[test]
fn the_scan_reads_an_excepted_file_past_its_test_module() {
    let mut wrong = Vec::new();
    for (case, source, expected) in [
        (
            "the library after the test module is read, the test module is not",
            AFTER_THE_TEST_MODULE,
            Ok(vec![14]),
        ),
        (
            "a second test module is read",
            A_SECOND_TEST_MODULE,
            Ok(vec![13]),
        ),
        (
            "an excepted file has exactly one `mod tests`",
            TWO_TEST_MODULES,
            Err(()),
        ),
    ] {
        let got = scanned(source, true);
        if got.clone().map_err(|_| ()) != expected {
            wrong.push(format!("{case}: expected {expected:?}, got {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_scan_reads_every_crate_of_the_workspace() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace's crates");
    let files = scanned_sources(crates);
    let mut unread = Vec::new();
    for entry in fs::read_dir(crates).expect("the crates directory") {
        let src = entry.expect("an entry").path().join("src");
        if src.is_dir() && !files.iter().any(|f| f.starts_with(&src)) {
            unread.push(src.display().to_string());
        }
    }
    assert!(unread.is_empty(), "the scan reads none of {unread:?}");
    let main = crates.join("takt-cli/src/main.rs");
    assert!(files.contains(&main), "the scan reads {}", main.display());
    assert!(
        excepted(&main),
        "the CLI's test module, whose fixtures start `git` and `aep`, is excepted"
    );
}

#[test]
fn no_module_but_the_seam_names_the_process_command() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace's crates");
    let seam = crates.join("takt/src/confinement.rs");
    let seam_text = fs::read_to_string(&seam).expect("the seam's source");
    assert!(
        !starts_a_process(&seam_text).is_empty(),
        "the scan sees the seam's own use of std::process::Command"
    );
    let files = scanned_sources(crates);
    assert!(
        files.iter().any(|f| f.ends_with("takt/src/run.rs"))
            && files.iter().any(|f| f.ends_with("takt/src/harness.rs")),
        "the scan reads takt's sources: {files:?}"
    );
    let mut named = Vec::new();
    for file in files.iter().filter(|f| **f != seam) {
        let text = fs::read_to_string(file).expect("a source");
        match scanned(&text, excepted(file)) {
            Ok(lines) => named.extend(
                lines
                    .into_iter()
                    .map(|line| format!("{}:{line}", file.display())),
            ),
            Err(why) => named.push(format!("{}: {why}", file.display())),
        }
    }
    assert!(
        named.is_empty(),
        "a process is started outside the seam:\n{}",
        named.join("\n")
    );
}

// --- Acceptance 2: an adapter that refuses every program. ---

/// The check the repositories here run: it writes `ran` beside the repository, so a run that
/// started it shows.
fn check(root: &Path) -> String {
    format!(
        "#!/bin/sh\ntouch '{}'\nexit 0\n",
        root.join("ran").display()
    )
}

#[test]
fn an_adapter_refusing_every_program_refuses_the_case_before_it_opens() {
    let held = held();
    let root = directory("refuses-open");
    let tree = {
        let tree = repository("refuses-open-tree", TEST_ONLY, &[]);
        let check = check(&root);
        fs::write(tree.join("check.sh"), check).expect("a check");
        fs::set_permissions(tree.join("check.sh"), fs::Permissions::from_mode(0o755))
            .expect("a mode");
        git(&tree, &["add", "check.sh"]);
        git(&tree, &["commit", "-q", "-m", "the check"]);
        tree
    };
    let refusing = Arc::new(RefusesAll::default());
    held.install(refusing.clone());
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let opened = Run::open(&tree, options(tree.protocol(), 4), &Never, &validators);
    drop(held);
    let error = match opened {
        Ok(_) => panic!("the case opened with every program refused"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("confinement refuses-all refused"),
        "the refusal is named: {error}"
    );
    assert!(
        refusing.asked.load(Ordering::SeqCst) > 0,
        "the adapter was asked"
    );
    assert!(evidence(&tree).is_empty(), "{:?}", evidence(&tree));
    assert!(!root.join("ran").exists(), "the check ran");
}

#[test]
fn an_adapter_refusing_every_program_stops_the_run_with_the_refusal_named() {
    let held = held();
    let root = directory("refuses-run");
    let tree = repository(
        "refuses-run-tree",
        TEST_ONLY,
        &[("check.sh", &check(&root), true)],
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let mut run = Run::open(&tree, options(tree.protocol(), 4), &Never, &validators)
        .expect("the case opens under none");
    let refusing = Arc::new(RefusesAll::default());
    held.install(refusing.clone());
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    drop(held);
    let text = String::from_utf8_lossy(&out);
    let said = match &stop {
        Ok(stop) => panic!("the run stopped {stop:?} with every program refused\n{text}"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        said.contains("confinement refuses-all refused"),
        "the refusal is named: {said}\n{text}"
    );
    assert!(
        refusing.asked.load(Ordering::SeqCst) > 0,
        "the adapter was asked"
    );
    assert!(evidence(&tree).is_empty(), "{:?}\n{text}", evidence(&tree));
    assert!(!root.join("ran").exists(), "the check ran\n{text}");
}

#[test]
fn the_test_command_itself_goes_through_the_adapter() {
    let held = held();
    let root = directory("refuses-test");
    let tree = repository(
        "refuses-test-tree",
        TEST_ONLY,
        &[("check.sh", &check(&root), true)],
    );
    let program = tree.join("check.sh");
    let refusing = Arc::new(RefusesOne {
        refused: PathBuf::from("./check.sh"),
        asked: AtomicUsize::new(0),
    });
    held.install(refusing.clone());
    let ran = run(&tree, "./check.sh", &Never);
    drop(held);
    assert!(
        ran.said()
            .contains("confinement refuses-one refused ./check.sh"),
        "the refusal names the test command: {}",
        ran.said()
    );
    assert_eq!(refusing.asked.load(Ordering::SeqCst), 1, "{}", ran.said());
    assert!(ran.records.is_empty(), "{:?}\n{}", ran.records, ran.said());
    assert!(
        !root.join("ran").exists(),
        "{} ran\n{}",
        program.display(),
        ran.said()
    );
}

/// A harness program that writes `started` beside the repository when it runs.
fn double(root: &Path) -> PathBuf {
    let dir = root.join("double");
    fs::create_dir_all(&dir).expect("a directory");
    let program = dir.join("claude");
    fs::write(
        &program,
        format!(
            "#!/bin/sh\ntouch '{}'\nexit 1\n",
            root.join("started").display()
        ),
    )
    .expect("the double");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("a mode");
    program
}

/// Runs the edit-and-test protocol with `harness`, whose program is refused by path.
fn harness_refused(name: &str, harness: impl FnOnce(&Path) -> Box<dyn Harness>) {
    let held = held();
    let root = directory(name);
    let program = double(&root);
    let tree = repository(
        &format!("{name}-tree"),
        EDIT_AND_TEST,
        &[("check.sh", "#!/bin/sh\nexit 1\n", true)],
    );
    let harness = harness(&program);
    let refusing = Arc::new(RefusesOne {
        refused: program.clone(),
        asked: AtomicUsize::new(0),
    });
    held.install(refusing.clone());
    let ran = run(&tree, "./check.sh", harness.as_ref());
    drop(held);
    assert!(
        ran.said().contains("confinement refuses-one refused")
            && ran.said().contains(&program.display().to_string()),
        "the refusal names the harness's program: {}",
        ran.said()
    );
    assert!(
        refusing.asked.load(Ordering::SeqCst) > 0,
        "the harness's program was asked for: {}",
        ran.said()
    );
    assert!(
        !root.join("started").exists(),
        "the harness's program ran: {}",
        ran.said()
    );
}

#[test]
fn the_session_harness_program_goes_through_the_adapter() {
    harness_refused("session-harness", |program| {
        Box::new(SessionHarness {
            program: program.display().to_string(),
            ..SessionHarness::of(Kind::Claude)
        })
    });
}

#[test]
fn the_print_mode_harness_program_goes_through_the_adapter() {
    harness_refused("print-harness", |program| {
        Box::new(ClaudeCode {
            program: program.display().to_string(),
            ..ClaudeCode::default()
        })
    });
}

// --- Acceptance 6: the first line and the events name the adapter. ---

#[test]
fn the_first_line_and_the_opened_event_name_the_adapter() {
    let held = held();
    let tree = repository(
        "named",
        TEST_ONLY,
        &[("check.sh", "#!/bin/sh\nexit 0\n", true)],
    );
    let ran = run(&tree, "./check.sh", &Never);
    drop(held);
    let first = ran.text.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("case ")
            && first.contains(&format!(
                "; confinement none: Substrate local, declared insecure: {}; proved: ",
                confinement::LOCAL_REASON
            )),
        "story:confinement-through-substrate-on-linux-and-macos: the first line names the adapter and the reason it is declared insecure: {first}\n{}",
        ran.said()
    );
    let opened: serde_json::Value = serde_json::from_str(
        ran.events
            .lines()
            .find(|line| line.contains("\"event\":\"opened\""))
            .expect("an opened line"),
    )
    .expect("json");
    assert_eq!(opened["confinement"], "none", "{opened}");
    assert_eq!(
        opened["substrate"]["adapter"], "local",
        "story:confinement-through-substrate-on-linux-and-macos: the opened event names Substrate's adapter: {opened}"
    );
    assert_eq!(
        opened["substrate"]["declared_insecure"],
        confinement::LOCAL_REASON,
        "story:confinement-through-substrate-on-linux-and-macos: and the reason: {opened}"
    );
    assert!(
        matches!(ran.stop, Ok(Stop::Completed(_))),
        "the run completes under none: {}",
        ran.said()
    );
}

// --- Substrate, each case in a process of its own. ---

/// Set in the child process of a case: it runs the case.
const DELEGATED_CASE: &str = "TAKT_CONFINEMENT_TEST_DELEGATED";
/// The file the child writes what it decided into.
const REPORT: &str = "TAKT_CONFINEMENT_TEST_REPORT";

/// Runs `case` again in this test binary, alone in a process of its own, with Substrate open
/// there, or skips naming what is missing: no adapter of this host serves a test command. A
/// process of its own, since a `systemd` adapter moves the process that opens it into a scope,
/// and one process holds one `systemd` around `linux` at a time. Since
/// story:confinement-through-substrate-on-linux-and-macos takt asks no delegated scope of
/// `systemd-run`: Substrate's adapters open where takt runs.
fn delegated(name: &str, case: impl FnOnce()) {
    if std::env::var_os(DELEGATED_CASE).is_some() {
        let report = PathBuf::from(std::env::var_os(REPORT).expect("the report path is given"));
        fs::write(&report, "started").expect("the report");
        let probe = directory(&format!("{name}-probe")).join("workspace");
        fs::create_dir_all(&probe).expect("a workspace");
        let opened = Substrate::open(&probe, &confinement::Options::default());
        match opened {
            Ok(probe) => {
                drop(probe);
                fs::write(&report, "ran").expect("the report");
                case();
            }
            Err(error) => {
                fs::write(&report, format!("a Substrate adapter ({error:#})")).expect("the report");
            }
        }
        return;
    }
    let state = directory(&format!("{name}-state"));
    let report = state.join("report");
    let output = Command::new(std::env::current_exe().expect("this binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(DELEGATED_CASE, "1")
        .env(REPORT, &report)
        // The scratch crates build into their own `target/`, which their tests write into; a
        // caller's `CARGO_TARGET_DIR` would send their builds into the one this binary runs from.
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("this test binary runs again");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match fs::read_to_string(&report).ok().as_deref() {
        None | Some("started") => panic!(
            "the child case stopped before it decided its prerequisites: {}\n{stdout}{stderr}",
            output.status
        ),
        Some("ran") => {
            assert!(
                output.status.success(),
                "the child case failed: {}\n{stdout}{stderr}",
                output.status
            );
            assert!(
                stdout.contains("1 passed"),
                "the child binary ran the case: {stdout}{stderr}"
            );
            eprintln!("ran in a process of its own: {name}");
        }
        Some(missing) => eprintln!("skipped: missing prerequisite: {missing}"),
    }
}

/// The Substrate adapter, opened on `tree`.
fn substrate(tree: &Path) -> Arc<dyn Confinement> {
    Arc::new(
        Substrate::open(tree, &confinement::Options::default())
            .expect("Substrate opens on the tree"),
    )
}

/// The `opened` event of a run's record.
fn opened(ran: &Ran) -> serde_json::Value {
    serde_json::from_str(
        ran.events
            .lines()
            .find(|line| line.contains("\"event\":\"opened\""))
            .expect("an opened line"),
    )
    .expect("json")
}

/// The test a scratch crate runs to show its `PATH`: written to `target/seen-path`.
const SEES_ITS_PATH: &str = r#"    let path = std::env::var("PATH").unwrap_or_default();
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/seen-path"), path).unwrap();"#;

#[test]
fn under_substrate_the_test_command_writes_the_record_it_writes_under_none() {
    delegated(
        "under_substrate_the_test_command_writes_the_record_it_writes_under_none",
        || {
            let held = held();
            let none = scratch_crate("record-none", TEST_ONLY, SEES_ITS_PATH);
            let under_none = run(&none, "cargo test --quiet --offline", &Never);
            let confined = confined_crate("record-substrate", TEST_ONLY, SEES_ITS_PATH);
            held.install(substrate(&confined));
            let under_substrate = run(&confined, "cargo test --quiet --offline", &Never);
            drop(held);
            let [unconfined] = under_none.records.as_slice() else {
                panic!("one record under none: {}", under_none.said());
            };
            let [record] = under_substrate.records.as_slice() else {
                panic!("one record under substrate: {}", under_substrate.said());
            };
            for field in ["format", "id", "kind", "result", "subject", "producer"] {
                assert_eq!(
                    record[field], unconfined[field],
                    "{field}: {record} against {unconfined}"
                );
            }
            assert_eq!(record["kind"], "test_result");
            assert_eq!(record["result"], "pass", "{}", under_substrate.said());
            assert!(
                matches!(under_substrate.stop, Ok(Stop::Completed(_))),
                "{}",
                under_substrate.said()
            );
            let seen = fs::read_to_string(confined.join("target/seen-path"))
                .expect("the confined test wrote where it was told");
            if cfg!(target_os = "linux") {
                assert_eq!(
                    seen, "/toolchain/rust/bin:/usr/bin:/bin",
                    "story:confinement-through-substrate-on-linux-and-macos: the test ran on the fixed PATH of the toolchain's read-only root"
                );
            } else {
                assert!(
                    seen.ends_with("/bin:/usr/bin:/bin"),
                    "story:confinement-through-substrate-on-linux-and-macos: the test ran on the toolchain's PATH: {seen}"
                );
            }
            assert_ne!(
                fs::read_to_string(none.join("target/seen-path")).expect("the unconfined test"),
                seen,
                "the unconfined test ran on the caller's PATH"
            );
            let first = under_substrate.text.lines().next().unwrap_or_default();
            assert!(
                first.contains("; confinement substrate: Substrate "),
                "story:confinement-through-substrate-on-linux-and-macos: the first line names the adapter: {first}"
            );
            let opened = opened(&under_substrate);
            assert_eq!(opened["confinement"], "substrate", "{opened}");
            assert!(
                ["linux", "systemd", "macos"]
                    .contains(&opened["substrate"]["adapter"].as_str().unwrap_or_default())
                    && opened["substrate"].get("declared_insecure").is_none(),
                "story:confinement-through-substrate-on-linux-and-macos: the opened event names the adapter chosen: {opened}"
            );
        },
    );
}

/// What a probe test recorded in `target/probe`.
fn probed(tree: &Path) -> String {
    fs::read_to_string(tree.join("target/probe")).expect("the probe recorded what it did")
}

/// A probe that writes a promotion record outside the build directory twice, as a test with
/// `XDG_STATE_HOME` would (M19): at the host path `root/state/…` and in the crate's own tree
/// beside `target`; it records what each write returned, `outside <result>` and `tree <result>`
/// on two lines.
fn writes_outside(root: &Path) -> String {
    let dir = root.join("state/takt/promotions");
    format!(
        r#"    let outside = std::fs::create_dir_all("{dir}")
        .and_then(|()| std::fs::write("{dir}/probe.json", "{{}}"));
    let tree = std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/promotion.json"), "{{}}");
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/probe"), format!("outside {{outside:?}}\ntree {{tree:?}}")).unwrap();"#,
        dir = dir.display()
    )
}

/// Observes the test command once on `tree`'s `HEAD`, outside any case.
fn observe(tree: &Path) -> anyhow::Result<takt::validator::Observation> {
    let revision = git(tree, &["rev-parse", "HEAD"]);
    TestsRun::new("cargo test --quiet --offline")
        .expect("a command")
        .observe(tree, &revision, 1)
}

/// The facts the adapter in use proved, by name.
fn proved() -> Vec<String> {
    confinement::current()
        .substrate()
        .map(|record| record.facts)
        .unwrap_or_default()
}

#[test]
fn under_substrate_a_test_cannot_write_outside_the_build_directory() {
    delegated(
        "under_substrate_a_test_cannot_write_outside_the_build_directory",
        || {
            let held = held();
            let none_root = directory("m19-none-root");
            let none = scratch_crate("m19-none", TEST_ONLY, &writes_outside(&none_root));
            let observed = observe(&none).expect("the test command runs under none");
            assert!(
                none_root.join("state/takt/promotions/probe.json").is_file()
                    && none.join("promotion.json").is_file(),
                "the premise: under none the test writes outside the tree and in it: {} {}",
                observed.summary,
                probed(&none)
            );
            let root = directory("m19-substrate-root");
            let confined = confined_crate("m19-substrate", TEST_ONLY, &writes_outside(&root));
            held.install(substrate(&confined));
            let observed = observe(&confined).expect("the test command runs under substrate");
            drop(held);
            // A host path outside the tree is not in the child's file system at all: the
            // directories it makes there are in the private root of its own mount namespace,
            // or refused by the profile on macOS, so nothing reaches the host's.
            assert!(
                !root.join("state").exists(),
                "story:confinement-through-substrate-on-linux-and-macos: the confined test wrote outside the tree: {} {}",
                observed.summary,
                probed(&confined)
            );
            let tree = probed(&confined)
                .lines()
                .find(|line| line.starts_with("tree "))
                .map(str::to_owned);
            assert!(
                tree.is_some(),
                "story:confinement-through-substrate-on-linux-and-macos: the probe ran in the workspace: {}",
                probed(&confined)
            );
            // The tree is Substrate's workspace. A Linux adapter shows it read-only but its
            // `target`, so the write beside `target` fails inside the run; `macos` shows it
            // writable whole, so there it lands in the tree, never outside it.
            if std::env::consts::OS == "linux" {
                assert!(
                    tree.as_deref()
                        .is_some_and(|line| line.starts_with("tree Err("))
                        && !confined.join("promotion.json").exists(),
                    "story:confinement-through-substrate-on-linux-and-macos: under a Linux adapter the test wrote its tree outside the build directory: {}",
                    probed(&confined)
                );
            }
        },
    );
}

/// A probe that removes the first loose object of its repository and writes another in its
/// place (M20), and records what that returned.
const PLANTS_AN_OBJECT: &str = r#"    let objects = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".git/objects");
    let planted = (|| -> std::io::Result<String> {
        let mut dirs: Vec<_> = std::fs::read_dir(&objects)?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<_, _>>()?;
        dirs.sort();
        for dir in dirs {
            if dir.file_name().is_some_and(|n| n.len() == 2) {
                if let Some(object) = std::fs::read_dir(&dir)?.next() {
                    let object = object?.path();
                    std::fs::remove_file(&object)?;
                    std::fs::write(&object, b"planted")?;
                    return Ok(object.display().to_string());
                }
            }
        }
        Err(std::io::Error::other("no loose object"))
    })();
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/target/probe"), format!("{planted:?}")).unwrap();"#;

/// Every loose object of `tree` and its bytes.
fn loose_objects(tree: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    for dir in fs::read_dir(tree.join(".git/objects")).expect("an object store") {
        let dir = dir.expect("an entry").path();
        if !dir.file_name().is_some_and(|n| n.len() == 2) {
            continue;
        }
        for object in fs::read_dir(&dir).expect("an object directory") {
            let object = object.expect("an entry").path();
            let bytes = fs::read(&object).expect("an object");
            found.push((object, bytes));
        }
    }
    found.sort();
    found
}

/// M20: the write is refused on every adapter. Where the adapter proves `sealed_paths`
/// (`macos`), the tree's `.git` is sealed; under `linux` and `systemd` around it, which prove
/// no `sealed_paths`, the workspace is read-only but its `target`, `.git` with the rest.
#[test]
fn under_substrate_a_test_cannot_write_the_object_store() {
    delegated(
        "under_substrate_a_test_cannot_write_the_object_store",
        || {
            let held = held();
            let none = scratch_crate("m20-none", TEST_ONLY, PLANTS_AN_OBJECT);
            let before = loose_objects(&none);
            observe(&none).expect("the test command runs under none");
            assert_ne!(
                loose_objects(&none),
                before,
                "the premise: under none the test rewrites an object: {}",
                probed(&none)
            );
            let confined = confined_crate("m20-substrate", TEST_ONLY, PLANTS_AN_OBJECT);
            let before = loose_objects(&confined);
            held.install(substrate(&confined));
            let sealed = proved().iter().any(|fact| fact == "sealed_paths");
            let observed = observe(&confined).expect("the test command runs under substrate");
            drop(held);
            assert!(
                sealed || std::env::consts::OS == "linux",
                "story:confinement-through-substrate-on-linux-and-macos: off Linux the adapter seals .git: {:?}",
                proved()
            );
            assert_eq!(
                loose_objects(&confined),
                before,
                "story:confinement-through-substrate-on-linux-and-macos: the confined test rewrote an object of .git: {}",
                observed.summary
            );
            assert!(
                probed(&confined).starts_with("Err("),
                "story:confinement-through-substrate-on-linux-and-macos: the write was refused: {}",
                probed(&confined)
            );
        },
    );
}

#[test]
fn under_substrate_a_python_test_command_is_refused_by_its_toolchain() {
    delegated(
        "under_substrate_a_python_test_command_is_refused_by_its_toolchain",
        || {
            let held = held();
            let root = directory("python-root");
            let tree = repository(
                "python",
                TEST_ONLY,
                &[(
                    "test.py",
                    &format!(
                        "open({:?}, 'w').close()\n",
                        root.join("ran").display().to_string()
                    ),
                    false,
                )],
            );
            let watching = Arc::new(Watching::new(substrate(&tree)));
            held.install(watching.clone());
            let ran = run(&tree, "python3 -I test.py", &Never);
            drop(held);
            let said = ran.said();
            assert!(ran.stop.is_err(), "the run ends with the refusal: {said}");
            assert!(
                said.contains("confinement substrate refused") && said.contains("Python"),
                "the refusal names the adapter and the toolchain: {said}"
            );
            assert!(ran.records.is_empty(), "{:?}\n{said}", ran.records);
            assert!(!root.join("ran").exists(), "the test command ran: {said}");
            let pythons: Vec<String> = watching
                .admitted()
                .into_iter()
                .filter(|program| {
                    Path::new(program)
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("python"))
                })
                .collect();
            assert!(
                pythons.is_empty(),
                "a Python was started unconfined: {pythons:?}\n{said}"
            );
        },
    );
}
