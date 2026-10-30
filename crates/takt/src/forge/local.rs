//! The forge a repository on this machine is, `--forge local`: the tree's `origin` remote names a
//! bare repository by its path (the hub), outside the tree, so no step in the tree can move its
//! refs. Each case's tree is a clone of the hub made with `git clone --no-local`, sharing no
//! object file with it; the case's base is a branch of it, and a merge goes into that branch in
//! the hub. Nothing leaves the machine, so nothing is scanned.
//!
//! | action | the local forge |
//! |---|---|
//! | push | `git push` of the case's branch to the hub |
//! | `pr.open` | a request record `<hub>/takt/requests/<n>.json` (branch, base, title, body), `n` taken with an exclusive create, so runs at once never share a number |
//! | read | the record, the head as the hub's `refs/heads/<branch>`, behind its base when the base's tip is not an ancestor of the head; merged only when the record names the head and a commit on the hub's base that holds it (the head or a commit whose history has it), so a record written by hand merges nothing |
//! | checks | the run file's `local_checks`, an argument list run without a shell as the test command is ([`crate::confinement::Confinement::run_test`]: the adapter in use, its environment and scratch home, no credential), in a fresh repository under the state directory that fetched the head from the hub by its id and names no remote; exit 0 passes, any other exit or the timeout fails, pending while it runs; a pass is recorded per head and per argument list (its digest), so a later read of the same checks does not run them again and other checks are run, and a fail is run again by the next read; not run while the run's `--min-free-gib` is not free |
//! | merge | in the hub, with no work tree: a base whose tip already holds the head moves no ref and the request is recorded merged by the commit that holds it; else `git merge-tree --write-tree` of the base's tip and the head, a conflict refused naming its paths ([`LocalRefused::Conflict`]); else `git commit-tree` of a merge commit under the run's commit identity, saying takt's merge message, and `git update-ref` of the base from the tip to it, so a base another writer moved meanwhile is refused, not overwritten ([`LocalRefused::BaseMoved`]) |
//! | target tip | the hub's `refs/heads/<base>`, fetched into the tree's repository; the merge goes onto the tip last reported |

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};

use super::{
    Checks, Forge, Head, IdentityScan, Limits, MergeMessage, Outgoing, PullRequest,
    PullRequestState, PullRequestView, TimedOut, exact_ref, is_object_id, put_back_tracking,
    tracking_refs,
};
use crate::git::CommitIdentity;

/// A merge the hub refused: nothing was written to the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalRefused {
    /// Merging the head into the base's tip conflicts at `paths`.
    Conflict {
        /// The request.
        number: u64,
        /// The base branch.
        base: String,
        /// The paths the merge conflicts at, as git lists them.
        paths: Vec<String>,
    },
    /// The base was not at `expected`, the tip the merge was made on, when the merge was to move
    /// it: another writer moved it; `found` is where it is, `None` when it is gone.
    BaseMoved {
        /// The request.
        number: u64,
        /// The base branch.
        base: String,
        /// The tip the merge was made on.
        expected: String,
        /// The tip the hub holds now.
        found: Option<String>,
    },
}

impl fmt::Display for LocalRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict {
                number,
                base,
                paths,
            } => write!(
                f,
                "request #{number} conflicts with {base} in the hub at {}; nothing merged, {base} stays where it was",
                paths.join(", ")
            ),
            Self::BaseMoved {
                number,
                base,
                expected,
                found,
            } => write!(
                f,
                "request #{number} was merged onto {base} at {expected}, and the hub's {base} moved to {} meanwhile; refused, nothing overwritten",
                found.as_deref().unwrap_or("nothing")
            ),
        }
    }
}

impl std::error::Error for LocalRefused {}

/// Whether `error`, the failure of a merge, is [`LocalRefused::BaseMoved`]: nothing was merged,
/// and the merge may be judged again on the base where it is now.
#[must_use]
pub fn refused_for_a_moved_base(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<LocalRefused>(),
            Some(LocalRefused::BaseMoved { .. })
        )
    })
}

