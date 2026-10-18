//! The `Forge` seam: the repository host behind a trait, one adapter that goes through the
//! organization's bot route, and an in-memory double for the tests. The bot credential never
//! enters takt: `b10x-gates bot` pushes under the bot, over HTTPS only, `b10x-gates api` writes
//! through JSON files and never a token in argv, the reads go through `gh`, and before any of
//! it leaves the machine `b10x-gates scan-text` reads what a push would publish (every commit
//! the remote's tracking refs do not hold: its author, committer, message and diff, the diff's
//! lines as the files hold them, every file as text) and a pull request's title and body. A
//! validator turns what the forge
//! reports into evidence; the forge itself writes no record.
//!
//! Every child process takt starts, the forge's and the others' (the test command, the
//! harness, `aep`), goes through [`run_child`]: no standard input, its own process group,
//! killed once it outlives its [`Limits`], what it started killed with it when it ends, and
//! only a bounded tail of what it prints kept. Its environment holds no credential but the ones
//! its adapter reads
//! ([`without_credentials`]): the forge keeps the GitHub ones the bot route's programs read.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};

/// The environment variable that names the `b10x-gates` policy file.
pub const POLICY_VAR: &str = "B10X_GATES_POLICY";

/// What a pull request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PullRequestState {
    #[default]
    Open,
    Merged,
    Closed,
}

impl PullRequestState {
    /// The state as a record's result: `open`, `merged` or `closed`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

impl fmt::Display for PullRequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the checks on a pull request's head report: every check passed, one failed, or some
/// are still to come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Checks {
    Pass,
    Fail,
    #[default]
    Pending,
}

impl Checks {
    /// The checks as a record's result: `pass`, `fail` or `pending`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Pending => "pending",
        }
    }
}

impl fmt::Display for Checks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A program a forge adapter ran that did not exit 0: the line it ran, its exit code (`None`
/// on a signal) and what it printed on standard error, the forge's own words. Shown as `<line>
/// exited <code>: <stderr>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    /// The line, its words joined by spaces.
    pub line: String,
    /// The exit code; `None` when a signal ended the program.
    pub code: Option<i32>,
    /// Standard error, trimmed.
    pub stderr: String,
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} exited {}: {}",
            self.line,
            self.code
                .map_or("on a signal".to_owned(), |code| code.to_string()),
            self.stderr
        )
    }
}

impl std::error::Error for Failed {}

/// How the forge words the state of a required check that has not reported, after `is` or
/// `are`.
const CHECK_PENDING: [&str; 4] = ["expected", "in progress", "pending", "queued"];

/// What `b10x-gates api` prints before the forge's own message for a refused write:
/// `b10x-gates: GitHub operation failed (<status>): <message>`.
const GATES_PREFIX: &str = "b10x-gates: GitHub operation failed (";

/// The heading of a ruleset's refusal, which runs into its first rule once `b10x-gates` has
/// dropped the newlines between them.
const RULESET_HEADING: &str = "repository rule violations found";

/// The forge's own words in one layer of an error: a [`Failed`] program's standard error, or,
/// from an adapter that reports a program's failure as text, what follows its `exited <code>: `.
/// The line the program ran (paths, the case id, the repository) is never among them.
fn forge_words(cause: &(dyn std::error::Error + 'static)) -> Option<String> {
    if let Some(failed) = cause.downcast_ref::<Failed>() {
        return Some(failed.stderr.clone());
    }
    let text = cause.to_string();
    text.match_indices("exited ").find_map(|(at, word)| {
        let rest = &text[at + word.len()..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        (digits > 0)
            .then(|| rest[digits..].strip_prefix(": "))
            .flatten()
            .map(str::to_owned)
    })
}

/// The forge's message in `words`, its prefix left out: what follows `b10x-gates`' own
/// prefix and status (`None` when `b10x-gates` withheld the message), or else what follows a
/// leading HTTP status code (`405 <message>`, as an adapter that reports text passes it on).
fn forge_message(words: &str) -> Option<&str> {
    let words = words.trim();
    if let Some(rest) = words.strip_prefix(GATES_PREFIX) {
        return rest.split_once("): ").map(|(_, message)| message);
    }
    let digits = words.chars().take_while(char::is_ascii_digit).count();
    Some(
        (digits > 0)
            .then(|| words[digits..].strip_prefix(' '))
            .flatten()
            .unwrap_or(words),
    )
}

/// Whether one clause of a refusal, lowercased and its quoted check name emptied to `""`, says
/// a required status check has not reported: `required status check "" is <pending>`, or
/// `<n> of <m> required status checks are <pending>`. Nothing else is.
fn pending_check_clause(clause: &str) -> bool {
    if let Some(state) = clause.strip_prefix("required status check \"\" is ") {
        return CHECK_PENDING.contains(&state);
    }
    let mut words = clause.splitn(4, ' ');
    let count = |word: Option<&str>| {
        word.is_some_and(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()))
    };
    count(words.next())
        && words.next() == Some("of")
        && count(words.next())
        && words
            .next()
            .and_then(|rest| rest.strip_prefix("required status checks are "))
            .is_some_and(|state| CHECK_PENDING.contains(&state))
}

/// Whether the forge's `words` refuse a merge only for required checks that have not reported.
/// A positive rule: past the forge's prefix ([`forge_message`]) and a ruleset's heading, every
/// clause (split at `.` and `;`, outside the quoted check names) must be a pending-check clause
/// ([`pending_check_clause`]). A failing check, an approving or code-owner review, an unresolved
/// conversation, a signed-commits rule or any clause it does not know makes the refusal an
/// error. A check name is a name, never a state (`"fail-fast"` is pending when it is expected);
/// words with an unbalanced quote cannot be read that way, and are an error.
fn pending_check_refusal(words: &str) -> bool {
    if !words.matches('"').count().is_multiple_of(2) {
        return false;
    }
    let Some(message) = forge_message(words) else {
        return false;
    };
    let mut status = String::new();
    let mut quoted = false;
    for c in message.chars() {
        if c == '"' {
            if !quoted {
                status.push_str("\"\"");
            }
            quoted = !quoted;
        } else if !quoted {
            status.extend(c.to_lowercase());
        }
    }
    let status = status.trim();
    let status = status.strip_prefix(RULESET_HEADING).unwrap_or(status);
    let mut clauses = status
        .split(['.', ';'])
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .peekable();
    clauses.peek().is_some() && clauses.all(pending_check_clause)
}

/// Whether `error`, the failure of a [`Forge::merge`], is the forge refusing the merge for a
/// required check that has not reported on the head yet: the API's `405 Required status check
/// "<name>" is expected`, or a check it names in progress, pending or queued. Nothing was
/// merged, and the same merge may be taken once the check reports. A required check that
/// failed, and every other refusal, is not: those stay errors. Only the forge's own words in
/// the error are read ([`forge_words`]), never the rest of its chain.
#[must_use]
pub fn refused_for_a_pending_check(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(forge_words)
        .any(|words| pending_check_refusal(&words))
}

/// What a pull request is opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// The head branch.
    pub branch: String,
    /// The base branch.
    pub base: String,
    /// The title.
    pub title: String,
    /// The body.
    pub body: String,
}

