//! The Workspace seam: the outer lifecycle of a dispatched child's working tree
//! (`design:continuous-orchestration` § 6), a sixth seam beside the five of `docs/design/takt.md`
//! § 3. It creates a managed worktree for a case (`worktree create --purpose takt:<case>`),
//! takes a lease on it (`worktree hook session-start`), and leaves it when the child's run
//! stops: archived and released after a stop that is not terminal, so the child stays resumable
//! in its tree; released and finished (`worktree finish --discard-cache --archive`) after a
//! terminal outcome.
//!
//! The seam is called only through the `worktree` command line, each call one process started
//! with an argument list, never through a shell. It never runs `gc`, never removes a tree, and
//! acts only on a tree it created: the one a case's `worktree` names, whose registry record it
//! checks for the case's purpose and an active lifecycle before it takes a lease on it. A tree
//! found by its purpose alone is no tree of the seam's.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::Value;

use crate::confinement::Command;

/// The purpose prefix of every tree the seam creates: `takt:<case id>`.
pub const PURPOSE_PREFIX: &str = "takt:";

/// How a dispatched child's run ended, to the seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    /// A stop that is not a terminal outcome (a failed setup, an approval to wait for, a spent
    /// budget): the tree is archived and the lease released, and the child resumes in it.
    Stopped,
    /// A terminal outcome (accepted, abandoned): the lease is released and the tree finished.
    Terminal,
}

/// A managed worktree the seam made for one case, and holds a lease on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    /// The id the `worktree` registry gave the tree.
    pub id: String,
    /// The tree's absolute path, as the registry records it.
    pub path: PathBuf,
    /// The case the tree was made for.
    pub case: String,
}

/// A refusal of the `worktree` command line: the code and the message it answered with.
#[derive(Debug)]
struct Refused {
    code: String,
    message: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Refused {}

/// The seam: the `worktree` program it starts, the lease session it holds its leases as, and the
/// environment it adds to each call.
#[derive(Debug, Clone)]
pub struct Workspace {
    program: OsString,
    session: String,
    env: Vec<(OsString, OsString)>,
}

impl Workspace {
    /// A seam that starts `worktree`, found where a process looks for a program, and holds its
    /// leases as the session `session`.
    #[must_use]
    pub fn new(session: impl Into<String>) -> Self {
        Self {
            program: OsString::from("worktree"),
            session: session.into(),
            env: Vec::new(),
        }
    }

    /// The same seam starting `program` in place of `worktree`.
    #[must_use]
    pub fn program(mut self, program: impl Into<OsString>) -> Self {
        self.program = program.into();
        self
    }

    /// The same seam adding `name=value` to the environment of each call.
    #[must_use]
    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    /// The lease session the seam holds its leases as.
    #[must_use]
    pub fn session(&self) -> &str {
        &self.session
    }

    /// Enters the tree of `case`, taking a lease on it. With none `recorded`, the tree is made:
    /// `worktree create --repo <repo> --purpose takt:<case>`. With the path a case's
    /// `worktree` recorded, that tree is reused, after the registry is found to hold it for
    /// that purpose and still active; a second entry makes no second tree, and a tree the
    /// registry holds for another purpose, or no longer active, is refused and left as it is.
    /// A tree made here whose lease could not be taken is finished again, since nothing is in
    /// it.
    pub fn enter(&self, repo: &Path, case: &str, recorded: Option<&Path>) -> Result<Tree> {
        let purpose = format!("{PURPOSE_PREFIX}{case}");
        let (tree, made) = match recorded {
            Some(path) => (self.registered(path, case, &purpose)?, false),
            None => (self.create(repo, case, &purpose)?, true),
        };
        if let Err(error) = self.lease(&tree) {
            if made {
                let _ = self.finish(&tree);
            }
            return Err(error.context(format!(
                "no lease could be taken on the tree {} of the case {case}",
                tree.path.display()
            )));
        }
        Ok(tree)
    }

    /// Leaves the tree as the child's run ended: [`Leave::Stopped`] archives it and releases the
    /// seam's own lease; [`Leave::Terminal`] releases it and finishes the tree. The lease is
    /// released whatever the archive did, and an error from either step is returned once both
    /// were tried.
    pub fn leave(&self, tree: &Tree, how: Leave) -> Result<()> {
        match how {
            Leave::Stopped => {
                let archived = self.archive(tree);
                let released = self.release(tree);
                archived?;
                released
            }
            Leave::Terminal => {
                self.release(tree)?;
                self.finish(tree)
            }
        }
    }