/// One request as the hub keeps it, `<hub>/takt/requests/<n>.json`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Request {
    branch: String,
    base: String,
    title: String,
    body: String,
    /// `open` or `merged`.
    state: String,
    /// The verdict of the checks, by head and argument list ([`Local::verdict_key`]).
    #[serde(default)]
    checks: BTreeMap<String, Verdict>,
    /// The commit on the base that holds the head, once merged: takt's merge commit, a merge
    /// made by hand, or the head itself when the base reached it without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    merge: Option<String>,
    /// The head merged, once merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head: Option<String>,
}

/// One `local_checks` on one head: `pass`, `fail`, or `pending` while the process `pid` runs
/// them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Verdict {
    result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
}

/// The adapter over the hub `origin` names.
pub struct Local {
    hub: PathBuf,
    checks: Vec<String>,
    state: PathBuf,
    identity: CommitIdentity,
    limits: Limits,
    /// The least free space, in bytes, the checks need on the state directory's file system and
    /// on that of `CARGO_TARGET_DIR` when it is set; `0` reads nothing.
    min_free: u64,
    /// The tip of each base last reported by [`Forge::target_tip`], what the merge goes onto.
    judged: RefCell<BTreeMap<String, String>>,
}

impl fmt::Debug for Local {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Local")
            .field("hub", &self.hub)
            .field("checks", &self.checks)
            .field("state", &self.state)
            .field("identity", &self.identity)
            .field("limits", &self.limits)
            .field("min_free", &self.min_free)
            .finish_non_exhaustive()
    }
}

/// The path a remote URL names on this machine: a plain path, or a `file://` URL with no host;
/// `None` for a URL with another scheme or a host, or git's `host:path` form (a `:` before any
/// `/`).
fn local_path(url: &str) -> Option<PathBuf> {
    if let Some(rest) = url.strip_prefix("file://") {
        return rest.starts_with('/').then(|| PathBuf::from(rest));
    }
    if url.contains("://") || url.is_empty() {
        return None;
    }
    match (url.find(':'), url.find('/')) {
        (Some(colon), Some(slash)) if colon < slash => None,
        (Some(_), None) => None,
        _ => Some(PathBuf::from(url)),
    }
}

/// Whether the object directory `tree` shares files with the hub's `hub`: `tree` resolved is
/// the hub's directory or lies under it (a link to it), a link inside `tree` resolves into it,
/// an object file of the tree with more than one link is the same file (device and inode) as
/// the hub's at the same path, as `git clone` of a path makes, or an alternate the tree borrows
/// from inside the hub, as `git clone --shared` makes.
fn shares_objects(tree: &Path, hub: &Path) -> bool {
    let hub = std::fs::canonicalize(hub).unwrap_or_else(|_| hub.to_path_buf());
    let into_hub = |path: &Path| std::fs::canonicalize(path).is_ok_and(|dir| dir.starts_with(&hub));
    if into_hub(tree) {
        return true;
    }
    if let Ok(text) = std::fs::read_to_string(tree.join("info/alternates"))
        && text
            .lines()
            .map(str::trim)
            .any(|line| !line.is_empty() && into_hub(&tree.join(line)))
    {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let mut dirs = vec![tree.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = std::fs::symlink_metadata(&path) else {
                    continue;
                };
                if meta.is_dir() {
                    dirs.push(path);
                } else if meta.is_symlink() && into_hub(&path) {
                    return true;
                } else if meta.is_file()
                    && meta.nlink() > 1
                    && let Ok(relative) = path.strip_prefix(tree)
                    && std::fs::symlink_metadata(hub.join(relative)).is_ok_and(|theirs| {
                        (theirs.dev(), theirs.ino()) == (meta.dev(), meta.ino())
                    })
                {
                    return true;
                }
            }
        }
    }
    false
}

impl Local {
    /// The directory of the hub the request records are in.
    pub const REQUESTS: &str = "takt/requests";

    /// The limits `local_checks` is held to: an hour, 8 MiB of each output stream kept.
    pub const CHECKS_LIMITS: Limits = Limits {
        timeout: Duration::from_secs(3600),
        output: 8 << 20,
    };

