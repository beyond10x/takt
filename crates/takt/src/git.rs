//! The few git reads and writes the kernel makes in a worktree, without a shell. A push goes
//! through the forge ([`crate::forge`]), never through here.
//!
//! What a run commits or discards is only ever its own: a case opens on a clean tree
//! ([`uncommitted`]), a judgement step is measured against a [`Checkpoint`] taken before it, an
//! accepted step commits exactly the paths it changed ([`commit_paths`]), and a refused or failed
//! one is put back to the checkpoint, the index, `HEAD`, ignored files and `.git/` included.
//! Paths are read from `git status --porcelain=v1 -z`, never from trimmed or quoted text.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::case::{CASE_ROOTS, holds_cases_root, is_under_cases};
use crate::confinement::Command;

/// What every git takt runs is told on its command line, over whatever the repository's, the
/// user's or the system's configuration says: run no code from configuration. No
/// `core.fsmonitor` (git runs it during `status`), no hooks (`commit` and `checkout` run them),
/// no credential helper, no SSH command, no signing program, and no automatic `gc` or
/// maintenance, which detaches a process of its own. A filter driver a `.gitattributes`
/// names is not covered: its name is the repository's choice. No signature check either, which
/// `log.showSignature` would make `git log` run.
const NO_CONFIGURED_CODE: [&str; 18] = [
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "credential.helper=",
    "-c",
    "core.sshCommand=false",
    "-c",
    "commit.gpgSign=false",
    "-c",
    "tag.gpgSign=false",
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
    "-c",
    "log.showSignature=false",
];

/// The variables of the caller's environment that would point git elsewhere than the tree, or
/// change how it reads a path, its configuration or its history (`GIT_TEST_COMMIT_GRAPH`
/// writes a commit-graph after every commit, `GIT_GRAFT_FILE` gives commits the parents a file
/// names, `GIT_SHALLOW_FILE` cuts the history where a file says); none reaches a git takt runs.
pub(crate) const REDIRECTING_ENV: [&str; 15] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_GLOB_PATHSPECS",
    "GIT_NOGLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
    "GIT_LITERAL_PATHSPECS",
    "GIT_TEST_COMMIT_GRAPH",
    "GIT_GRAFT_FILE",
    "GIT_SHALLOW_FILE",
];

/// Runs `git <args>` in `tree` with `input` on its standard input, with no code from
/// configuration ([`NO_CONFIGURED_CODE`]) and no credential in its environment; answers what
/// it printed, whatever its exit status. Git runs pinned to the tree: its `.git` is the git
/// directory and the tree the work tree, so a `.git` a step garbled makes the call fail rather
/// than find a repository further up; and for a tree without a `.git` git searches no further
/// than the tree itself.
fn output(tree: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Output> {
    output_with(tree, args, input, &[])
}

/// [`output`] with `env` set in git's environment over the caller's.
fn output_with(
    tree: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    env: &[(&str, &str)],
) -> Result<Output> {
    let mut child = command(tree, args)?
        .envs(env.iter().copied())
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(input)
            .with_context(|| format!("write to git {}", args.join(" ")))?;
    }
    child
        .wait_with_output()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))
}

/// Runs `git <args>` in `tree` as [`run`] does, with nothing on its standard input, and answers
/// its standard output as text (invalid UTF-8 replaced, nothing trimmed), or `None` as soon as it
/// passes `limit` bytes: git is killed then and nothing more is read. Git reads no replacement
/// object (`GIT_NO_REPLACE_OBJECTS`): what it shows is what the repository holds, as a push sends
/// it. Exiting non-zero is an error naming what git said.
pub fn run_bounded(tree: &Path, args: &[&str], limit: usize) -> Result<Option<String>> {
    use std::io::Read as _;
    let mut child = command(tree, args)?
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))?;
    let stderr = child.stderr.take().map(|mut stream| {
        std::thread::spawn(move || {
            let mut said = Vec::new();
            let _ = stream.by_ref().take(64 << 10).read_to_end(&mut said);
            let _ = std::io::copy(&mut stream, &mut std::io::sink());
            said
        })
    });
    let mut kept = Vec::new();
    let mut over = false;
    if let Some(stdout) = child.stdout.take() {
        let read = stdout
            .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
            .read_to_end(&mut kept);
        read.with_context(|| format!("read git {}", args.join(" ")))?;
        over = kept.len() > limit;
    }
    if over {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))?;
    let said = stderr
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    if over {
        return Ok(None);
    }
    if !status.success() {
        bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
            status,
            String::from_utf8_lossy(&said).trim()
        );
    }
    Ok(Some(String::from_utf8_lossy(&kept).into_owned()))
}

/// The `git <args>` [`output`] and [`run_bounded`] run in `tree`, set up as every git takt runs
/// is: pinned to the tree, with no code from configuration and no credential or redirecting
/// variable in its environment.
fn command(tree: &Path, args: &[&str]) -> Result<Command> {
    let tree =
        std::path::absolute(tree).with_context(|| format!("make {} absolute", tree.display()))?;
    let tree = tree.as_path();
    let mut command = Command::new("git");
    crate::forge::without_credentials(&mut command, |_| false);
    let dot_git = tree.join(".git");
    if std::fs::symlink_metadata(&dot_git).is_ok() {
        let mut git_dir = std::ffi::OsString::from("--git-dir=");
        git_dir.push(&dot_git);
        let mut work_tree = std::ffi::OsString::from("--work-tree=");
        work_tree.push(tree);
        command.arg(git_dir).arg(work_tree);
    }
    for name in REDIRECTING_ENV {
        command.env_remove(name);
    }
    // Nor a template directory, which a repository git makes is filled from (an
    // `objects/info/alternates` among it would lend it another store's objects unchecked).
    command.env_remove("GIT_TEMPLATE_DIR");
    if let Some(parent) = tree.parent() {
        command.env("GIT_CEILING_DIRECTORIES", parent);
    }
    // A replace ref (`refs/replace/<object>`) would make every read of an object return
    // another's content: no git takt runs honours one.
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
    // Nor does one fetch an object a partial clone lacks from its promisor remote: that fetch
    // would run the repository's own `remote.<name>.uploadpack` through a shell, and outlive the
    // git that started it. An object not in the repository is missing.
    command.env("GIT_NO_LAZY_FETCH", "1");
    // Nor a commit-graph, a cache of the history a step can forge (a parent re-pointed, its
    // checksum fixed): every ancestry and history takt reads comes from the commits themselves.
    command
        .args(NO_CONFIGURED_CODE)
        .args(["-c", "core.commitGraph=false"])
        .args(args)
        .current_dir(tree);
    Ok(command)
}

/// Runs `git <args>` in `tree`, `input` on its standard input; answers its standard output as
/// it is, byte for byte, or an error naming what git said when it exits non-zero.
fn raw(tree: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    raw_with(tree, args, input, &[])
}