/// A pull request's head: the branch and the commit it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The head branch.
    pub branch: String,
    /// The commit the head branch names.
    pub sha: String,
}

/// What the forge reports about a pull request: its state and its head. A validator checks
/// the head against the case's branch and revision before it trusts a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestView {
    /// The state.
    pub state: PullRequestState,
    /// The head.
    pub head: Head,
}

/// What an `origin` URL names: the host and the `<owner>/<repo>` on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The host, `github.com` for the bot route.
    pub host: String,
    /// `<owner>/<repo>`.
    pub repository: String,
}

/// What a push is about to publish: the commit `head` pushed as `branch`, so every commit of
/// `head` the remote does not hold yet, and, when the pull request is opened with the push, its
/// title and body.
#[derive(Debug, Clone, Copy)]
pub struct Outgoing<'a> {
    /// The branch pushed.
    pub branch: &'a str,
    /// The branch the case forked from: what the remote holds when no tracking ref says.
    pub base: &'a str,
    /// The commit pushed.
    pub head: &'a str,
    /// The pull request opened after the push; none when it is open already.
    pub request: Option<&'a PullRequest>,
}

/// The repository host: pushes a branch, opens its pull request, reports the pull request's
/// state and head and the checks on its head, and merges it.
pub trait Forge {
    /// The adapter's name.
    fn name(&self) -> &str;
    /// Scans everything `outgoing` would publish, before any of it leaves the machine; `dir` is
    /// where the adapter keeps the text it scanned. A refusal is an error, and the caller pushes
    /// nothing. The default scans nothing: it is for an adapter that publishes nothing off the
    /// machine, such as the in-memory double; an adapter that pushes overrides it.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        let _ = (tree, dir, outgoing);
        Ok(())
    }
    /// Pushes `branch` of `tree` to the repository.
    fn push(&self, tree: &Path, branch: &str) -> Result<()>;
    /// Pushes the commit `revision` of `tree` to the repository as `branch`: [`Self::push`]
    /// when `branch` is at `revision` locally; else the revision itself, for a branch that
    /// holds an approval's own commit on top of it. An adapter that cannot push a revision
    /// refuses that, naming both.
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        let at = crate::git::run(tree, &["rev-parse", &format!("refs/heads/{branch}")])?;
        if at == revision {
            return self.push(tree, branch);
        }
        bail!(
            "the forge {} pushes a branch as it is, and {branch} is at {at}, not {revision}",
            self.name()
        )
    }
    /// Opens the pull request `request` describes, for a head branch already pushed (`pr.open`
    /// scans it, [`Self::scan_outgoing`], and pushes it first); `dir` is where the adapter keeps
    /// what it sent and what came back, when it keeps anything. The pull request's number.
    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64>;
    /// The state and the head of pull request `number`.
    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView>;
    /// The checks on the head of pull request `number`.
    fn checks(&self, tree: &Path, number: u64) -> Result<Checks>;
    /// Merges pull request `number`, whose head must still be `head`: a head that moved since it
    /// was read is refused, not merged. `dir` is where the adapter keeps what it sent and what
    /// came back, when it keeps anything. What the merge made of the pull request is read back
    /// through [`Self::pull_request`]; the merge itself answers nothing a validator records.
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()>;
}

/// What a program the adapter ran answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    /// The exit code; `None` when a signal ended the program.
    pub code: Option<i32>,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

impl Exit {
    /// Whether the program exited 0.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs one argument line, the program first, in a tree. The adapter builds every line it
/// runs and hands it here, so a test can read the lines without running them.
pub trait Runner {
    /// Runs `argv` in `tree`.
    fn run(&self, tree: &Path, argv: &[String]) -> Result<Exit>;
    /// Runs `argv` in `tree` with `env` set for this line alone. A runner that starts no
    /// process, a test's, runs it as [`Self::run`].
    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> Result<Exit> {
        let _ = env;
        self.run(tree, argv)
    }
}

/// What a child process takt starts is held to: killed, with what it started, once it has run
/// for `timeout`; of each of its output streams only the last `output` bytes are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How long it may run.
    pub timeout: Duration,
    /// How many bytes of each output stream are kept, the last ones.
    pub output: usize,
}

/// How a child process ended and the tail of what it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    /// The exit code; `None` when a signal ended the process.
    pub code: Option<i32>,
    /// The last bytes of standard output, at most the limit.
    pub stdout: Vec<u8>,
    /// The last bytes of standard error, at most the limit.
    pub stderr: Vec<u8>,
    /// Whether standard output was longer than the limit and its beginning dropped.
    pub stdout_cut: bool,
}

impl Finished {
    /// Whether the process exited 0.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// How long the rest of a process's output may take to arrive once it has ended and its group
/// has been killed.
const GRACE: Duration = Duration::from_secs(2);

/// Whether the environment variable `name` carries a credential: a GitHub, GitLab or Anthropic
/// token, the SSH agent's socket, or any name that ends like one (`_TOKEN`, `_API_KEY`,
/// `_SECRET`, `_PASSWORD`, ...).
#[must_use]
pub fn is_credential(name: &str) -> bool {
    const SUFFIXES: [&str; 9] = [
        "_TOKEN",
        "_API_KEY",
        "_APIKEY",
        "_SECRET",
        "_SECRET_KEY",
        "_ACCESS_KEY",
        "_PASSWORD",
        "_CREDENTIALS",
        "_PRIVATE_KEY",
    ];
    let name = name.to_ascii_uppercase();
    name.starts_with("GITLAB_")
        || name == "SSH_AUTH_SOCK"
        || SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// The credentials the bot route's programs read: `gh`'s GitHub tokens and `b10x-gates`' own
/// variables. Not the SSH agent: a push or a fetch goes under the bot's credential, never the
/// user's.
#[must_use]
pub fn bot_route_reads(name: &str) -> bool {
    matches!(
        name,
        "GH_TOKEN" | "GITHUB_TOKEN" | "GH_ENTERPRISE_TOKEN" | "GITHUB_ENTERPRISE_TOKEN"
    ) || name.starts_with("B10X_")
}

/// Removes from what `command` inherits every variable that [`is_credential`] and `keep` does
/// not admit.
pub fn without_credentials(command: &mut Command, keep: fn(&str) -> bool) {
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if is_credential(&text) && !keep(&text) {
            command.env_remove(&name);
        }
    }
}

/// Reads `stream` to its end on a thread of its own, keeping its last `limit` bytes; answers
/// on the channel with them and whether anything was dropped.
fn tail(
    stream: Option<impl std::io::Read + Send + 'static>,
    limit: usize,
) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut cut = false;
        if let Some(mut stream) = stream {
            let mut buffer = [0_u8; 8192];
            while let Ok(n) = stream.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                kept.extend_from_slice(&buffer[..n]);
                if kept.len() > limit.saturating_mul(2).max(8192) {
                    let drop = kept.len() - limit;
                    kept.drain(..drop);
                    cut = true;
                }
            }
        }
        if kept.len() > limit {
            let drop = kept.len() - limit;
            kept.drain(..drop);
            cut = true;
        }
        let _ = send.send((kept, cut));
    });
    receive
}