    /// Over the hub `tree`'s `origin` names, the checks `checks` (the run file's `local_checks`,
    /// held to [`Self::CHECKS_LIMITS`]), `state` the directory the checks' clones and output go
    /// under, and `identity` the run's commit identity the merge commits carry. Refused, before
    /// anything is written, when `checks` names no program, or `origin` is not a local path to a
    /// bare repository outside the tree ([`Self::hub_of`]).
    pub fn for_tree(
        tree: &Path,
        checks: Vec<String>,
        state: impl Into<PathBuf>,
        identity: CommitIdentity,
    ) -> Result<Self> {
        if checks
            .first()
            .is_none_or(|program| program.trim().is_empty())
        {
            bail!(
                "`local_checks` is an argument list whose first element is the program, and it names none; nothing was opened"
            );
        }
        Ok(Self {
            hub: Self::hub_of(tree)?,
            checks,
            state: state.into(),
            identity,
            limits: Self::CHECKS_LIMITS,
            min_free: 0,
            judged: RefCell::new(BTreeMap::new()),
        })
    }

    /// The checks held to `limits` instead.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// The checks run only while the state directory's file system, and that of
    /// `CARGO_TARGET_DIR` when it is set, has `bytes` free, read before each run: the run's
    /// `--min-free-gib`. Under it the read of the checks is an error, nothing run or recorded.
    #[must_use]
    pub fn with_min_free(mut self, bytes: u64) -> Self {
        self.min_free = bytes;
        self
    }