/// [`raw`] with `env` set in git's environment over the caller's.
fn raw_with(
    tree: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    env: &[(&str, &str)],
) -> Result<Vec<u8>> {
    let output = output_with(tree, args, input, env)?;
    if !output.status.success() {
        bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

/// Runs `git <args>` in `tree` and answers its standard output trimmed: for answers of one
/// value (a revision, a branch, a count, a URL). Never for paths or `status`, whose leading
/// white space is part of the answer; those are read byte for byte with `-z` ([`bytes`]).
pub fn run(tree: &Path, args: &[&str]) -> Result<String> {
    let stdout = raw(tree, args, None)?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

/// Runs `git <args>` in `tree` and answers its standard output byte for byte, nothing trimmed
/// and nothing decoded: for answers that hold paths, asked for with `-z`, whose every byte,
/// leading white space included, is part of the path. The caller decodes each path, and one
/// that is not UTF-8 is its to refuse.
pub fn bytes(tree: &Path, args: &[&str]) -> Result<Vec<u8>> {
    raw(tree, args, None)
}

/// The bytes of the blob `object` names (`<revision>:<path>`, or a blob id) as git holds them:
/// nothing trimmed, nothing decoded.
pub fn blob(tree: &Path, object: &str) -> Result<Vec<u8>> {
    raw(tree, &["cat-file", "blob", object], None)
}

/// The bytes of the object `id` (a full object id) of `kind` (`commit`, `tree`, `blob`),
/// checked as [`ObjectReader::read`] checks every object; an object of another kind, or none,
/// is an error.
pub fn verified_object(tree: &Path, kind: &str, id: &str) -> Result<Vec<u8>> {
    let mut reader = ObjectReader::open(tree)?;
    match reader.read(id)? {
        Some((found, bytes)) if found == kind => Ok(bytes),
        Some((found, _)) => bail!("the object {id} is a {found}, not a {kind}"),
        None => bail!("the repository holds no object {id}"),
    }
}

/// What a commit's header says, read by git's own rule ([`parse_commit`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitHeader {
    /// Its tree.
    pub tree: String,
    /// Its parents, in order.
    pub parents: Vec<String>,
    /// Its committer time, in seconds since the epoch; none when the header names none that
    /// reads.
    pub committer_time: Option<i64>,
}

/// The header of the commit `id` from its `bytes`, by git's rule: the first line is
/// `tree <id>`, and the parents are the `parent <id>` lines directly after it, each line exact
/// (no carriage return stripped); a `parent` line further down the header is no parent, as git
/// takes none. A first line that is no tree, or a `parent` line there that is no object id of
/// `hex` digits, is an error naming the commit.
pub fn parse_commit(id: &str, bytes: &[u8], hex: usize) -> Result<CommitHeader> {
    let header_end = bytes
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .unwrap_or(bytes.len());
    let mut lines = bytes[..header_end].split(|b| *b == b'\n').peekable();
    let is_id = |text: &[u8]| {
        text.len() == hex
            && text
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    };
    let tree = lines
        .next()
        .and_then(|line| line.strip_prefix(b"tree "))
        .filter(|tree| is_id(tree))
        .ok_or_else(|| anyhow!("the commit {id} does not begin with its tree"))?;
    let tree = String::from_utf8_lossy(tree).into_owned();
    let mut parents = Vec::new();
    while let Some(parent) = lines.peek().and_then(|line| line.strip_prefix(b"parent ")) {
        if !is_id(parent) {
            bail!("the commit {id} names a parent that is no object id");
        }
        parents.push(String::from_utf8_lossy(parent).into_owned());
        lines.next();
    }
    let committer_time = lines
        .find_map(|line| line.strip_prefix(b"committer "))
        .and_then(|line| {
            let text = String::from_utf8_lossy(line);
            let mut words = text.rsplit(' ');
            let _zone = words.next()?;
            words.next()?.parse::<i64>().ok()
        });
    Ok(CommitHeader {
        tree,
        parents,
        committer_time,
    })
}

/// The repository's object format, from its `extensions.objectFormat`: `sha1` or `sha256`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    /// SHA-1, 40 hexadecimal digits.
    Sha1,
    /// SHA-256, 64 hexadecimal digits.
    Sha256,
}

impl ObjectFormat {
    /// The object format of the repository at `tree`, as git reports it.
    pub fn of(tree: &Path) -> Result<Self> {
        match run(tree, &["rev-parse", "--show-object-format"])?.as_str() {
            "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            other => bail!("the repository's object format {other} is not one takt reads"),
        }
    }

    /// The id of an object of `kind` holding `bytes`: the hash of `<kind> <len>\0<bytes>`, in
    /// lower-case hexadecimal. SHA-1 is computed with collision detection (SHA-1DC, as git
    /// computes it): bytes made to collide are an error, never an id.
    pub fn id(self, kind: &str, bytes: &[u8]) -> Result<String> {
        use sha2::Digest as _;
        let header = format!("{kind} {}\0", bytes.len());
        let digest: Vec<u8> = match self {
            Self::Sha1 => {
                let mut hasher = sha1_checked::Sha1::new();
                hasher.update(header.as_bytes());
                hasher.update(bytes);
                match hasher.try_finalize() {
                    sha1_checked::CollisionResult::Ok(hash) => hash.to_vec(),
                    _ => bail!("the {kind}'s bytes are made to collide under SHA-1"),
                }
            }
            Self::Sha256 => {
                let mut hasher = sha2::Sha256::new();
                hasher.update(header.as_bytes());
                hasher.update(bytes);
                hasher.finalize().to_vec()
            }
        };
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

/// The largest blob [`ObjectReader`] reads, in bytes: the cap on the loop's metric definitions,
/// the only blob takt reads through it. A blob git reports larger is refused before any of it
/// is read.
pub const MAX_BLOB: u64 = 256 << 10;

/// The largest commit or tree [`ObjectReader`] reads, in bytes; one git reports larger is
/// refused before any of it is read.
pub const MAX_COMMIT_OR_TREE: u64 = 1 << 20;

/// How long one read of [`ObjectReader`] may take: past it, `git cat-file` is killed and the
/// read is an error naming the object.
pub const READ_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// What the watchdog of an [`ObjectReader`] is told: the instant the read in progress must end
/// by, whether it fired, and whether the reader is gone.
#[derive(Default)]
struct Watch {
    deadline: Option<std::time::Instant>,
    fired: bool,
    stop: bool,
}

/// The watchdog of an [`ObjectReader`]: one thread that kills `git cat-file` when a read runs
/// past its deadline, so no read blocks for longer than [`READ_DEADLINE`] whatever git does.
struct Watchdog {
    watch: std::sync::Arc<(std::sync::Mutex<Watch>, std::sync::Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watchdog {
    /// A watchdog over the process `pid`, which its owner keeps unwaited for while the watchdog
    /// lives, so the id names no other process.
    fn over(pid: u32) -> Self {
        let watch = std::sync::Arc::new((
            std::sync::Mutex::new(Watch::default()),
            std::sync::Condvar::new(),
        ));
        let shared = std::sync::Arc::clone(&watch);
        let thread = std::thread::spawn(move || {
            let (lock, wake) = &*shared;
            let Ok(mut watch) = lock.lock() else {
                return;
            };
            loop {
                if watch.stop {
                    return;
                }
                let now = std::time::Instant::now();
                watch = match watch.deadline {
                    Some(deadline) if now >= deadline => {
                        kill(pid);
                        watch.fired = true;
                        watch.deadline = None;
                        continue;
                    }
                    Some(deadline) => match wake.wait_timeout(watch, deadline - now) {
                        Ok((watch, _)) => watch,
                        Err(_) => return,
                    },
                    None => match wake.wait(watch) {
                        Ok(watch) => watch,
                        Err(_) => return,
                    },
                };
            }
        });
        Self {
            watch,
            thread: Some(thread),
        }
    }

    /// Arms the deadline of a read beginning now: `until`.
    fn arm(&self, until: std::time::Instant) {
        let (lock, wake) = &*self.watch;
        if let Ok(mut watch) = lock.lock() {
            watch.deadline = Some(until);
            watch.fired = false;
        }
        wake.notify_one();
    }

    /// Disarms the deadline; whether it fired during the read.
    fn disarm(&self) -> bool {
        let (lock, wake) = &*self.watch;
        let fired = lock.lock().is_ok_and(|mut watch| {
            watch.deadline = None;
            watch.fired
        });
        wake.notify_one();
        fired
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        let (lock, wake) = &*self.watch;
        if let Ok(mut watch) = lock.lock() {
            watch.stop = true;
        }
        wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Kills the reader's `git cat-file`, `pid`. It starts no process of its own: no git takt runs
/// fetches lazily (`GIT_NO_LAZY_FETCH`), so nothing else is left to kill. It stays in takt's
/// own process group: one of its own would be taken by the forge's sweep of what a child left
/// behind ([`crate::forge::sweep_left`]) for a leftover and killed under a live reader.
fn kill(pid: u32) {
    #[cfg(unix)]
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: `kill` takes a process id and a signal; the process is the reader's own
        // child, not yet waited for, so the id names no other.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = pid;
}

/// Objects read through one `git cat-file --batch`, each checked in this process: git does not
/// check a loose object's bytes against the id it is stored under, so the id is computed again
/// from the kind and the bytes read, in the repository's object format, and an object whose
/// bytes hash to another id is an error naming it. Every read is bounded: an object of a kind
/// other than commit, tree or blob, or one git reports larger than its cap ([`MAX_BLOB`],
/// [`MAX_COMMIT_OR_TREE`]), is refused before any of it is read, and a read that takes longer
/// than [`READ_DEADLINE`] (a header that overstates the body, a git that stalls) kills the
/// process and is an error naming the object; a reader
/// opened until an instant ([`Self::open_until`], the run's deadline) ends each read at the
/// earlier of the two. After any error the reader reads nothing more. One process reads any
/// number of objects; it is killed and waited for when the reader is dropped.
pub struct ObjectReader {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    stdout: std::io::BufReader<std::process::ChildStdout>,
    format: ObjectFormat,
    watchdog: Option<Watchdog>,
    until: Option<std::time::Instant>,
}

impl ObjectReader {
    /// A reader of the objects of the repository at `tree`, each read held to
    /// [`READ_DEADLINE`].
    pub fn open(tree: &Path) -> Result<Self> {
        Self::open_until(tree, None)
    }

    /// A reader as [`Self::open`] opens one, each read also ending by `until` (the run's
    /// deadline) when that comes first.
    pub fn open_until(tree: &Path, until: Option<std::time::Instant>) -> Result<Self> {
        let format = ObjectFormat::of(tree)?;
        let mut child = command(tree, &["cat-file", "--batch"])?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("run git cat-file --batch in {}", tree.display()))?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .map(std::io::BufReader::new)
            .ok_or_else(|| anyhow!("git cat-file --batch has no standard output"))?;
        let watchdog = Some(Watchdog::over(child.id()));
        Ok(Self {
            child,
            stdin,
            stdout,
            format,
            watchdog,
            until,
        })
    }

    /// The kind and the bytes of the object `id` (a full object id), checked; none when the
    /// repository holds no such object. An object whose bytes do not hash to `id` is an error
    /// naming it, and so is one over its cap, a read past [`READ_DEADLINE`], and a reply git
    /// does not document.
    pub fn read(&mut self, id: &str) -> Result<Option<(String, Vec<u8>)>> {
        let hex = id.len() == 40 || id.len() == 64;
        if !hex
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("{id} is no full object id");
        }
        let own = std::time::Instant::now() + READ_DEADLINE;
        let until = self.until.map_or(own, |until| until.min(own));
        if let Some(watchdog) = &self.watchdog {
            watchdog.arm(until);
        }
        let read = self.read_bounded(id);
        let fired = self.watchdog.as_ref().is_some_and(Watchdog::disarm);
        let read = if fired {
            Err(if until < own {
                anyhow!(
                    "reading the object {id} did not end by the run's deadline; git cat-file was stopped and nothing was read from it"
                )
            } else {
                anyhow!(
                    "reading the object {id} took longer than {} s; git cat-file was stopped and nothing was read from it",
                    READ_DEADLINE.as_secs()
                )
            })
        } else {
            read
        };
        if read.is_err() {
            // The stream may be anywhere in an object: nothing more is read from it.
            self.stdin = None;
            kill(self.child.id());
        }
        read
    }

    /// [`Self::read`] once its deadline is armed.
    fn read_bounded(&mut self, id: &str) -> Result<Option<(String, Vec<u8>)>> {
        use std::io::{BufRead as _, Read as _};
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("git cat-file --batch was stopped after an earlier error"))?;
        stdin
            .write_all(format!("{id}\n").as_bytes())
            .and_then(|()| stdin.flush())
            .context("write to git cat-file --batch")?;
        let mut header = String::new();
        (&mut self.stdout)
            .take(256)
            .read_line(&mut header)
            .with_context(|| format!("read the header of the object {id}"))?;
        let fields: Vec<&str> = header.trim_end_matches('\n').split(' ').collect();
        match fields.as_slice() {
            [named, "missing"] if *named == id => Ok(None),
            [named, kind, size] if *named == id => {
                let size: u64 = size.parse().with_context(|| {
                    format!("the object {id}: git cat-file --batch replied {header:?}")
                })?;
                let cap = match *kind {
                    "blob" => MAX_BLOB,
                    "commit" | "tree" => MAX_COMMIT_OR_TREE,
                    other => bail!("the object {id} is a {other}, which takt does not read"),
                };
                if size > cap {
                    bail!(
                        "the {kind} {id} is {size} bytes, above the {cap} bytes takt reads of a {kind}; nothing was read from it"
                    );
                }
                let with_newline = size
                    .checked_add(1)
                    .and_then(|length| usize::try_from(length).ok())
                    .ok_or_else(|| anyhow!("the {kind} {id} is {size} bytes, too many to read"))?;
                let mut bytes = vec![0; with_newline];
                self.stdout
                    .read_exact(&mut bytes)
                    .with_context(|| format!("read the {kind} {id}"))?;
                bytes.truncate(with_newline - 1);
                let computed = self
                    .format
                    .id(kind, &bytes)
                    .with_context(|| format!("the {kind} {id}"))?;
                if computed != id {
                    bail!(
                        "the {kind} {id} reads as bytes whose id is {computed}: the object store holds other bytes under its id; nothing was read from it"
                    );
                }
                Ok(Some(((*kind).to_owned(), bytes)))
            }
            _ => bail!("the object {id}: git cat-file --batch replied {header:?}"),
        }
    }
}

impl Drop for ObjectReader {
    fn drop(&mut self) {
        // The watchdog stops first; then git is killed (it reads, it writes nothing elsewhere)
        // and waited for, so no process is left, even one blocked writing a body nobody reads.
        self.watchdog = None;
        drop(self.stdin.take());
        kill(self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The commit `HEAD` names.
pub fn head(tree: &Path) -> Result<String> {
    run(tree, &["rev-parse", "HEAD"])
}

/// The branch `HEAD` is on; `None` when it is detached.
pub fn current_branch(tree: &Path) -> Result<Option<String>> {
    let name = run(tree, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok((name != "HEAD").then_some(name))
}

/// Creates `branch` and checks it out, at `from` when given, else at `HEAD`; a branch that
/// exists is an error.
pub fn create_branch(tree: &Path, branch: &str, from: Option<&str>) -> Result<()> {
    let mut args = vec!["checkout", "-q", "-b", branch];
    args.extend(from);
    run(tree, &args)?;
    Ok(())
}

/// Checks out `target`, a branch, and deletes `branch`; `None` detaches `HEAD` instead.
pub fn leave_branch(tree: &Path, branch: &str, target: Option<&str>) -> Result<()> {
    match target {
        Some(target) => run(tree, &["checkout", "-q", target])?,
        None => run(tree, &["checkout", "-q", "--detach"])?,
    };
    run(tree, &["branch", "-q", "-D", branch])?;
    Ok(())
}

/// The variables that hand git configuration through the environment and that every other git
/// takt runs is started without ([`REDIRECTING_ENV`]); `GIT_CONFIG_COUNT` brings its
/// `GIT_CONFIG_KEY_<n>` and `GIT_CONFIG_VALUE_<n>` with it. A forge's git, which fetches and
/// pushes a remote, reads them as the operator's git does ([`crate::forge::Process`]), so a
/// remote's URLs are read under them too ([`remote_url`], [`remote_urls`]): a rewrite
/// (`url.<base>.insteadOf`) they hand git is read before the fetch, as the fetch reads it.
pub(crate) const ENVIRONMENT_CONFIG: [&str; 2] = ["GIT_CONFIG_PARAMETERS", "GIT_CONFIG_COUNT"];

/// What of [`ENVIRONMENT_CONFIG`] takt's environment holds, each by its name; an error when one
/// is not UTF-8.
fn environment_config() -> Result<Vec<(&'static str, String)>> {
    ENVIRONMENT_CONFIG
        .into_iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (name, value)))
        .map(|(name, value)| {
            value
                .into_string()
                .map(|value| (name, value))
                .map_err(|_| anyhow!("{name} is not UTF-8; no remote URL was read"))
        })
        .collect()
}

/// The trimmed standard output of `git <args>` in `tree` read under the configuration the
/// environment hands git ([`environment_config`]), as a remote's own git reads it.
fn run_as_remote_git(tree: &Path, args: &[&str]) -> Result<String> {
    let config = environment_config()?;
    let env: Vec<(&str, &str)> = config
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let stdout = raw_with(tree, args, None, &env)?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

/// The URL of `remote`, `insteadOf` applied as the environment's configuration and the
/// repository's give it ([`ENVIRONMENT_CONFIG`]); an error when the tree has no such remote.
pub fn remote_url(tree: &Path, remote: &str) -> Result<String> {
    run_as_remote_git(tree, &["remote", "get-url", remote])
}

/// Every URL git reaches `remote` at, as git reads them, the environment's configuration
/// included ([`ENVIRONMENT_CONFIG`]): a fetch's (`url`, `insteadOf` applied), then each a push
/// goes to (`pushurl`, else `url` with `pushInsteadOf` applied). A remote whose
/// `remote.<name>.vcs` names a remote helper is reached through it, so each of its URLs is
/// answered in git's `<helper>::<address>` form ([`remote_transport`]). An error when the tree
/// has no such remote.
pub fn remote_urls(tree: &Path, remote: &str) -> Result<(Vec<String>, Vec<String>)> {
    let vcs = run_as_remote_git(tree, &["config", "--get", &format!("remote.{remote}.vcs")])
        .ok()
        .filter(|vcs| !vcs.is_empty());
    let lines = |args: &[&str]| -> Result<Vec<String>> {
        Ok(run_as_remote_git(tree, args)?
            .lines()
            .map(|url| match &vcs {
                Some(vcs) => format!("{vcs}::{url}"),
                None => url.to_owned(),
            })
            .collect())
    };
    Ok((
        lines(&["remote", "get-url", "--all", remote])?,
        lines(&["remote", "get-url", "--push", "--all", remote])?,
    ))
}

/// The remote helper and the address of `url` when it is in git's `<helper>::<address>` form
/// (`http::http://example.com/r.git`): a letter, then letters, digits, `+`, `.` or `-`, then
/// `::`, as git reads it before any scheme.
fn helper_form(url: &str) -> Option<(&str, &str)> {
    let (helper, address) = url.split_once("::")?;
    let mut chars = helper.chars();
    (chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)))
    .then_some((helper, address))
}

/// How git reaches a remote URL for a fetch or a push ([`remote_transport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteTransport {
    /// `https://`.
    Https,
    /// `http://` to a loopback host (`localhost`, `127.0.0.0/8`, `[::1]`): a relay on this
    /// machine, or in this pod, that holds whatever credential the remote takes, so takt holds
    /// none and nothing crosses the network in the clear. A line to it follows no redirect
    /// ([`NO_REDIRECT`]), so a relay cannot send the fetch or the push on to another host.
    LoopbackHttp,
    /// Any other: SSH, a path on this machine, `file://`, `git://`.
    Other,
}

impl RemoteTransport {
    /// What `GIT_ALLOW_PROTOCOL` lists for a line held to HTTPS that goes to a remote of this
    /// transport: `https`, and `http` beside it for a loopback relay alone.
    #[must_use]
    pub fn allowed_protocols(self) -> &'static str {
        match self {
            Self::LoopbackHttp => "http:https",
            Self::Https | Self::Other => "https",
        }
    }
}

/// What a fetch or a push to a loopback relay is given on git's command line: follow no
/// redirect, not even the first request's (`http.followRedirects` is `initial` by default).
pub const NO_REDIRECT: [&str; 2] = ["-c", "http.followRedirects=false"];

/// How git reaches `url` ([`RemoteTransport`]). An `http://` or `ftp://` URL whose host is no
/// loopback host (`crate::harness::loopback`) is refused, naming it without its userinfo, which
/// may hold a password: what git sends there, and any credential with it, would cross the
/// network in the clear. A URL in git's `<helper>::<address>` form is read by its address when
/// the helper is git's own `http`, `https`, `ftp` or `ftps`, which fetch the address as a URL
/// (one without a scheme over plain `http://`, as curl takes it); any other helper is
/// [`RemoteTransport::Other`].
pub fn remote_transport(url: &str) -> Result<RemoteTransport> {
    if let Some((helper, address)) = helper_form(url) {
        if !["http", "https", "ftp", "ftps"]
            .iter()
            .any(|curl| helper.eq_ignore_ascii_case(curl))
        {
            return Ok(RemoteTransport::Other);
        }
        return if address.contains("://") {
            remote_transport(address)
        } else {
            remote_transport(&format!("http://{address}"))
        };
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return Ok(RemoteTransport::Other);
    };
    if scheme.eq_ignore_ascii_case("https") {
        return Ok(RemoteTransport::Https);
    }
    let ftp = scheme.eq_ignore_ascii_case("ftp");
    if !scheme.eq_ignore_ascii_case("http") && !ftp {
        return Ok(RemoteTransport::Other);
    }
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    match crate::harness::host_of(host_port) {
        Some(host) if crate::harness::loopback(host) && ftp => Ok(RemoteTransport::Other),
        Some(host) if crate::harness::loopback(host) => Ok(RemoteTransport::LoopbackHttp),
        host => bail!(
            "the remote URL {} is `{}://` to {}, no loopback host (`localhost`, `127.0.0.0/8`, `::1`): what git sends there would cross the network in the clear; give `https://`, or `http://` to a relay on a loopback host",
            crate::forge::without_userinfo(url),
            scheme.to_ascii_lowercase(),
            host.unwrap_or("no host takt reads")
        ),
    }
}

/// The full names of the refs under each prefix of `prefixes` (`refs/heads/trial/`), in ref
/// order; none when nothing is there.
pub fn refs(tree: &Path, prefixes: &[&str]) -> Result<Vec<String>> {
    let mut args = vec!["for-each-ref", "--format=%(refname)"];
    args.extend(prefixes);
    let listed = raw(tree, &args, None)?;
    Ok(String::from_utf8_lossy(&listed)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// One entry of a file in git's configuration syntax: its key and, unless it has no `=`, its
/// value.
pub type ConfigEntry = (String, Option<String>);

/// The entries of `path`, a file in git's configuration syntax, as `revision` holds it: each
/// `(key, value)` in file order, read with `git config --no-includes --blob`, so nothing is
/// checked out and no `include.path` or `includeIf` is followed: an include is listed as its
/// own entry, never as the entries of the file it names. A key with no `=` has no value. `None` when `revision` names no
/// commit or holds no file at `path`.
pub fn config_at(tree: &Path, revision: &str, path: &str) -> Result<Option<Vec<ConfigEntry>>> {
    let blob = format!("{revision}:{path}");
    if !output(tree, &["cat-file", "-e", &blob], None)?
        .status
        .success()
    {
        return Ok(None);
    }
    let listed = raw(
        tree,
        &["config", "--no-includes", "--blob", &blob, "--list", "-z"],
        None,
    )?;
    listed
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let text = String::from_utf8(entry.to_vec()).map_err(|_| {
                anyhow!(
                    "{blob} holds an entry that is not UTF-8: {}",
                    String::from_utf8_lossy(entry)
                )
            })?;
            Ok(match text.split_once('\n') {
                Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
                None => (text, None),
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

/// Whether the history of `revision` holds a commit whose subject is `subject`, exactly; false
/// when `revision` names nothing.
pub fn has_subject(tree: &Path, revision: &str, subject: &str) -> Result<bool> {
    let grep = format!("--grep={subject}");
    let listed = output(
        tree,
        &[
            "log",
            "-z",
            "--format=%s",
            "--fixed-strings",
            &grep,
            revision,
            "--",
        ],
        None,
    )?;
    if !listed.status.success() {
        return Ok(false);
    }
    Ok(listed
        .stdout
        .split(|byte| *byte == 0)
        .any(|line| line == subject.as_bytes()))
}

/// One commit as takt reads it: its subject, its parents, and the paths it changes against its
/// first parent, both sides of a rename, read byte for byte with `-z`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// The first line of the message.
    pub subject: String,
    /// The parents, in order.
    pub parents: Vec<String>,
    /// The paths changed against the first parent; none for a commit without a parent.
    pub paths: Vec<String>,
}

/// Reads the commit `revision` names.
pub fn commit(tree: &Path, revision: &str) -> Result<Commit> {
    let subject = run(tree, &["log", "-1", "--format=%s", revision, "--"])?;
    let line = run(tree, &["rev-list", "--parents", "-n", "1", revision, "--"])?;
    let parents: Vec<String> = line.split_whitespace().skip(1).map(str::to_owned).collect();
    let paths = match parents.first() {
        Some(parent) => {
            let listed = raw(
                tree,
                &[
                    "diff-tree",
                    "-r",
                    "-z",
                    "--no-renames",
                    "--name-only",
                    parent,
                    revision,
                    "--",
                ],
                None,
            )?;
            listed
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
                .map(|path| {
                    String::from_utf8(path.to_vec()).map_err(|_| {
                        anyhow!(
                            "git diff-tree named a path that is not UTF-8: {}",
                            String::from_utf8_lossy(path)
                        )
                    })
                })
                .collect::<Result<Vec<_>>>()?
        }
        None => Vec::new(),
    };
    Ok(Commit {
        subject,
        parents,
        paths,
    })
}

/// One entry of `git status --porcelain=v1 -z`: the two-letter code, the path, and for a rename
/// or a copy the path it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// `XY`: the index's and the work tree's state; `??` untracked, `!!` ignored.
    pub code: String,
    /// The path, relative to the tree; an ignored directory ends in `/`.
    pub path: String,
    /// The path a rename or copy came from.
    pub from: Option<String>,
}

/// Reads `git status --porcelain=v1 -z`: `XY <path>` NUL, and after a rename or a copy its
/// source NUL. Nothing is quoted and nothing trimmed, so a path keeps its leading space, its
/// spaces and its non-ASCII characters. A path that is not UTF-8 is an error, not a guess.
pub fn parse_status(bytes: &[u8]) -> Result<Vec<Entry>> {
    fn text(bytes: &[u8]) -> Result<String> {
        String::from_utf8(bytes.to_vec()).map_err(|_| {
            anyhow!(
                "git status named a path that is not UTF-8: {}",
                String::from_utf8_lossy(bytes)
            )
        })
    }
    let mut fields = bytes.split(|byte| *byte == 0);
    let mut entries = Vec::new();
    while let Some(field) = fields.next() {
        if field.is_empty() {
            continue;
        }
        if field.len() < 4 || field[2] != b' ' {
            bail!(
                "git status gave an entry it does not document: {}",
                String::from_utf8_lossy(field)
            );
        }
        let code = text(&field[..2])?;
        let path = text(&field[3..])?;
        let from = if code.contains(['R', 'C']) {
            let source = fields
                .next()
                .filter(|source| !source.is_empty())
                .ok_or_else(|| {
                    anyhow!("git status named the rename to {path} without its source")
                })?;
            Some(text(source)?)
        } else {
            None
        };
        entries.push(Entry { code, path, from });
    }
    Ok(entries)
}

/// `git status` of the tree, every untracked file by name, and with `ignored` the ignored
/// paths too, an ignored directory as itself.
fn status(tree: &Path, ignored: bool) -> Result<Vec<Entry>> {
    let mut args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all"];
    if ignored {
        args.push("--ignored=matching");
    }
    parse_status(&raw(tree, &args, None)?)
}

/// The paths with uncommitted changes (staged, unstaged or untracked), relative to the tree, as
/// git names them: a rename names both its source and its target.
pub fn changed_paths(tree: &Path) -> Result<Vec<String>> {
    Ok(status(tree, false)?
        .into_iter()
        .flat_map(|entry| entry.from.into_iter().chain([entry.path]))
        .collect())
}

/// The uncommitted paths outside the cases directory, in path order: what makes a tree not
/// clean. Ignored files do not; they are no one's work in progress.
pub fn uncommitted(tree: &Path) -> Result<Vec<String>> {
    Ok(changed_paths(tree)?
        .into_iter()
        .filter(|path| !is_under_cases(path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// The ignored paths outside the cases directory, an ignored directory as itself (`target/`),
/// not file by file.
fn ignored(entries: &[Entry]) -> BTreeSet<String> {
    entries
        .iter()
        .filter(|entry| entry.code == "!!")
        .map(|entry| entry.path.clone())
        .filter(|path| !is_under_cases(path.trim_end_matches('/')))
        .collect()
}

/// The paths of `entries` that are uncommitted (not ignored) and outside the cases directory,
/// both sides of a rename.
fn dirty(entries: &[Entry]) -> BTreeSet<String> {
    entries
        .iter()
        .filter(|entry| entry.code != "!!")
        .flat_map(|entry| entry.from.iter().cloned().chain([entry.path.clone()]))
        .filter(|path| !is_under_cases(path))
        .collect()
}

/// Where a repository configures the identity takt commits with, in git's configuration syntax:
/// `[identity]` with `name` and `email`, read at a revision ([`configured_identity`]).
pub const COMMIT_IDENTITY_FILE: &str = ".engineering/commit-identity.conf";

/// The author and committer of a commit takt makes ([`commit_paths`]): a name and an email,
/// each exactly as git records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitIdentity {
    /// The name, `release-bot[bot]` for one.
    pub name: String,
    /// The email, without its angle brackets.
    pub email: String,
}

impl CommitIdentity {
    /// takt's own identity, `takt <takt@localhost>`: what a commit carries when no identity is
    /// configured and none is required.
    #[must_use]
    pub fn takt() -> Self {
        Self {
            name: "takt".to_owned(),
            email: "takt@localhost".to_owned(),
        }
    }

    /// The identity `name <email>`, refused when git would not record it exactly: either part
    /// empty, holding `<`, `>` or a control character, starting or ending with a character git
    /// strips there (white space, `.`, `,`, `:`, `;`, `"`, `\`, `'`), or an email holding white
    /// space.
    pub fn new(name: &str, email: &str) -> Result<Self> {
        fn check(part: &str, what: &str) -> Result<()> {
            let stripped = |c: char| c <= ' ' || ".,:;<>\"\\'".contains(c);
            if part.is_empty() {
                bail!("the {what} is empty");
            }
            if part.contains(['<', '>']) || part.chars().any(char::is_control) {
                bail!("the {what} `{part}` holds `<`, `>` or a control character");
            }
            if part.starts_with(stripped) || part.ends_with(stripped) {
                bail!("the {what} `{part}` starts or ends with a character git strips there");
            }
            Ok(())
        }
        check(name, "name")?;
        check(email, "email")?;
        if email.chars().any(char::is_whitespace) {
            bail!("the email `{email}` holds white space");
        }
        Ok(Self {
            name: name.to_owned(),
            email: email.to_owned(),
        })
    }

    /// Reads `name <email>`, as git prints an identity and `--commit-identity` takes one.
    pub fn parse(text: &str) -> Result<Self> {
        let refused = || anyhow!("`{text}` is no identity: give it as `<name> <email>`");
        let inner = text.strip_suffix('>').ok_or_else(refused)?;
        let (name, email) = inner.rsplit_once(" <").ok_or_else(refused)?;
        Self::new(name, email).with_context(refused)
    }
}

impl std::fmt::Display for CommitIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <{}>", self.name, self.email)
    }
}

impl std::str::FromStr for CommitIdentity {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        Self::parse(text)
    }
}

/// The refusal of a run whose commits leave the machine when `revision`, where the identity was
/// read, configures none and none was given: it names the file, its keys and the flag.
#[must_use]
pub fn no_identity(revision: &str) -> anyhow::Error {
    anyhow!(
        "no commit identity: {revision} holds no {COMMIT_IDENTITY_FILE} giving `identity.name` and `identity.email`, and no --commit-identity '<name> <email>' was given; this run's commits leave the machine, and takt pushes none as {} or under the operator's git configuration; nothing was written",
        CommitIdentity::takt()
    )
}

/// The identity [`COMMIT_IDENTITY_FILE`] names at `revision`, read without a checkout as
/// [`config_at`] reads a file: `None` when `revision` holds no such file, or names no commit.
/// A file that does not give `identity.name` and `identity.email` once each, gives any other
/// key, or names an identity git would not record exactly ([`CommitIdentity::new`]) is refused,
/// naming the file and the key; it is never read as none.
pub fn configured_identity(tree: &Path, revision: &str) -> Result<Option<CommitIdentity>> {
    let Some(entries) = config_at(tree, revision, COMMIT_IDENTITY_FILE)? else {
        return Ok(None);
    };
    let at = format!("{revision}:{COMMIT_IDENTITY_FILE}");
    let (mut name, mut email) = (None, None);
    for (key, value) in entries {
        let slot = match key.as_str() {
            "identity.name" => &mut name,
            "identity.email" => &mut email,
            other => bail!(
                "{at} gives `{other}`, which takt does not read; it gives `identity.name` and `identity.email`"
            ),
        };
        let value = value.ok_or_else(|| anyhow!("{at} gives `{key}` no value"))?;
        if slot.replace(value).is_some() {
            bail!("{at} gives `{key}` twice");
        }
    }
    let missing =
        |key: &str| anyhow!("{at} gives no `{key}`; it gives `identity.name` and `identity.email`");
    let name = name.ok_or_else(|| missing("identity.name"))?;
    let email = email.ok_or_else(|| missing("identity.email"))?;
    CommitIdentity::new(&name, &email)
        .with_context(|| {
            format!(
                "{at} names no identity git records as given (`identity.name`, `identity.email`)"
            )
        })
        .map(Some)
}

/// The paths, NUL-separated, for `--pathspec-from-file=- --pathspec-file-nul`.
fn pathspec(paths: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
    }
    bytes
}

/// Commits exactly `paths`, as they are in the work tree (a removed path as its removal), with
/// `message`, and nothing else: whatever else the index holds stays staged and uncommitted.
/// With no path the commit is empty. The paths are literal, never patterns, and are passed on
/// standard input, so no count or name of them is too many for a command line. Answers the
/// new `HEAD`. A nested repository among `paths`, which git would record as a gitlink `HEAD`
/// does not have, is refused and nothing committed ([`new_gitlinks`]).
///
/// `identity` is the commit's author and committer both, set as `GIT_AUTHOR_NAME`,
/// `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME` and `GIT_COMMITTER_EMAIL` in the commit's own
/// environment, which outrank every `user.*`, `author.*` and `committer.*` of git's
/// configuration and replace the same variables in takt's environment: nothing of the
/// operator's identity reaches the commit. Every commit takt makes is made here.
pub fn commit_paths(
    tree: &Path,
    identity: &CommitIdentity,
    paths: &[String],
    message: &str,
) -> Result<String> {
    let env = [
        ("GIT_AUTHOR_NAME", identity.name.as_str()),
        ("GIT_AUTHOR_EMAIL", identity.email.as_str()),
        ("GIT_COMMITTER_NAME", identity.name.as_str()),
        ("GIT_COMMITTER_EMAIL", identity.email.as_str()),
    ];
    let file = MessageFile::write(tree, message)?;
    let named = file.path.to_string_lossy().into_owned();
    let mut commit: Vec<&str> = vec!["commit", "-q", "--only", "-F", &named];
    if paths.is_empty() {
        commit.push("--allow-empty");
        raw_with(tree, &commit, None, &env)?;
    } else {
        let list = pathspec(paths);
        let from_input = ["--pathspec-from-file=-", "--pathspec-file-nul"];
        let mut add = vec!["--literal-pathspecs", "add", "-A"];
        add.extend(from_input);
        raw(tree, &add, Some(&list))?;
        let gitlinks = new_gitlinks(tree, paths)?;
        if !gitlinks.is_empty() {
            bail!(
                "not committed: {} would be recorded as a gitlink, a nested repository's commit, which HEAD does not have",
                gitlinks.join(", ")
            );
        }
        let mut literal = vec!["--literal-pathspecs"];
        literal.extend(commit);
        literal.extend(from_input);
        raw_with(tree, &literal, Some(&list), &env)?;
    }
    head(tree)
}

/// Writes the commit of the tree object `tree` over `parents`, in that order, saying `message`,
/// under `identity` as author and committer, as [`commit_paths`] sets them, into the repository
/// at `repository` (a bare one among them); the new commit's id. No ref moves: the caller moves
/// the one it means to.
pub fn commit_tree(
    repository: &Path,
    tree: &str,
    parents: &[&str],
    message: &str,
    identity: &CommitIdentity,
) -> Result<String> {
    let env = [
        ("GIT_AUTHOR_NAME", identity.name.as_str()),
        ("GIT_AUTHOR_EMAIL", identity.email.as_str()),
        ("GIT_COMMITTER_NAME", identity.name.as_str()),
        ("GIT_COMMITTER_EMAIL", identity.email.as_str()),
    ];
    let file = MessageFile::write(repository, message)?;
    let named = file.path.to_string_lossy().into_owned();
    let mut args = vec!["commit-tree", tree];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    args.extend(["-F", &named]);
    let stdout = raw_with(repository, &args, None, &env)?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

/// A commit message in a file of the repository's own git directory, which `git commit -F` reads:
/// the message never rides on a command line, whatever its length, and the file is removed when
/// this is dropped.
struct MessageFile {
    path: PathBuf,
}

impl MessageFile {
    fn write(tree: &Path, message: &str) -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let git_dir = PathBuf::from(run(tree, &["rev-parse", "--absolute-git-dir"])?);
        let path = git_dir.join(format!(
            "takt-commit-message.{}.{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, message)
            .with_context(|| format!("write the commit message to {}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for MessageFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The paths among `paths`, or under one of them, that the index holds as a gitlink (mode
/// `160000`) where `HEAD` holds none: a nested repository `add` recorded. None when there is no
/// `HEAD` to compare with.
fn new_gitlinks(tree: &Path, paths: &[String]) -> Result<Vec<String>> {
    if resolve(tree, "HEAD")?.is_none() {
        return Ok(Vec::new());
    }
    let listed = raw(
        tree,
        &[
            "diff-index",
            "--cached",
            "--raw",
            "--no-renames",
            "-z",
            "HEAD",
        ],
        None,
    )?;
    let listed = String::from_utf8_lossy(&listed);
    let mut tokens = listed.split('\0');
    let mut found = Vec::new();
    while let Some(meta) = tokens.next() {
        let Some(path) = tokens.next() else { break };
        let fields: Vec<&str> = meta.trim_start_matches(':').split_whitespace().collect();
        let (Some(old), Some(new)) = (fields.first(), fields.get(1)) else {
            continue;
        };
        let given = paths.iter().any(|given| {
            let given = given.trim_end_matches('/');
            path == given || path.starts_with(&format!("{given}/"))
        });
        if *new == "160000" && *old != "160000" && given {
            found.push(path.to_owned());
        }
    }
    Ok(found)
}

/// Discards every uncommitted change but what is under the cases directory: the index and the
/// tracked files back to `HEAD`, untracked files removed; ignored files stay. For a tree that was
/// clean when the work being discarded began, so that everything uncommitted is that work's.
pub fn discard_uncommitted(tree: &Path) -> Result<()> {
    run(tree, &["reset", "-q", "--hard", "HEAD"])?;
    clean_untracked(tree)
}

/// `git clean -fdq` with each cases root kept (the new one and the legacy one): the untracked
/// files a case holds are the case's, and a `.takt/.gitignore` that un-ignores them leaves them
/// untracked and in reach of a clean.
fn clean_untracked(tree: &Path) -> Result<()> {
    let keep: Vec<String> = CASE_ROOTS.iter().map(|root| format!("{root}/")).collect();
    let mut args = vec!["clean", "-fdq"];
    for pattern in &keep {
        args.extend(["-e", pattern.as_str()]);
    }
    run(tree, &args)?;
    Ok(())
}

/// The largest file whose bytes a checkpoint keeps; a larger one is compared by its size, mode
/// and modification time and cannot be put back.
const KEPT_AT_MOST: u64 = 64 * 1024 * 1024;

/// What is at one path: nothing, a file with its bytes and whether it is executable, a link with
/// its target, a directory taken as itself, or something never opened (a FIFO, a socket, a
/// device, a file that cannot be read or is too large to keep), compared by its kind and
/// [`Stamp`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum Content {
    Absent,
    File { bytes: Vec<u8>, executable: bool },
    Link(PathBuf),
    Dir,
    Opaque { what: &'static str, stamp: Stamp },
}

/// What a path that is never opened is compared by: its size, mode and modification time,
/// which a step can set back, and its change time, device and inode, which it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    mode: u32,
    modified: Option<std::time::SystemTime>,
    changed: (i64, i64),
    inode: (u64, u64),
}

impl Stamp {
    #[cfg(unix)]
    fn of(meta: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;
        Self {
            len: meta.len(),
            mode: mode(meta),
            modified: meta.modified().ok(),
            changed: (meta.ctime(), meta.ctime_nsec()),
            inode: (meta.dev(), meta.ino()),
        }
    }

    #[cfg(not(unix))]
    fn of(meta: &std::fs::Metadata) -> Self {
        Self {
            len: meta.len(),
            mode: mode(meta),
            modified: meta.modified().ok(),
            changed: (0, 0),
            inode: (0, 0),
        }
    }
}

impl Content {
    /// What is at `path` now; a link is read as the link, never followed; nothing but a regular
    /// file is ever opened, and only one small enough to keep and readable.
    fn of(path: &Path) -> Result<Self> {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.kind() == std::io::ErrorKind::NotADirectory =>
            {
                return Ok(Self::Absent);
            }
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        let kind = meta.file_type();
        let opaque = |what| Self::Opaque {
            what,
            stamp: Stamp::of(&meta),
        };
        if kind.is_symlink() {
            let target =
                std::fs::read_link(path).with_context(|| format!("read {}", path.display()))?;
            return Ok(Self::Link(target));
        }
        if kind.is_dir() {
            return Ok(Self::Dir);
        }
        if !kind.is_file() {
            return Ok(opaque("not a file (a FIFO, a socket or a device)"));
        }
        if meta.len() > KEPT_AT_MOST {
            return Ok(opaque("a file too large to keep"));
        }
        match std::fs::read(path) {
            Ok(bytes) => Ok(Self::File {
                bytes,
                executable: mode(&meta) & 0o111 != 0,
            }),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                Ok(opaque("a file that cannot be read"))
            }
            Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
        }
    }

    /// Whether the bytes are kept, so that the path can be made again.
    fn can_be_put_back(&self) -> bool {
        !matches!(self, Self::Opaque { .. })
    }

    /// Puts `path` back to this: nothing when it is this already (a directory is never emptied
    /// to be made again); else whatever is there now goes (a link as the link, a directory with
    /// what is in it), then the file, link or directory is made again. Something never opened
    /// cannot be made again: putting a path back to one is an error once the path is cleared.
    fn put_back(&self, path: &Path) -> Result<()> {
        if Self::of(path)? == *self {
            return Ok(());
        }
        remove(path)?;
        if matches!(self, Self::Absent) {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            // A parent the step turned into a file or a link goes first.
            if std::fs::symlink_metadata(parent).is_ok_and(|meta| !meta.is_dir()) {
                std::fs::remove_file(parent)
                    .with_context(|| format!("remove {}", parent.display()))?;
            }
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        match self {
            Self::Absent => {}
            Self::File { bytes, executable } => {
                std::fs::write(path, bytes)
                    .with_context(|| format!("put back {}", path.display()))?;
                set_executable(path, *executable)?;
            }
            Self::Link(target) => make_link(target, path)?,
            Self::Dir => std::fs::create_dir_all(path)
                .with_context(|| format!("put back {}", path.display()))?,
            Self::Opaque { what, .. } => bail!(
                "{} was {what} before the step; nothing of it was kept to put back",
                path.display()
            ),
        }
        Ok(())
    }
}

/// Removes whatever is at `path`: a link as the link, a directory with everything in it (made
/// listable first, should the step have taken that away), anything else as itself.
fn remove(path: &Path) -> Result<()> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if meta.is_dir() {
        let _ = make_listable(path);
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .with_context(|| format!("remove {}", path.display()))
}

#[cfg(unix)]
fn mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode()
}

#[cfg(not(unix))]
fn mode(_: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn make_listable(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn make_listable(_: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("set the mode of {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = if executable { 0o755 } else { 0o644 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("set the mode of {}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_: &Path, _: bool) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn make_link(target: &Path, at: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, at)
        .with_context(|| format!("put back the link {}", at.display()))
}

#[cfg(not(unix))]
fn make_link(target: &Path, at: &Path) -> Result<()> {
    bail!(
        "the link {} to {} cannot be put back on this platform",
        at.display(),
        target.display()
    )
}

/// The entries of a git directory whose files git runs, reads as configuration, or reads to
/// find the repository's objects and history: the hooks, `info/` (attributes, exclude,
/// sparse-checkout), a linked worktree's `commondir` and `gitdir` (where its common git
/// directory is), `shallow` (which commits have no parents), `objects/info/alternates` (other
/// object directories read as this one's) and the commit-graph, single or split
/// (`objects/info/commit-graph`, `objects/info/commit-graphs/`), a cache of the history. Every
/// other file there (git's own state, other tools' files and locks, an editor's entries) is not
/// the step's concern.
const GUARDED_ENTRIES: [&str; 8] = [
    "hooks",
    "info",
    "commondir",
    "gitdir",
    "shallow",
    "objects/info/alternates",
    "objects/info/commit-graph",
    "objects/info/commit-graphs",
];

/// The configuration files of a git directory, guarded by the keys in them that run code or
/// change what git does ([`is_guarded_key`]).
const CONFIG_FILES: [&str; 2] = ["config", "config.worktree"];

/// The other worktrees' git directories under the shared git directory `dir`, by path relative
/// to it (`worktrees/<name>`), but `own`, the tree's own, which is guarded by itself.
fn siblings(dir: &Path, own: &Path) -> Vec<PathBuf> {
    let Ok(listing) = std::fs::read_dir(dir.join("worktrees")) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = listing
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path != own && path.is_dir())
        .filter_map(|path| path.strip_prefix(dir).ok().map(Path::to_path_buf))
        .collect();
    found.sort();
    found
}

/// The guarded entries of the git directory `dir`, relative to it ([`GUARDED_ENTRIES`]), and
/// each sibling worktree's `info/` (its sparse-checkout).
fn guarded_roots(dir: &Path, own: &Path) -> Vec<PathBuf> {
    GUARDED_ENTRIES
        .iter()
        .map(PathBuf::from)
        .chain(
            siblings(dir, own)
                .into_iter()
                .map(|sibling| sibling.join("info")),
        )
        .collect()
}

/// The configuration files of the git directory `dir`, relative to it: its own, and each
/// sibling worktree's `config.worktree`.
fn config_files(dir: &Path, own: &Path) -> Vec<PathBuf> {
    CONFIG_FILES
        .iter()
        .map(PathBuf::from)
        .chain(
            siblings(dir, own)
                .into_iter()
                .map(|sibling| sibling.join("config.worktree")),
        )
        .collect()
}

/// Whether the configuration key `key`, as `git config --list` writes it (section and name in
/// lower case), runs code or changes what git does: `core.*`, `filter.*`, `diff.*`,
/// `merge.*`, `alias.*`, `include.*`, `includeIf.*`, `credential.*`, `remote.<name>.*`,
/// `url.*`, `gpg.*`, `sequence.*`, `pager.*`, `trailer.*`, `difftool.*`, `mergetool.*`,
/// `submodule.*`, `extensions.*`; and what changes what a push sends, where it goes or how a
/// commit records its message: `push.*`, `http.*`, `i18n.*`. Any other key (`branch.<name>.vscode-merge-base`, a tool's
/// own section) is not.
fn is_guarded_key(key: &str) -> bool {
    let section = key.split('.').next().unwrap_or_default();
    match section {
        "core" | "filter" | "diff" | "merge" | "alias" | "include" | "includeif" | "credential"
        | "url" | "gpg" | "sequence" | "pager" | "trailer" | "difftool" | "mergetool"
        | "submodule" | "extensions" | "push" | "http" | "i18n" => true,
        "remote" => key.split('.').count() >= 3,
        _ => false,
    }
}

/// A configuration file as the guard sees it: its bytes, to put back, and its guarded entries,
/// to compare; `None` when git cannot read it as configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    content: Content,
    guarded: Option<Vec<(String, Option<String>)>>,
}

impl Config {
    fn of(tree: &Path, path: &Path) -> Result<Self> {
        let content = Content::of(path)?;
        let guarded = match &content {
            Content::Absent => Some(Vec::new()),
            Content::File { .. } => {
                let file = path.to_string_lossy();
                let listed = output(
                    tree,
                    &["config", "--file", &file, "--no-includes", "--list", "-z"],
                    None,
                )?;
                listed.status.success().then(|| {
                    listed
                        .stdout
                        .split(|byte| *byte == 0)
                        .filter(|entry| !entry.is_empty())
                        .map(|entry| {
                            let entry = String::from_utf8_lossy(entry);
                            match entry.split_once('\n') {
                                Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
                                None => (entry.into_owned(), None),
                            }
                        })
                        .filter(|(key, _)| is_guarded_key(key))
                        .collect()
                })
            }
            _ => None,
        };
        Ok(Self { content, guarded })
    }

    /// Whether `after` differs from this in what git runs or does.
    fn differs(&self, after: &Self) -> bool {
        self.guarded.is_none() || after.guarded.is_none() || self.guarded != after.guarded
    }
}

/// What is under the guarded directories of a git directory: every entry but a directory, and
/// every directory with its mode, by path relative to the git directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Guarded {
    files: BTreeMap<PathBuf, Content>,
    dirs: BTreeMap<PathBuf, u32>,
}

/// Every entry under the directories `roots` of the git directory `dir`. A directory that
/// cannot be listed is an entry of its own, never opened.
fn guarded(dir: &Path, roots: &[PathBuf]) -> Result<Guarded> {
    fn walk(root: &Path, dir: &Path, found: &mut Guarded) -> Result<()> {
        let relative = |path: &Path| path.strip_prefix(root).unwrap_or(path).to_path_buf();
        if let Ok(meta) = std::fs::symlink_metadata(dir) {
            found.dirs.insert(relative(dir), mode(&meta));
        }
        let entries = &mut found.files;
        let listing = match std::fs::read_dir(dir) {
            Ok(listing) => listing,
            Err(_) => {
                let meta = std::fs::symlink_metadata(dir)
                    .with_context(|| format!("read {}", dir.display()))?;
                entries.insert(
                    relative(dir),
                    Content::Opaque {
                        what: "a directory that cannot be listed",
                        stamp: Stamp::of(&meta),
                    },
                );
                return Ok(());
            }
        };
        let mut subdirs = Vec::new();
        for entry in listing {
            let path = entry?.path();
            let content = Content::of(&path)?;
            if content == Content::Dir {
                subdirs.push(path);
            } else {
                found.files.insert(relative(&path), content);
            }
        }
        for sub in subdirs {
            walk(root, &sub, found)?;
        }
        Ok(())
    }
    let mut found = Guarded::default();
    for name in roots {
        let sub = dir.join(name);
        match Content::of(&sub)? {
            Content::Dir => walk(dir, &sub, &mut found)?,
            Content::Absent => {}
            other => {
                found.files.insert(name.clone(), other);
            }
        }
    }
    Ok(found)
}

/// Makes every directory under `roots` of the git directory `dir` listable and writable, so
/// that what a step put there can be removed and what it removed written again; answers the
/// mode each had, by path relative to `dir`. A directory that cannot be made so is left as it
/// is; putting back below it then fails and says so.
fn unlock(dir: &Path, roots: &[PathBuf]) -> BTreeMap<PathBuf, u32> {
    fn walk(root: &Path, dir: &Path, modes: &mut BTreeMap<PathBuf, u32>) {
        let Ok(meta) = std::fs::symlink_metadata(dir) else {
            return;
        };
        if !meta.is_dir() {
            return;
        }
        let relative = dir.strip_prefix(root).unwrap_or(dir).to_path_buf();
        modes.insert(relative, mode(&meta));
        let _ = make_listable(dir);
        let Ok(listing) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in listing.flatten() {
            let path = entry.path();
            if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir()) {
                walk(root, &path, modes);
            }
        }
    }
    let mut modes = BTreeMap::new();
    for name in roots {
        walk(dir, &dir.join(name), &mut modes);
    }
    modes
}

/// The guarded part of one git directory: its [`GUARDED_ENTRIES`] (and its sibling worktrees'
/// `info/`) file by file with every directory's mode, and its configuration files (and its
/// sibling worktrees' `config.worktree`).
#[derive(Debug, Clone)]
struct GitDir {
    dir: PathBuf,
    own: PathBuf,
    roots: Vec<PathBuf>,
    guarded: Guarded,
    configs: Vec<(PathBuf, Config)>,
}

impl GitDir {
    /// The guarded part of `dir`; `own` is the tree's own git directory, guarded by itself.
    fn of(tree: &Path, dir: &Path, own: &Path) -> Result<Self> {
        let roots = guarded_roots(dir, own);
        let configs = config_files(dir, own)
            .into_iter()
            .map(|name| {
                let path = dir.join(name);
                Config::of(tree, &path).map(|config| (path, config))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            dir: dir.to_path_buf(),
            own: own.to_path_buf(),
            guarded: guarded(dir, &roots)?,
            roots,
            configs,
        })
    }
}

/// What putting the guarded part of the git directories back found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitDirPutBack {
    /// The paths the step changed, as `.git/<path>` for the tree's own git directory.
    pub touched: Vec<String>,
    /// The paths that could not be put back, each with why.
    pub failed: Vec<String>,
}

/// The tree as it was before a judgement step or a validator's observation: what its change is
/// measured against and what a refused or failed one is put back to. `HEAD` and its branch,
/// and every other ref of the repository (the case's base, a sibling case's branch, a tag, a
/// remote-tracking ref) but a branch checked out in another worktree, which is that session's
/// ([`unguarded_refs`]), the case's base and its own branch excepted
/// ([`Checkpoint::take_guarding`]); the index's entries
/// and the index file itself; every uncommitted path outside the cases directory with what was
/// at it (a validator's run may leave one); the ignored paths with what is at each a file git
/// names, and below each ignored directory every path by its kind and stamp; and what git runs,
/// reads as configuration, or reads to find the objects and the history in the git
/// directories: the hooks, `info/`, the configuration's keys, `commondir`, `gitdir`, `shallow`,
/// `objects/info/alternates` and the commit-graph. Beside them, where discarding the step moves
/// what it rewrote inside an ignored directory ([`Checkpoint::setting_aside_in`]).
#[derive(Debug, Clone)]
pub struct Checkpoint {
    head: String,
    branch: Option<String>,
    refs: BTreeMap<String, String>,
    unguarded: BTreeSet<String>,
    index: Vec<u8>,
    index_file: Option<(PathBuf, Vec<u8>)>,
    dirty: BTreeMap<String, Content>,
    ignored: BTreeMap<OsString, Content>,
    dot_git: Content,
    head_file: (PathBuf, Content),
    index_lock: (PathBuf, bool),
    git_dirs: Vec<GitDir>,
    aside: Option<Aside>,
}

/// What a judgement step did to the tree, against its [`Checkpoint`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepChange {
    /// The paths outside the cases directory whose content the step changed, in path order:
    /// what an accepted step commits.
    pub paths: Vec<String>,
    /// Those of [`Self::paths`] that were uncommitted before the step already.
    pub already_uncommitted: Vec<String>,
    /// Whether the index differs: the step staged, unstaged or flagged something.
    pub staged: bool,
    /// How `HEAD` moved, when it did: a commit, a reset or a checkout of the step's own.
    pub head: Option<String>,
    /// The guarded refs the step moved, made or deleted, each with how.
    pub refs: Vec<String>,
    /// Ignored paths the step created, inside an ignored directory that was there before it too;
    /// each path as the file system holds it ([`shown`] names it).
    pub ignored_added: Vec<OsString>,
    /// Ignored files git names, and paths inside an ignored directory, that the step rewrote.
    pub ignored_changed: Vec<OsString>,
    /// Ignored paths the step removed, inside an ignored directory too.
    pub ignored_removed: Vec<OsString>,
    /// Those of the rewritten or removed ignored paths of which nothing was kept to put back (a
    /// whole ignored directory, a path inside one, a file too large to keep or unreadable).
    pub ignored_lost: Vec<OsString>,
    /// Those of the rewritten files inside an ignored directory that discarding the step moves
    /// out of the tree, into [`Checkpoint::aside`] at the same relative path: nothing of them
    /// was kept to put back, and nothing the operator had is deleted.
    pub ignored_set_aside: Vec<OsString>,
}

/// The index's entries: mode, object, stage, the assume-unchanged and skip-worktree flags, path.
fn index_entries(tree: &Path) -> Result<Vec<u8>> {
    raw(tree, &["ls-files", "-z", "--stage", "-v"], None)
}

fn short(revision: &str) -> &str {
    &revision[..8.min(revision.len())]
}

/// The commit `reference` names; `None` when it names nothing.
fn resolve(tree: &Path, reference: &str) -> Result<Option<String>> {
    let found = output(tree, &["rev-parse", "--verify", "-q", reference], None)?;
    Ok(found
        .status
        .success()
        .then(|| String::from_utf8_lossy(&found.stdout).trim().to_owned()))
}

/// The refs a checkpoint leaves out of its guard, read when it is taken: `branch`, `HEAD`'s own,
/// which is guarded as `HEAD`, and every branch checked out in a worktree of the repository
/// (`git worktree list --porcelain`), which is another session's work: putting it back would
/// destroy that work, as putting back other tools' files under `.git` would. The branches
/// `kept` (the case's base) stay guarded wherever they are checked out.
fn unguarded_refs(tree: &Path, branch: Option<&str>, kept: &[String]) -> Result<BTreeSet<String>> {
    let listed = raw(tree, &["worktree", "list", "--porcelain"], None)?;
    let listed =
        String::from_utf8(listed).map_err(|_| anyhow!("git worktree list is not in UTF-8"))?;
    let kept: BTreeSet<String> = kept
        .iter()
        .map(|name| format!("refs/heads/{name}"))
        .collect();
    let mut refs: BTreeSet<String> = listed
        .lines()
        .filter_map(|line| line.strip_prefix("branch "))
        .filter(|reference| !kept.contains(*reference))
        .map(str::to_owned)
        .collect();
    refs.extend(branch.map(|name| format!("refs/heads/{name}")));
    Ok(refs)
}

/// The refs a checkpoint guards beside `HEAD`: every ref of the repository, branches, tags and
/// remote-tracking refs (what the scan before a push reads as what a remote holds) and any other
/// (`refs/notes/`, `refs/replace/`, a ref named like another, which git's lookup of a short
/// name would find), each with the object it names, but those `unguarded` names
/// ([`unguarded_refs`]).
fn guarded_refs(tree: &Path, unguarded: &BTreeSet<String>) -> Result<BTreeMap<String, String>> {
    let listed = raw(
        tree,
        &["for-each-ref", "--format=%(objectname) %(refname)"],
        None,
    )?;
    let listed = String::from_utf8(listed).map_err(|_| anyhow!("git named a ref not in UTF-8"))?;
    let mut refs = BTreeMap::new();
    for line in listed.lines().filter(|line| !line.is_empty()) {
        let (object, reference) = line
            .split_once(' ')
            .ok_or_else(|| anyhow!("git for-each-ref gave a line it does not document: {line}"))?;
        if !unguarded.contains(reference) {
            refs.insert(reference.to_owned(), object.to_owned());
        }
    }
    Ok(refs)
}

/// What is at each ignored path of `entries`: the content of a file git names, a directory as
/// itself and, since git names an ignored directory as one entry, every path below it as well
/// ([`below`]). Each path is kept as the bytes the file system holds, a name that is not UTF-8
/// among them, so that what is found is what is put back.
fn ignored_contents(tree: &Path, entries: &[Entry]) -> Result<BTreeMap<OsString, Content>> {
    let mut found = BTreeMap::new();
    for path in ignored(entries) {
        let path = OsString::from(path);
        let content = Content::of(&at(tree, &path))?;
        // A directory that holds a cases root is kept as what is in it, never as a whole: the
        // step's own files below it are put back or removed, the cases root is not. The files
        // directly in it are kept as their bytes, since git names none of them when the
        // directory is ignored as a whole, so each can be put back or set aside as it was.
        if content == Content::Dir && holds_cases_root(&path.to_string_lossy()) {
            below(tree, &path, true, &mut found)?;
            continue;
        }
        if content == Content::Dir {
            below(tree, &path, false, &mut found)?;
        }
        // A directory that cannot be listed was recorded as such by [`below`].
        found.entry(path).or_insert(content);
    }
    Ok(found)
}

/// The full path of `path`, relative to `tree`, a directory's `/` at its end dropped.
fn at(tree: &Path, path: &OsStr) -> PathBuf {
    tree.join(Path::new(path).components().as_path())
}

/// Whether `path`, as an ignored path is kept, names a directory: it ends in `/`.
fn names_dir(path: &OsStr) -> bool {
    path.as_encoded_bytes().ends_with(b"/")
}

/// How a refusal names a path: as text where it is UTF-8, each `\` doubled and each byte that is
/// not UTF-8 as `\x..`, so a name holding the text `\xff` and one holding the byte 0xff read
/// apart; each control character escaped and the home directory as `~`
/// ([`crate::trace::escape`]), so no name can shape a line of its own.
#[must_use]
pub fn shown(path: &OsStr) -> String {
    let mut text = String::new();
    for chunk in path.as_encoded_bytes().utf8_chunks() {
        text.push_str(&chunk.valid().replace('\\', "\\\\"));
        for byte in chunk.invalid() {
            text.push_str(&format!("\\x{byte:02x}"));
        }
    }
    crate::trace::escape(&text)
}

/// Each path below the ignored directory `dir` (relative to the tree, ending in `/`) outside the
/// cases directory, into `found`: a directory as itself, its path ending in `/`, and what is in
/// it in turn; a link as the link, never followed; anything else by its kind and [`Stamp`],
/// never opened, so that a large `target/` costs one `lstat` a path and nothing of it is kept to
/// put back. With `kept`, the regular files directly in `dir` are kept as their bytes instead,
/// as a directory that holds a cases root (`.takt`) keeps its own files. A directory that cannot
/// be listed is compared by its stamp alone.
fn below(
    tree: &Path,
    dir: &OsStr,
    kept: bool,
    found: &mut BTreeMap<OsString, Content>,
) -> Result<()> {
    let full = at(tree, dir);
    let listed = match std::fs::read_dir(&full) {
        Ok(listed) => listed,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let meta = std::fs::symlink_metadata(&full)
                .with_context(|| format!("read {}", full.display()))?;
            found.insert(
                dir.to_owned(),
                Content::Opaque {
                    what: "a directory that cannot be listed",
                    stamp: Stamp::of(&meta),
                },
            );
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("list {}", full.display())),
    };
    // A name is joined to `dir` with its `/`, which an ignored directory's name carries and a
    // name given without one, such as a link's, does not.
    let mut prefix = dir.to_owned();
    if !names_dir(&prefix) {
        prefix.push("/");
    }
    for entry in listed {
        let entry = entry.with_context(|| format!("list {}", full.display()))?;
        let mut path = prefix.clone();
        path.push(entry.file_name());
        // The cases directory's name is ASCII: a name that is not UTF-8 is never under it.
        if is_under_cases(&path.to_string_lossy()) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", entry.path().display()));
            }
        };
        let kind = meta.file_type();
        if kind.is_dir() {
            path.push("/");
            below(tree, &path, false, found)?;
            found.entry(path).or_insert(Content::Dir);
        } else if kind.is_symlink() {
            let target = std::fs::read_link(entry.path())
                .with_context(|| format!("read {}", entry.path().display()))?;
            found.insert(path, Content::Link(target));
        } else if kept && kind.is_file() {
            found.insert(path, Content::of(&entry.path())?);
        } else {
            let what = if kind.is_file() {
                "a file in an ignored directory"
            } else {
                "not a file (a FIFO, a socket or a device)"
            };
            found.insert(
                path,
                Content::Opaque {
                    what,
                    stamp: Stamp::of(&meta),
                },
            );
        }
    }
    Ok(())
}

/// What discarding a step did with the files it rewrote inside an ignored directory
/// ([`Checkpoint::restore`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetAside {
    /// The directory they were moved to, made new for this step; none when there was nothing to
    /// move or it could not be made.
    pub dir: Option<PathBuf>,
    /// The paths moved there, each at its own path relative to the tree under [`Self::dir`].
    pub moved: Vec<OsString>,
    /// The paths that could not be moved, each with why; they are as the step left them.
    pub failed: Vec<(OsString, String)>,
}

/// How many paths a refusal names before it counts the rest; a build inside an ignored
/// directory changes thousands.
const NAMED_AT_MOST: usize = 20;

/// How a refusal names paths: the first [`NAMED_AT_MOST`], each as [`shown`] names it, and how
/// many more.
#[must_use]
pub fn paths_named(paths: &[OsString]) -> String {
    let mut named = paths
        .iter()
        .take(NAMED_AT_MOST)
        .map(|path| shown(path))
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > NAMED_AT_MOST {
        named.push_str(&format!(" and {} more", paths.len() - NAMED_AT_MOST));
    }
    named
}

impl SetAside {
    /// Each of `paths` named as not moved, with `why`: what a discard says when it moves
    /// nothing because the tree could not be put back whole.
    #[must_use]
    pub fn not_moved(paths: &[OsString], why: &str) -> Self {
        Self {
            failed: paths
                .iter()
                .map(|path| (path.clone(), why.to_owned()))
                .collect(),
            ..Self::default()
        }
    }

    /// What every message about a discarded harness step adds for these files: where they went
    /// and which they are, and which could not be moved and why, each path and each reason
    /// escaped; nothing when there were none. The one place that says it, whichever way the
    /// step was discarded, and the error a restore that could not finish ends on carries it too.
    #[must_use]
    pub fn named(&self) -> String {
        let mut named = String::new();
        if let Some(dir) = &self.dir
            && !self.moved.is_empty()
        {
            named.push_str(&format!(
                "; moved out of the tree, not deleted, into {}: {}",
                shown(dir.as_os_str()),
                paths_named(&self.moved)
            ));
        }
        if !self.failed.is_empty() {
            let mut failed = self
                .failed
                .iter()
                .take(NAMED_AT_MOST)
                .map(|(path, why)| format!("{} ({})", shown(path), crate::trace::escape(why)))
                .collect::<Vec<_>>()
                .join(", ");
            if self.failed.len() > NAMED_AT_MOST {
                failed.push_str(&format!(" and {} more", self.failed.len() - NAMED_AT_MOST));
            }
            named.push_str(&format!(
                "; could not be moved out of the tree, left as the step left it: {failed}"
            ));
        }
        named
    }
}

/// `dir` resolved, when a set-aside root can be: absolute, outside `tree` (through any link on the
/// way to either), and with a directory, not a file, where its way ends among what exists. The
/// path returned holds no link: every link on the way that exists now is resolved, so a link
/// planted on it later is refused, never followed ([`DirFd::open`]). Nothing is made; the error
/// says which it is not.
pub fn set_aside_root(dir: &Path, tree: &Path) -> Result<PathBuf> {
    if !dir.is_absolute() {
        bail!(
            "the set-aside directory {} is not an absolute path",
            shown(dir.as_os_str())
        );
    }
    // The longest part of the way that exists, and the rest as it is.
    let mut rest = PathBuf::new();
    let mut base = dir;
    while std::fs::symlink_metadata(base).is_err() {
        match (base.parent(), base.file_name()) {
            (Some(parent), Some(name)) => {
                rest = if rest.as_os_str().is_empty() {
                    PathBuf::from(name)
                } else {
                    Path::new(name).join(rest)
                };
                base = parent;
            }
            _ => break,
        }
    }
    if !std::fs::metadata(base).is_ok_and(|meta| meta.is_dir()) {
        bail!(
            "the set-aside directory {} cannot be made: {} is not a directory",
            shown(dir.as_os_str()),
            shown(base.as_os_str())
        );
    }
    let resolved = std::fs::canonicalize(base)
        .with_context(|| format!("resolve {}", shown(base.as_os_str())))?
        .join(rest);
    let tree_resolved = std::fs::canonicalize(tree).unwrap_or_else(|_| tree.to_path_buf());
    if resolved.starts_with(&tree_resolved) || dir.starts_with(tree) {
        bail!(
            "the set-aside directory {} is inside the tree {}, where a later restore would remove what was moved",
            shown(dir.as_os_str()),
            shown(tree.as_os_str())
        );
    }
    Ok(resolved)
}

/// Where discarding a harness step moves what it rewrote: the root [`set_aside_root`] accepted,
/// resolved; the case's own directory under it; the step's directory under that, its name
/// before a `-2`, `-3` is put after it ([`fresh_dir`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Aside {
    root: PathBuf,
    case: OsString,
    step: String,
}