/// Kills the process group `child` leads: what it started and what stayed in its group. A
/// group with no process left is no error.
fn kill_group(child: &std::process::Child) {
    #[cfg(unix)]
    if let Ok(group) = libc::pid_t::try_from(child.id()) {
        // SAFETY: `kill` takes two integers and touches no memory of this process.
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = child;
}

/// The process groups of the children [`run_child`] has running. Held while one is spawned and
/// while what one left behind is swept, so that a sweep never takes another call's child, or
/// what stayed in its group, for a leftover.
static RUNNING: std::sync::Mutex<std::collections::BTreeSet<u32>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// The variable every child of [`run_child`] carries, its value unique to the call: a process
/// that still carries it after the child ended was started by the child.
const MARK: &str = "TAKT_CHILD";

/// What a child left behind once it ended: on Linux, takt is the subreaper of everything it
/// starts (`PR_SET_CHILD_SUBREAPER`), so a process the child started and orphaned, in a session
/// or a process group of its own, becomes takt's child instead of `init`'s and is found here.
#[cfg(target_os = "linux")]
mod leftovers {
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    /// Makes this process the subreaper of what it starts, once.
    pub fn adopt() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            // SAFETY: `prctl` with `PR_SET_CHILD_SUBREAPER` takes integers only.
            unsafe {
                libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
            }
        });
    }

    /// A process's parent, process group and session, from `/proc/<pid>/stat`.
    fn stat(pid: libc::pid_t) -> Option<(libc::pid_t, libc::pid_t, libc::pid_t)> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // The command name is in parentheses and may hold anything; the fields follow the last.
        let rest = text.get(text.rfind(')')? + 2..)?;
        let mut fields = rest.split(' ').skip(1);
        let mut next = || fields.next()?.parse().ok();
        Some((next()?, next()?, next()?))
    }

    /// Whether the process `pid` was started with `mark` in its environment.
    fn carries(pid: libc::pid_t, mark: &[u8]) -> bool {
        std::fs::read(format!("/proc/{pid}/environ"))
            .is_ok_and(|bytes| bytes.split(|byte| *byte == 0).any(|entry| entry == mark))
    }

    /// The processes this one adopted that the ended child in `group`, marked `mark`, left:
    /// in its group, in another session, in a process group no running child of `run_child`
    /// leads and not this process's own, or carrying its mark. A child of this process that
    /// another thread started directly stays in this process's own group and session.
    fn adopted(group: libc::pid_t, mark: &[u8], running: &BTreeSet<u32>) -> Vec<libc::pid_t> {
        let me = libc::pid_t::try_from(std::process::id()).unwrap_or(0);
        // SAFETY: `getsid` and `getpgrp` take integers and touch no memory of this process.
        let (session, own) = unsafe { (libc::getsid(0), libc::getpgrp()) };
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
            .filter(|pid| *pid != me)
            .filter(|pid| {
                stat(*pid).is_some_and(|(parent, pgrp, sid)| {
                    parent == me
                        && (pgrp == group
                            || sid != session
                            || (pgrp != own
                                && !u32::try_from(pgrp).is_ok_and(|g| running.contains(&g)))
                            || carries(*pid, mark))
                })
            })
            .collect()
    }

    /// Kills and reaps what the ended child in `group` left, round after round: killing one
    /// hands what it started to this process, which the next round finds. An error when
    /// something is still there after the last round.
    pub fn sweep(group: u32, mark: &str, running: &BTreeSet<u32>) -> anyhow::Result<()> {
        let Ok(group) = libc::pid_t::try_from(group) else {
            return Ok(());
        };
        let mark = format!("{}={mark}", super::MARK).into_bytes();
        for _ in 0..64 {
            let found = adopted(group, &mark, running);
            if found.is_empty() {
                return Ok(());
            }
            for pid in &found {
                // SAFETY: `kill` takes two integers and touches no memory of this process.
                unsafe {
                    libc::kill(*pid, libc::SIGKILL);
                }
            }
            let until = Instant::now() + Duration::from_secs(2);
            let mut left: Vec<libc::pid_t> = found;
            while !left.is_empty() && Instant::now() < until {
                left.retain(|pid| {
                    let mut status = 0;
                    // SAFETY: `waitpid` writes the status into the integer it is given.
                    let reaped = unsafe { libc::waitpid(*pid, &raw mut status, libc::WNOHANG) };
                    reaped == 0
                });
                if !left.is_empty() {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        anyhow::bail!("processes the child started were still running after 64 rounds of killing")
    }
}

/// Kills and reaps what this process started or adopted that is in a process group of its own
/// no running child of [`run_child`] leads, or in a session of its own: what a judgement step's
/// harness, or a validator that ran a program by itself, left running once it returned (a writer
/// in the background). On Linux this process is made the subreaper of what it starts first, so
/// an orphan is found too; elsewhere nothing is done. The run calls it after each judgement step
/// and each observation.
pub fn sweep_left() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        leftovers::adopt();
        let running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Group 0 leads nothing and no process carries an empty mark: what is swept is what
        // left the run's own group or session.
        leftovers::sweep(0, "", &running)
    }
    #[cfg(not(target_os = "linux"))]
    Ok(())
}

