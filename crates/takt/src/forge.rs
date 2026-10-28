//! The `Forge` seam: the repository host behind a trait, its adapters and an in-memory double
//! for the tests ([`Memory`]). An adapter pushes a case's branch, opens and merges its pull
//! request and reads its state and checks; `--forge` names it:
//!
//! | `--forge` | adapter | what it goes through |
//! |---|---|---|
//! | `bot` | [`bot`] ([`BotRoute`]) | the organization's bot route, whose credential never enters takt |
//! | `gitlab` | [`gitlab`] ([`gitlab::Glab`]) | `glab` under the user's own GitLab login |
//! | `connectors` | [`connectors`] ([`connectors::ConnectorsForge`]) | `connectors operations invoke` on the run file's `forge_instance`; named, never a default |
//! | `none` | none | nothing; nothing leaves the machine |
//!
//! Before any of it leaves the machine, an adapter that publishes has a scan read what a push
//! would publish (the object set it sends: every object of the head the repository's base does
//! not hold, as the tracking ref of the case's base says while the repository's own base, read
//! through the adapter, agrees with it, or every object of the head; each commit object raw,
//! every header and the message, every name in every tree object and every path in full from
//! each commit's root (`filename:<path>`), a gitlink's and an empty tree's included, every blob
//! whole and, behind a UTF-16 or UTF-32 mark, decoded too, `git replace` notwithstanding; a head
//! that is not a commit, a tree entry in a mode git does not write and a disagreeing tracking
//! ref refused) and a pull request's title and body: the bot route its own, whatever the run's
//! scan adapter is; the `gitlab` adapter the run's scan adapter's ([`crate::scan::Scan`]), and
//! none under a declared `none`. A validator turns what the forge reports into evidence; the
//! forge itself writes no record.
//!
//! Every child process takt starts, the forge's and the others' (the test command, the
//! harness, `aep`), goes through [`run_child`]: no standard input, its own process group,
//! killed once it outlives its [`Limits`], what it started killed with it when it ends, and
//! only a bounded tail of what it prints kept (and a bounded head, for a harness step's
//! transcript). Its environment holds no credential but the ones its adapter reads
//! ([`without_credentials`]): the forge keeps the GitHub ones the bot route's programs read.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::confinement::Command;

pub mod bot;
pub mod command;
pub mod connectors;
pub mod credential;
pub mod gitlab;

pub use bot::BotRoute;

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

/// The heading of a ruleset's refusal, which runs into its first rule once the bot route's
/// program has dropped the newlines between them.
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

/// The forge's message in `words`, its prefix left out: what follows the bot route's own
/// prefix and status ([`bot::API_REFUSAL_PREFIX`]; `None` when the bot route withheld the
/// message), or else what follows a leading HTTP status code (`405 <message>`, as an adapter
/// that reports text passes it on).
fn forge_message(words: &str) -> Option<&str> {
    let words = words.trim();
    if let Some(rest) = words.strip_prefix(bot::API_REFUSAL_PREFIX) {
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

/// What the forge reports about a pull request: its state, its head and its base. A validator
/// checks the head against the case's branch and revision before it trusts a number; the
/// base is the branch a merge goes into, whatever the case's own records name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestView {
    /// The state.
    pub state: PullRequestState,
    /// The head.
    pub head: Head,
    /// The base branch, the merge's target, as the forge holds it.
    pub base: String,
}

/// Whether `text` is a full object id as git writes one: 40 (SHA-1) or 64 (SHA-256) lowercase
/// hexadecimal digits, never a name or an expression git would resolve.
#[must_use]
pub fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    /// The branch the case forked from: the bot route reads what the remote holds from its
    /// tracking ref of it.
    pub base: &'a str,
    /// The commit pushed.
    pub head: &'a str,
    /// The pull request opened after the push; none when it is open already.
    pub request: Option<&'a PullRequest>,
}

/// What the scan of a commit identity found when a run opened ([`Forge::scan_identity`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityScan {
    /// The scan read the identity and refused nothing.
    Passed,
    /// No scan read the identity, and why: no policy found, no scanner to run, a forge that has
    /// none. Not a pass: the scan before the push still applies.
    NotScanned(String),
}

/// The repository host: pushes a branch, opens its pull request, reports the pull request's
/// state and head and the checks on its head, and merges it.
pub trait Forge {
    /// The adapter's name.
    fn name(&self) -> &str;
    /// Scans everything `outgoing` would publish, before any of it leaves the machine; `dir` is
    /// where the adapter keeps the text it scanned. A refusal is an error, and the caller pushes
    /// nothing. No default: every adapter says what it scans, one that publishes nothing off the
    /// machine (the in-memory double) that it scans nothing.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()>;
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
    /// Scans the commit identity `identity`, as a commit object would carry it, with the scan
    /// [`Self::scan_outgoing`] applies before a push, when a run whose commits leave the machine
    /// opens; `dir` is where the adapter keeps the text it scanned. A refusal is an error naming
    /// the scan's; a scan that cannot run (no policy, no scanner) is [`IdentityScan::NotScanned`],
    /// never a pass. By default an adapter has no such scan and says so.
    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &crate::git::CommitIdentity,
    ) -> Result<IdentityScan> {
        let _ = (tree, dir, identity);
        Ok(IdentityScan::NotScanned(format!(
            "the forge {} has no scan for a commit identity",
            self.name()
        )))
    }
    /// The commit the branch `base`, a merge's target, is at as the repository reports it when
    /// the merge is decided: where `repository.merge` reads the judge allow-list and the
    /// delegation record from, never the case's tree. Refused by default, so an adapter that
    /// cannot say merges nothing: without the target's tip every path is a judge surface. An
    /// adapter that answers leaves the commit it names held by the tree's repository.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        let _ = tree;
        bail!(
            "the forge {} cannot report the tip of {base}, the merge target; nothing is merged through it",
            self.name()
        )
    }
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
    /// Runs `argv` in `tree` with `env` set for this line alone. No default: every runner says
    /// what it does with `env`, one that starts no process (a test's) that it drops it.
    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> Result<Exit>;
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
    /// The first bytes of standard output, at most the head [`run_child_keeping_head`] was
    /// asked to keep; empty from [`run_child`]. They may overlap `stdout` when the whole was
    /// shorter than the two together.
    pub stdout_head: Vec<u8>,
    /// How many bytes standard output carried in all.
    pub stdout_len: u64,
}

