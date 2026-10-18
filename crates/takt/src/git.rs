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
/// names is not covered: its name is the repository's choice.
const NO_CONFIGURED_CODE: [&str; 16] = [
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
];

/// Runs `git <args>` in `tree` with `input` on its standard input, with no code from
/// configuration ([`NO_CONFIGURED_CODE`]) and no credential in its environment; answers what
/// it printed, whatever its exit status.
fn output(tree: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Output> {
    let mut command = Command::new("git");
    crate::forge::without_credentials(&mut command, |_| false);
    let mut child = command
        .args(NO_CONFIGURED_CODE)
        .args(args)
        .current_dir(tree)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
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

/// What is at one path: nothing, a file with its bytes and whether it is executable, a link with
/// its target, or a directory, taken as itself.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Content {
    Absent,
    File { bytes: Vec<u8>, executable: bool },
    Link(PathBuf),
    Dir,
}

impl Content {
    /// What is at `path` now; a link is read as the link, never followed.
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
        if kind.is_symlink() {
            let target =
                std::fs::read_link(path).with_context(|| format!("read {}", path.display()))?;
            Ok(Self::Link(target))
        } else if kind.is_dir() {
            Ok(Self::Dir)
        } else {
            let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
            Ok(Self::File {
                bytes,
                executable: executable(&meta),
            })
        }
    }

    /// Puts `path` back to this: nothing when it is this already (a directory is never emptied
    /// to be made again); else whatever is there now goes (a link as the link, a directory with
    /// what is in it), then the file, link or directory is made again.
    fn put_back(&self, path: &Path) -> Result<()> {
        if Self::of(path)? == *self {
            return Ok(());
        }
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if meta.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            }
            .with_context(|| format!("remove {}", path.display()))?;
        }
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
        }
        Ok(())
    }
}

#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    false
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

/// The entries at the top of a git directory that are git's own state, which git itself writes
/// while it works (objects, refs, logs, the index, the `*_HEAD` files, a merge or rebase in
/// progress, other worktrees, submodules' repositories). Everything else there, the hooks, the
/// config, `info/` and any file nobody named, is guarded: git reads it, and runs some of it,
/// inside takt's own git calls.
const GIT_OWN: &[&str] = &[
    "objects",
    "refs",
    "logs",
    "worktrees",
    "modules",
    "index",
    "packed-refs",
    "HEAD",
    "ORIG_HEAD",
    "FETCH_HEAD",
    "MERGE_HEAD",
    "MERGE_MSG",
    "MERGE_MODE",
    "MERGE_AUTOSTASH",
    "AUTO_MERGE",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "REBASE_HEAD",
    "BISECT_LOG",
    "BISECT_START",
    "BISECT_TERMS",
    "BISECT_EXPECTED_REV",
    "BISECT_ANCESTORS_OK",
    "BISECT_NAMES",
    "BISECT_RUN",
    "MERGE_RR",
    "COMMIT_EDITMSG",
    "SQUASH_MSG",
    "TAG_EDITMSG",
    "sequencer",
    "rebase-merge",
    "rebase-apply",
    "rr-cache",
    "lfs",
    "gc.log",
    "gc.pid",
    "shallow",
    "fsmonitor--daemon",
    "fsmonitor--daemon.ipc",
];

/// Every guarded entry under the git directory `dir`, by path relative to it.
fn guarded(dir: &Path) -> Result<BTreeMap<PathBuf, Content>> {
    fn walk(root: &Path, dir: &Path, entries: &mut BTreeMap<PathBuf, Content>) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
            let path = entry?.path();
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if dir == root {
                let name = relative.to_string_lossy();
                if GIT_OWN.contains(&name.as_ref()) || name.ends_with(".lock") {
                    continue;
                }
            }
            let content = Content::of(&path)?;
            if content == Content::Dir {
                walk(root, &path, entries)?;
            } else {
                entries.insert(relative, content);
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    walk(dir, dir, &mut entries)?;
    Ok(entries)
}

/// The tree as it was before a judgement step: what the step's change is measured against and
/// what a refused or failed step is put back to. `HEAD` and its branch; the index's entries and
/// the index file itself; every uncommitted path outside the cases directory with what was at
/// it (a validator's run may leave one); the ignored paths; and the guarded part of the git
/// directories, read from the file system so that putting it back runs no git at all.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    head: String,
    branch: Option<String>,
    index: Vec<u8>,
    index_file: Option<(PathBuf, Vec<u8>)>,
    dirty: BTreeMap<String, Content>,
    ignored: BTreeSet<String>,
    dot_git: Content,
    git_dirs: Vec<(PathBuf, BTreeMap<PathBuf, Content>)>,
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
    /// Ignored paths the step created.
    pub ignored_added: Vec<String>,
    /// Ignored paths the step removed; nothing was kept of them to put back.
    pub ignored_removed: Vec<String>,
}