/// Runs `command` held to `limits`: standard input empty, standard output and error read into
/// a bounded tail each, in a process group of its own. Past its timeout it is killed with its
/// group and the answer is an error naming the program and the timeout. Once it has ended, what
/// it started is killed with it before this returns: its process group, and on Linux every
/// process it started and orphaned that takt adopted as their subreaper, however it left the
/// group (a session of its own included). Not killed: one that left the group into takt's own
/// group and dropped the call's mark from its environment; on other systems, one that left the
/// group at all. An error too when its output stays held open after that.
pub fn run_child(command: &mut Command, limits: Limits) -> Result<Finished> {
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mark = format!(
        "{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    command.env(MARK, &mark);
    #[cfg(target_os = "linux")]
    leftovers::adopt();
    let program = Path::new(command.get_program())
        .file_name()
        .map_or_else(
            || command.get_program().to_string_lossy(),
            std::ffi::OsStr::to_string_lossy,
        )
        .into_owned();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    let mut busy = 0;
    let mut child = loop {
        let mut running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match command.spawn() {
            Ok(child) => {
                running.insert(child.id());
                break child;
            }
            // An executable written a moment ago is busy while a child another thread forked
            // meanwhile still holds the write handle it inherited; that is over as soon as the
            // child has exec'd, so the spawn is retried briefly before it is an error.
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy && busy < 50 => {
                drop(running);
                busy += 1;
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("run {}", command.get_program().to_string_lossy()));
            }
        }
    };
    let stdout = tail(child.stdout.take(), limits.output);
    let stderr = tail(child.stderr.take(), limits.output);
    let started = Instant::now();
    let mut pause = Duration::from_millis(5);
    let status = loop {
        if let Some(status) = child.try_wait().context("wait for the child")? {
            break Some(status);
        }
        if started.elapsed() >= limits.timeout {
            kill_group(&child);
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(pause.min(limits.timeout.saturating_sub(started.elapsed())));
        pause = (pause * 2).min(Duration::from_millis(100));
    };
    // What the child started goes before anything it may have written is looked at.
    kill_group(&child);
    let swept = {
        let mut running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        running.remove(&child.id());
        #[cfg(target_os = "linux")]
        let swept = leftovers::sweep(child.id(), &mark, &running);
        #[cfg(not(target_os = "linux"))]
        let swept: Result<()> = Ok(());
        swept
    };
    let Some(status) = status else {
        bail!(
            "{program} timed out after {:.1} s and was killed",
            limits.timeout.as_secs_f64()
        );
    };
    swept.with_context(|| format!("{program} ended"))?;
    let read = |stream: &mpsc::Receiver<(Vec<u8>, bool)>| {
        stream.recv_timeout(GRACE).map_err(|_| {
            anyhow!(
                "{program} ended, but a process it started outside its process group still holds its output open"
            )
        })
    };
    let (stdout, stdout_cut) = read(&stdout)?;
    let (stderr, _) = read(&stderr)?;
    Ok(Finished {
        code: status.code(),
        stdout,
        stderr,
        stdout_cut,
    })
}

/// The runner that runs the line as a process, without a shell, held to its limits, with no
/// credential but the ones the bot route reads ([`bot_route_reads`]), and with nothing that can
/// wait on a person ([`NO_PROMPT`]): standard input is `/dev/null` ([`run_child`]), git asks no
/// question, neither on the terminal nor through an askpass program, and SSH runs in batch mode,
/// so a line that cannot authenticate fails at once instead of waiting until its timeout.
#[derive(Debug, Clone, Copy)]
pub struct Process {
    /// What the process is held to.
    pub limits: Limits,
}

impl Process {
    /// The limits a forge program is held to by default: five minutes, 8 MiB of output.
    pub const LIMITS: Limits = Limits {
        timeout: Duration::from_secs(300),
        output: 8 << 20,
    };
}

/// What every [`Process`] child is started with, over what takt inherited: git never prompts
/// on the terminal, and an empty `GIT_ASKPASS` ends git's search for an askpass program before
/// it reaches `core.askPass` or the user's `GIT_ASKPASS` or `SSH_ASKPASS`; SSH, should a line
/// reach it, never asks for a password or a host key.
pub const NO_PROMPT: [(&str, &str); 3] = [
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_ASKPASS", ""),
    ("GIT_SSH_COMMAND", "ssh -o BatchMode=yes"),
];

impl Default for Process {
    fn default() -> Self {
        Self {
            limits: Self::LIMITS,
        }
    }
}

impl Runner for Process {
    fn run(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        self.run_with(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> Result<Exit> {
        self.run_with(tree, argv, env)
    }
}

impl Process {
    /// Runs `argv` in `tree` as [`Runner::run`] does, with `env` set over [`NO_PROMPT`]: what
    /// one line needs that every other line must not get.
    pub fn run_with(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> Result<Exit> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| anyhow!("an empty argument line"))?;
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(tree)
            .envs(NO_PROMPT)
            .envs(env.iter().copied());
        without_credentials(&mut command, bot_route_reads);
        let finished = run_child(&mut command, self.limits)
            .with_context(|| format!("run {} in {}", argv.join(" "), tree.display()))?;
        Ok(Exit {
            code: finished.code,
            stdout: String::from_utf8_lossy(&finished.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&finished.stderr).into_owned(),
        })
    }
}

/// The adapter over the organization's bot route: `b10x-gates scan-text` checks what a push and
/// a pull request would publish before any of it leaves the machine, `b10x-gates bot -- push`
/// pushes the branch over HTTPS only, `b10x-gates api` opens the pull request and merges it with
/// JSON files, and read-only `gh pr view` and `gh pr checks` report its state and checks. The
/// scan and every API write name the repository and, when one is known, the policy file as
/// `--policy`; `b10x-gates` reads its own environment otherwise.
pub struct BotRoute {
    /// `<owner>/<repo>`, from the `origin` URL.
    pub repository: String,
    /// The `b10x-gates` policy file; none leaves it to the program's environment.
    pub policy: Option<PathBuf>,
    /// The remote the branch is pushed to.
    pub remote: String,
    /// The most bytes of commit messages and diff one scan before a push is given; more is
    /// refused, never cut ([`Self::SCAN_LIMIT`] by default).
    pub scan_limit: usize,
    runner: Box<dyn Runner>,
}

impl fmt::Debug for BotRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BotRoute")
            .field("repository", &self.repository)
            .field("policy", &self.policy)
            .field("remote", &self.remote)
            .field("scan_limit", &self.scan_limit)
            .finish_non_exhaustive()
    }
}

impl BotRoute {
    /// The file under the directory `open_pull_request` is given that holds what was sent.
    pub const REQUEST: &str = "pull-request.request.json";
    /// The file under the directory `open_pull_request` is given that holds what came back.
    pub const RESPONSE: &str = "pull-request.response.json";
    /// The file under the directory `open_pull_request` is given that holds the title and the
    /// body as they are published, decoded: the text `scan-text` reads line by line.
    pub const PUBLISHED: &str = "pull-request.published.txt";
    /// The file under the directory `merge` is given that holds what was sent.
    pub const MERGE_REQUEST: &str = "pull-request.merge.request.json";
    /// The file under the directory `merge` is given that holds what came back.
    pub const MERGE_RESPONSE: &str = "pull-request.merge.response.json";
    /// The file under the directory `scan_outgoing` is given that holds, for each commit a push
    /// would publish, its id, its author's and its committer's name and email, and its message.
    pub const MESSAGES: &str = "push.messages.txt";
    /// The file under the directory `scan_outgoing` is given that holds the diff of each commit
    /// a push would publish, every file as text, its lines as the file holds them
    /// ([`published_lines`]).
    pub const DIFF: &str = "push.diff.txt";
    /// The most bytes of messages and diff one scan before a push is given by default.
    pub const SCAN_LIMIT: usize = 32 << 20;
    /// What a push's child is started with beyond every forge child's: git may push over HTTPS
    /// alone, whatever URL the remote or the repository's configuration names.
    pub const PUSH_ENV: [(&str, &str); 1] = [("GIT_ALLOW_PROTOCOL", "https")];