/// The error [`run_child`] answers for a child it killed at its timeout, its text naming the
/// program and the timeout, with what the child printed until then: what had arrived once its
/// process group was killed, within [`GRACE`], else nothing.
#[derive(Debug)]
pub struct TimedOut {
    /// The program's file name.
    pub program: String,
    /// The timeout it outlived.
    pub timeout: Duration,
    /// What it printed, `code` none.
    pub output: Finished,
}

impl fmt::Display for TimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} timed out after {:.1} s and was killed",
            self.program,
            self.timeout.as_secs_f64()
        )
    }
}

impl std::error::Error for TimedOut {}

impl Finished {
    /// Whether the process exited 0.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// How long the rest of a process's output may take to arrive once it has ended and its group
/// has been killed.
pub(crate) const GRACE: Duration = Duration::from_secs(2);

/// The names of the variables a run file names as a credential source (`credential: {env: NAME}`),
/// whatever they look like: no child takt starts inherits one.
static WITHHELD: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Records `name` as a credential's variable: [`is_credential`] holds for it from now on, and
/// [`without_credentials`] removes it from every child, whatever `keep` says.
pub fn withhold(name: &str) {
    let mut names = WITHHELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !names.iter().any(|held| held == name) {
        names.push(name.to_owned());
    }
}

fn withheld(name: &str) -> bool {
    WITHHELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .any(|held| held == name)
}

/// Whether the environment variable `name` carries a credential: a GitHub, GitLab or Anthropic
/// token, the SSH agent's socket, or any name that ends like one (`_TOKEN`, `_API_KEY`,
/// `_SECRET`, `_PASSWORD`, ...), or one a run file named as a credential's source ([`withhold`]).
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
    let original = name;
    let name = name.to_ascii_uppercase();
    withheld(original)
        || name.starts_with("GITLAB_")
        || name == "SSH_AUTH_SOCK"
        || SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// The credentials the bot route's programs read: `gh`'s GitHub tokens and the variables the
/// bot route's own program reads (`B10X_*`). Not the SSH agent: a push or a fetch goes under
/// the bot's credential, never the user's.
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
        if withheld(&text) || (is_credential(&text) && !keep(&text)) {
            command.env_remove(&name);
        }
    }
}

/// The variables that tell a program where a login of the user's is found besides `HOME`: the
/// session bus and the runtime directory it is found in (the keyring is reached through it), the
/// XDG base directories, and the configuration a forge client, git, GnuPG, a cloud or container
/// client reads its login from. A test command run on the host under `none`
/// ([`crate::confinement::unconfined`]) inherits none of them.
pub const LOGIN_LOCATIONS: [&str; 17] = [
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "GNOME_KEYRING_CONTROL",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    "XDG_CACHE_HOME",
    "GH_CONFIG_DIR",
    "GLAB_CONFIG_DIR",
    "GIT_CONFIG_GLOBAL",
    "GNUPGHOME",
    "NETRC",
    "AWS_CONFIG_FILE",
    "AWS_SHARED_CREDENTIALS_FILE",
    "CLOUDSDK_CONFIG",
    "DOCKER_CONFIG",
    "KUBECONFIG",
];

/// A home directory of its own for a child, empty and readable by this user only, under the
/// system's temporary directory; removed with what is in it when dropped.
#[derive(Debug)]
pub struct ScratchHome(PathBuf);