/// The index's entries: mode, object, stage, the assume-unchanged and skip-worktree flags, path.
fn index_entries(tree: &Path) -> Result<Vec<u8>> {
    raw(tree, &["ls-files", "-z", "--stage", "-v"], None)
}

fn short(revision: &str) -> &str {
    &revision[..8.min(revision.len())]
}

impl Checkpoint {
    /// Takes the checkpoint of `tree`.
    pub fn take(tree: &Path) -> Result<Self> {
        let git_dir = PathBuf::from(run(tree, &["rev-parse", "--absolute-git-dir"])?);
        let common = PathBuf::from(run(
            tree,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?);
        let mut git_dirs = vec![(common.clone(), guarded(&common)?)];
        if git_dir != common {
            git_dirs.push((git_dir.clone(), guarded(&git_dir)?));
        }
        // A `.git` directory is guarded through its contents; a `.git` file (a linked
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
        Ok(Self {
            head: head(tree)?,
            branch: current_branch(tree)?,
            index: index_entries(tree)?,
            index_file,
            dirty,
            ignored: ignored(&entries),
            dot_git,
            git_dirs,
        })
    }

    /// Puts the guarded part of the git directories and the tree's `.git` back as they were,
    /// from the file system alone: no git runs before this, because git reads and runs what a
    /// step may have planted there (a hook, `core.fsmonitor`, a filter, `info/exclude`).
    /// Answers the paths that differed, as `.git/<path>` for the tree's own git directory.
    pub fn put_back_git_dir(&self, tree: &Path) -> Result<Vec<String>> {
        let mut touched = Vec::new();
        let dot_git = tree.join(".git");
        let now = Content::of(&dot_git)?;
        if now != self.dot_git {
            if self.dot_git == Content::Dir {
                bail!(
                    "{} was a directory and is no longer one; it cannot be put back",
                    dot_git.display()
                );
            }
            self.dot_git.put_back(&dot_git)?;
            touched.push(".git".to_owned());
        }
        for (dir, before) in &self.git_dirs {
            let after = guarded(dir)?;
            let differing: BTreeSet<&PathBuf> = before
                .keys()
                .chain(after.keys())
                .filter(|path| before.get(*path) != after.get(*path))
                .collect();
            for path in differing {
                before
                    .get(path)
                    .unwrap_or(&Content::Absent)
                    .put_back(&dir.join(path))?;
                let shown = dir.join(path);
                touched.push(match shown.strip_prefix(tree) {
                    Ok(inside) => inside.display().to_string(),
                    Err(_) => shown.display().to_string(),
                });
            }
        }
        Ok(touched)
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
        let ignored = ignored(&entries);
        Ok(StepChange {
            already_uncommitted: paths
                .iter()
                .filter(|path| self.dirty.contains_key(*path))
                .cloned()
                .collect(),
            paths: paths.into_iter().collect(),
            staged: index_entries(tree)? != self.index,
            head: moved,
            ignored_added: ignored.difference(&self.ignored).cloned().collect(),
            ignored_removed: self.ignored.difference(&ignored).cloned().collect(),
        })
    }

    /// Puts the tree back to this checkpoint: everything the step did is discarded and
    /// everything that was there before it, uncommitted or not, is there again. `.git/` first;
    /// then `HEAD` and its branch; the tracked files and the index to `HEAD`; untracked files
    /// and the ignored paths the step created removed; the paths uncommitted before written
    /// again as they were; the index file as it was. The cases directory is not touched here
    /// (it is the kernel's, put back by the run). Ignored paths the step removed cannot be put
    /// back. Checked afterwards: a tree that still differs is an error, not a refusal.
    pub fn restore(&self, tree: &Path) -> Result<()> {
        self.put_back_git_dir(tree)?;
        match &self.branch {
            Some(branch) => {
                let reference = format!("refs/heads/{branch}");
                if current_branch(tree)?.as_deref() != Some(branch.as_str()) {
                    run(tree, &["symbolic-ref", "HEAD", &reference])?;
                }
                let at = output(tree, &["rev-parse", "--verify", "-q", &reference], None)?;
                if String::from_utf8_lossy(&at.stdout).trim() != self.head {
                    run(tree, &["update-ref", &reference, &self.head])?;
                }
            }
            None => {
                if head(tree)? != self.head || current_branch(tree)?.is_some() {
                    run(tree, &["update-ref", "--no-deref", "HEAD", &self.head])?;
                }
            }
        }
        let keep = format!("{CASES_DIR}/");
        run(tree, &["reset", "-q", "--hard", &self.head])?;
        run(tree, &["clean", "-fdq", "-e", &keep])?;
        let now = ignored(&status(tree, true)?);
        for path in now.difference(&self.ignored) {
            Content::Absent.put_back(&tree.join(path.trim_end_matches('/')))?;
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
            || !left.ignored_added.is_empty()
        {
            bail!("the tree could not be put back as it was before the step: {left:?}");
        }
        Ok(())
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
}