    /// Over `repository` with `policy`, running every line through `runner`.
    #[must_use]
    pub fn new(
        repository: impl Into<String>,
        policy: Option<PathBuf>,
        runner: Box<dyn Runner>,
    ) -> Self {
        Self {
            repository: repository.into(),
            policy,
            remote: "origin".to_owned(),
            scan_limit: Self::SCAN_LIMIT,
            runner,
        }
    }

    /// The host the bot route serves: it pushes under a GitHub App and reads through `gh`.
    pub const HOST: &str = "github.com";

    /// What `gh pr checks` says on stderr, exiting 1 with nothing on stdout, for a head no
    /// check has been reported for yet: `gh` 2.100.0's message as read from its binary, not
    /// verified against a live pull request.
    pub const NO_CHECKS_REPORTED: &str = "no checks reported";

    /// Whether the bot route serves `host`; host names are case-insensitive.
    #[must_use]
    pub fn serves(host: &str) -> bool {
        host.eq_ignore_ascii_case(Self::HOST)
    }

    /// For `tree`: the repository from its `origin` URL, which must be on [`Self::HOST`], the
    /// policy from the environment, every line run as a process. A refusal names the URL
    /// without its userinfo: a credential in it never reaches what takt prints.
    pub fn for_tree(tree: &Path) -> Result<Self> {
        let url = crate::git::remote_url(tree, "origin")
            .context("the forge `bot` needs an origin remote")?;
        let shown = without_userinfo(&url);
        let origin = repository_of(&url).ok_or_else(|| {
            anyhow!(
                "the origin remote {shown} names no <owner>/<repo> on a host; give --forge none"
            )
        })?;
        if !Self::serves(&origin.host) {
            bail!(
                "the origin remote {shown} is on {}, not {}: the bot route pushes under a GitHub App and reads through gh; give --forge none",
                origin.host,
                Self::HOST
            );
        }
        let policy = std::env::var_os(POLICY_VAR).map(PathBuf::from);
        Ok(Self::new(
            origin.repository,
            policy,
            Box::new(Process::default()),
        ))
    }

    /// The line that pushes `branch`.
    #[must_use]
    pub fn push_argv(&self, branch: &str) -> Vec<String> {
        ["b10x-gates", "bot", "--", "push", &self.remote, branch]
            .map(str::to_owned)
            .to_vec()
    }

    /// The line that pushes the commit `revision` as `branch`: `<revision>:refs/heads/<branch>`,
    /// both made by takt (a commit id and the case's branch), never read from a file a step can
    /// write.
    #[must_use]
    pub fn push_revision_argv(&self, branch: &str, revision: &str) -> Vec<String> {
        let refspec = format!("{revision}:refs/heads/{branch}");
        ["b10x-gates", "bot", "--", "push", &self.remote, &refspec]
            .map(str::to_owned)
            .to_vec()
    }

    /// The options every `b10x-gates` line that reads the policy carries, the scan and the API
    /// writes alike: `--repository`, and `--policy` when the policy file is known.
    fn scope(&self) -> Vec<String> {
        let mut options = vec!["--repository".to_owned(), self.repository.clone()];
        if let Some(policy) = &self.policy {
            options.push("--policy".to_owned());
            options.push(policy.display().to_string());
        }
        options
    }

    /// The line that scans `path` before it leaves the machine, under the same repository and
    /// policy as the API write that would send it; the path comes last.
    #[must_use]
    pub fn scan_argv(&self, path: &Path) -> Vec<String> {
        self.scan_paths_argv(&[path])
    }

    /// The line that scans a pull request before it is opened: the JSON `request` the API write
    /// sends and, last, the `published` title and body, decoded, as the forge shows them. The
    /// scanner's rules match line by line, and the JSON holds the body as one escaped line.
    #[must_use]
    pub fn scan_request_argv(&self, request: &Path, published: &Path) -> Vec<String> {
        self.scan_paths_argv(&[request, published])
    }

    fn scan_paths_argv(&self, paths: &[&Path]) -> Vec<String> {
        let mut argv = vec!["b10x-gates".to_owned(), "scan-text".to_owned()];
        argv.extend(self.scope());
        argv.extend(paths.iter().map(|path| path.display().to_string()));
        argv
    }

    /// The line of an API write: `method` on `path`, the document read from `input`, the
    /// response written to `output`.
    fn api_argv(&self, method: &str, path: String, input: &Path, output: &Path) -> Vec<String> {
        let mut argv = vec![
            "b10x-gates".to_owned(),
            "api".to_owned(),
            "--method".to_owned(),
            method.to_owned(),
            "--path".to_owned(),
            path,
            "--input".to_owned(),
            input.display().to_string(),
            "--output".to_owned(),
            output.display().to_string(),
        ];
        argv.extend(self.scope());
        argv
    }

    /// The line that opens the pull request `input` describes, the response written to
    /// `output`.
    #[must_use]
    pub fn open_argv(&self, input: &Path, output: &Path) -> Vec<String> {
        self.api_argv(
            "POST",
            format!("/repos/{}/pulls", self.repository),
            input,
            output,
        )
    }

    /// The line that merges pull request `number`, the head to match read from `input`, the
    /// response written to `output`.
    #[must_use]
    pub fn merge_argv(&self, number: u64, input: &Path, output: &Path) -> Vec<String> {
        self.api_argv(
            "PUT",
            format!("/repos/{}/pulls/{number}/merge", self.repository),
            input,
            output,
        )
    }

    /// The line that reads the state of pull request `number` of the repository: named, never
    /// taken from the tree's remotes, which a step may have changed.
    #[must_use]
    pub fn view_argv(&self, number: u64) -> Vec<String> {
        vec![
            "gh".to_owned(),
            "pr".to_owned(),
            "view".to_owned(),
            number.to_string(),
            "--json".to_owned(),
            "state,mergedAt,headRefName,headRefOid".to_owned(),
            "--repo".to_owned(),
            self.repository.clone(),
        ]
    }

    /// The line that reads the checks on the head of pull request `number` of the repository:
    /// named, never taken from the tree's remotes.
    #[must_use]
    pub fn checks_argv(&self, number: u64) -> Vec<String> {
        vec![
            "gh".to_owned(),
            "pr".to_owned(),
            "checks".to_owned(),
            number.to_string(),
            "--json".to_owned(),
            "bucket".to_owned(),
            "--repo".to_owned(),
            self.repository.clone(),
        ]
    }