impl ScratchHome {
    /// Makes a new one; an error when it cannot be made.
    pub fn new() -> Result<Self> {
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "takt-home-{}-{}",
            std::process::id(),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&dir)
            .with_context(|| format!("make the scratch home {}", dir.display()))?;
        Ok(Self(dir))
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What [`tail`] kept of a stream.
struct Taken {
    /// Its first bytes, at most the head asked for.
    head: Vec<u8>,
    /// Its last bytes, at most the limit.
    tail: Vec<u8>,
    /// Whether the last bytes are not all of it.
    cut: bool,
    /// How many bytes it carried.
    len: u64,
}

/// Reads `stream` to its end on a thread of its own, keeping its first `head` bytes and its
/// last `limit` bytes; answers on the channel with them, whether anything was dropped from the
/// last ones, and how many bytes it carried.
fn tail(
    stream: Option<impl std::io::Read + Send + 'static>,
    head: usize,
    limit: usize,
) -> mpsc::Receiver<Taken> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut first = Vec::new();
        let mut len = 0_u64;
        let mut kept = Vec::new();
        let mut cut = false;
        if let Some(mut stream) = stream {
            let mut buffer = [0_u8; 8192];
            while let Ok(n) = stream.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                len += n as u64;
                let take = head.saturating_sub(first.len()).min(n);
                first.extend_from_slice(&buffer[..take]);
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
        let _ = send.send(Taken {
            head: first,
            tail: kept,
            cut,
            len,
        });
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

/// Held for reading while a child another thread cannot know of runs: one started outside
/// [`run_child`], in a process group or a session of its own, by a caller that waits on it
/// itself ([`hold_sweeps`]). Held for writing by every sweep, [`sweep_left`]'s and the one
/// [`run_child`] makes once its child has ended, so that a sweep waits until no such child is
/// running instead of killing it and taking the exit status its caller waits for. Taken before
/// [`RUNNING`], never while holding it.
static SWEEPS: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// A hold on this process's sweeps, from [`hold_sweeps`]: while it lives no sweep runs, and
/// one that is due waits until it is dropped.
#[must_use = "sweeps are held only while the hold lives"]
pub struct SweepsHeld {
    _held: std::sync::RwLockReadGuard<'static, ()>,
}

/// Holds this process's sweeps off for as long as the answer lives: what starts a child outside
/// [`run_child`], in a process group or a session of its own, and waits on it itself holds it
/// for as long as that child can run, so that no sweep on another thread takes the child for a
/// leftover, kills it and reaps it, leaving its own wait an error ("No child processes"). Held
/// by any number of threads at once. Not re-entrant: the thread holding it must not call
/// [`sweep_left`] or [`run_child`] before dropping it, as their sweep would wait for it forever,
/// nor hold it a second time, which waits forever behind a sweep that is due. On Linux what
/// this process adopted is still reaped meanwhile once it has ended, if it leads no process
/// group and is in a group other than this process's own. The child the hold is for is never
/// one: it leads a group or a session of its own, or stays in this process's group, and is
/// never started into another process's group.
pub fn hold_sweeps() -> SweepsHeld {
    SweepsHeld {
        _held: SWEEPS
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    }
}

/// Takes this process's sweeps for one sweep: it waits until no [`SweepsHeld`] lives.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn sweeping() -> std::sync::RwLockWriteGuard<'static, ()> {
    SWEEPS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The marks of the children [`run_child`] has running, on macOS: [`sweep_left`] kills what
/// carries a mark of this process's calls but none of these. Changed only while [`RUNNING`]
/// is held.
#[cfg(target_os = "macos")]
static LIVE: std::sync::Mutex<BTreeSet<String>> = std::sync::Mutex::new(BTreeSet::new());

/// The variable every child of [`run_child`] carries, its value unique to the call: a process
/// that still carries it after the child ended was started by the child.
pub(crate) const MARK: &str = "TAKT_CHILD";

/// What a child left behind once it ended: on Linux, takt is the subreaper of everything it
/// starts (`PR_SET_CHILD_SUBREAPER`), so a process the child started and orphaned, in a session
/// or a process group of its own, becomes takt's child instead of `init`'s and is found here.
/// One that has ended is reaped as soon as it has, while the child still runs ([`reap_ended`]).
#[cfg(target_os = "linux")]
mod leftovers {
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    /// How long the reaper sleeps between two looks for an ended orphan ([`reap_ended`]).
    const REAP_EVERY: Duration = Duration::from_millis(20);

    /// Held while this process reaps what it adopted: by the reaper for each of its looks
    /// ([`reap_ended`]), and by [`sweep`] and [`sweep_marked`] for the whole of theirs, so that
    /// a process one of them found ended or killed is reaped by that one only, by an id still
    /// its own. Taken last: nothing else is taken while it is held.
    static REAPING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Makes this process the subreaper of what it starts, once, and starts the thread that
    /// reaps what it adopted as soon as that has ended ([`reap_ended`]), for as long as the
    /// process lives.
    pub fn adopt() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            // SAFETY: `prctl` with `PR_SET_CHILD_SUBREAPER` takes integers only.
            let adopted = unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } == 0;
            // Without the thread an ended orphan is reaped by the next sweep, as it was before.
            if adopted {
                let _ = std::thread::Builder::new()
                    .name("takt-reaper".to_owned())
                    .spawn(|| {
                        loop {
                            std::thread::sleep(REAP_EVERY);
                            reap_ended();
                        }
                    });
            }
        });
    }

    /// Reaps every process this one adopted that has ended: a zombie whose parent this process
    /// is, that leads no process group, in a group other than this process's own. Its process
    /// group then no longer answers for it, so a test that killed a group and waits until it is
    /// gone sees it gone under takt as on a plain shell, where `init` reaps it.
    ///
    /// None of these is a child another caller waits on: every child this process starts stays
    /// in this process's own process group and session ([`std::process::Command`]'s default),
    /// or leads a group or a session of its own (`process_group(0)`: [`super::run_child`],
    /// Substrate's adapters, the gate provider's commands; `setsid`: a harness's session).
    /// Nothing it links starts a child into another process's group (`process_group` with any
    /// other group). A process in a foreign group that leads none and whose parent is this one
    /// is therefore one a child started, orphaned and this process adopted. A zombie that leads
    /// a group, adopted or not, is left to its caller or to the next sweep.
    ///
    /// Each one is reaped by its id while [`REAPING`] is held: between the read that found it
    /// ended and its reap no other reaper of this process can take it, so its id is still its
    /// own and names no process that took it over.
    fn reap_ended() {
        if !any_ended_child() {
            return;
        }
        let _reaping = REAPING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let me = libc::pid_t::try_from(std::process::id()).unwrap_or(0);
        // SAFETY: `getpgrp` takes nothing and touches no memory of this process.
        let own = unsafe { libc::getpgrp() };
        for pid in children(me) {
            let orphan = state(pid).is_some_and(|(state, parent, group)| {
                state == 'Z' && parent == me && group != pid && group != own
            });
            if orphan {
                let mut status = 0;
                // SAFETY: `waitpid` writes the status into the integer it is given.
                unsafe { libc::waitpid(pid, &raw mut status, libc::WNOHANG) };
            }
        }
    }

    /// Whether a child of this process has ended and is not reaped yet: one `waitid` that
    /// leaves it as it is (`WNOWAIT`), so the reaper's look costs one call while none has.
    fn any_ended_child() -> bool {
        // SAFETY: `siginfo_t` is plain data; `waitid` writes into the one it is given.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `waitid` writes into `info` and reads nothing else of this process.
        let found = unsafe {
            libc::waitid(
                libc::P_ALL,
                0,
                &raw mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        // SAFETY: `waitid` filled `info` as a child's, or left it zeroed when none had ended.
        found == 0 && unsafe { info.si_pid() } != 0
    }

    /// This process's children: from each of its threads' `children` list, or, on a kernel that
    /// keeps none, from every process's parent.
    fn children(me: libc::pid_t) -> Vec<libc::pid_t> {
        let listed: Option<Vec<libc::pid_t>> =
            std::fs::read_dir("/proc/self/task").ok().and_then(|tasks| {
                let mut read = false;
                let mut found = Vec::new();
                for task in tasks.filter_map(Result::ok) {
                    if let Ok(text) = std::fs::read_to_string(task.path().join("children")) {
                        read = true;
                        found.extend(
                            text.split_whitespace()
                                .filter_map(|pid| pid.parse::<libc::pid_t>().ok()),
                        );
                    }
                }
                read.then_some(found)
            });
        listed.unwrap_or_else(|| {
            let Ok(entries) = std::fs::read_dir("/proc") else {
                return Vec::new();
            };
            entries
                .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
                .filter(|pid| stat(*pid).is_some_and(|(parent, _, _)| parent == me))
                .collect()
        })
    }

    /// A process's state, parent and process group, from `/proc/<pid>/stat`.
    fn state(pid: libc::pid_t) -> Option<(char, libc::pid_t, libc::pid_t)> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let rest = text.get(text.rfind(')')? + 2..)?;
        let mut fields = rest.split(' ');
        let state = fields.next()?.chars().next()?;
        let mut next = || fields.next()?.parse().ok();
        Some((state, next()?, next()?))
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
        let _reaping = REAPING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    /// Whether the process `pid` has ended: gone, or a zombie its parent has not reaped yet.
    pub fn ended(pid: libc::pid_t) -> bool {
        let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return true;
        };
        let state = text
            .rfind(')')
            .and_then(|at| text.get(at + 2..))
            .and_then(|rest| rest.chars().next());
        matches!(state, None | Some('Z' | 'X'))
    }

    /// Kills, round after round, every process but `spare` that is in process group `group` or
    /// carries `mark`, and reaps each that is this process's child: what a run on the host left
    /// once its leader `spare` ended, which Substrate's `local` adapter reaps itself. A zombie
    /// another process will reap is left to it. An error when one is still there after the last
    /// round.
    pub fn sweep_marked(group: libc::pid_t, mark: &str, spare: libc::pid_t) -> anyhow::Result<()> {
        let me = libc::pid_t::try_from(std::process::id()).unwrap_or(0);
        let mark = format!("{}={mark}", super::MARK).into_bytes();
        let _reaping = REAPING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let found = || -> Vec<libc::pid_t> {
            let Ok(entries) = std::fs::read_dir("/proc") else {
                return Vec::new();
            };
            entries
                .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
                .filter(|pid| *pid != me && *pid != spare)
                .filter(|pid| {
                    stat(*pid).is_some_and(|(parent, pgrp, _)| {
                        (pgrp == group || carries(*pid, &mark)) && (parent == me || !ended(*pid))
                    })
                })
                .collect()
        };
        for _ in 0..64 {
            let left = found();
            if left.is_empty() {
                return Ok(());
            }
            for pid in &left {
                // SAFETY: `kill` takes two integers and touches no memory of this process.
                unsafe {
                    libc::kill(*pid, libc::SIGKILL);
                }
            }
            let until = Instant::now() + Duration::from_secs(2);
            let mut waiting = left;
            while !waiting.is_empty() && Instant::now() < until {
                waiting.retain(|pid| match stat(*pid) {
                    None => false,
                    Some((parent, _, _)) if parent == me => {
                        let mut status = 0;
                        // SAFETY: `waitpid` writes the status into the integer it is given.
                        let reaped = unsafe { libc::waitpid(*pid, &raw mut status, libc::WNOHANG) };
                        reaped == 0
                    }
                    Some(_) => !ended(*pid),
                });
                if !waiting.is_empty() {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        anyhow::bail!(
            "processes the test command started were still running after 64 rounds of killing"
        )
    }
}

/// A process takt started outside [`run_child`] and waits on through another owner (Substrate's
/// `local` adapter), counted among [`run_child`]'s running children for as long as this lives,
/// so that no sweep takes it or what stays in its process group for a leftover; on macOS its
/// mark is counted as live too.
pub(crate) struct Led {
    pid: u32,
    #[cfg(target_os = "macos")]
    mark: String,
}

impl Led {
    /// The process's id, which is its process group's.
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for Led {
    fn drop(&mut self) {
        RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.pid);
        #[cfg(target_os = "macos")]
        LIVE.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.mark);
    }
}

/// A mark for one child, unique to this process: `<pid>-<n>`, the value of [`MARK`] in its
/// environment.
pub(crate) fn next_mark() -> String {
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

/// Starts a process through `start`, which answers its owner's handle and the process's id, a
/// process group leader carrying `mark`: no sweep runs between its start and its being counted
/// ([`Led`]). On Linux this process is made the subreaper of what it starts first.
///
/// # Errors
///
/// What `start` returns.
pub(crate) fn lead<T>(mark: &str, start: impl FnOnce() -> Result<(T, u32)>) -> Result<(T, Led)> {
    #[cfg(target_os = "linux")]
    leftovers::adopt();
    let mut running = RUNNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (owned, pid) = start()?;
    running.insert(pid);
    #[cfg(target_os = "macos")]
    LIVE.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(mark.to_owned());
    #[cfg(not(target_os = "macos"))]
    let _ = mark;
    Ok((
        owned,
        Led {
            pid,
            #[cfg(target_os = "macos")]
            mark: mark.to_owned(),
        },
    ))
}

/// Whether the process `pid` has ended: gone, or a zombie not reaped yet. Elsewhere than on Linux
/// and macOS, always.
pub(crate) fn exited(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return true;
    };
    #[cfg(target_os = "linux")]
    {
        leftovers::ended(pid)
    }
    #[cfg(target_os = "macos")]
    {
        crate::marked::ended(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        true
    }
}

/// Kills what the ended leader `leader` of a run on the host left: on Linux every process but it
/// in its process group, while the leader is not reaped yet and so still holds the group's id,
/// or carrying `mark`, reaping this process's children among them; on macOS every process of
/// this user carrying `mark` (the leader, ended, carries none the kernel shows). Never the leader
/// itself, which its owner reaps.
///
/// # Errors
///
/// When one is still there after the last round of killing.
pub(crate) fn sweep_led(leader: u32, mark: &str) -> Result<()> {
    let Ok(leader) = i32::try_from(leader) else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    {
        // Once reaped, its id may name another process's group: only the mark is swept then.
        let group = if Path::new(&format!("/proc/{leader}")).exists() {
            leader
        } else {
            -1
        };
        leftovers::sweep_marked(group, mark, leader)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = leader;
        crate::marked::sweep(MARK, |carried| carried == mark.as_bytes())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (leader, mark);
        Ok(())
    }
}

/// Kills every process of this user that carries `mark` as its [`MARK`], wherever it went, and
/// reaps this process's children among them: what a confined run left once it ended, when its
/// adapter proves no `whole_tree_kill` (story:confinement-through-substrate-on-linux-and-macos;
/// `macos`, whose run a build script can leave in a session of its own). Not killed: one that
/// dropped the mark. Elsewhere than on Linux and macOS, nothing is done.
///
/// # Errors
///
/// When one is still there after the last round of killing.
pub(crate) fn sweep_mark(mark: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        // No process is in group -1 and none is spared: only the mark is swept.
        leftovers::sweep_marked(-1, mark, 0)
    }
    #[cfg(target_os = "macos")]
    {
        crate::marked::sweep(MARK, |carried| carried == mark.as_bytes())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = mark;
        Ok(())
    }
}

/// Kills and reaps what this process started or adopted that is in a process group of its own
/// no running child of [`run_child`] leads, or in a session of its own: what a judgement step's
/// harness, or a validator that ran a program by itself, left running once it returned (a writer
/// in the background). On Linux this process is made the subreaper of what it starts first, so
/// an orphan is found too. On macOS, where there is no subreaper, what is killed is such a
/// child of this process, and every process of this user that carries the mark of one of this
/// process's [`run_child`] calls (`TAKT_CHILD`) whose child is no longer running, wherever it
/// went; elsewhere nothing is done. The run calls it after each judgement step
/// and each observation. It waits while a [`SweepsHeld`] lives: a child another thread started
/// outside [`run_child`] and waits on itself (a gate's command, a harness's session) is not a
/// leftover, and is swept, if at all, only once it has ended and its caller has reaped it.
pub fn sweep_left() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        leftovers::adopt();
        let _sweeping = sweeping();
        let running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Group 0 leads nothing and no process carries an empty mark: what is swept is what
        // left the run's own group or session.
        leftovers::sweep(0, "", &running)
    }
    #[cfg(target_os = "macos")]
    {
        let _sweeping = sweeping();
        let running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::marked::sweep_children(&running)?;
        let live = LIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ours = format!("{}-", std::process::id());
        crate::marked::sweep(MARK, |mark| {
            mark.starts_with(ours.as_bytes())
                && !live.iter().any(|running| running.as_bytes() == mark)
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Ok(())
}

/// Runs `command` held to `limits`: standard input empty, standard output and error read into
/// a bounded tail each, in a process group of its own. Past its timeout it is killed with its
/// group and the answer is an error naming the program and the timeout, a [`TimedOut`] holding
/// what it printed until then. Once it has ended, what
/// it started is killed with it before this returns: its process group, and on Linux every
/// process it started and orphaned that takt adopted as their subreaper, however it left the
/// group (a session of its own included). Not killed: one that left the group into takt's own
/// group and dropped the call's mark from its environment. On macOS every process of this user
/// that carries the call's mark is killed too, wherever it went; not killed there: one that left
/// the group and dropped the mark. On other systems, one that left the group at all. An error
/// too when its output stays held open after that. On Linux, while it runs, a process it
/// started and orphaned that has ended is reaped as soon as it has, unless it leads a process
/// group or went into takt's own: its group no longer answers for it, as under `init`.
pub fn run_child(command: &mut Command, limits: Limits) -> Result<Finished> {
    run_child_keeping_head(command, limits, 0)
}

/// [`run_child`], keeping besides the tail of standard output its first `head` bytes
/// ([`Finished::stdout_head`]).
pub fn run_child_keeping_head(
    command: &mut Command,
    limits: Limits,
    head: usize,
) -> Result<Finished> {
    let mark = next_mark();
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
    command.process_group(0);
    let mut busy = 0;
    let mut child = loop {
        let mut running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match command.spawn() {
            Ok(child) => {
                running.insert(child.id());
                #[cfg(target_os = "macos")]
                LIVE.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(mark.clone());
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
    let stdout = tail(child.stdout.take(), head, limits.output);
    let stderr = tail(child.stderr.take(), 0, limits.output);
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
        // A child another thread waits on outside this call is not this child's leftover:
        // the sweep waits until none is running ([`hold_sweeps`]).
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let _sweeping = sweeping();
        let mut running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        running.remove(&child.id());
        #[cfg(target_os = "linux")]
        let swept = leftovers::sweep(child.id(), &mark, &running);
        #[cfg(target_os = "macos")]
        let swept = {
            LIVE.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&mark);
            crate::marked::sweep(MARK, |carried| carried == mark.as_bytes())
        };
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let swept: Result<()> = Ok(());
        swept
    };
    let Some(status) = status else {
        // What it printed until it was killed goes with the error, as far as it arrives
        // within the grace.
        let until = Instant::now() + GRACE;
        let mut taken = [&stdout, &stderr].map(|stream| {
            stream
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .ok()
        });
        let stderr = taken[1].take().map(|taken| taken.tail).unwrap_or_default();
        let stdout = taken[0].take().unwrap_or(Taken {
            head: Vec::new(),
            tail: Vec::new(),
            cut: false,
            len: 0,
        });
        return Err(anyhow::Error::new(TimedOut {
            program,
            timeout: limits.timeout,
            output: Finished {
                code: None,
                stdout: stdout.tail,
                stderr,
                stdout_cut: stdout.cut,
                stdout_head: stdout.head,
                stdout_len: stdout.len,
            },
        }));
    };
    swept.with_context(|| format!("{program} ended"))?;
    let read = |stream: &mpsc::Receiver<Taken>| {
        stream.recv_timeout(GRACE).map_err(|_| {
            anyhow!(
                "{program} ended, but a process it started outside its process group still holds its output open"
            )
        })
    };
    let stdout = read(&stdout)?;
    let stderr = read(&stderr)?;
    Ok(Finished {
        code: status.code(),
        stdout: stdout.tail,
        stderr: stderr.tail,
        stdout_cut: stdout.cut,
        stdout_head: stdout.head,
        stdout_len: stdout.len,
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
/// reach it, never asks for a password or a host key; and no git under it honours a replace ref.
pub const NO_PROMPT: [(&str, &str); 4] = [
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_ASKPASS", ""),
    ("GIT_SSH_COMMAND", "ssh -o BatchMode=yes"),
    ("GIT_NO_REPLACE_OBJECTS", "1"),
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

/// Runs `argv` in `tree` through `runner`; a program that did not exit 0 is an error naming the
/// line and what it said.
fn succeed(runner: &dyn Runner, tree: &Path, argv: &[String]) -> Result<Exit> {
    let exit = runner.run(tree, argv)?;
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

/// Runs the push line `argv` in `tree` through `runner` over HTTPS alone
/// ([`BotRoute::PUSH_ENV`]), the line of every adapter that pushes to `url`. A push git refuses
/// because a URL rewrite in the repository's configuration (`insteadOf`, `pushInsteadOf`) would
/// send it over another transport is refused naming `url` and that transport; `route` names
/// the adapter in the refusal.
fn push_over_https(
    runner: &dyn Runner,
    tree: &Path,
    argv: &[String],
    url: &str,
    route: &str,
) -> Result<()> {
    push_over_https_with(runner, tree, argv, url, route, &BotRoute::PUSH_ENV)
}

/// [`push_over_https`] with `env` as the line's environment, which holds [`BotRoute::PUSH_ENV`]
/// and what the adapter's credential helper reads besides.
fn push_over_https_with(
    runner: &dyn Runner,
    tree: &Path,
    argv: &[String],
    url: &str,
    route: &str,
    env: &[(&str, &str)],
) -> Result<()> {
    let exit = runner.run_env(tree, argv, env)?;
    if exit.success() {
        return Ok(());
    }
    if let Some(transport) = refused_transport(&exit.stderr) {
        bail!(
            "the push to {url} would go over {transport}, not https, as a URL rewrite in the repository's configuration names it: {route} pushes over HTTPS only, so nothing was pushed"
        );
    }
    Err(Failed {
        line: argv.join(" "),
        code: exit.code,
        stderr: exit.stderr.trim().to_owned(),
    }
    .into())
}

/// The commit the repository holds as the case's `base`, what the scan before a push need not
/// read again: the tracking ref `refs/remotes/<remote>/<base>`, read by its full name, but only
/// while the repository's own `base`, as `remote_base` reads it through the adapter, is at the
/// same commit. A tracking ref at another commit (stale, or written by hand) refuses the push,
/// naming both; a repository with no `base` holds nothing of it; no tracking ref says nothing,
/// and the repository is then not asked. `None` has every commit of the head scanned.
fn held_by(
    tree: &Path,
    remote: &str,
    base: &str,
    remote_base: impl FnOnce() -> Result<Option<String>>,
) -> Result<Option<String>> {
    let tracking = format!("refs/remotes/{remote}/{base}");
    let Some(tracked) = exact_ref(tree, &tracking) else {
        return Ok(None);
    };
    match remote_base()? {
        None => Ok(None),
        Some(remote) if remote == tracked => Ok(Some(tracked)),
        Some(remote) => bail!(
            "{tracking} is at {tracked}, but the repository's {base} is at {remote}: what the repository holds is not what the tracking ref says; refused, nothing was pushed"
        ),
    }
}

/// The scan before anything leaves the machine, shared by every adapter that publishes under
/// the `gates` scan adapter: one line of it ([`crate::scan::gates::outgoing_argv`]) under
/// `--repository` and, when the policy file is known, `--policy`, run through `runner`.
struct Scanner<'a> {
    runner: &'a dyn Runner,
    repository: &'a str,
    policy: Option<&'a Path>,
}

impl Scanner<'_> {
    /// The line that scans `paths`, the paths last.
    fn argv(&self, paths: &[&Path]) -> Vec<String> {
        crate::scan::gates::outgoing_argv(self.repository, self.policy, paths)
    }

    /// One `scan-text` line over everything pushing `outgoing.head` would publish: the request
    /// `write` puts under `dir` (the JSON sent and the title and body as published, as the
    /// `request` noun names it), when one is opened, and the objects the push sends that the
    /// repository does not hold (`held`, [`published_commits`], [`outgoing_text`]): every commit
    /// object raw ([`BotRoute::MESSAGES`]), every tree's entry names, every path in full and
    /// every blob ([`BotRoute::FILES`]), all written under `dir`. Once git prints more than
    /// `limit` bytes for them, it is read no further and the push is refused, nothing written or
    /// scanned.
    #[allow(clippy::too_many_arguments)]
    fn outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
        held: Option<&str>,
        limit: usize,
        write: fn(&Path, &PullRequest) -> Result<(PathBuf, PathBuf)>,
        noun: &str,
    ) -> Result<()> {
        let Outgoing {
            branch,
            head,
            request,
            ..
        } = *outgoing;
        let (text, commits) = outgoing_text(tree, held, head, limit)?;
        let Some((messages, files)) = text else {
            bail!(
                "{commits}, which pushing {branch} would publish, come to more than the {limit} bytes of messages and files one scan is given: refused, not cut; nothing was pushed"
            );
        };
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let mut paths = Vec::new();
        if let Some(request) = request {
            let (input, published) = write(dir, request)?;
            paths.extend([input, published]);
        }
        for (name, text) in [(BotRoute::MESSAGES, messages), (BotRoute::FILES, files)] {
            let path = dir.join(name);
            std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
            paths.push(path);
        }
        let paths: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
        succeed(self.runner, tree, &self.argv(&paths)).with_context(|| {
            format!(
                "what pushing {branch} would publish did not pass scan-text: {commits}, with their identities, messages and files{}; nothing was pushed{}",
                if request.is_some() {
                    format!(", and the {noun}'s title and body")
                } else {
                    String::new()
                },
                if request.is_some() {
                    format!(" and no {noun} opened")
                } else {
                    String::new()
                }
            )
        })?;
        Ok(())
    }

    /// One `scan-text` line over [`BotRoute::IDENTITY`] written under `dir`: the identity as the
    /// `author` and `committer` headers of a commit object ([`identity_headers`]). A scanner
    /// that is not on the program path, or that finds no policy file
    /// ([`crate::scan::gates::NO_POLICY`]), scanned nothing: [`IdentityScan::NotScanned`],
    /// naming what it said. A refusal names `route`, whose scan before a push it is.
    fn identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &crate::git::CommitIdentity,
        route: &str,
    ) -> Result<IdentityScan> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let path = dir.join(BotRoute::IDENTITY);
        std::fs::write(&path, identity_headers(identity))
            .with_context(|| format!("write {}", path.display()))?;
        let argv = self.argv(&[&path]);
        let exit = match self.runner.run(tree, &argv) {
            Ok(exit) => exit,
            Err(error)
                if error.chain().any(|cause| {
                    cause
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
                }) =>
            {
                return Ok(IdentityScan::NotScanned(format!("{error:#}")));
            }
            Err(error) => return Err(error),
        };
        if exit.success() {
            return Ok(IdentityScan::Passed);
        }
        let said = exit.stderr.trim();
        if said.contains(crate::scan::gates::NO_POLICY) {
            return Ok(IdentityScan::NotScanned(
                said.lines().collect::<Vec<_>>().join(" | "),
            ));
        }
        Err(anyhow::Error::new(Failed {
            line: argv.join(" "),
            code: exit.code,
            stderr: said.to_owned(),
        }))
        .with_context(|| {
            format!(
                "the commit identity {identity} did not pass scan-text, the scan {route} runs over every commit before a push"
            )
        })
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
/// The commit identity `identity` as a commit object carries it, the text the scan before a
/// push reads for each commit: an `author` and a `committer` header, `<name> <<email>>`, a time
/// and a zone after it.
#[must_use]
pub fn identity_headers(identity: &crate::git::CommitIdentity) -> String {
    format!("author {identity} 0 +0000\ncommitter {identity} 0 +0000\n")
}

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

/// What every git read of the scan starts with: no replacement object, so git shows the objects
/// a push sends whatever `refs/replace/` holds (`run_bounded` sets `GIT_NO_REPLACE_OBJECTS`
/// too), and file names as they are.
const READ_OPTIONS: [&str; 3] = ["--no-replace-objects", "-c", "core.quotePath=false"];

/// Runs the scan's git read `args` ([`READ_OPTIONS`] first) in `tree`, reading at most `left`
/// bytes and counting what it read off `left`; `None` past it.
fn read(tree: &Path, args: &[&str], left: &mut usize) -> Result<Option<String>> {
    let mut line: Vec<&str> = READ_OPTIONS.to_vec();
    line.extend(args);
    let Some(text) = crate::git::run_bounded(tree, &line, *left)? else {
        return Ok(None);
    };
    *left = left.saturating_sub(text.len());
    Ok(Some(text))
}

/// The commit `revision` names, by its id, read without replacement; an error when it names
/// none.
fn commit_id(tree: &Path, revision: &str) -> Result<String> {
    let mut left = 4096;
    let args = [
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &format!("{revision}^{{commit}}"),
    ];
    read(tree, &args, &mut left)
        .ok()
        .flatten()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("{revision} names no commit"))
}

/// Refuses a `head` of `branch` that is not a commit, read without peeling: an annotated tag
/// the branch names would be sent by the push as it is, its own text unscanned.
fn head_is_a_commit(tree: &Path, branch: &str, head: &str) -> Result<()> {
    if head.is_empty() || !head.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("the head {head:?} of {branch} is not an object id: refused, nothing was pushed");
    }
    let mut left = 64;
    let kind = read(tree, &["cat-file", "-t", head], &mut left)?.unwrap_or_default();
    let kind = kind.trim();
    if kind != "commit" {
        bail!(
            "the head {head} of {branch} is a {kind}, not a commit: a push of it would send it as it is; refused, nothing was pushed"
        );
    }
    Ok(())
}

/// The commit the ref named exactly `name` (a full name, `refs/...`) holds; `None` when no ref
/// has that name. Never git's lookup of a short name, which would take `refs/<name>` or
/// `refs/tags/<name>` for it.
fn exact_ref(tree: &Path, name: &str) -> Option<String> {
    let mut left = 4096;
    let held = read(tree, &["show-ref", "--verify", "--hash", name], &mut left)
        .ok()
        .flatten()?;
    commit_id(tree, held.trim()).ok()
}

/// The commits pushing `head` publishes, as the revision arguments of a `git rev-list`, and the
/// words for them: the commits of `head` that `held` does not hold, the commit the repository
/// holds as the case's base ([`BotRoute::held`]); with none, every commit of `head`. The
/// revisions are commit ids, so none is read as an option.
fn published_commits(tree: &Path, held: Option<&str>, head: &str) -> Result<(Vec<String>, String)> {
    let head = commit_id(tree, head)?;
    let short = head[..head.len().min(12)].to_owned();
    Ok(match held {
        Some(held) => (
            vec![head, format!("^{held}")],
            format!("the commits of {short} the repository's base at {held} does not hold"),
        ),
        None => (
            vec![head],
            format!("every commit of {short} (nothing says the repository holds any)"),
        ),
    })
}

/// The entries of the raw tree object `bytes`, each `<mode> <name>\0<id>`: the mode, the name
/// as the tree holds it and the id in hex, then what does not parse after them, kept as it is.
/// `hash` is the length of an id in bytes, 20 for SHA-1 and 32 for SHA-256.
fn parse_tree(bytes: &[u8], hash: usize) -> (Vec<(String, String, String)>, &[u8]) {
    let mut entries = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let Some(space) = rest.iter().position(|b| *b == b' ') else {
            break;
        };
        let Some(nul) = rest[space..]
            .iter()
            .position(|b| *b == 0)
            .map(|n| space + n)
        else {
            break;
        };
        if rest.len() < nul + 1 + hash {
            break;
        }
        let id: String = rest[nul + 1..nul + 1 + hash]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        entries.push((
            String::from_utf8_lossy(&rest[..space]).into_owned(),
            String::from_utf8_lossy(&rest[space + 1..nul]).into_owned(),
            id,
        ));
        rest = &rest[nul + 1 + hash..];
    }
    (entries, rest)
}

/// The entries of the raw tree object `bytes` as lines of text, `<mode> <name> <id>`
/// ([`parse_tree`]); what does not parse is kept as it is, so nothing in the object goes
/// unscanned.
fn tree_entries(bytes: &[u8], hash: usize) -> String {
    let (entries, rest) = parse_tree(bytes, hash);
    let mut text = String::new();
    for (mode, name, id) in entries {
        text.push_str(&format!("{mode} {name} {id}\n"));
    }
    if !rest.is_empty() {
        text.push_str(&String::from_utf8_lossy(rest));
        text.push('\n');
    }
    text
}

/// The bytes [`crate::git::read_objects`] counts for `object`: its header line and its bytes.
fn read_size(object: &crate::git::Object) -> usize {
    object.id.len() + object.kind.len() + 24 + object.bytes.len()
}

/// The modes git writes for a tree entry: a file, an executable, a link, a tree, a gitlink.
/// git walks and pushes other spellings too (`040000` as a tree), so the scan refuses them.
const GIT_MODES: [&str; 5] = ["100644", "100755", "120000", "40000", "160000"];

/// Every path the trees `roots` hold, each in full from its root as a push publishes it (the
/// unit gates scans a path in, `filename:<path>`): every entry, an empty tree's and a gitlink's
/// included, through every subtree. An entry whose mode is not one git writes ([`GIT_MODES`]) is
/// refused, naming its path and mode. The trees come from `trees` (the object set read already)
/// or are read through one `git cat-file --batch` per level of the walk
/// ([`crate::git::read_objects`]); what is read and every path, as it is scanned
/// (`filename:<path>` and a line end), are counted off `left`, and `None` once that is spent. A
/// subtree met again at the same path is walked once.
fn full_paths(
    tree: &Path,
    trees: &mut BTreeMap<String, Vec<u8>>,
    roots: Vec<String>,
    left: &mut usize,
) -> Result<Option<BTreeSet<String>>> {
    let mut paths = BTreeSet::new();
    let mut walked: BTreeSet<(String, String)> = BTreeSet::new();
    let mut level: Vec<(String, String)> =
        roots.into_iter().map(|id| (String::new(), id)).collect();
    while !level.is_empty() {
        let missing: Vec<String> = level
            .iter()
            .map(|(_, id)| id.clone())
            .filter(|id| !trees.contains_key(id))
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect();
        if !missing.is_empty() {
            let Some(read) = crate::git::read_objects(tree, &missing, *left)? else {
                return Ok(None);
            };
            for object in read {
                *left = left.saturating_sub(read_size(&object));
                trees.insert(object.id, object.bytes);
            }
        }
        let mut next = Vec::new();
        for (prefix, id) in level {
            if !walked.insert((prefix.clone(), id.clone())) {
                continue;
            }
            let Some(bytes) = trees.get(&id) else {
                continue;
            };
            let (entries, _) = parse_tree(bytes, id.len() / 2);
            for (mode, name, entry) in entries {
                let path = format!("{prefix}{name}");
                if !GIT_MODES.contains(&mode.as_str()) {
                    bail!(
                        "the tree entry {path} has the mode {mode}, which git does not write: refused, nothing was pushed"
                    );
                }
                let Some(rest) = left.checked_sub("filename:\n".len() + path.len()) else {
                    return Ok(None);
                };
                *left = rest;
                if mode == "40000" {
                    next.push((format!("{path}/"), entry));
                }
                paths.insert(path);
            }
        }
        level = next;
    }
    Ok(Some(paths))
}

/// The text of `bytes` when they start with a UTF-32 or UTF-16 byte-order mark, decoded, with
/// the encoding's name: what a reader of the file sees, which its bytes, a NUL beside every
/// ASCII character, never match. `None` for bytes with no such mark (UTF-8, Latin-1 and any
/// other encoding without a mark are scanned as their bytes alone).
fn decoded(bytes: &[u8]) -> Option<(&'static str, String)> {
    let utf32 = |rest: &[u8], unit: fn([u8; 4]) -> u32| -> String {
        rest.chunks(4)
            .map(|chunk| {
                let mut four = [0_u8; 4];
                four[..chunk.len()].copy_from_slice(chunk);
                char::from_u32(unit(four)).unwrap_or(char::REPLACEMENT_CHARACTER)
            })
            .collect()
    };
    let utf16 = |rest: &[u8], unit: fn([u8; 2]) -> u16| -> String {
        let units: Vec<u16> = rest
            .chunks(2)
            .map(|chunk| unit([chunk[0], chunk.get(1).copied().unwrap_or(0)]))
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, 0x00, 0x00, rest @ ..] => Some(("UTF-32LE", utf32(rest, u32::from_le_bytes))),
        [0x00, 0x00, 0xFE, 0xFF, rest @ ..] => Some(("UTF-32BE", utf32(rest, u32::from_be_bytes))),
        [0xFF, 0xFE, rest @ ..] => Some(("UTF-16LE", utf16(rest, u16::from_le_bytes))),
        [0xFE, 0xFF, rest @ ..] => Some(("UTF-16BE", utf16(rest, u16::from_be_bytes))),
        _ => None,
    }
}

/// What pushing `head` would publish, read in at most `limit` bytes: the object set the push
/// sends, `git rev-list --objects` over [`published_commits`], every object read raw through
/// one `git cat-file --batch` ([`crate::git::read_objects`]). Every commit object as the push
/// sends it, bytes as they are (every header, the encoding's and a signature's included, then
/// the message, never converted); every tree object's entries, whatever an entry's mode (a
/// gitlink's, an empty tree's); every path each commit's tree holds, in full from its root, as
/// `filename:<path>` ([`full_paths`]); every blob's whole content, raw (no text conversion, no
/// filter). `None` once git prints more than `limit` bytes for all of it, read no further. The
/// words for the commits.
fn outgoing_text(
    tree: &Path,
    held: Option<&str>,
    head: &str,
    limit: usize,
) -> Result<(Option<(String, String)>, String)> {
    let (revisions, words) = published_commits(tree, held, head)?;
    let mut left = limit;
    let mut args = vec!["rev-list", "--objects", "--no-object-names"];
    args.extend(revisions.iter().map(String::as_str));
    let Some(listing) = read(tree, &args, &mut left)? else {
        return Ok((None, words));
    };
    let ids: Vec<String> = listing
        .lines()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    let Some(objects) = crate::git::read_objects(tree, &ids, left)? else {
        return Ok((None, words));
    };
    let mut messages = String::new();
    let mut files = String::new();
    let mut trees: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut roots = Vec::new();
    for object in objects {
        left = left.saturating_sub(read_size(&object));
        let id = &object.id;
        match object.kind.as_str() {
            "commit" => {
                messages.push_str(&format!("commit {id}\n"));
                let text = String::from_utf8_lossy(&object.bytes);
                if let Some(root) = text
                    .lines()
                    .next()
                    .and_then(|line| line.strip_prefix("tree "))
                {
                    roots.push(root.trim().to_owned());
                }
                messages.push_str(&text);
                if !messages.ends_with('\n') {
                    messages.push('\n');
                }
            }
            "tree" => {
                files.push_str(&format!("tree {id}\n"));
                files.push_str(&tree_entries(&object.bytes, id.len() / 2));
                trees.insert(id.clone(), object.bytes);
            }
            kind => {
                files.push_str(&format!("{kind} {id}\n"));
                files.push_str(&String::from_utf8_lossy(&object.bytes));
                if !files.ends_with('\n') {
                    files.push('\n');
                }
                if let Some((encoding, text)) = decoded(&object.bytes) {
                    let Some(rest) = left.checked_sub(text.len()) else {
                        return Ok((None, words));
                    };
                    left = rest;
                    files.push_str(&format!("{kind} {id} decoded from {encoding}\n"));
                    files.push_str(&text);
                    if !files.ends_with('\n') {
                        files.push('\n');
                    }
                }
            }
        }
    }
    let Some(paths) = full_paths(tree, &mut trees, roots, &mut left)? else {
        return Ok((None, words));
    };
    for path in paths {
        files.push_str(&format!("filename:{path}\n"));
    }
    Ok((Some((messages, files)), words))
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

    /// Scans nothing: the double publishes nothing off the machine.
    fn scan_outgoing(&self, _: &Path, _: &Path, _: &Outgoing<'_>) -> Result<()> {
        Ok(())
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
        let base = self.opened.borrow()[index].base.clone();
        Ok(PullRequestView { state, head, base })
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

    /// The tree's own `refs/heads/<base>`: the double's repository is the tree. Not logged, so
    /// the calls read as the forge's writes and reads of the pull request.
    ///
    /// A test double, never a production forge: D3 of story:judge-guard forbids reading the
    /// merge target's tip from a local branch at merge time, because a step's test command can
    /// move it. The double may, because it stands for a repository the test itself owns: the
    /// test moves `refs/heads/<base>` as the owner's merge on the forge would, and no step runs
    /// between that move and this read. The CLI never builds a `Memory`; `--forge none`
    /// registers no merge validator at all.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        crate::git::run(
            tree,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &format!("refs/heads/{base}^{{commit}}"),
            ],
        )
        .with_context(|| format!("the double's {base} is not a branch of the tree"))
    }
}