    /// The hub `tree`'s `origin` names, canonical: a local path (a relative one read from the
    /// tree) or a `file://` URL with no host, to a bare repository that is neither inside the
    /// tree nor holds it, whose git directory the tree's repository does not share (a worktree
    /// of the hub, or a repository inside it) and whose object files it does not hard-link, link
    /// to or borrow (`git clone --no-local` makes such a tree). Anything else is refused naming
    /// the URL, nothing written.
    pub fn hub_of(tree: &Path) -> Result<PathBuf> {
        let url = crate::git::remote_url(tree, "origin").map_err(|_| {
            anyhow!(
                "--forge local merges into the bare repository the tree's origin remote names, and the tree has no origin; nothing was opened"
            )
        })?;
        let shown = super::without_userinfo(&url);
        let path = local_path(&url).ok_or_else(|| {
            anyhow!(
                "--forge local merges into a bare repository on this machine, and origin {shown} is no local path; nothing was opened"
            )
        })?;
        let tree = std::fs::canonicalize(tree)
            .with_context(|| format!("resolve the tree {}", tree.display()))?;
        let hub = std::fs::canonicalize(tree.join(&path)).with_context(|| {
            format!(
                "--forge local: origin {shown} is no directory on this machine; nothing was opened"
            )
        })?;
        if hub.starts_with(&tree) || tree.starts_with(&hub) {
            bail!(
                "--forge local: origin {shown} is inside the tree or holds it, where a step could move its refs; the hub is a bare repository outside the tree; nothing was opened"
            );
        }
        let common = crate::git::run(
            &tree,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .ok()
        .and_then(|dir| std::fs::canonicalize(dir).ok())
        .ok_or_else(|| {
            anyhow!(
                "--forge local: the tree {} names no git directory; nothing was opened",
                tree.display()
            )
        })?;
        if common.starts_with(&hub) {
            bail!(
                "--forge local: the tree's repository shares its git directory with origin {shown}, where a step could move the hub's refs; the tree is a clone of the hub, not a worktree of it or a repository inside it; nothing was opened"
            );
        }
        let bare = crate::git::run(&hub, &["rev-parse", "--is-bare-repository"]).ok();
        let git_dir = crate::git::run(&hub, &["rev-parse", "--absolute-git-dir"])
            .ok()
            .and_then(|dir| std::fs::canonicalize(dir).ok());
        if bare.as_deref() != Some("true") || git_dir.as_deref() != Some(hub.as_path()) {
            bail!(
                "--forge local: origin {shown} is not a bare repository; the hub is one, outside the tree; nothing was opened"
            );
        }
        if shares_objects(&common.join("objects"), &hub.join("objects")) {
            bail!(
                "--forge local: the tree's object files are origin {shown}'s own (a `git clone` of a path hard-links them, `--shared` borrows them, a symbolic link points at them), so a step that writes the tree's git directory writes the hub's objects; clone the hub with `git clone --no-local`; nothing was opened"
            );
        }
        Ok(hub)
    }

    /// The hub.
    #[must_use]
    pub fn hub(&self) -> &Path {
        &self.hub
    }

    /// The record of request `number`.
    fn record(&self, number: u64) -> PathBuf {
        self.hub.join(Self::REQUESTS).join(format!("{number}.json"))
    }

    /// Request `number` as the hub keeps it.
    fn read(&self, number: u64) -> Result<Request> {
        let path = self.record(number);
        let text = std::fs::read(&path)
            .with_context(|| format!("no request #{number} in the hub: read {}", path.display()))?;
        serde_json::from_slice(&text).with_context(|| {
            format!(
                "request #{number} in the hub: {} does not read",
                path.display()
            )
        })
    }

    /// Writes request `number` whole, through a file beside it renamed over it, so a reader
    /// never reads half of it.
    fn write(&self, number: u64, request: &Request) -> Result<()> {
        let path = self.record(number);
        let next = path.with_extension(format!("json.{}.next", std::process::id()));
        let bytes = serde_json::to_vec_pretty(request).context("serialize the request")?;
        std::fs::write(&next, bytes).with_context(|| format!("write {}", next.display()))?;
        std::fs::rename(&next, &path).with_context(|| format!("write {}", path.display()))
    }

    /// The commit the hub's branch `branch` is at, read by its full name.
    fn tip(&self, branch: &str) -> Option<String> {
        exact_ref(&self.hub, &format!("refs/heads/{branch}"))
    }

    /// [`Self::tip`], or an error naming the branch.
    fn tip_of(&self, branch: &str) -> Result<String> {
        self.tip(branch)
            .ok_or_else(|| anyhow!("the hub {} has no branch {branch}", self.hub.display()))
    }

    /// The head request `number` merged, for a record that says merged. What is believed: the
    /// record names the head and the commit that holds it, that commit is on the hub's base, and
    /// the head is that commit or an ancestor of it (takt's merge holds it as its second parent;
    /// a merge made by hand anywhere in its history, through a commit built on the head; a base
    /// that reached the head without a merge commit as itself); the branch, while the hub still
    /// has it, is at the head.
    /// Otherwise an error: a record says merged of a merge the hub does not hold, and nothing is
    /// believed of it.
    fn merged_head(&self, number: u64, request: &Request) -> Result<String> {
        let untrusted = |why: &str| {
            anyhow!(
                "request #{number} in the hub says merged, and {why}; the record is not believed"
            )
        };
        let merge = request
            .merge
            .as_deref()
            .filter(|merge| is_object_id(merge))
            .ok_or_else(|| untrusted("names no merge commit"))?;
        let head = request
            .head
            .as_deref()
            .filter(|head| is_object_id(head))
            .ok_or_else(|| untrusted("names no head"))?;
        let base = self.tip_of(&request.base)?;
        crate::git::run(&self.hub, &["merge-base", "--is-ancestor", merge, &base])
            .map_err(|_| untrusted(&format!("its merge {merge} is not on {}", request.base)))?;
        crate::git::run(&self.hub, &["merge-base", "--is-ancestor", head, merge])
            .map_err(|_| untrusted(&format!("its merge {merge} does not hold the head {head}")))?;
        if let Some(tip) = self.tip(&request.branch)
            && tip != head
        {
            return Err(untrusted(&format!(
                "its merge {merge} holds {head}, while {} is at {tip}",
                request.branch
            )));
        }
        Ok(head.to_owned())
    }

    /// The key a verdict of these checks on `head` is recorded under: `<head> sha256:<digest>`,
    /// the digest of the `local_checks` argument list as a JSON array, so a verdict under
    /// another argument list is never read as one of these.
    fn verdict_key(&self, head: &str) -> String {
        use sha2::Digest as _;
        let argv = serde_json::to_vec(&self.checks).unwrap_or_default();
        format!("{head} sha256:{:x}", sha2::Sha256::digest(argv))
    }

    /// Whether the checks may run: the state directory's file system, and that of
    /// `CARGO_TARGET_DIR` when it is set and absolute, each has [`Local::with_min_free`]'s bytes
    /// free, read now. A file system under it is an error naming it; one that cannot be read is
    /// not checked, as the run's own floor is not.
    fn room_for_checks(&self, number: u64, head: &str) -> Result<()> {
        if self.min_free == 0 {
            return Ok(());
        }
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute());
        for path in std::iter::once(self.state.clone()).chain(target) {
            if let Ok(free) = crate::run::free_space(&path)
                && free.bytes < self.min_free
            {
                bail!(
                    "the checks on request #{number} at {head} were not run: {} has {} bytes free, under the {} the run needs (--min-free-gib); nothing recorded",
                    path.display(),
                    free.bytes,
                    self.min_free
                );
            }
        }
        Ok(())
    }