    /// Runs `argv` in `tree`; a program that did not exit 0 is an error naming the line and
    /// what it said.
    fn succeed(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        let exit = self.runner.run(tree, argv)?;
        if !exit.success() {
            return Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: exit.stderr.trim().to_owned(),
            }
            .into());
        }
        Ok(exit)
    }

    /// Runs `argv` in `tree` and reads what it printed as JSON; anything else is an error
    /// naming the line and what the program said, never a reading.
    fn read_json(&self, tree: &Path, argv: &[String]) -> Result<serde_json::Value> {
        let exit = self.runner.run(tree, argv)?;
        reading_of(argv, &exit)
    }

    /// Runs the push line `argv` in `tree` over HTTPS alone ([`Self::PUSH_ENV`]). A push git
    /// refuses because the remote, or a URL rewrite in the repository's configuration, would
    /// send it over another transport is refused naming the remote, the URL it is configured
    /// with and that transport.
    fn push_line(&self, tree: &Path, argv: &[String]) -> Result<()> {
        let exit = self.runner.run_env(tree, argv, &Self::PUSH_ENV)?;
        if exit.success() {
            return Ok(());
        }
        if let Some(transport) = refused_transport(&exit.stderr) {
            let configured = |key: &str| {
                crate::git::run(
                    tree,
                    &["config", "--get", &format!("remote.{}.{key}", self.remote)],
                )
                .ok()
                .filter(|url| !url.is_empty())
            };
            let url = configured("pushurl")
                .or_else(|| configured("url"))
                .map_or_else(
                    || "no URL takt could read".to_owned(),
                    |url| without_userinfo(&url),
                );
            bail!(
                "the remote {} ({url}) would push over {transport}, not https: the bot route pushes over HTTPS only, so nothing was pushed",
                self.remote
            );
        }
        Err(Failed {
            line: argv.join(" "),
            code: exit.code,
            stderr: exit.stderr.trim().to_owned(),
        }
        .into())
    }
}

/// The transport git names when `GIT_ALLOW_PROTOCOL` refused it: `transport 'ssh' not allowed`.
fn refused_transport(stderr: &str) -> Option<&str> {
    stderr.lines().find_map(|line| {
        let (_, rest) = line.split_once("transport '")?;
        let (transport, rest) = rest.split_once('\'')?;
        rest.trim_start()
            .starts_with("not allowed")
            .then_some(transport)
    })
}

/// Writes the pull request `request` into `dir`: the JSON the API write sends as
/// [`BotRoute::REQUEST`], and the title and body as they are published, decoded, as
/// [`BotRoute::PUBLISHED`]. Both paths.
fn write_request(dir: &Path, request: &PullRequest) -> Result<(PathBuf, PathBuf)> {
    let input = dir.join(BotRoute::REQUEST);
    let document = serde_json::json!({
        "title": request.title,
        "head": request.branch,
        "base": request.base,
        "body": request.body,
    });
    let bytes = serde_json::to_vec_pretty(&document).context("serialize the pull request")?;
    std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
    let published = dir.join(BotRoute::PUBLISHED);
    std::fs::write(&published, format!("{}\n\n{}", request.title, request.body))
        .with_context(|| format!("write {}", published.display()))?;
    Ok((input, published))
}

/// The options of every `git log` that reads what a push would publish: file names as they are,
/// no colour, no mailmap, whatever the repository's configuration says.
const LOG_OPTIONS: [&str; 5] = [
    "-c",
    "core.quotePath=false",
    "log",
    "--no-color",
    "--no-use-mailmap",
];

/// The rest of the options of the `git log` that reads the commits' messages: each commit's id,
/// its author's and its committer's name and email, then its message.
const MESSAGE_OPTIONS: [&str; 2] = [
    "--encoding=UTF-8",
    "--format=commit %H%nauthor %an <%ae>%ncommitter %cn <%ce>%n%n%B",
];

/// The rest of the options of the `git log` that reads the commits' diffs: each commit's own
/// diff, a merge as its dense combined diff, every file as text whatever its attributes say, no
/// external diff and no text conversion.
const DIFF_OPTIONS: [&str; 6] = [
    "--patch",
    "--cc",
    "--text",
    "--no-ext-diff",
    "--no-textconv",
    "--format=commit %H",
];

/// The commit `revision` names, by its id; an error when it names none.
fn commit_id(tree: &Path, revision: &str) -> Result<String> {
    crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{revision}^{{commit}}"),
        ],
    )
    .with_context(|| format!("{revision} names no commit"))
}

/// The commits pushing `head` to `remote` publishes, as the revision arguments of one `git log`
/// each, and the words for them. The commits of `head` the remote's tracking refs do not hold;
/// with no tracking ref of the remote, the commits of `head` not on `base` and the commits of
/// `base` no remote-tracking ref holds. Every revision is a commit id, so none is read as an
/// option.
fn published_commits(
    tree: &Path,
    remote: &str,
    base: &str,
    head: &str,
) -> Result<(Vec<Vec<String>>, String)> {
    let head = commit_id(tree, head)?;
    let short = head[..head.len().min(12)].to_owned();
    if !crate::git::refs(tree, &[&format!("refs/remotes/{remote}/")])?.is_empty() {
        return Ok((
            vec![vec![
                head,
                "--not".to_owned(),
                format!("--remotes={remote}"),
            ]],
            format!("the commits of {short} no tracking ref of {remote} holds"),
        ));
    }
    let base_id = commit_id(tree, base)?;
    Ok((
        vec![
            vec![head, format!("^{base_id}")],
            vec![base_id, "--not".to_owned(), "--remotes".to_owned()],
        ],
        format!(
            "the commits {base}..{short} and those of {base} no remote-tracking ref holds ({remote} has no tracking ref)"
        ),
    ))
}

