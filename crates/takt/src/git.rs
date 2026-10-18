//! The few git reads and writes the kernel makes in a worktree, without a shell. A push goes
//! through the forge ([`crate::forge`]), under the bot route's credential, never through here.
//!
//! What a run commits or discards is only ever its own: a case opens on a clean tree
//! ([`uncommitted`]), a judgement step is measured against a [`Checkpoint`] taken before it, an
//! accepted step commits exactly the paths it changed ([`commit_paths`]), and a refused or failed
//! one is put back to the checkpoint, the index, `HEAD`, ignored files and `.git/` included.
//! Paths are read from `git status --porcelain=v1 -z`, never from trimmed or quoted text.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::case::{CASES_DIR, is_under_cases};

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
/// writes a commit-graph after every commit); none reaches a git takt runs.
const REDIRECTING_ENV: [&str; 13] = [
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
];

/// Runs `git <args>` in `tree` with `input` on its standard input, with no code from
/// configuration ([`NO_CONFIGURED_CODE`]) and no credential in its environment; answers what
/// it printed, whatever its exit status. Git runs pinned to the tree: its `.git` is the git
/// directory and the tree the work tree, so a `.git` a step garbled makes the call fail rather
/// than find a repository further up; and for a tree without a `.git` git searches no further
/// than the tree itself.
fn output(tree: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Output> {
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
    if let Some(parent) = tree.parent() {
        command.env("GIT_CEILING_DIRECTORIES", parent);
    }
    // A replace ref (`refs/replace/<object>`) would make every read of an object return
    // another's content: no git takt runs honours one.
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
    // Nor a commit-graph, a cache of the history a step can forge (a parent re-pointed, its
    // checksum fixed): every ancestry and history takt reads comes from the commits themselves.
    let mut child = command
        .args(NO_CONFIGURED_CODE)
        .args(["-c", "core.commitGraph=false"])
        .args(args)
        .current_dir(tree)
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

/// Runs `git <args>` in `tree`, `input` on its standard input; answers its standard output as
/// it is, byte for byte, or an error naming what git said when it exits non-zero.
fn raw(tree: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    let output = output(tree, args, input)?;
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
/// white space is part of the answer; those are read byte for byte with `-z`.
pub fn run(tree: &Path, args: &[&str]) -> Result<String> {
    let stdout = raw(tree, args, None)?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

/// The bytes of the blob `object` names (`<revision>:<path>`, or a blob id) as git holds them:
/// nothing trimmed, nothing decoded.
pub fn blob(tree: &Path, object: &str) -> Result<Vec<u8>> {
    raw(tree, &["cat-file", "blob", object], None)
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

/// The URL of `remote`; an error when the tree has no such remote.
pub fn remote_url(tree: &Path, remote: &str) -> Result<String> {
    run(tree, &["remote", "get-url", remote])
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

/// The identity takt commits under.
const IDENTITY: [&str; 4] = ["-c", "user.name=takt", "-c", "user.email=takt@localhost"];

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
/// new `HEAD`.
pub fn commit_paths(tree: &Path, paths: &[String], message: &str) -> Result<String> {
    let mut commit: Vec<&str> = Vec::from(IDENTITY);
    commit.extend(["commit", "-q", "--only", "-m", message]);
    if paths.is_empty() {
        commit.push("--allow-empty");
        raw(tree, &commit, None)?;
    } else {
        let list = pathspec(paths);
        let from_input = ["--pathspec-from-file=-", "--pathspec-file-nul"];
        let mut add = vec!["--literal-pathspecs", "add", "-A"];
        add.extend(from_input);
        raw(tree, &add, Some(&list))?;
        let mut literal = vec!["--literal-pathspecs"];
        literal.extend(commit);
        literal.extend(from_input);
        raw(tree, &literal, Some(&list))?;
    }
    head(tree)
}

/// Discards every uncommitted change but what is under the cases directory: the index and the
/// tracked files back to `HEAD`, untracked files removed; ignored files stay. For a tree that was
/// clean when the work being discarded began, so that everything uncommitted is that work's.
pub fn discard_uncommitted(tree: &Path) -> Result<()> {
    let keep = format!("{CASES_DIR}/");
    run(tree, &["reset", "-q", "--hard", "HEAD"])?;
    run(tree, &["clean", "-fdq", "-e", &keep])?;
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
/// `submodule.*`, `extensions.*`. Any other key (`branch.<name>.vscode-merge-base`, a tool's
/// own section) is not.
fn is_guarded_key(key: &str) -> bool {
    let section = key.split('.').next().unwrap_or_default();
    match section {
        "core" | "filter" | "diff" | "merge" | "alias" | "include" | "includeif" | "credential"
        | "url" | "gpg" | "sequence" | "pager" | "trailer" | "difftool" | "mergetool"
        | "submodule" | "extensions" => true,
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
/// every replace ref, and every other branch and tag of the repository (the case's base, a
/// sibling case's branch) but one checked out in another worktree, which is that session's
/// ([`unguarded_refs`]), the
/// case's base and its own branch excepted ([`Checkpoint::take_guarding`]); the index's entries
/// and the index file itself; every uncommitted path outside the cases directory with what was
/// at it (a validator's run may leave one); the ignored paths with what is at each a file git
/// names; and what git runs, reads as configuration, or reads to find the objects and the
/// history in the git directories: the hooks, `info/`, the configuration's keys, `commondir`,
/// `gitdir`, `shallow`, `objects/info/alternates` and the commit-graph.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    head: String,
    branch: Option<String>,
    refs: BTreeMap<String, String>,
    unguarded: BTreeSet<String>,
    index: Vec<u8>,
    index_file: Option<(PathBuf, Vec<u8>)>,
    dirty: BTreeMap<String, Content>,
    ignored: BTreeMap<String, Content>,
    dot_git: Content,
    head_file: (PathBuf, Content),
    index_lock: (PathBuf, bool),
    git_dirs: Vec<GitDir>,
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
    /// Ignored paths the step created.
    pub ignored_added: Vec<String>,
    /// Ignored files git names that the step rewrote.
    pub ignored_changed: Vec<String>,
    /// Ignored paths the step removed.
    pub ignored_removed: Vec<String>,
    /// Those of the rewritten or removed ignored paths of which nothing was kept to put back (a
    /// whole ignored directory, a file too large to keep or unreadable).
    pub ignored_lost: Vec<String>,
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

/// The refs a checkpoint guards beside `HEAD`: every branch, tag and replace ref of the
/// repository, each with the object it names, but those `unguarded` names ([`unguarded_refs`]).
fn guarded_refs(tree: &Path, unguarded: &BTreeSet<String>) -> Result<BTreeMap<String, String>> {
    let listed = raw(
        tree,
        &[
            "for-each-ref",
            "--format=%(objectname) %(refname)",
            "refs/heads/",
            "refs/tags/",
            "refs/replace/",
        ],
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
/// itself.
fn ignored_contents(tree: &Path, entries: &[Entry]) -> Result<BTreeMap<String, Content>> {
    ignored(entries)
        .into_iter()
        .map(|path| {
            Content::of(&tree.join(path.trim_end_matches('/'))).map(|content| (path, content))
        })
        .collect()
}

impl Checkpoint {
    /// Takes the checkpoint of `tree`, guarding `HEAD` and its branch, every replace ref, and
    /// every other branch and tag of the repository but one another worktree has checked out.
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
        let shown = |path: &Path| match path.strip_prefix(tree) {
            Ok(inside) => inside.display().to_string(),
            Err(_) => path.display().to_string(),
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
        Ok(change)
    }

    /// Puts the tree back to this checkpoint: everything the step did is discarded and
    /// everything that was there before it, uncommitted or not, is there again. What git runs or
    /// reads as configuration first (a path it could not put back was reported by
    /// [`Self::put_back_git_dir`] and is not again here); then `HEAD`, its branch and the
    /// guarded refs; the tracked files and the index to `HEAD`; untracked files and the ignored
    /// paths the step created removed; ignored files git names written again as they were; the
    /// paths uncommitted before written again as they were; the index file as it was. The cases
    /// directory is not touched here (it is the kernel's, put back by the run). Of an ignored
    /// directory, a file too large to keep or one that could not be read nothing is kept to put
    /// back. Checked afterwards: a tree that still differs is an error, not a refusal.
    pub fn restore(&self, tree: &Path) -> Result<()> {
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
        let keep = format!("{CASES_DIR}/");
        run(tree, &["reset", "-q", "--hard", &self.head])?;
        run(tree, &["clean", "-fdq", "-e", &keep])?;
        let now = ignored(&status(tree, true)?);
        for path in &now {
            if !self.ignored.contains_key(path) {
                Content::Absent.put_back(&tree.join(path.trim_end_matches('/')))?;
            }
        }
        for (path, content) in &self.ignored {
            if content.can_be_put_back() && *content != Content::Dir {
                content.put_back(&tree.join(path.trim_end_matches('/')))?;
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
            bail!("the tree could not be put back as it was before the step: {left:?}");
        }
        Ok(())
    }
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
    let output = output(tree, &args, None)?;
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

/// The tree merging `revision` into `base` would leave: `git merge-tree --write-tree`, which
/// writes the merge's tree object and touches neither the work tree nor the index, with no code
/// from configuration ([`NO_CONFIGURED_CODE`]). A merge with conflicts still answers its tree,
/// the conflicted files in it as git leaves them; any exit but 0 (clean) or 1 (conflicts) is an
/// error naming what git said.
pub fn merged_tree(tree: &Path, base: &str, revision: &str) -> Result<String> {
    let args = [
        "merge-tree",
        "--write-tree",
        "--no-messages",
        base,
        revision,
    ];
    let output = output(tree, &args, None)?;
    match output.status.code() {
        Some(0 | 1) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let merged = stdout.lines().next().unwrap_or_default().trim().to_owned();
            if merged.is_empty() {
                bail!("git {} in {} named no tree", args.join(" "), tree.display());
            }
            Ok(merged)
        }
        _ => bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
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