    /// The commit on `tip`'s first-parent line that holds `head`, which `tip` holds: the newest
    /// whose first parent does not hold it, or `head` itself when `head` is on that line.
    fn holder(&self, tip: &str, head: &str) -> Result<String> {
        let line = crate::git::run(
            &self.hub,
            &["rev-list", "--first-parent", tip, &format!("^{head}")],
        )?;
        let mut holder = head.to_owned();
        for commit in line.lines() {
            let first = format!("{commit}^1");
            let holds =
                crate::git::run(&self.hub, &["merge-base", "--is-ancestor", head, &first]).is_ok();
            if !holds {
                holder = commit.to_owned();
                break;
            }
        }
        Ok(holder)
    }

    /// Runs the checks on `head` of request `number` as the test command is run
    /// ([`crate::confinement::Confinement::run_test`]), in a fresh repository under the state
    /// directory that fetched `head` from the hub by its id, shares no object with it and names
    /// no remote, removed after; their output kept beside it. A run past the timeout fails.
    /// Under `--confinement none` they inherit the run's `CARGO_TARGET_DIR`, as the test command
    /// does, so runs at once share one build cache; under `substrate` the adapter admits only the
    /// workspace's own `target/` as writable, as it does for the test command, so the checks
    /// build there, in the fresh repository, and share nothing.
    fn run_checks(&self, number: u64, head: &str) -> Result<Checks> {
        let dir = self.state.join("checks");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let name = format!("{number}-{head}-{}", std::process::id());
        let clone = dir.join(&name);
        let _ = std::fs::remove_dir_all(&clone);
        let hub = self.hub.to_string_lossy().into_owned();
        let ran = (|| {
            crate::git::run(&dir, &["init", "--quiet", "--template=", &name])?;
            crate::git::run(
                &clone,
                &[
                    "fetch",
                    "--quiet",
                    "--no-tags",
                    "--no-write-fetch-head",
                    "--recurse-submodules=no",
                    &hub,
                    head,
                ],
            )?;
            crate::git::run(&clone, &["checkout", "--quiet", "--detach", head])?;
            crate::confinement::current().run_test(&crate::confinement::TestRun {
                argv: &self.checks,
                tree: &clone,
                limits: self.limits,
            })
        })();
        let _ = std::fs::remove_dir_all(&clone);
        let (finished, timed_out) = match ran {
            Ok(finished) => (finished, false),
            Err(error) => match error.downcast::<TimedOut>() {
                Ok(timed_out) => (timed_out.output, true),
                Err(error) => {
                    return Err(error.context(format!(
                        "the checks on request #{number} at {head} could not be run in a clone of the hub"
                    )));
                }
            },
        };
        let passed = !timed_out && finished.success();
        let log = dir.join(format!("{number}-{head}.log"));
        let mut text = finished.stdout;
        text.extend_from_slice(&finished.stderr);
        if timed_out {
            text.extend_from_slice(
                format!(
                    "takt: the checks ran past {:?} and were stopped\n",
                    self.limits.timeout
                )
                .as_bytes(),
            );
        }
        std::fs::write(&log, text).with_context(|| format!("write {}", log.display()))?;
        Ok(if passed { Checks::Pass } else { Checks::Fail })
    }
}