/// The lines of `diff`, a `git log --patch` print, as the files the commits publish hold them:
/// in each hunk the lines the commit's tree holds (context and added) without their one prefix
/// column (one per parent in a merge's combined diff), the removed ones left out, which the
/// parent holds and which would otherwise stand between two lines the file holds next to each
/// other; git's `\ No newline` note left out; every other line, the file headers and the hunk
/// headers included, as git printed it.
fn published_lines(diff: &str) -> String {
    let mut text = String::with_capacity(diff.len());
    let mut columns: Option<usize> = None;
    for line in diff.lines() {
        if let Some(width) = columns {
            let prefix = line.get(..width.min(line.len())).unwrap_or("");
            let content = prefix.chars().all(|c| matches!(c, ' ' | '+' | '-'))
                && (prefix.len() == width || line.is_empty());
            if line.starts_with('\\') {
                continue;
            }
            if content {
                if !prefix.contains('-') {
                    text.push_str(line.get(width..).unwrap_or(""));
                    text.push('\n');
                }
                continue;
            }
            columns = None;
        }
        let ats = line.chars().take_while(|c| *c == '@').count();
        if ats >= 2 && line[ats..].starts_with(' ') {
            columns = Some(ats - 1);
        }
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// What pushing `head` to `remote` would publish ([`published_commits`]), read in at most
/// `limit` bytes: the commits' ids, identities and messages, and their diffs as the files hold
/// the lines ([`published_lines`]); `None` once git prints more than `limit` bytes, read no
/// further. The words for the commits.
fn outgoing_text(
    tree: &Path,
    remote: &str,
    base: &str,
    head: &str,
    limit: usize,
) -> Result<(Option<(String, String)>, String)> {
    let (selections, words) = published_commits(tree, remote, base, head)?;
    let mut left = limit;
    let mut messages = String::new();
    let mut diff = String::new();
    for (options, into) in [
        (&MESSAGE_OPTIONS[..], &mut messages),
        (&DIFF_OPTIONS[..], &mut diff),
    ] {
        for revisions in &selections {
            let mut args: Vec<&str> = LOG_OPTIONS.to_vec();
            args.extend(options);
            args.extend(revisions.iter().map(String::as_str));
            let Some(text) = crate::git::run_bounded(tree, &args, left)? else {
                return Ok((None, words));
            };
            left = left.saturating_sub(text.len());
            into.push_str(&text);
        }
    }
    Ok((Some((messages, published_lines(&diff))), words))
}

/// `path` with nothing at it: a response file left from before is removed, so what is read
/// after the call is what the call wrote, never a file something else put there.
fn fresh(path: PathBuf) -> Result<PathBuf> {
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        Err(error) => Err(error).with_context(|| format!("remove {}", path.display())),
    }
}

/// What `argv` printed, as JSON; anything else is an error naming the line and what the
/// program said, never a reading.
fn reading_of(argv: &[String], exit: &Exit) -> Result<serde_json::Value> {
    serde_json::from_str(&exit.stdout).map_err(|error| {
        anyhow!(
            "{} exited {} and printed no reading ({error}): {}",
            argv.join(" "),
            exit.code
                .map_or("on a signal".to_owned(), |code| code.to_string()),
            exit.stderr.trim()
        )
    })
}

impl Forge for BotRoute {
    fn name(&self) -> &str {
        "bot"
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        self.push_line(tree, &self.push_argv(branch))
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        let at = crate::git::run(tree, &["rev-parse", &format!("refs/heads/{branch}")])?;
        if at == revision {
            return self.push(tree, branch);
        }
        self.push_line(tree, &self.push_revision_argv(branch, revision))
    }

    /// One `scan-text` line over everything pushing `head` to [`Self::remote`] would publish:
    /// the pull request, when one is opened (the JSON request and the title and body as
    /// published), and the commits the remote does not hold ([`published_commits`]): their ids,
    /// authors, committers and messages ([`Self::MESSAGES`]) and their diffs as the files hold
    /// the lines ([`Self::DIFF`]), all written under `dir`. Once git prints more than
    /// [`Self::scan_limit`] bytes for them, it is read no further and the push is refused,
    /// nothing written or scanned.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        let Outgoing {
            branch,
            base,
            head,
            request,
        } = *outgoing;
        let (text, commits) = outgoing_text(tree, &self.remote, base, head, self.scan_limit)?;
        let Some((messages, diff)) = text else {
            bail!(
                "{commits}, which pushing {branch} would publish, come to more than the {} bytes of messages and diff one scan is given: refused, not cut; nothing was pushed",
                self.scan_limit
            );
        };
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let mut paths = Vec::new();
        if let Some(request) = request {
            let (input, published) = write_request(dir, request)?;
            paths.extend([input, published]);
        }
        for (name, text) in [(Self::MESSAGES, messages), (Self::DIFF, diff)] {
            let path = dir.join(name);
            std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
            paths.push(path);
        }
        let paths: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
        self.succeed(tree, &self.scan_paths_argv(&paths))
            .with_context(|| {
                format!(
                    "what pushing {branch} would publish did not pass scan-text: {commits}, with their identities, messages and diffs{}; nothing was pushed{}",
                    if request.is_some() {
                        ", and the pull request's title and body"
                    } else {
                        ""
                    },
                    if request.is_some() {
                        " and no pull request opened"
                    } else {
                        ""
                    }
                )
            })?;
        Ok(())
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::RESPONSE))?;
        let (input, published) = write_request(dir, request)?;
        self.succeed(tree, &self.scan_request_argv(&input, &published))
            .with_context(|| {
                format!(
                    "the pull request for {branch} did not pass scan-text over {} and was not opened; its title and body stayed on this machine, but the branch {branch} was pushed before this scan",
                    published.display(),
                    branch = request.branch
                )
            })?;
        self.succeed(tree, &self.open_argv(&input, &output))?;
        let text = std::fs::read_to_string(&output)
            .with_context(|| format!("read the response {}", output.display()))?;
        let response: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("read the response {}", output.display()))?;
        response["number"]
            .as_u64()
            .ok_or_else(|| anyhow!("the response {} has no number", output.display()))
    }

    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let argv = self.view_argv(number);
        let document = self.read_json(tree, &argv)?;
        let state = match document["state"].as_str() {
            Some("OPEN") => PullRequestState::Open,
            Some("MERGED") => PullRequestState::Merged,
            Some("CLOSED") => PullRequestState::Closed,
            other => bail!(
                "{} reported the state {other:?}, not OPEN, MERGED or CLOSED",
                argv.join(" ")
            ),
        };
        let field = |name: &str| -> Result<String> {
            document[name]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{} reported no {name}", argv.join(" ")))
        };
        Ok(PullRequestView {
            state,
            head: Head {
                branch: field("headRefName")?,
                sha: field("headRefOid")?,
            },
        })
    }

    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
        let argv = self.checks_argv(number);
        let exit = self.runner.run(tree, &argv)?;
        // A head no check has been reported for yet, the state of every pull request in the
        // seconds after its push: pending, not an error that ends the run. Anything else that
        // is not a reading stays an error.
        if !exit.success()
            && exit.stdout.trim().is_empty()
            && exit.stderr.contains(Self::NO_CHECKS_REPORTED)
        {
            return Ok(Checks::Pending);
        }
        let document = reading_of(&argv, &exit)?;
        let checks = document
            .as_array()
            .ok_or_else(|| anyhow!("{} printed no list of checks", argv.join(" ")))?;
        let mut pending = checks.is_empty();
        let mut failed = false;
        for check in checks {
            match check["bucket"].as_str() {
                Some("pass" | "skipping") => {}
                Some("fail" | "cancel") => failed = true,
                Some("pending") => pending = true,
                other => bail!(
                    "{} reported a check in the bucket {other:?}",
                    argv.join(" ")
                ),
            }
        }
        Ok(if failed {
            Checks::Fail
        } else if pending {
            Checks::Pending
        } else {
            Checks::Pass
        })
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let input = dir.join(Self::MERGE_REQUEST);
        let output = fresh(dir.join(Self::MERGE_RESPONSE))?;
        // The head to match: the API refuses the merge when the pull request's head is no longer
        // this commit, so what is merged is what was read and recorded, never a later push.
        let document = serde_json::json!({ "sha": head });
        let bytes = serde_json::to_vec_pretty(&document).context("serialize the merge request")?;
        std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
        let argv = self.merge_argv(number, &input, &output);
        self.succeed(tree, &argv)?;
        let text = std::fs::read_to_string(&output)
            .with_context(|| format!("read the response {}", output.display()))?;
        let response: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("read the response {}", output.display()))?;
        if response["merged"] != true {
            bail!(
                "{} did not merge pull request #{number}: {}",
                argv.join(" "),
                response["message"]
                    .as_str()
                    .unwrap_or("the response does not say `merged: true`")
            );
        }
        Ok(())
    }
}