    /// One call of the `worktree` program with `args`, `--json` after them: what it answered, or
    /// the refusal it made.
    fn call(&self, args: &[&OsStr]) -> Result<Value> {
        let output = Command::new(&self.program)
            .args(args)
            .arg("--json")
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("start {}", self.program.to_string_lossy()))?;
        let parsed = serde_json::from_slice::<Value>(&output.stdout)
            .or_else(|_| serde_json::from_slice::<Value>(&output.stderr));
        if output.status.success() {
            return Ok(parsed.unwrap_or(Value::Null));
        }
        let shown: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        match parsed {
            Ok(answer) if answer["code"].is_string() => Err(Refused {
                code: answer["code"].as_str().unwrap_or_default().to_owned(),
                message: format!(
                    "worktree {}: {}",
                    shown.join(" "),
                    answer["message"].as_str().unwrap_or("refused")
                ),
            }
            .into()),
            _ => bail!(
                "worktree {} ended {}: {}",
                shown.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }
    }

    /// `worktree create --repo <repo> --purpose <purpose>`: the tree it made.
    fn create(&self, repo: &Path, case: &str, purpose: &str) -> Result<Tree> {
        let answer = self.call(&[
            "create".as_ref(),
            "--repo".as_ref(),
            repo.as_os_str(),
            "--purpose".as_ref(),
            purpose.as_ref(),
        ])?;
        let evidence = &answer["evidence"];
        let (Some(id), Some(path)) = (evidence["id"].as_str(), evidence["path"].as_str()) else {
            bail!("worktree create named no tree for the case {case}: {answer}");
        };
        Ok(Tree {
            id: id.to_owned(),
            path: PathBuf::from(path),
            case: case.to_owned(),
        })
    }

    /// The registry's record of the tree at `path`, checked to be one made for `purpose` and
    /// active: what `worktree status` lists.
    fn registered(&self, path: &Path, case: &str, purpose: &str) -> Result<Tree> {
        let answer = self.call(&["status".as_ref()])?;
        let records = answer["records"]
            .as_array()
            .ok_or_else(|| anyhow!("worktree status listed no records: {answer}"))?;
        let Some(record) = records
            .iter()
            .find(|record| record["path"].as_str().map(Path::new) == Some(path))
        else {
            bail!(
                "the case {case} names the tree {}, which the worktree registry does not hold; nothing was changed",
                path.display()
            );
        };
        if record["purpose"] != purpose {
            bail!(
                "the case {case} names the tree {}, which the worktree registry holds for another purpose than {purpose}; it is not the seam's, and was left as it is",
                path.display()
            );
        }
        if record["lifecycle"] != "active" {
            bail!(
                "the case {case} names the tree {}, which is {}, not active; nothing was changed",
                path.display(),
                record["lifecycle"]
                    .as_str()
                    .unwrap_or("in no known lifecycle")
            );
        }
        let id = record["id"]
            .as_str()
            .ok_or_else(|| anyhow!("worktree status gave the tree {} no id", path.display()))?;
        Ok(Tree {
            id: id.to_owned(),
            path: path.to_path_buf(),
            case: case.to_owned(),
        })
    }

    /// `worktree hook session-start`: the seam's lease on the tree, taken or renewed.
    fn lease(&self, tree: &Tree) -> Result<()> {
        self.call(&[
            "hook".as_ref(),
            "session-start".as_ref(),
            "--session".as_ref(),
            self.session.as_ref(),
            "--path".as_ref(),
            tree.path.as_os_str(),
        ])
        .map(drop)
    }

    /// `worktree hook session-end`: the seam's own lease on the tree, released; another
    /// session's is not touched.
    fn release(&self, tree: &Tree) -> Result<()> {
        self.call(&[
            "hook".as_ref(),
            "session-end".as_ref(),
            "--session".as_ref(),
            self.session.as_ref(),
            "--path".as_ref(),
            tree.path.as_os_str(),
        ])
        .map(drop)
    }

    /// `worktree archive <tree>`; where the tree already has an archive, `--replace`, which
    /// moves the old one aside and deletes nothing.
    fn archive(&self, tree: &Tree) -> Result<()> {
        let args: [&OsStr; 2] = ["archive".as_ref(), tree.path.as_os_str()];
        match self.call(&args) {
            Err(error)
                if error
                    .downcast_ref::<Refused>()
                    .is_some_and(|refused| refused.code == "archive-exists") =>
            {
                self.call(&[
                    "archive".as_ref(),
                    "--replace".as_ref(),
                    tree.path.as_os_str(),
                ])
                .map(drop)
            }
            other => other.map(drop),
        }
    }

    /// `worktree finish --discard-cache --archive <tree>`.
    fn finish(&self, tree: &Tree) -> Result<()> {
        self.call(&[
            "finish".as_ref(),
            "--discard-cache".as_ref(),
            "--archive".as_ref(),
            tree.path.as_os_str(),
        ])
        .map(drop)
    }
}