impl Forge for Local {
    fn name(&self) -> &str {
        "local"
    }

    /// Scans nothing: the hub is on this machine, and nothing the push sends leaves it.
    fn scan_outgoing(&self, _: &Path, _: &Path, _: &Outgoing<'_>) -> Result<()> {
        Ok(())
    }

    /// Scans nothing, as [`Self::scan_outgoing`]: no commit leaves the machine.
    fn scan_identity(&self, _: &Path, _: &Path, _: &CommitIdentity) -> Result<IdentityScan> {
        Ok(IdentityScan::NotScanned(
            "the forge local's hub is on this machine, and nothing leaves it".to_owned(),
        ))
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let at = exact_ref(tree, &format!("refs/heads/{branch}"))
            .ok_or_else(|| anyhow!("{branch} names no commit; nothing was pushed"))?;
        self.push_revision(tree, branch, &at)
    }

    /// `git push` of the commit `revision` to the hub as `branch`, no tag and no submodule with
    /// it; a push the hub does not take (not a fast-forward) is the error. A git that writes the
    /// tracking ref of a remote whose URL is the hub's on such a push (2.56 does, 2.55 does not)
    /// has it put back, so the push moves no ref of the tree, which the step guard would charge
    /// to `pr.open`.
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        let hub = self.hub.to_string_lossy().into_owned();
        let before = tracking_refs(tree)?;
        let pushed = crate::git::run(
            tree,
            &[
                "push",
                "--quiet",
                "--no-follow-tags",
                "--recurse-submodules=no",
                &hub,
                &format!("{revision}:refs/heads/{branch}"),
            ],
        )
        .map(drop)
        .with_context(|| format!("push {branch} to the hub {hub}"));
        put_back_tracking(tree, &before)?;
        pushed
    }

    /// Writes the request's record under the lowest number no record holds, taken with an
    /// exclusive create, so a number held is passed over and never written over; the head and
    /// the base must be branches of the hub.
    fn open_pull_request(&self, _: &Path, _: &Path, request: &PullRequest) -> Result<u64> {
        self.tip_of(&request.branch)?;
        self.tip_of(&request.base)?;
        let dir = self.hub.join(Self::REQUESTS);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let record = Request {
            branch: request.branch.clone(),
            base: request.base.clone(),
            title: request.title.clone(),
            body: request.body.clone(),
            state: PullRequestState::Open.as_str().to_owned(),
            checks: BTreeMap::new(),
            merge: None,
            head: None,
        };
        let bytes = serde_json::to_vec_pretty(&record).context("serialize the request")?;
        let mut number = 1;
        loop {
            let path = self.record(number);
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(&bytes)
                        .with_context(|| format!("write {}", path.display()))?;
                    return Ok(number);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => number += 1,
                Err(error) => {
                    return Err(error).with_context(|| format!("create {}", path.display()));
                }
            }
        }
    }

    /// The record's state and its head as the hub holds them; a record that says merged is
    /// believed only of a merge the hub holds ([`Local::merged_head`]).
    fn pull_request(&self, _: &Path, number: u64) -> Result<PullRequestView> {
        let request = self.read(number)?;
        let (state, sha) = match request.state.as_str() {
            "open" => (PullRequestState::Open, self.tip_of(&request.branch)?),
            "merged" => (
                PullRequestState::Merged,
                self.merged_head(number, &request)?,
            ),
            "closed" => (PullRequestState::Closed, self.tip_of(&request.branch)?),
            other => bail!(
                "request #{number} in the hub has the state `{other}`, which takt does not write"
            ),
        };
        Ok(PullRequestView {
            state,
            head: Head {
                sha,
                branch: request.branch,
            },
            base: request.base,
        })
    }

    /// Behind when the hub's base tip is not an ancestor of the request's head, the hub's
    /// `refs/heads/<branch>`; not known for a request that is not open.
    fn behind_base(&self, _: &Path, number: u64) -> Result<Option<bool>> {
        let request = self.read(number)?;
        if request.state != PullRequestState::Open.as_str() {
            return Ok(None);
        }
        let head = self.tip_of(&request.branch)?;
        let tip = self.tip_of(&request.base)?;
        let holds = crate::git::run(
            &self.hub,
            &[
                "merge-base",
                "--is-ancestor",
                "--end-of-options",
                &tip,
                &head,
            ],
        )
        .is_ok();
        Ok(Some(!holds))
    }

    /// A pass recorded for the head under these checks ([`Local::verdict_key`]), else `pending`
    /// while another live process runs them on it, else the checks run now
    /// ([`Local::run_checks`]) and their verdict recorded; a fail recorded is run again, so a
    /// flaky check or a timeout does not hold the head for good, and a pass under another
    /// `local_checks` is no pass of these.
    fn checks(&self, _: &Path, number: u64) -> Result<Checks> {
        let mut request = self.read(number)?;
        let head = self.tip_of(&request.branch)?;
        let key = self.verdict_key(&head);
        match request.checks.get(&key) {
            Some(verdict) if verdict.result == "pass" => return Ok(Checks::Pass),
            Some(Verdict { pid: Some(pid), .. })
                if *pid != std::process::id() && !super::exited(*pid) =>
            {
                return Ok(Checks::Pending);
            }
            _ => {}
        }
        self.room_for_checks(number, &head)?;
        request.checks.insert(
            key.clone(),
            Verdict {
                result: Checks::Pending.as_str().to_owned(),
                pid: Some(std::process::id()),
            },
        );
        self.write(number, &request)?;
        let ran = self.run_checks(number, &head);
        let mut request = self.read(number)?;
        match &ran {
            Ok(checks) => {
                request.checks.insert(
                    key,
                    Verdict {
                        result: checks.as_str().to_owned(),
                        pid: None,
                    },
                );
            }
            Err(_) => {
                request.checks.remove(&key);
            }
        }
        self.write(number, &request)?;
        ran
    }

    /// [`Self::merge_with`], the message `Merge request #<n>: <title>`.
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        let title = format!("Merge request #{number}: {}", self.read(number)?.title);
        self.merge_with(
            tree,
            dir,
            number,
            head,
            &MergeMessage {
                title,
                body: String::new(),
            },
        )
    }

    /// Merges in the hub, with no work tree: the head must still be `head`; the merge goes onto
    /// the base's tip last reported by [`Self::target_tip`], else its tip now. A tip that already
    /// holds `head` (a run that ended between the move of the base and the record, or a merge
    /// made by hand) moves no ref: the record is marked merged by the commit that holds it
    /// ([`Local::holder`]). A conflict is [`LocalRefused::Conflict`]; else the merge commit, the
    /// tip its first parent and `head` its second, carries the run's identity and `message`, and
    /// the base moves to it only from that tip ([`LocalRefused::BaseMoved`]). The record is
    /// marked merged after, with the merge and the head.
    fn merge_with(
        &self,
        _: &Path,
        _: &Path,
        number: u64,
        head: &str,
        message: &MergeMessage,
    ) -> Result<()> {
        let mut request = self.read(number)?;
        if request.state != PullRequestState::Open.as_str() {
            bail!(
                "request #{number} is {}, not open; nothing merged",
                request.state
            );
        }
        let at = self.tip_of(&request.branch)?;
        if at != head {
            bail!(
                "request #{number} has the head {} at {at}, not {head}; not merged",
                request.branch
            );
        }
        let base = request.base.clone();
        let judged = self.judged.borrow_mut().remove(&base);
        let tip = match judged {
            Some(tip) => tip,
            None => self.tip_of(&base)?,
        };
        if crate::git::run(&self.hub, &["merge-base", "--is-ancestor", head, &tip]).is_ok() {
            request.state = PullRequestState::Merged.as_str().to_owned();
            request.merge = Some(self.holder(&tip, head)?);
            request.head = Some(head.to_owned());
            return self.write(number, &request);
        }
        let merged = crate::git::merged_tree(&self.hub, &tip, head)?;
        if !merged.conflicted.is_empty() {
            return Err(LocalRefused::Conflict {
                number,
                base,
                paths: merged.conflicted,
            }
            .into());
        }
        let text = if message.body.is_empty() {
            format!("{}\n", message.title)
        } else {
            format!("{}\n\n{}\n", message.title, message.body)
        };
        let commit = crate::git::commit_tree(
            &self.hub,
            &merged.tree,
            &[&tip, head],
            &text,
            &self.identity,
        )?;
        let reference = format!("refs/heads/{base}");
        let reason = format!("takt: merge request #{number}");
        if let Err(error) = crate::git::run(
            &self.hub,
            &["update-ref", "-m", &reason, &reference, &commit, &tip],
        ) {
            let found = self.tip(&base);
            if found.as_deref() != Some(tip.as_str()) {
                return Err(LocalRefused::BaseMoved {
                    number,
                    base,
                    expected: tip,
                    found,
                }
                .into());
            }
            return Err(error.context(format!("move {base} in the hub to the merge {commit}")));
        }
        request.state = PullRequestState::Merged.as_str().to_owned();
        request.merge = Some(commit);
        request.head = Some(head.to_owned());
        self.write(number, &request)
    }

    /// The hub's `refs/heads/<base>`, fetched by its id into the tree's repository with no ref
    /// written, so the tree holds it and no branch, tag or tracking ref a step can see has moved;
    /// remembered as the tip the next merge into `base` goes onto.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        let tip = self.tip_of(base)?;
        if !is_object_id(&tip) {
            bail!(
                "the hub reports {base} at `{}`, which is no object id",
                tip.escape_debug()
            );
        }
        let hub = self.hub.to_string_lossy().into_owned();
        crate::git::run(
            tree,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-write-fetch-head",
                "--recurse-submodules=no",
                &hub,
                &tip,
            ],
        )
        .with_context(|| format!("fetch {base} at {tip} from the hub {hub}"))?;
        self.judged
            .borrow_mut()
            .insert(base.to_owned(), tip.clone());
        Ok(tip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tracking ref the push made is deleted and one it moved is moved back; the others stay.
    #[test]
    fn tracking_refs_a_push_wrote_are_put_back() {
        let dir = std::env::temp_dir().join(format!("takt-local-tracking-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory");
        let git = |args: &[&str]| crate::git::run(&dir, args).expect("git runs");
        let commit = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
        ];
        git(&["init", "--quiet", "--template=", "-b", "main"]);
        git(&[&commit[..], &["--quiet", "--allow-empty", "-m", "one"]].concat());
        let one = git(&["rev-parse", "HEAD"]);
        git(&[&commit[..], &["--quiet", "--allow-empty", "-m", "two"]].concat());
        let two = git(&["rev-parse", "HEAD"]);
        git(&["update-ref", "refs/remotes/origin/main", &one]);
        git(&["update-ref", "refs/remotes/origin/kept", &one]);
        let before = tracking_refs(&dir).expect("the refs are read");
        git(&["update-ref", "refs/remotes/origin/main", &two]);
        git(&["update-ref", "refs/remotes/origin/takt/case", &two]);
        put_back_tracking(&dir, &before).expect("put back");
        assert_eq!(tracking_refs(&dir).expect("the refs are read"), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_local_path_is_a_path_or_a_file_url_without_a_host() {
        for (url, path) in [
            ("/srv/hub.git", Some("/srv/hub.git")),
            ("../hub.git", Some("../hub.git")),
            ("hub.git", Some("hub.git")),
            ("./a:b.git", Some("./a:b.git")),
            ("file:///srv/hub.git", Some("/srv/hub.git")),
            ("file://host/srv/hub.git", None),
            ("https://forge.example/g/p.git", None),
            ("ssh://git@forge.example/g/p.git", None),
            ("git@forge.example:g/p.git", None),
            ("forge.example:p.git", None),
            ("", None),
        ] {
            assert_eq!(local_path(url), path.map(PathBuf::from), "{url}");
        }
    }
}