/// The host and the `<owner>/<repo>` an `origin` URL names, in the forms git writes them:
/// `git@host:owner/repo.git`, `ssh://git@host[:port]/owner/repo.git`, `https://host/owner/repo`.
/// A local path, or a URL with no host (`file://`), names none.
#[must_use]
pub fn repository_of(url: &str) -> Option<Origin> {
    let url = url.trim().trim_end_matches('/');
    let (authority, path) = if let Some((_, rest)) = url.split_once("://") {
        rest.split_once('/')?
    } else {
        url.split_once(':')?
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split_once(':').map_or(host, |(host, _)| host);
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.rsplit('/');
    let repo = parts.next()?;
    let owner = parts.next()?;
    (!host.is_empty() && !owner.is_empty() && !repo.is_empty()).then(|| Origin {
        host: host.to_owned(),
        repository: format!("{owner}/{repo}"),
    })
}

/// `url` with its userinfo (`user:password@`) removed, what takt prints of a remote URL: a
/// credential a checkout carries in its origin never reaches an error or a line of output.
#[must_use]
pub fn without_userinfo(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, None), |(authority, path)| (authority, Some(path)));
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        match path {
            Some(path) => format!("{scheme}://{host}/{path}"),
            None => format!("{scheme}://{host}"),
        }
    } else {
        match url.split_once(':') {
            Some((authority, path)) if authority.contains('@') => {
                let host = authority
                    .rsplit_once('@')
                    .map_or(authority, |(_, host)| host);
                format!("{host}:{path}")
            }
            _ => url.to_owned(),
        }
    }
}

/// The in-memory double: answers what it is set to and logs every call, for the tests. A pull
/// request it opened has the head it was opened for, at the revision last pushed for that
/// branch (the tree's `HEAD` at the push), unless `head` says otherwise; one it merged is
/// `merged` from then on. A merge whose head is not the revision last pushed for the branch is
/// refused, as the API refuses one given a head to match.
#[derive(Debug, Default)]
pub struct Memory {
    /// What `checks` answers for a pull request it opened.
    pub checks: Checks,
    /// The state `pull_request` answers for a pull request it opened and has not merged.
    pub state: PullRequestState,
    /// The head `pull_request` answers instead of the one it was opened for, when set.
    pub head: Option<Head>,
    /// Every call, in order: `push <branch>`, `open <branch> #<n>`, `view #<n>`, `checks #<n>`,
    /// `merge #<n>`.
    pub calls: RefCell<Vec<String>>,
    /// The pull requests opened, numbered from 1 in this order.
    pub opened: RefCell<Vec<PullRequest>>,
    /// The revision last pushed, by branch.
    pub pushed: RefCell<BTreeMap<String, String>>,
    /// The pull requests merged, by number.
    pub merged: RefCell<Vec<u64>>,
}

impl Memory {
    /// The index of pull request `number` among the opened ones.
    fn index(&self, number: u64) -> Result<usize> {
        let opened = self.opened.borrow().len();
        match usize::try_from(number) {
            Ok(n) if n >= 1 && n <= opened => Ok(n - 1),
            _ => bail!("no pull request #{number} was opened through the double"),
        }
    }
}

impl Forge for Memory {
    fn name(&self) -> &str {
        "memory"
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let sha = crate::git::head(tree)?;
        self.pushed.borrow_mut().insert(branch.to_owned(), sha);
        self.calls.borrow_mut().push(format!("push {branch}"));
        Ok(())
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        let at = crate::git::run(tree, &["rev-parse", &format!("refs/heads/{branch}")])?;
        if at == revision {
            return self.push(tree, branch);
        }
        self.pushed
            .borrow_mut()
            .insert(branch.to_owned(), revision.to_owned());
        self.calls
            .borrow_mut()
            .push(format!("push {revision} as {branch}"));
        Ok(())
    }

    fn open_pull_request(&self, _: &Path, _: &Path, request: &PullRequest) -> Result<u64> {
        let mut opened = self.opened.borrow_mut();
        opened.push(request.clone());
        let number = opened.len() as u64;
        self.calls
            .borrow_mut()
            .push(format!("open {} #{number}", request.branch));
        Ok(number)
    }

    fn pull_request(&self, _: &Path, number: u64) -> Result<PullRequestView> {
        let index = self.index(number)?;
        self.calls.borrow_mut().push(format!("view #{number}"));
        let head = self.head.clone().unwrap_or_else(|| {
            let branch = self.opened.borrow()[index].branch.clone();
            let sha = self
                .pushed
                .borrow()
                .get(&branch)
                .cloned()
                .unwrap_or_default();
            Head { branch, sha }
        });
        let state = if self.merged.borrow().contains(&number) {
            PullRequestState::Merged
        } else {
            self.state
        };
        Ok(PullRequestView { state, head })
    }

    fn checks(&self, _: &Path, number: u64) -> Result<Checks> {
        self.index(number)?;
        self.calls.borrow_mut().push(format!("checks #{number}"));
        Ok(self.checks)
    }

    fn merge(&self, _: &Path, _: &Path, number: u64, head: &str) -> Result<()> {
        let index = self.index(number)?;
        self.calls.borrow_mut().push(format!("merge #{number}"));
        let branch = self.opened.borrow()[index].branch.clone();
        let pushed = self.pushed.borrow().get(&branch).cloned();
        if pushed.as_deref() != Some(head) {
            bail!(
                "pull request #{number} has the head {branch} at {}, not {head}; not merged",
                pushed.as_deref().unwrap_or("nothing pushed")
            );
        }
        self.merged.borrow_mut().push(number);
        Ok(())
    }
}