/// A directory, opened so that no link on the way to it was followed ([`Self::open`]): a harness
/// step's rewrites are moved through these, never through a path the step could have turned
/// into a link after it was checked.
#[cfg(unix)]
struct DirFd(std::os::fd::OwnedFd);

/// `name` as the C string a system call takes.
#[cfg(unix)]
fn c_name(name: &OsStr) -> std::io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt as _;
    std::ffi::CString::new(name.as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

#[cfg(unix)]
impl DirFd {
    /// `path`, absolute, opened from `/` one part at a time, each a directory opened without
    /// following a link (`O_NOFOLLOW`); a part that does not exist is made first (`mkdirat`,
    /// mode 0700) when `make`. A link, a file or anything else on the way is the error.
    fn open(path: &Path, make: bool) -> std::io::Result<Self> {
        use std::path::Component;
        let mut dir = Self::at(libc::AT_FDCWD, OsStr::new("/"))?;
        for part in path.components() {
            match part {
                Component::RootDir => {}
                Component::Normal(name) => dir = dir.child(name, make)?,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{} is not a plain absolute path", path.display()),
                    ));
                }
            }
        }
        Ok(dir)
    }

    /// The directory `name` in `fd`, opened without following a link.
    fn at(fd: libc::c_int, name: &OsStr) -> std::io::Result<Self> {
        use std::os::fd::FromRawFd as _;
        let name = c_name(name)?;
        // SAFETY: a NUL-terminated name and a descriptor (or `AT_FDCWD`) that outlive the call.
        let opened = unsafe {
            libc::openat(
                fd,
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if opened < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: a descriptor this call opened, owned by nothing else.
        Ok(Self(unsafe { std::os::fd::OwnedFd::from_raw_fd(opened) }))
    }

    fn raw(&self) -> libc::c_int {
        use std::os::fd::AsRawFd as _;
        self.0.as_raw_fd()
    }

    /// The directory `name` in this one, made first when it does not exist and `make`.
    fn child(&self, name: &OsStr, make: bool) -> std::io::Result<Self> {
        match Self::at(self.raw(), name) {
            Err(error) if make && error.kind() == std::io::ErrorKind::NotFound => {
                self.make(name)?;
                Self::at(self.raw(), name)
            }
            opened => opened,
        }
    }

    /// Makes the directory `name` in this one: true when this call made it, false when
    /// something was there already.
    fn make(&self, name: &OsStr) -> std::io::Result<bool> {
        let name = c_name(name)?;
        // SAFETY: a NUL-terminated name and a descriptor that outlive the call.
        if unsafe { libc::mkdirat(self.raw(), name.as_ptr(), 0o700) } == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Ok(false)
        } else {
            Err(error)
        }
    }

    /// Removes `name` in this one, a link as the link.
    fn unlink(&self, name: &OsStr) -> std::io::Result<()> {
        let name = c_name(name)?;
        // SAFETY: a NUL-terminated name and a descriptor that outlive the call.
        if unsafe { libc::unlinkat(self.raw(), name.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

/// `step`, or `step-2`, `step-3` and so on, in `case`: the first this call made, never one that
/// was there, so that no two steps ever share one; its name and the directory, opened.
#[cfg(unix)]
fn fresh_dir(case: &DirFd, step: &str) -> std::io::Result<(String, DirFd)> {
    for n in 1..=10_000_u32 {
        let name = if n == 1 {
            step.to_owned()
        } else {
            format!("{step}-{n}")
        };
        if case.make(OsStr::new(&name))? {
            let dir = case.child(OsStr::new(&name), false)?;
            return Ok((name, dir));
        }
    }
    Err(std::io::Error::other(format!(
        "no new directory beside {step} could be made"
    )))
}

/// Moves what is at `from` (a link as the link) to `name` in `dir`, never over anything there:
/// renamed without replacing, else copied ([`copy_into`]) when the two are on different file
/// systems.
#[cfg(unix)]
fn move_into(from: &Path, dir: &DirFd, name: &OsStr) -> Result<()> {
    let (source, target) = (c_name(from.as_os_str())?, c_name(name)?);
    match rename_no_replace(&source, dir, &target) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::EXDEV) => copy_into(from, dir, name),
        Err(error) => Err(error).with_context(|| format!("move {} aside", from.display())),
    }
}

/// Renames `from` to `name` in `dir`, failing when something is there, never replacing it:
/// `renameat2` with `RENAME_NOREPLACE`, else, where the file system or the kernel has no such
/// flag, a hard link (which fails on what is there) and then the unlink.
#[cfg(unix)]
fn rename_no_replace(
    from: &std::ffi::CStr,
    dir: &DirFd,
    name: &std::ffi::CStr,
) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: NUL-terminated names and a descriptor that outlive the call.
        let done = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                dir.raw(),
                name.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if done == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::EINVAL | libc::ENOSYS)) {
            return Err(error);
        }
    }
    // SAFETY: NUL-terminated names and a descriptor that outlive the call.
    if unsafe { libc::linkat(libc::AT_FDCWD, from.as_ptr(), dir.raw(), name.as_ptr(), 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a NUL-terminated name that outlives the call.
    if unsafe { libc::unlink(from.as_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Copies the regular file `from` to `name` in `dir`, which must not exist, then removes
/// `from`: the bytes synced to disk, their count checked against the source's length, its mode
/// and modification time kept. Anything but a regular file is refused, and so is a source that
/// became a link; a copy that fails is removed, never left half made.
#[cfg(unix)]
fn copy_into(from: &Path, dir: &DirFd, name: &OsStr) -> Result<()> {
    use std::os::fd::FromRawFd as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let meta =
        std::fs::symlink_metadata(from).with_context(|| format!("read {}", from.display()))?;
    if !meta.is_file() {
        bail!(
            "{} is not a regular file, and only a file is copied across file systems",
            from.display()
        );
    }
    let mut source = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(from)
        .with_context(|| format!("open {}", from.display()))?;
    let target_name = c_name(name)?;
    // SAFETY: a NUL-terminated name and a descriptor that outlive the call.
    let opened = unsafe {
        libc::openat(
            dir.raw(),
            target_name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    };
    if opened < 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("make the copy of {}", from.display()));
    }
    // SAFETY: a descriptor this call opened, owned by nothing else.
    let mut target = std::fs::File::from(unsafe { std::os::fd::OwnedFd::from_raw_fd(opened) });
    let copied = (|| -> Result<()> {
        let count = std::io::copy(&mut source, &mut target)?;
        if count != meta.len() {
            bail!("copied {count} bytes of {}", meta.len());
        }
        target.set_permissions(meta.permissions())?;
        if let Ok(modified) = meta.modified() {
            target.set_modified(modified)?;
        }
        target.sync_all()?;
        Ok(())
    })();
    drop(target);
    if let Err(error) = copied {
        let _ = dir.unlink(name);
        return Err(error.context(format!("copy {} aside", from.display())));
    }
    std::fs::remove_file(from).with_context(|| format!("remove {}", from.display()))
}

impl Checkpoint {
    /// Takes the checkpoint of `tree`, guarding `HEAD` and its branch and every other ref of the
    /// repository, replace refs and notes among them, but a branch another worktree checked out.
    pub fn take(tree: &Path) -> Result<Self> {
        Self::take_guarding(tree, &[])
    }

    /// Takes the checkpoint of `tree` as [`Self::take`] does, the branches `kept` (a case's
    /// base) guarded even when another worktree has them checked out.
    pub fn take_guarding(tree: &Path, kept: &[String]) -> Result<Self> {
        let git_dir = PathBuf::from(run(tree, &["rev-parse", "--absolute-git-dir"])?);
        let common = PathBuf::from(run(
            tree,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?);
        let mut git_dirs = vec![GitDir::of(tree, &common, &git_dir)?];
        if git_dir != common {
            git_dirs.push(GitDir::of(tree, &git_dir, &git_dir)?);
        }
        let head_path = git_dir.join("HEAD");
        let head_file = (head_path.clone(), Content::of(&head_path)?);
        let lock = git_dir.join("index.lock");
        let index_lock = (lock.clone(), std::fs::symlink_metadata(&lock).is_ok());
        // A `.git` directory is guarded through what is in it; a `.git` file (a linked
        // worktree's pointer to its git directory) by its bytes.
        let dot_git = Content::of(&tree.join(".git"))?;
        let index_path = git_dir.join("index");
        let index_file = match std::fs::read(&index_path) {
            Ok(bytes) => Some((index_path, bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", index_path.display()));
            }
        };
        let entries = status(tree, true)?;
        let dirty = dirty(&entries)
            .into_iter()
            .map(|path| Content::of(&tree.join(&path)).map(|content| (path, content)))
            .collect::<Result<_>>()?;
        let branch = current_branch(tree)?;
        let unguarded = unguarded_refs(tree, branch.as_deref(), kept)?;
        let refs = guarded_refs(tree, &unguarded)?;
        Ok(Self {
            head: head(tree)?,
            branch,
            refs,
            unguarded,
            index: index_entries(tree)?,
            index_file,
            dirty,
            ignored: ignored_contents(tree, &entries)?,
            aside: None,
            dot_git,
            head_file,
            index_lock,
            git_dirs,
        })
    }

    /// Puts what git runs or reads as configuration back as it was: the tree's `.git`; the
    /// tree's own `HEAD`, before anything here runs git, since a `HEAD` git cannot read makes
    /// the git directory no repository; an `index.lock` the step left (only a process the step
    /// started takes the tree's own index lock, and the step's processes are gone when the
    /// harness returns); the hooks and `info/` file by file, their directories made writable
    /// for it and given their modes back; a configuration file whose guarded keys changed byte
    /// for byte. Run before any other git call after the step: those would read and run what
    /// the step planted. Nothing here ends early: a path that cannot be read or put back is
    /// named in [`GitDirPutBack::failed`] and the rest is still put back.
    pub fn put_back_git_dir(&self, tree: &Path) -> GitDirPutBack {
        let mut found = GitDirPutBack::default();
        // Each path as [`shown`] names it: a name the step chose shapes no line of a refusal.
        let shown = |path: &Path| match path.strip_prefix(tree) {
            Ok(inside) => shown(inside.as_os_str()),
            Err(_) => shown(path.as_os_str()),
        };
        let dot_git = tree.join(".git");
        match Content::of(&dot_git) {
            Ok(now) if now == self.dot_git => {}
            Ok(_) => {
                found.touched.push(".git".to_owned());
                if self.dot_git == Content::Dir {
                    found
                        .failed
                        .push(".git: was a directory and is no longer one".to_owned());
                } else if let Err(error) = self.dot_git.put_back(&dot_git) {
                    found.failed.push(format!(".git: {error:#}"));
                }
            }
            Err(error) => {
                found.touched.push(".git".to_owned());
                found.failed.push(format!(".git: {error:#}"));
            }
        }
        let (head_path, head_before) = &self.head_file;
        if Content::of(head_path).ok().as_ref() != Some(head_before) {
            found.touched.push(shown(head_path));
            if let Err(error) = head_before.put_back(head_path) {
                found
                    .failed
                    .push(format!("{}: {error:#}", shown(head_path)));
            }
        }
        let (lock, existed) = &self.index_lock;
        if !existed && std::fs::symlink_metadata(lock).is_ok() {
            found.touched.push(shown(lock));
            if let Err(error) = remove(lock) {
                found.failed.push(format!("{}: {error:#}", shown(lock)));
            }
        }
        for before in &self.git_dirs {
            let mut roots = before.roots.clone();
            for root in guarded_roots(&before.dir, &before.own) {
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
            let modes = unlock(&before.dir, &roots);
            let after = match guarded(&before.dir, &roots) {
                Ok(after) => after,
                Err(error) => {
                    found.touched.push(shown(&before.dir));
                    found
                        .failed
                        .push(format!("{}: {error:#}", shown(&before.dir)));
                    continue;
                }
            };
            let differing: BTreeSet<&PathBuf> = before
                .guarded
                .files
                .keys()
                .chain(after.files.keys())
                .filter(|path| before.guarded.files.get(*path) != after.files.get(*path))
                .collect();
            for path in differing {
                let full = before.dir.join(path);
                found.touched.push(shown(&full));
                if let Err(error) = before
                    .guarded
                    .files
                    .get(path)
                    .unwrap_or(&Content::Absent)
                    .put_back(&full)
                {
                    found.failed.push(format!("{}: {error:#}", shown(&full)));
                }
            }
            // A directory the step made goes; one it made read-only or listable only gets its
            // mode back, now that what is in it is put back.
            for (path, mode) in modes.iter().rev() {
                let full = before.dir.join(path);
                match before.guarded.dirs.get(path) {
                    None => {
                        found.touched.push(shown(&full));
                        if let Err(error) = remove(&full) {
                            found.failed.push(format!("{}: {error:#}", shown(&full)));
                        }
                    }
                    Some(was) => {
                        if was != mode {
                            found.touched.push(shown(&full));
                        }
                        if let Err(error) = set_mode(&full, *was) {
                            found.failed.push(format!("{}: {error:#}", shown(&full)));
                        }
                    }
                }
            }
            // A directory the step removed was made again by putting its files back.
            for (path, was) in before.guarded.dirs.iter().rev() {
                if modes.contains_key(path) {
                    continue;
                }
                let full = before.dir.join(path);
                found.touched.push(shown(&full));
                if std::fs::symlink_metadata(&full).is_ok_and(|meta| meta.is_dir())
                    && let Err(error) = set_mode(&full, *was)
                {
                    found.failed.push(format!("{}: {error:#}", shown(&full)));
                }
            }
            let mut configs = before.configs.clone();
            for name in config_files(&before.dir, &before.own) {
                let path = before.dir.join(name);
                if !configs.iter().any(|(known, _)| *known == path) {
                    configs.push((
                        path,
                        Config {
                            content: Content::Absent,
                            guarded: Some(Vec::new()),
                        },
                    ));
                }
            }
            for (path, config) in &configs {
                let now = match Config::of(tree, path) {
                    Ok(now) => now,
                    Err(error) => {
                        found.touched.push(shown(path));
                        found.failed.push(format!("{}: {error:#}", shown(path)));
                        continue;
                    }
                };
                if config.differs(&now) {
                    found.touched.push(shown(path));
                    if let Err(error) = config.content.put_back(path) {
                        found.failed.push(format!("{}: {error:#}", shown(path)));
                    }
                }
            }
        }
        // What went wrong quotes paths too; escaped once more, which changes nothing already
        // escaped ([`crate::trace::escape`]).
        found.failed = found
            .failed
            .iter()
            .map(|failed| crate::trace::escape(failed))
            .collect();
        found
    }

    /// What the step did, against this checkpoint.
    pub fn change(&self, tree: &Path) -> Result<StepChange> {
        let head = head(tree)?;
        let branch = current_branch(tree)?;
        let moved = (head != self.head || branch != self.branch).then(|| {
            let on = |branch: &Option<String>| {
                branch
                    .as_deref()
                    .map_or_else(|| "detached".to_owned(), |name| format!("on {name}"))
            };
            format!(
                "from {} {} to {} {}",
                short(&self.head),
                on(&self.branch),
                short(&head),
                on(&branch)
            )
        });
        let now = guarded_refs(tree, &self.unguarded)?;
        let mut refs = Vec::new();
        for reference in self.refs.keys().chain(now.keys()).collect::<BTreeSet<_>>() {
            let (before, after) = (self.refs.get(reference), now.get(reference));
            if before != after {
                let at = |at: Option<&String>| at.map_or("nothing", |at| short(at)).to_owned();
                refs.push(format!("{reference} from {} to {}", at(before), at(after)));
            }
        }
        let entries = status(tree, true)?;
        let mut paths = BTreeSet::new();
        for path in dirty(&entries) {
            if !self.dirty.contains_key(&path) {
                paths.insert(path);
            }
        }
        for (path, before) in &self.dirty {
            if Content::of(&tree.join(path))? != *before {
                paths.insert(path.clone());
            }
        }
        let ignored = ignored_contents(tree, &entries)?;
        let mut change = StepChange {
            already_uncommitted: paths
                .iter()
                .filter(|path| self.dirty.contains_key(*path))
                .cloned()
                .collect(),
            paths: paths.into_iter().collect(),
            staged: index_entries(tree)? != self.index,
            head: moved,
            refs,
            ..StepChange::default()
        };
        for (path, content) in &ignored {
            match self.ignored.get(path) {
                None => change.ignored_added.push(path.clone()),
                Some(before) if before != content && *before != Content::Dir => {
                    change.ignored_changed.push(path.clone());
                    if !before.can_be_put_back() {
                        change.ignored_lost.push(path.clone());
                    }
                }
                Some(_) => {}
            }
        }
        for (path, before) in &self.ignored {
            if !ignored.contains_key(path) {
                change.ignored_removed.push(path.clone());
                if !before.can_be_put_back() || *before == Content::Dir {
                    change.ignored_lost.push(path.clone());
                }
            }
        }
        change.ignored_set_aside = self.set_aside_of(&ignored);
        Ok(change)
    }

    /// Where discarding the step moves what it rewrote inside an ignored directory
    /// ([`StepChange::ignored_set_aside`]): `<root>/<case>/<step>`, or, when something is there
    /// already as the step is discarded, `<step>-2`, `<step>-3` and so on, each made new for
    /// this step and never shared with another ([`fresh_dir`]). `root` is one
    /// [`set_aside_root`] returned; every directory from `/` down is opened without following a
    /// link ([`DirFd::open`]), so a link a step planted there since is refused, never followed.
    /// None unless given; nothing is moved then.
    #[must_use]
    pub fn setting_aside_in(mut self, root: PathBuf, case: OsString, step: String) -> Self {
        self.aside = Some(Aside { root, case, step });
        self
    }

    /// The files of `now`, inside an ignored directory of this checkpoint, that the step
    /// rewrote, of which nothing was kept: what discarding it moves aside, when it was given
    /// somewhere to move them ([`Self::setting_aside_in`]).
    fn set_aside_of(&self, now: &BTreeMap<OsString, Content>) -> Vec<OsString> {
        if self.aside.is_none() {
            return Vec::new();
        }
        now.iter()
            .filter(|(path, content)| {
                !names_dir(path)
                    && self.ignored.get(*path).is_some_and(|before| {
                        !before.can_be_put_back() && before != *content && self.is_inside_dir(path)
                    })
            })
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Moves each of `paths` ([`Self::restore`] answers them), relative to `tree`, to the same
    /// relative path under a directory made new for this step ([`fresh_dir`]): called last, once
    /// everything else of the step is put back, since their way runs through the tree. A file
    /// inside an ignored directory the step rewrote, of which nothing was kept, may decide the
    /// next test run (a build's output, a cache), so it leaves the tree; it may be the operator's
    /// own file the step only touched, so it is moved aside, never deleted. Every one is
    /// attempted whatever became of the one before; what could not be moved is named with why,
    /// and stays as the step left it. When
    /// the root or the case's directory is no longer a real directory (a link, a file), nothing
    /// is moved, and each file is named with that.
    #[cfg(unix)]
    #[must_use]
    pub fn set_aside(&self, tree: &Path, paths: &[OsString]) -> SetAside {
        let mut done = SetAside::default();
        let Some(aside) = &self.aside else {
            return done;
        };
        if paths.is_empty() {
            return done;
        }
        let case_path = aside.root.join(&aside.case);
        let step = DirFd::open(&aside.root, true)
            .map_err(|error| {
                format!(
                    "the set-aside directory {} is not a directory that can be reached without a link: {error}",
                    aside.root.display()
                )
            })
            .and_then(|root| {
                root.child(&aside.case, true).map_err(|error| {
                    format!(
                        "the set-aside directory {} is not a directory that can be reached without a link: {error}",
                        case_path.display()
                    )
                })
            })
            .and_then(|case| {
                fresh_dir(&case, &aside.step).map_err(|error| {
                    format!(
                        "no directory for the step could be made in {}: {error}",
                        case_path.display()
                    )
                })
            });
        let (name, step) = match step {
            Ok(step) => step,
            Err(why) => {
                done.failed = paths
                    .iter()
                    .map(|path| (path.clone(), why.clone()))
                    .collect();
                return done;
            }
        };
        for path in paths {
            let moved = (|| -> Result<()> {
                let relative = Path::new(path);
                let file = relative
                    .file_name()
                    .ok_or_else(|| anyhow!("{} names no file", relative.display()))?;
                let mut parent = DirFd::at(step.raw(), OsStr::new("."))?;
                if let Some(dirs) = relative.parent() {
                    for part in dirs.components() {
                        parent = parent.child(part.as_os_str(), true)?;
                    }
                }
                move_into(&at(tree, path), &parent, file)
            })();
            match moved {
                Ok(()) => done.moved.push(path.clone()),
                Err(error) => done.failed.push((path.clone(), format!("{error:#}"))),
            }
        }
        done.dir = Some(case_path.join(name));
        done
    }

    /// Without a file system that opens a directory without following a link, nothing is moved
    /// aside; each file is named with why and stays as the step left it.
    #[cfg(not(unix))]
    #[must_use]
    pub fn set_aside(&self, _tree: &Path, paths: &[OsString]) -> SetAside {
        SetAside {
            failed: paths
                .iter()
                .map(|path| {
                    (
                        path.clone(),
                        "moving a file aside needs a unix file system".to_owned(),
                    )
                })
                .collect(),
            ..SetAside::default()
        }
    }

    /// Puts the tree back to this checkpoint: everything the step did is discarded and
    /// everything that was there before it, uncommitted or not, is there again. What git runs or
    /// reads as configuration first (a path it could not put back was reported by
    /// [`Self::put_back_git_dir`] and is not again here); then `HEAD`, its branch and the
    /// guarded refs; the tracked files and the index to `HEAD`; untracked files and the ignored
    /// paths the step created removed, inside an ignored directory too; ignored files git names
    /// written again as they were; the paths uncommitted before written again as they were; the
    /// index file as it was. The cases directory is not touched here (it is the kernel's, put
    /// back by the run). Of an ignored directory, a file inside one, a file too large to keep or
    /// one that could not be read nothing is kept to put back. Checked afterwards: a tree that
    /// still differs is an error, not a refusal. Answers the files inside an ignored directory the
    /// step rewrote, of which nothing was kept, due to be moved aside ([`Self::set_aside`]) once
    /// everything else of the step is put back too; nothing is moved here.
    pub fn restore(&self, tree: &Path) -> Result<Vec<OsString>> {
        let _ = self.put_back_git_dir(tree);
        match &self.branch {
            Some(branch) => {
                let reference = format!("refs/heads/{branch}");
                if current_branch(tree)?.as_deref() != Some(branch.as_str()) {
                    run(tree, &["symbolic-ref", "HEAD", &reference])?;
                }
                if resolve(tree, &reference)?.as_deref() != Some(self.head.as_str()) {
                    run(tree, &["update-ref", &reference, &self.head])?;
                }
            }
            None => {
                if head(tree)? != self.head || current_branch(tree)?.is_some() {
                    run(tree, &["update-ref", "--no-deref", "HEAD", &self.head])?;
                }
            }
        }
        let now = guarded_refs(tree, &self.unguarded)?;
        for (reference, at) in &self.refs {
            if now.get(reference) != Some(at) {
                run(tree, &["update-ref", reference, at])?;
            }
        }
        for reference in now.keys() {
            if !self.refs.contains_key(reference) {
                run(tree, &["update-ref", "-d", reference])?;
            }
        }
        run(tree, &["reset", "-q", "--hard", &self.head])?;
        clean_untracked(tree)?;
        let now = ignored_contents(tree, &status(tree, true)?)?;
        let to_move = self.set_aside_of(&now);
        for path in now.keys() {
            if !self.ignored.contains_key(path) {
                Content::Absent.put_back(&at(tree, path))?;
            }
        }
        for (path, content) in &self.ignored {
            if content.can_be_put_back() && *content != Content::Dir {
                content.put_back(&at(tree, path))?;
            }
        }
        for (path, content) in &self.dirty {
            content.put_back(&tree.join(path))?;
        }
        if let Some((path, bytes)) = &self.index_file {
            std::fs::write(path, bytes).with_context(|| format!("put back {}", path.display()))?;
        }
        let left = self.change(tree)?;
        if !left.paths.is_empty()
            || left.staged
            || left.head.is_some()
            || !left.refs.is_empty()
            || !left.ignored_added.is_empty()
        {
            bail!(
                "the tree could not be put back as it was before the step: {left:?}{}",
                SetAside::not_moved(
                    &to_move,
                    "the tree could not be put back, so nothing was moved out of it"
                )
                .named()
            );
        }
        Ok(to_move)
    }

    /// Whether the ignored path `path` lies inside an ignored directory of this checkpoint,
    /// rather than being one git names by itself.
    fn is_inside_dir(&self, path: &OsStr) -> bool {
        Path::new(path)
            .ancestors()
            .skip(1)
            .filter(|dir| !dir.as_os_str().is_empty())
            .any(|dir| {
                let mut key = dir.as_os_str().to_owned();
                key.push("/");
                self.ignored.get(&key) == Some(&Content::Dir)
            })
    }
}

/// The paths of `paths` (each a plain path relative to the tree) that git ignores in `tree` as it
/// stands, in the order given: `git check-ignore`, by the rules [`status`] reads (a `.gitignore`
/// in any directory, `info/exclude`, `core.excludesFile`), a path inside an ignored directory
/// among them and a tracked path never (git ignores no tracked file), with no code from
/// configuration ([`NO_CONFIGURED_CODE`]). Each path goes to git as `./<path>`, so none reads as
/// pathspec magic, on its standard input, written beside the read so that no number of paths
/// stalls git. Exit 0 names some, 1 none; any other exit, or a path not handed over whole, is an
/// error naming what git said.
pub fn ignored_among(tree: &Path, paths: &[&str]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(b"./");
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let args = ["check-ignore", "-z", "--stdin"];
    let mut child = command(tree, &args)?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("git {} took no standard input", args.join(" ")))?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let output = child
        .wait_with_output()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))?;
    let written = writer
        .join()
        .map_err(|_| anyhow!("the writer of git {}'s input panicked", args.join(" ")))?;
    if !matches!(output.status.code(), Some(0 | 1)) {
        bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    written.with_context(|| format!("write the paths to git {}", args.join(" ")))?;
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| {
            let path = String::from_utf8(field.to_vec()).map_err(|_| {
                anyhow!(
                    "git check-ignore named a path that is not UTF-8: {}",
                    String::from_utf8_lossy(field)
                )
            })?;
            Ok(path.strip_prefix("./").unwrap_or(&path).to_owned())
        })
        .collect()
}

/// Whether the trees of the commits `from` and `to` differ at any path outside the directory
/// `outside` (relative to the tree): `git diff --quiet`, with no external diff and no text
/// conversion, and no code from configuration ([`NO_CONFIGURED_CODE`]). Exit 0 is no
/// difference, 1 a difference; any other exit is an error naming what git said.
pub fn trees_differ_outside(tree: &Path, from: &str, to: &str, outside: &str) -> Result<bool> {
    let exclude = format!(":(exclude){outside}");
    let args = [
        "diff",
        "--quiet",
        "--no-ext-diff",
        "--no-textconv",
        from,
        to,
        "--",
        ".",
        exclude.as_str(),
    ];
    let output = isolated_output(tree, &args)?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    }
}

/// The paths at which the trees of `from` and `to` (commits or trees) differ outside the
/// directory `outside` (relative to the tree), in git's order: `git diff --name-only -z` with
/// no rename detection (a renamed file is its old path and its new one), no external diff and
/// no text conversion, and no code from configuration ([`NO_CONFIGURED_CODE`]). A name that is
/// not UTF-8 is read lossily.
///
/// # Errors
/// A git that does not run or exits other than 0, naming what git said.
pub fn paths_differing_outside(
    tree: &Path,
    from: &str,
    to: &str,
    outside: &str,
) -> Result<Vec<String>> {
    diff_names_outside(tree, from, to, outside, None)
}

/// The paths [`paths_differing_outside`] names that `to` deletes: `git diff --diff-filter=D`
/// under the same options.
///
/// # Errors
/// A git that does not run or exits other than 0, naming what git said.
pub fn paths_deleted_outside(
    tree: &Path,
    from: &str,
    to: &str,
    outside: &str,
) -> Result<Vec<String>> {
    diff_names_outside(tree, from, to, outside, Some("--diff-filter=D"))
}

fn diff_names_outside(
    tree: &Path,
    from: &str,
    to: &str,
    outside: &str,
    filter: Option<&str>,
) -> Result<Vec<String>> {
    let exclude = format!(":(exclude){outside}");
    let mut args = vec![
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        "--no-ext-diff",
        "--no-textconv",
    ];
    args.extend(filter);
    args.extend([from, to, "--", ".", exclude.as_str()]);
    let output = isolated_output(tree, &args)?;
    if !output.status.success() {
        bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect())
}

/// A bare repository made for git commands that read no configured code, removed when
/// dropped: no work tree, no configuration but what `git init` writes, no `info/attributes`
/// and no hook (`--template=` copies nothing), `HEAD` on a branch with no commit, so no
/// attributes come from a tree. Every command runs with no code and no attributes from any
/// configuration: no system or global configuration (`GIT_CONFIG_NOSYSTEM`,
/// `GIT_CONFIG_GLOBAL=/dev/null`, `HOME` and `XDG_CONFIG_HOME` an empty directory), no system
/// or global attributes file (`GIT_ATTR_NOSYSTEM`, `core.attributesFile=/dev/null`), none from
/// a work tree, `$GIT_DIR/info/attributes` or a committed `.gitattributes`, and so no merge
/// driver, which only an attribute names and only configuration defines; besides
/// [`NO_CONFIGURED_CODE`]. Needs git 2.32 for `GIT_CONFIG_GLOBAL` (`--path-format` is 2.31,
/// `--object-format` 2.29). It holds no ref, so every revision a command names is an object id.
struct Isolated {
    dir: PathBuf,
    repository: PathBuf,
    home: PathBuf,
}

impl Drop for Isolated {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Isolated {
    /// A new one for the repository of `tree`, in its object format, under the temporary
    /// directory.
    fn new(tree: &Path) -> Result<Self> {
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let format = run(tree, &["rev-parse", "--show-object-format"])?;
        let dir = std::env::temp_dir().join(format!(
            "takt-git-{}-{}",
            std::process::id(),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir)
            .with_context(|| format!("make the isolated repository {}", dir.display()))?;
        let isolated = Self {
            repository: dir.join("repository"),
            home: dir.join("home"),
            dir,
        };
        std::fs::create_dir(&isolated.home)
            .with_context(|| format!("make {}", isolated.home.display()))?;
        let made = isolated.git(
            &[],
            &[
                "init",
                "--quiet",
                "--bare",
                "--template=",
                &format!("--object-format={format}"),
                "repository",
            ],
        )?;
        if !made.status.success() {
            bail!(
                "git init of the isolated repository {} exited {}: {}",
                isolated.repository.display(),
                made.status,
                String::from_utf8_lossy(&made.stderr).trim()
            );
        }
        Ok(isolated)
    }

    /// Runs `git <args>` with `extra` set in its environment, as the type says; answers what it
    /// printed, whatever its exit status.
    fn git(&self, extra: &[(&str, &Path)], args: &[&str]) -> Result<Output> {
        let extra: Vec<(&str, &OsStr)> = extra
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        self.git_with(&extra, args, None)
    }

    /// [`Isolated::git`], `input` on git's standard input when given.
    fn git_with(
        &self,
        extra: &[(&str, &OsStr)],
        args: &[&str],
        input: Option<&[u8]>,
    ) -> Result<Output> {
        let mut command = self.command(extra, args);
        let Some(input) = input else {
            return command
                .stdin(Stdio::null())
                .output()
                .with_context(|| format!("run git {} isolated", args.join(" ")));
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("run git {} isolated", args.join(" ")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input)
                .with_context(|| format!("write to git {} isolated", args.join(" ")))?;
        }
        child
            .wait_with_output()
            .with_context(|| format!("run git {} isolated", args.join(" ")))
    }

    /// The `git <args>` [`Isolated::git_with`] runs, with `extra` set in its environment, not
    /// yet started.
    fn command(&self, extra: &[(&str, &OsStr)], args: &[&str]) -> Command {
        let mut command = Command::new("git");
        crate::forge::without_credentials(&mut command, |_| false);
        for name in REDIRECTING_ENV {
            command.env_remove(name);
        }
        command
            .env_remove("GIT_ATTR_SOURCE")
            .env_remove("GIT_CEILING_DIRECTORIES")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.home)
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_NO_LAZY_FETCH", "1");
        for (name, value) in extra {
            command.env(name, value);
        }
        command
            .args(NO_CONFIGURED_CODE)
            .args([
                "-c",
                "core.attributesFile=/dev/null",
                "-c",
                "core.commitGraph=false",
            ])
            .args(args)
            .current_dir(&self.dir);
        command
    }

    /// Runs `git <args>` for the repository of `tree`, whose object directory is `objects`, as
    /// [`isolated_output`] does, with `env` set and `input` on its standard input; its standard
    /// output trimmed, or an error naming what git said when it exits non-zero.
    fn run_on(
        &self,
        objects: &Path,
        env: &[(&str, &str)],
        args: &[&str],
        input: Option<&[u8]>,
    ) -> Result<String> {
        let mut extra: Vec<(&str, &OsStr)> = vec![
            ("GIT_DIR", self.repository.as_os_str()),
            ("GIT_OBJECT_DIRECTORY", objects.as_os_str()),
        ];
        extra.extend(env.iter().map(|(name, value)| (*name, OsStr::new(*value))));
        let output = self.git_with(&extra, args, input)?;
        if !output.status.success() {
            bail!(
                "git {} isolated exited {}: {}",
                args.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

/// The object directory of the repository of `tree`, absolute.
fn objects_of(tree: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from(run(
        tree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?)
    .join("objects"))
}

/// What [`diff_bounded`] read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedDiff {
    /// The whole diff, byte for byte, no more than the bound.
    Within(Vec<u8>),
    /// More than the bound: the bytes git printed, all of them (`whole`) or as many as were
    /// counted before git was stopped ([`DIFF_COUNTED_BYTES`]).
    Past { bytes: u64, whole: bool },
}

/// The most bytes of a diff past its bound [`diff_bounded`] counts, by default, before it stops
/// git.
pub const DIFF_COUNTED_BYTES: u64 = 64 << 20;

/// How much of each file [`diff_bounded`] shows around its changed lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffContext {
    /// 3 lines around each change (`-U3`).
    Hunks,
    /// The whole file: as many lines of context as the bound has bytes (`-U<limit>`). A context
    /// line is at least 2 bytes, so a diff within the bound shows every line of each file it
    /// names, and one that would leave a line out is past the bound.
    Whole,
}

/// The patch from the commit `from` to the commit `to` (object ids) over each part's paths,
/// each taken literally, every path under `outside` left out, the parts one after another, a
/// part with no path left out (git never runs on the exclusion alone). Read the way
/// [`trees_differ_outside`] reads, through an [`Isolated`] repository over the repository's own
/// objects ([`objects_of`]): no code and no attributes from any configuration, no external
/// diff, no text conversion; and so that it is the same bytes on every host, no color, the
/// part's context ([`DiffContext`]), the Myers algorithm with the indent heuristic, no rename
/// detection and the `a/` and `b/` prefixes, whatever any configuration says. At most `limit`
/// bytes are kept in all; past it the bytes are counted, up to `count_to` (at least one past
/// the bound), and then git is stopped. A git that exits non-zero is an error naming what it
/// said.
pub fn diff_bounded(
    tree: &Path,
    (from, to): (&str, &str),
    parts: &[(&[String], DiffContext)],
    outside: &str,
    limit: usize,
    count_to: u64,
) -> Result<BoundedDiff> {
    use std::io::Read as _;
    if !crate::forge::is_object_id(from) || !crate::forge::is_object_id(to) {
        bail!("a diff is read between object ids, not {from:?} and {to:?}");
    }
    let count_to = count_to.max(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1));
    let objects = objects_of(tree)?;
    let isolated = Isolated::new(tree)?;
    let whole = format!("-U{limit}");
    let mut kept = Vec::new();
    let mut counted = 0u64;
    for (paths, context) in parts {
        if paths.is_empty() {
            continue;
        }
        let pathspec: Vec<String> = paths
            .iter()
            .map(|path| format!(":(literal){path}"))
            .chain([format!(":(exclude){outside}")])
            .collect();
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            match context {
                DiffContext::Hunks => "-U3",
                DiffContext::Whole => whole.as_str(),
            },
            "--diff-algorithm=myers",
            "--indent-heuristic",
            "--no-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            from,
            to,
            "--",
        ];
        args.extend(pathspec.iter().map(String::as_str));
        let mut child = isolated
            .command(
                &[
                    ("GIT_DIR", isolated.repository.as_os_str()),
                    ("GIT_OBJECT_DIRECTORY", objects.as_os_str()),
                ],
                &args,
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("run git {} isolated", args.join(" ")))?;
        let stderr = child.stderr.take().map(|mut stream| {
            std::thread::spawn(move || {
                let mut said = Vec::new();
                let _ = stream.by_ref().take(64 << 10).read_to_end(&mut said);
                let _ = std::io::copy(&mut stream, &mut std::io::sink());
                said
            })
        });
        if let Some(stdout) = child.stdout.take() {
            let mut stdout = stdout.take(count_to - counted);
            let mut buffer = [0u8; 64 << 10];
            loop {
                let read = stdout
                    .read(&mut buffer)
                    .with_context(|| format!("read git {}", args.join(" ")))?;
                if read == 0 {
                    break;
                }
                counted += read as u64;
                if kept.len() <= limit {
                    kept.extend_from_slice(&buffer[..read.min(limit + 1 - kept.len())]);
                }
            }
        }
        let stopped = counted >= count_to;
        if stopped {
            let _ = child.kill();
        }
        let status = child
            .wait()
            .with_context(|| format!("run git {} isolated", args.join(" ")))?;
        let said = stderr
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        if stopped {
            return Ok(BoundedDiff::Past {
                bytes: counted,
                whole: false,
            });
        }
        if !status.success() {
            bail!(
                "git {} in {} exited {}: {}",
                args.join(" "),
                tree.display(),
                status,
                String::from_utf8_lossy(&said).trim()
            );
        }
    }
    if kept.len() > limit {
        return Ok(BoundedDiff::Past {
            bytes: counted,
            whole: true,
        });
    }
    Ok(BoundedDiff::Within(kept))
}

/// What [`replay`] made of a list of commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replayed {
    /// Every commit replayed: the last commit made, the replay's tip.
    Made(String),
    /// The commit whose change does not apply on what was replayed before it, and the paths
    /// its merge conflicts at; nothing was named, and the commits made before it are named by
    /// no ref.
    Conflict {
        /// The commit, as it was given.
        commit: String,
        /// The paths it conflicts at.
        paths: Vec<String>,
    },
}

/// The `<seconds> <zone>` of the header line `field` (`author`, `committer`) of a commit's
/// `header`, as git records it, read from the end of the line so a name holding spaces is no
/// part of it.
fn header_date(header: &str, field: &str) -> Option<String> {
    let line = header
        .lines()
        .find_map(|line| line.strip_prefix(field)?.strip_prefix(' '))?;
    let mut words = line.rsplitn(3, ' ');
    let zone = words.next()?;
    let seconds = words.next()?;
    (seconds.bytes().all(|b| b.is_ascii_digit())
        && !seconds.is_empty()
        && zone.len() == 5
        && zone.starts_with(['+', '-']))
    .then(|| format!("@{seconds} {zone}"))
}

/// Replays `commits` of `tree`'s repository, in order, on `onto`: each one's change against
/// its one parent applied with `git merge-tree --write-tree --merge-base=<parent>` to what was
/// replayed before it, and committed with `git commit-tree` on it, keeping the commit's message
/// byte for byte and its author and committer dates, with `identity` as its author and
/// committer. Every git runs isolated ([`Isolated`]: no configured code, no attribute, no merge
/// driver) on the repository's own objects, so the commits land in its store and neither the
/// work tree, the index nor any ref is touched. The same commits on the same `onto` with the
/// same identity give the same commits, whenever it runs. A commit with no parent or more than
/// one is an error naming it; a change that conflicts answers [`Replayed::Conflict`].
pub fn replay(
    tree: &Path,
    onto: &str,
    commits: &[String],
    identity: &CommitIdentity,
) -> Result<Replayed> {
    let objects = objects_of(tree)?;
    let isolated = Isolated::new(tree)?;
    let mut tip = onto.to_owned();
    for commit in commits {
        let bytes = raw(tree, &["cat-file", "commit", commit], None)?;
        let header_end = bytes
            .windows(2)
            .position(|pair| pair == b"\n\n")
            .ok_or_else(|| anyhow!("the commit {commit} has no message"))?;
        let parsed = parse_commit(commit, &bytes, commit.len())?;
        let [parent] = parsed.parents.as_slice() else {
            bail!(
                "the commit {commit} has {} parents; only a commit with one is replayed",
                parsed.parents.len()
            );
        };
        let header = String::from_utf8_lossy(&bytes[..header_end]);
        let dated = |field: &str| {
            header_date(&header, field)
                .ok_or_else(|| anyhow!("the commit {commit} gives no {field} date git reads"))
        };
        let (authored, committed) = (dated("author")?, dated("committer")?);
        // `commit-tree` records its message as UTF-8: one in another encoding would change.
        if let Some(encoding) = header
            .lines()
            .find_map(|line| line.strip_prefix("encoding "))
            .filter(|encoding| !encoding.eq_ignore_ascii_case("utf-8"))
        {
            bail!("the commit {commit} is encoded in {encoding}, not UTF-8; it is not replayed");
        }
        let merge_base = format!("--merge-base={parent}");
        let args = [
            "merge-tree",
            "--write-tree",
            "-z",
            "--name-only",
            "--no-messages",
            merge_base.as_str(),
            tip.as_str(),
            commit.as_str(),
        ];
        let output = isolated.git_with(
            &[
                ("GIT_DIR", isolated.repository.as_os_str()),
                ("GIT_OBJECT_DIRECTORY", objects.as_os_str()),
            ],
            &args,
            None,
        )?;
        let merged = read_merged(&args, &output, &tree.display().to_string())?;
        if !merged.conflicted.is_empty() {
            return Ok(Replayed::Conflict {
                commit: commit.clone(),
                paths: merged.conflicted,
            });
        }
        tip = isolated.run_on(
            &objects,
            &[
                ("GIT_AUTHOR_NAME", identity.name.as_str()),
                ("GIT_AUTHOR_EMAIL", identity.email.as_str()),
                ("GIT_AUTHOR_DATE", authored.as_str()),
                ("GIT_COMMITTER_NAME", identity.name.as_str()),
                ("GIT_COMMITTER_EMAIL", identity.email.as_str()),
                ("GIT_COMMITTER_DATE", committed.as_str()),
            ],
            &["commit-tree", &merged.tree, "-p", &tip],
            Some(&bytes[header_end + 2..]),
        )?;
    }
    Ok(Replayed::Made(tip))
}

/// Runs `git <args>` for the repository of `tree` in an [`Isolated`] bare repository whose
/// objects are the repository's own (`GIT_OBJECT_DIRECTORY`): what it reads is what the
/// repository's store holds, not hashed again, and what it writes (a merge's tree) the
/// repository holds after. Answers what it printed, whatever its exit status. What a judge
/// reads goes through a [`Mirror`] instead.
fn isolated_output(tree: &Path, args: &[&str]) -> Result<Output> {
    let objects = PathBuf::from(run(
        tree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?)
    .join("objects");
    let isolated = Isolated::new(tree)?;
    isolated.git(
        &[
            ("GIT_DIR", isolated.repository.as_path()),
            ("GIT_OBJECT_DIRECTORY", objects.as_path()),
        ],
        args,
    )
}

/// An [`Isolated`] repository holding a copy of some commits of `tree`'s repository and all
/// they reach, received through git's transport (`git fetch` of their ids from the tree's
/// path), every object hashed as it is received and checked (`transfer.fsckObjects`): an object
/// overwritten in the tree's store (a loose file, or a pack that names an id for other
/// content) does not arrive as its id, and the fetch fails naming it. Every read after the fetch
/// is of the copy, never of the tree's store, so what is read is what the ids name.
pub struct Mirror {
    isolated: Isolated,
}

impl Mirror {
    /// The copy of the commits `ids` (object ids) of `tree`'s repository; the inner error, what
    /// git said, when they do not arrive as their ids or are not there.
    pub fn of(tree: &Path, ids: &[&str]) -> Result<std::result::Result<Self, String>> {
        if ids.is_empty() || !ids.iter().all(|id| crate::forge::is_object_id(id)) {
            bail!("a mirror copies commits named by their object ids, not {ids:?}");
        }
        let source = std::path::absolute(tree)
            .with_context(|| format!("make {} absolute", tree.display()))?;
        let source = source
            .to_str()
            .ok_or_else(|| anyhow!("the tree {} is no UTF-8 path", tree.display()))?
            .to_owned();
        let mirror = Self {
            isolated: Isolated::new(tree)?,
        };
        let mut args = vec![
            "-c",
            "transfer.fsckObjects=true",
            "fetch",
            "--quiet",
            "--no-tags",
            "--no-write-fetch-head",
            "--recurse-submodules=no",
            "--",
            source.as_str(),
        ];
        args.extend(ids);
        let fetched = mirror.output_with(&[("GIT_ALLOW_PROTOCOL", Path::new("file"))], &args)?;
        if !fetched.status.success() {
            return Ok(Err(String::from_utf8_lossy(&fetched.stderr)
                .trim()
                .replace('\n', "; ")));
        }
        Ok(Ok(mirror))
    }

    fn output_with(&self, extra: &[(&str, &Path)], args: &[&str]) -> Result<Output> {
        let mut env = vec![("GIT_DIR", self.isolated.repository.as_path())];
        env.extend_from_slice(extra);
        self.isolated.git(&env, args)
    }

    /// Runs `git <args>` on the copy; answers what it printed, whatever its exit status.
    pub fn output(&self, args: &[&str]) -> Result<Output> {
        self.output_with(&[], args)
    }

    /// Runs `git <args>` on the copy; its standard output byte for byte, or an error naming
    /// what git said when it exits non-zero.
    pub fn bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = self.output(args)?;
        if !output.status.success() {
            bail!(
                "git {} on the judge's copy exited {}: {}",
                args.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output.stdout)
    }

    /// Runs `git <args>` on the copy; its standard output trimmed, for answers of one value.
    pub fn run(&self, args: &[&str]) -> Result<String> {
        let bytes = self.bytes(args)?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
    }

    /// What merging `revision` into `base` would leave, as [`merged_tree`] reads it, on the
    /// copy: the merge's tree is written into the copy.
    pub fn merged_tree(&self, base: &str, revision: &str) -> Result<Merged> {
        let args = merge_tree_args(base, revision);
        let output = self.output(&args)?;
        read_merged(&args, &output, "the judge's copy")
    }
}

/// What merging a revision into a base would leave ([`merged_tree`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The merge's tree object; with conflicts, the conflicted files in it as git leaves them,
    /// which may be the base's side (a file one side deleted, a binary file both changed).
    pub tree: String,
    /// The paths the merge conflicts at, relative to the tree, as git lists them; none for a
    /// clean merge.
    pub conflicted: Vec<String>,
}

/// What merging `revision` into `base` would leave: `git merge-tree --write-tree -z
/// --name-only`, which writes the merge's tree object and touches neither the work tree nor
/// the index, with no code from configuration ([`NO_CONFIGURED_CODE`]): the tree and the paths
/// it conflicts at, read byte for byte. A merge with conflicts (exit 1) still answers its tree;
/// any exit but 0 (clean) or 1, a conflict that names no path, or a path that is not UTF-8 is
/// an error naming what git said. `--write-tree` needs git 2.38 or later; an older git exits
/// with a usage error, which is that error, so nothing is judged or merged on it.
pub fn merged_tree(tree: &Path, base: &str, revision: &str) -> Result<Merged> {
    let args = merge_tree_args(base, revision);
    let output = isolated_output(tree, &args)?;
    read_merged(&args, &output, &tree.display().to_string())
}

/// The argument line of [`merged_tree`].
fn merge_tree_args<'a>(base: &'a str, revision: &'a str) -> [&'a str; 7] {
    [
        "merge-tree",
        "--write-tree",
        "-z",
        "--name-only",
        "--no-messages",
        base,
        revision,
    ]
}

/// What `git merge-tree` run as `args` in `place` answered ([`merged_tree`]).
fn read_merged(args: &[&str], output: &Output, place: &str) -> Result<Merged> {
    match output.status.code() {
        Some(code @ (0 | 1)) => {
            let mut fields = output
                .stdout
                .split(|byte| *byte == 0)
                .filter(|field| !field.is_empty());
            let merged = fields
                .next()
                .and_then(|field| std::str::from_utf8(field).ok())
                .map(str::trim)
                .unwrap_or_default()
                .to_owned();
            if merged.is_empty() {
                bail!("git {} in {} named no tree", args.join(" "), place);
            }
            let conflicted = fields
                .map(|path| {
                    String::from_utf8(path.to_vec()).map_err(|_| {
                        anyhow!(
                            "git merge-tree named a path that is not UTF-8: {}",
                            path.escape_ascii()
                        )
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            if code == 1 && conflicted.is_empty() {
                bail!(
                    "git {} in {} reported a conflict and named no path",
                    args.join(" "),
                    place
                );
            }
            Ok(Merged {
                tree: merged,
                conflicted,
            })
        }
        _ => bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            place,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_entries_are_read_byte_for_byte() {
        let bytes = b" M src/lib.rs\0?? tests/a b.rs\0?? tests/\xc3\xbc.rs\0R  src/mo ved.rs\0src/lib.rs\0!! target/\0??  lead.rs\0";
        let entries = parse_status(bytes).expect("reads");
        let paths: Vec<(&str, &str, Option<&str>)> = entries
            .iter()
            .map(|e| (e.code.as_str(), e.path.as_str(), e.from.as_deref()))
            .collect();
        assert_eq!(
            paths,
            [
                (" M", "src/lib.rs", None),
                ("??", "tests/a b.rs", None),
                ("??", "tests/ü.rs", None),
                ("R ", "src/mo ved.rs", Some("src/lib.rs")),
                ("!!", "target/", None),
                ("??", " lead.rs", None),
            ]
        );
        assert!(parse_status(b"R  only-target\0").is_err());
        assert!(parse_status(b"?? \xff\0").is_err());
        assert!(parse_status(b"").expect("reads").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_path_is_shown_byte_for_byte_and_shapes_no_line() {
        use std::os::unix::ffi::OsStrExt as _;
        assert_eq!(shown(OsStr::new("build/a b.o")), "build/a b.o");
        assert_eq!(shown(OsStr::from_bytes(b"build/\xffx.o")), "build/\\xffx.o");
        assert_ne!(
            shown(OsStr::from_bytes(b"build/\xffx.o")),
            shown(OsStr::from_bytes(b"build/\xfex.o"))
        );
        assert_eq!(shown(OsStr::new("a\n  effect: b")), "a\\u{a}  effect: b");
        // The text `\xff` and the byte 0xff read apart: a `\` is doubled.
        assert_eq!(shown(OsStr::new("build/\\xffx.o")), "build/\\\\xffx.o");
        assert_ne!(
            shown(OsStr::new("build/\\xffx.o")),
            shown(OsStr::from_bytes(b"build/\xffx.o"))
        );
        assert_eq!(
            at(Path::new("/srv/t"), OsStr::new("build/sub/")),
            Path::new("/srv/t/build/sub")
        );
        assert!(
            !at(Path::new("/srv/t"), OsStr::new("build/sub/"))
                .as_os_str()
                .as_encoded_bytes()
                .ends_with(b"/")
        );
    }

    /// A scratch directory under this worktree's build directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> Scratch {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp")
            .join(format!("git-unit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Scratch(std::fs::canonicalize(&dir).expect("resolves"))
    }

    #[test]
    fn a_set_aside_root_is_absolute_outside_the_tree_and_makeable() {
        let s = scratch("root");
        let tree = s.0.join("tree");
        std::fs::create_dir_all(tree.join("sub")).expect("a directory");
        let outside = s.0.join("state/takt/set-aside");
        assert_eq!(set_aside_root(&outside, &tree).expect("accepted"), outside);
        assert!(
            !set_aside_root(&outside, &tree)
                .expect("accepted")
                .as_os_str()
                .as_encoded_bytes()
                .ends_with(b"/"),
            "a root that does not exist yet is named without a `/` after it"
        );
        // A link on the way that is there now is resolved, so none is left to follow later.
        #[cfg(unix)]
        {
            std::fs::create_dir_all(s.0.join("state")).expect("a directory");
            std::os::unix::fs::symlink(s.0.join("state"), s.0.join("via")).expect("a link");
            assert_eq!(
                set_aside_root(&s.0.join("via/takt"), &tree).expect("accepted"),
                s.0.join("state/takt")
            );
        }
        assert!(set_aside_root(Path::new("relative/aside"), &tree).is_err());
        // Relative, though its way starts where the test runs and exists.
        assert!(set_aside_root(Path::new("./aside"), &tree).is_err());
        assert!(set_aside_root(&tree.join("sub/aside"), &tree).is_err());
        assert!(set_aside_root(&tree, &tree).is_err());
        // Through a link that leads into the tree.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&tree, s.0.join("into-tree")).expect("a link");
            assert!(set_aside_root(&s.0.join("into-tree/aside"), &tree).is_err());
        }
        // A file where the way should go on.
        std::fs::write(s.0.join("a-file"), "x").expect("a file");
        assert!(set_aside_root(&s.0.join("a-file/aside"), &tree).is_err());
        assert!(set_aside_root(&s.0.join("a-file"), &tree).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_fresh_dir_is_never_one_that_exists() {
        let s = scratch("fresh");
        let case = DirFd::open(&s.0.join("rv"), true).expect("the case directory");
        assert_eq!(fresh_dir(&case, "step-2").expect("made").0, "step-2");
        assert_eq!(fresh_dir(&case, "step-2").expect("made").0, "step-2-2");
        std::fs::write(s.0.join("rv/step-2-3"), "x").expect("a file in the way");
        std::os::unix::fs::symlink(&s.0, s.0.join("rv/step-2-4")).expect("a link in the way");
        assert_eq!(fresh_dir(&case, "step-2").expect("made").0, "step-2-5");
        assert!(s.0.join("rv/step-2-5").is_dir());
    }

    /// A link anywhere on the way, the last part or one before it, is refused, never followed;
    /// a part that is not there is made only when asked.
    #[cfg(unix)]
    #[test]
    fn a_directory_is_opened_without_following_a_link_on_the_way() {
        let s = scratch("nofollow");
        let real = s.0.join("real");
        std::fs::create_dir_all(real.join("inner")).expect("a directory");
        std::os::unix::fs::symlink(&real, s.0.join("link")).expect("a link");
        assert!(DirFd::open(&s.0.join("link"), true).is_err());
        assert!(DirFd::open(&s.0.join("link/inner"), true).is_err());
        assert!(DirFd::open(&s.0.join("link/made"), true).is_err());
        assert!(!real.join("made").exists(), "made through the link");
        assert!(DirFd::open(&s.0.join("absent"), false).is_err());
        assert!(!s.0.join("absent").exists());
        DirFd::open(&s.0.join("made/deeper"), true).expect("made");
        assert!(s.0.join("made/deeper").is_dir());
        let dir = DirFd::open(&real, false).expect("opened");
        assert!(dir.child(OsStr::new("inner"), false).is_ok());
        std::os::unix::fs::symlink(&real, real.join("back")).expect("a link");
        assert!(dir.child(OsStr::new("back"), true).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn moving_aside_never_replaces_what_is_there() {
        let s = scratch("noreplace");
        let from = s.0.join("from.o");
        std::fs::write(&from, "step").expect("a file");
        let dir = DirFd::open(&s.0.join("aside"), true).expect("a directory");
        std::fs::write(s.0.join("aside/from.o"), "there").expect("a file");
        assert!(move_into(&from, &dir, OsStr::new("from.o")).is_err());
        assert_eq!(std::fs::read_to_string(&from).expect("kept"), "step");
        assert_eq!(
            std::fs::read_to_string(s.0.join("aside/from.o")).expect("kept"),
            "there"
        );
        move_into(&from, &dir, OsStr::new("moved.o")).expect("moved");
        assert!(!from.exists());
        assert_eq!(
            std::fs::read_to_string(s.0.join("aside/moved.o")).expect("moved"),
            "step"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_aside_keeps_bytes_mode_and_time_and_removes_the_source() {
        use std::os::unix::fs::PermissionsExt as _;
        let s = scratch("copy");
        let from = s.0.join("from.o");
        let dir = DirFd::open(&s.0.join("aside"), true).expect("a directory");
        let to = s.0.join("aside/to.o");
        std::fs::write(&from, "the operator's bytes").expect("a file");
        std::fs::set_permissions(&from, std::fs::Permissions::from_mode(0o640)).expect("a mode");
        let when = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        std::fs::File::options()
            .write(true)
            .open(&from)
            .expect("opens")
            .set_modified(when)
            .expect("a time");
        copy_into(&from, &dir, OsStr::new("to.o")).expect("copied");
        assert!(!from.exists(), "the source is removed");
        let meta = std::fs::metadata(&to).expect("the copy");
        assert_eq!(
            std::fs::read_to_string(&to).expect("read"),
            "the operator's bytes"
        );
        assert_eq!(meta.permissions().mode() & 0o777, 0o640);
        assert_eq!(meta.modified().expect("a time"), when);
        // Never over what is there.
        std::fs::write(&from, "again").expect("a file");
        assert!(copy_into(&from, &dir, OsStr::new("to.o")).is_err());
        assert_eq!(std::fs::read_to_string(&from).expect("kept"), "again");
        assert_eq!(
            std::fs::read_to_string(&to).expect("kept"),
            "the operator's bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_aside_takes_nothing_but_a_regular_file_and_leaves_no_half_copy() {
        use std::os::unix::fs::PermissionsExt as _;
        let s = scratch("copy-refused");
        let dir = DirFd::open(&s.0.join("aside"), true).expect("a directory");
        // As long as the link's own target name, so only the kind tells them apart.
        std::fs::write(s.0.join("target.o"), "12345678").expect("a file");
        std::os::unix::fs::symlink("target.o", s.0.join("link.o")).expect("a link");
        assert!(copy_into(&s.0.join("link.o"), &dir, OsStr::new("to-link.o")).is_err());
        assert!(
            std::fs::symlink_metadata(s.0.join("link.o")).is_ok(),
            "the link stays"
        );
        assert!(std::fs::symlink_metadata(s.0.join("aside/to-link.o")).is_err());
        // A source that cannot be read: refused before anything is made.
        let locked = s.0.join("locked.o");
        std::fs::write(&locked, "x").expect("a file");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("a mode");
        let refused = copy_into(&locked, &dir, OsStr::new("to-locked.o"));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o600)).expect("a mode");
        assert!(refused.is_err());
        assert!(!s.0.join("aside/to-locked.o").exists(), "no half copy");
        assert!(locked.exists());
    }

    /// A regular file whose bytes are not as many as its length says (the kernel's own, read
    /// only): the copy is refused and removed, not kept half made.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_copy_aside_whose_count_is_not_the_length_is_removed() {
        let s = scratch("copy-count");
        let dir = DirFd::open(&s.0.join("aside"), true).expect("a directory");
        assert!(copy_into(Path::new("/proc/self/status"), &dir, OsStr::new("status.o")).is_err());
        assert!(!s.0.join("aside/status.o").exists(), "no half copy");
    }

    #[test]
    fn the_guarded_configuration_keys_are_those_that_run_code_or_change_git() {
        for key in [
            "core.fsmonitor",
            "core.hookspath",
            "filter.lfs.clean",
            "diff.x.textconv",
            "merge.x.driver",
            "alias.st",
            "include.path",
            "includeif.gitdir:/x/.path",
            "credential.helper",
            "remote.origin.url",
            "url.https://x/.insteadof",
            "gpg.program",
            "sequence.editor",
            "pager.log",
            "trailer.x.cmd",
            "difftool.x.cmd",
            "mergetool.x.cmd",
            "submodule.x.update",
            "extensions.worktreeconfig",
        ] {
            assert!(is_guarded_key(key), "{key}");
        }
        for key in [
            "branch.main.vscode-merge-base",
            "branch.main.remote",
            "remote.pushdefault",
            "user.name",
            "gk.x",
        ] {
            assert!(!is_guarded_key(key), "{key}");
        }
    }
}
