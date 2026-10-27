//! The `Frontier` seam: the planning store behind a trait, the ready rule as a pure function
//! over the artifacts the store lists, and one adapter, [`Aep`], which reads and writes the
//! store only through the `aep` command line. No model takes part in the choice of the unit.
//!
//! A story is ready when it is `proposed`, nothing blocks it and every artifact it depends on
//! is done (`implemented` or `archived`); among the ready ones the first in id order is the
//! unit. Taking a story moves it `proposed` → `active`, so a story with an open case is never
//! taken twice. A store may refuse the move; then the story is not taken, the refusal is printed
//! verbatim and the next ready story is tried. A run opens the case before it takes the story
//! and removes the case again when the store refuses ([`crate::Run::open_next`]), so a story is
//! never `active` without a case to work it. A story with an open case is resumed by the next
//! run, never given a second case: the case directory records the story, its take is committed
//! on the case's branch, and the store on the base says which stories are being worked; which
//! case a run resumes, and from which branch, is [`crate::Run::next_to_resume`]'s. The operator's grant of a capability to a case is
//! recorded on the story as `approval` evidence through the same adapter ([`approve`]). A run on
//! a story whose case terminates `accepted` records the `test_result` pass record a validator of
//! the case wrote as [`TEST_RESULT`] evidence on the story, at the record's own instant, and
//! moves the story to [`IMPLEMENTED`]; each write is a commit on the case's branch, store paths
//! only ([`write_on_case_branch`]), made as [`ACTOR`], and a refusal is relayed, never retried.
//! With a forge both are made before the merge and ride to the base with it
//! ([`crate::case::approvals_only`]).

use std::path::Path;

use anyhow::{Context as _, Result, anyhow};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::case::Approval;
use crate::confinement::Command;

/// The status a story is ready at: refined and agreed enough to pick up.
pub const READY: &str = "proposed";
/// The status a taken story is moved to: being worked on now.
pub const TAKEN: &str = "active";
/// The statuses an artifact a story depends on is done at.
pub const DONE: [&str; 2] = ["implemented", "archived"];
/// The status a story whose case terminated `accepted` is moved to.
pub const IMPLEMENTED: &str = "implemented";
/// The evidence kind a run records on the story it works.
pub const TEST_RESULT: &str = "test_result";
/// The source the store records for the `test_result` evidence a run writes.
pub const SOURCE: &str = "takt";
/// Who the store records as acting for a run's writes: the evidence's actor and the move's
/// executor.
pub const ACTOR: &str = "agent:takt";
/// The variable `aep` reads the actor of a write from.
const ACTOR_VARIABLE: &str = "AEP_ACTOR";

/// A story taken from the planning store: the unit of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    /// The artifact id, `story:<name>`.
    pub id: String,
    /// The title.
    pub title: String,
    /// The markdown body, without the frontmatter.
    pub body: String,
}

impl Story {
    /// The case's `intent.md`: the title, a blank line, the body.
    #[must_use]
    pub fn intent(&self) -> String {
        format!("{}\n\n{}", self.title, self.body)
    }

    /// A case id from the story id and `instant`: `story-<name>-<instant>`, the instant as
    /// [`crate::now`] writes it with its separators dropped.
    #[must_use]
    pub fn case_id(&self, instant: &str) -> String {
        format!(
            "{}-{}",
            self.id.replace(':', "-"),
            instant.replace([':', '-'], "")
        )
    }
}

/// What a run works on: an intent given on the command line, or a story the frontier pulled
/// from the planning store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    /// An intent, as given.
    Intent(String),
    /// A story the frontier took.
    Story(Story),
}

/// The planning store behind a trait: it names the ready stories in the order to try them and
/// the stories being worked, hands one over with its body, records in the store that a story is
/// being worked, and records an approval on it.
pub trait Frontier {
    /// The adapter's name.
    fn name(&self) -> &str;
    /// The ids of the ready stories of the store in `repo`, in the order to try them.
    fn candidates(&self, repo: &Path) -> Result<Vec<String>>;
    /// The ids of the stories the store in `repo` lists [`TAKEN`], being worked: one with an
    /// open case is resumed.
    fn active(&self, repo: &Path) -> Result<Vec<String>>;
    /// The story `id`, with its body.
    fn show(&self, repo: &Path, id: &str) -> Result<Story>;
    /// Tells the store the story `id` is being worked, `proposed` → `active`, the store judging
    /// the move; its refusal, verbatim, when it would not.
    fn take(&self, repo: &Path, id: &str) -> Result<Take>;
    /// Records `approval` on the story `story` as one `approval` evidence record, appended and
    /// never edited: the trail of the grant written into the case.
    fn record_approval(&self, repo: &Path, story: &str, approval: &Approval) -> Result<()>;
    /// Records on the story `story` one [`TEST_RESULT`] evidence record, source [`SOURCE`],
    /// referring to `reference`, the record's path in the repository with its sha256 and fields
    /// ([`test_result_reference`]), at `at`, that record's `observed_at`; the store's refusal,
    /// verbatim, when it would not. An adapter that records
    /// none refuses.
    fn record_test_result(
        &self,
        _repo: &Path,
        story: &str,
        _reference: &str,
        _at: &str,
    ) -> Result<Written> {
        Ok(Written::Refused(format!(
            "the frontier {} records no {TEST_RESULT} evidence on {story}",
            self.name()
        )))
    }
    /// Moves the story `story` to [`IMPLEMENTED`], the store judging the move; its refusal,
    /// verbatim, when it would not. An adapter that moves none refuses.
    fn implemented(&self, _repo: &Path, story: &str) -> Result<Written> {
        Ok(Written::Refused(format!(
            "the frontier {} moves no story to {IMPLEMENTED}: {story}",
            self.name()
        )))
    }
}

/// An artifact as `aep plan artifact list --format json` lists it: the fields the rule reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listed {
    /// The artifact id, `<kind>:<name>`.
    pub id: String,
    /// The kind, `story` for a unit.
    pub kind: String,
    /// The status on the kind's ladder.
    pub status: String,
    /// The title.
    #[serde(default)]
    pub title: String,
    /// The edges leaving the artifact.
    #[serde(default)]
    pub relations: Vec<Relation>,
    /// What the store says blocks the artifact.
    #[serde(default)]
    pub blocked_by: Vec<Blocker>,
}

/// An edge from the listed artifact to `target`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Relation {
    /// The relation, `depends_on` among others.
    pub relation: String,
    /// The artifact the edge points at.
    pub target: String,
}

/// Something the store says blocks the listed artifact.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Blocker {
    /// The blocking artifact's id.
    pub blocker: String,
}

/// The ready rule: `story` is a story at [`READY`], nothing blocks it, and every artifact it
/// `depends_on` is listed at one of [`DONE`]. A dependency the store does not list is not done.
#[must_use]
pub fn is_ready(story: &Listed, all: &[Listed]) -> bool {
    story.kind == "story"
        && story.status == READY
        && story.blocked_by.is_empty()
        && story
            .relations
            .iter()
            .filter(|r| r.relation == "depends_on")
            .all(|r| {
                all.iter()
                    .any(|a| a.id == r.target && DONE.contains(&a.status.as_str()))
            })
}

/// The ready stories among `all`, in id order; the first is the unit.
#[must_use]
pub fn ready(all: &[Listed]) -> Vec<&Listed> {
    let mut ready: Vec<&Listed> = all.iter().filter(|a| is_ready(a, all)).collect();
    ready.sort_by(|a, b| a.id.cmp(&b.id));
    ready
}

/// What the store answered a take with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Take {
    /// The story was moved to [`TAKEN`].
    Taken,
    /// The store refused the move; what it said, verbatim.
    Refused(String),
}

/// What the store answered a run's write with: an evidence record or a move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Written {
    /// The store made the write.
    Done,
    /// The store refused it; what it said, verbatim.
    Refused(String),
}

/// The subject of the commit that holds a story's [`TEST_RESULT`] record on the case's branch:
/// `takt: test_result <story>`.
#[must_use]
pub fn test_result_subject(story: &str) -> String {
    format!("takt: {TEST_RESULT} {story}")
}

/// The reference the store records for a `test_result` pass record of the case: the record's
/// path in the repository, then `#sha256=<hex>&result=<result>&revision=<revision>&producer=<producer>`,
/// the sha256 lower-case hex of the record's `bytes` and the other three the record's own fields.
/// A reader of the store's line finds the record's bytes and what they say without the case
/// directory, which another clone does not hold.
#[must_use]
pub fn test_result_reference(
    path: &str,
    bytes: &[u8],
    result: &str,
    revision: &str,
    producer: &str,
) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{path}#sha256={hex}&result={result}&revision={revision}&producer={producer}")
}

/// The subject of the commit that holds a story's move to [`IMPLEMENTED`] on the case's branch:
/// `takt: implemented <story>`.
#[must_use]
pub fn implemented_subject(story: &str) -> String {
    format!("takt: {IMPLEMENTED} {story}")
}

/// Whether `path`, relative to the tree, is one a run's write for `story` may change: an
/// artifact of the planning store, or a file in the story's own evidence directory
/// (`.engineering/evidence/story/<name>/`).
#[must_use]
pub fn is_story_store_path(path: &str, story: &str) -> bool {
    let evidence = format!(".engineering/evidence/{}/", story.replacen(':', "/", 1));
    path.starts_with(STORE_DIRS[0])
        || path
            .strip_prefix(&evidence)
            .is_some_and(|name| !name.is_empty())
}

/// Makes one store write for `story` on the case's branch `branch` and commits exactly what it
/// changed as `subject`, through [`crate::git`]: the commit's sha, `None` when it changed
/// nothing, or the store's refusal verbatim. The tree must be on `branch` and clean outside the
/// cases directory first, or nothing is written; what the write changed must lie in the planning
/// store or the story's evidence directory ([`is_story_store_path`]), or it is discarded. Every
/// such failure is the error, with nothing left behind; the implementation revision is not
/// the run's to move here and is not moved.
///
/// `recorded` writes the run's record of the write, given the commit (`None` when the write
/// changed nothing), as soon as the commit is made and before anything else: the two stand or
/// fall together, and a record that cannot be written takes the commit back, the case's branch
/// reset to where it was, and is the error. A refusal records nothing here; it is the
/// caller's to record. The commit carries `identity`, the case's ([`crate::case::CaseDir::committer`]).
pub fn write_on_case_branch(
    tree: &Path,
    branch: &str,
    identity: &crate::git::CommitIdentity,
    story: &str,
    subject: &str,
    write: impl FnOnce() -> Result<Written>,
    recorded: impl FnOnce(Option<&str>) -> Result<()>,
) -> Result<std::result::Result<Option<String>, String>> {
    let on = crate::git::current_branch(tree)?;
    if on.as_deref() != Some(branch) {
        return Err(anyhow!(
            "the tree {} is on {}, not the case's branch {branch}: the store's write is committed there; nothing was written",
            tree.display(),
            on.as_deref().unwrap_or("a detached HEAD")
        ));
    }
    let uncommitted = crate::git::uncommitted(tree)?;
    if !uncommitted.is_empty() {
        return Err(anyhow!(
            "the tree {} has uncommitted work: {}; the store's write is committed on {branch} with exactly what it changes; nothing was written",
            tree.display(),
            uncommitted.join(", ")
        ));
    }
    let before = crate::git::head(tree)?;
    let discard = |why: String| -> anyhow::Error {
        match crate::git::discard_uncommitted(tree) {
            Ok(()) => anyhow!("{why}; what it changed was discarded"),
            Err(error) => anyhow!("{why}; what it changed could not be discarded: {error:#}"),
        }
    };
    let written = match write() {
        Ok(written) => written,
        Err(error) => {
            return Err(if crate::git::uncommitted(tree)?.is_empty() {
                error
            } else {
                discard(format!("{error:#}"))
            });
        }
    };
    let changed = crate::git::uncommitted(tree)?;
    if let Written::Refused(refusal) = written {
        if !changed.is_empty() {
            return Err(discard(format!(
                "the store refused the write on {story} ({refusal}) and changed {}",
                changed.join(", ")
            )));
        }
        return Ok(Err(refusal));
    }
    if changed.is_empty() {
        recorded(None)?;
        return Ok(Ok(None));
    }
    if let Some(outside) = changed
        .iter()
        .find(|path| !is_story_store_path(path, story))
    {
        return Err(discard(format!(
            "the store's write on {story} changed {outside}, outside the planning store and the story's evidence"
        )));
    }
    let head = crate::git::commit_paths(tree, identity, &changed, subject).map_err(|error| {
        discard(format!(
            "the store's write on {story} could not be committed on {branch}: {error:#}"
        ))
    })?;
    if let Err(error) = recorded(Some(&head)) {
        return Err(
            match crate::git::run(tree, &["reset", "-q", "--hard", &before]) {
                Ok(_) => error.context(format!(
                    "the store's write on {story} could not be recorded; its commit {head} was taken back, {branch} reset to {before}"
                )),
                Err(reset) => error.context(format!(
                    "the store's write on {story} could not be recorded, and its commit {head} could not be taken back: {reset:#}"
                )),
            },
        );
    }
    Ok(Ok(Some(head)))
}

/// The directories of the planning store a record in it may change, relative to the tree: the
/// artifacts and their evidence.
pub const STORE_DIRS: [&str; 2] = [".engineering/planning/", ".engineering/evidence/"];

/// Whether `path`, relative to the tree, lies in the planning store ([`STORE_DIRS`]).
#[must_use]
pub fn is_store_path(path: &str) -> bool {
    STORE_DIRS.iter().any(|dir| path.starts_with(dir))
}

/// The subject of the commit that holds an approval's record on the case's branch:
/// `takt: approve <story> <capability>`.
#[must_use]
pub fn approve_subject(story: &str, capability: &str) -> String {
    format!("takt: approve {story} {capability}")
}

/// What `takt approve <case> <capability> --by <approver>` does: checks the grant and holds the
/// case ([`crate::case::prepare_grant`]), records it on the story through `frontier` for a case
/// opened for a story, then writes the grant into the case's `authority.yaml` at the revision
/// the case reached the gate at. The record comes first, so a record the store refuses leaves
/// no grant behind and the same approval can simply be given again; the case is held
/// throughout, so no run starts between the record and the grant. A case opened on an intent
/// has no story, and the grant is all.
///
/// The record a story's case gets is committed on the case's branch, exactly the paths it
/// changed in the planning store, as [`approve_subject`], so the run that merges observes a
/// clean tree; that commit is the approval's own and leaves the case at the revision the grant
/// authorises ([`crate::case::CaseDir::resume`]). So the tree must be on the case's branch and
/// clean before anything is recorded, or the approval is refused with nothing recorded; a
/// record that changes a path outside the store, or a commit that fails, is discarded with no
/// grant written, and the same approval can be given again. The commit carries the identity the
/// case was opened with ([`crate::case::commit_identity_of`]), takt's own when it recorded none.
pub fn approve(
    tree: &Path,
    frontier: &dyn Frontier,
    case: &str,
    capability: &str,
    approver: &str,
) -> Result<Approval> {
    let pending = crate::case::prepare_grant(tree, case, capability, approver)?;
    if let Some(story) = &pending.approval().story {
        let branch = crate::case::Branch::name_of(case);
        let on = crate::git::current_branch(tree)?;
        if on.as_deref() != Some(branch.as_str()) {
            return Err(anyhow!(
                "the tree {} is on {}, not the case's branch {branch}: the approval's record is committed there; check it out and approve again; nothing was recorded",
                tree.display(),
                on.as_deref().unwrap_or("a detached HEAD")
            ));
        }
        let uncommitted = crate::git::uncommitted(tree)?;
        if !uncommitted.is_empty() {
            return Err(anyhow!(
                "the tree {} has uncommitted work: {}; the approval's record is committed on {branch} with exactly what it changes: commit the work, or move it out of the tree, first; nothing was recorded",
                tree.display(),
                uncommitted.join(", ")
            ));
        }
        frontier
            .record_approval(tree, story, pending.approval())
            .with_context(|| {
                format!(
                    "recording the approval on {story} through the frontier {} failed; no grant is written, give the approval again",
                    frontier.name()
                )
            })?;
        let identity =
            crate::case::commit_identity_of(&tree.join(crate::case::CASES_DIR).join(case))?
                .unwrap_or_else(crate::git::CommitIdentity::takt);
        commit_record(tree, &identity, &branch, story, capability)?;
    }
    let reference = pending.approval().reference();
    let story = pending.approval().story.clone();
    pending.write().with_context(|| match story {
        Some(story) => format!(
            "the approval is recorded on {story}, and the grant could not be written to {reference}"
        ),
        None => format!("the grant could not be written to {reference}"),
    })
}

/// Commits what the approval's record changed, on a tree that was clean before it: exactly
/// those paths, which must all lie in the planning store, as [`approve_subject`], by `identity`.
/// Anything else, or a commit that fails, discards the record and is the error; no grant is
/// written then.
fn commit_record(
    tree: &Path,
    identity: &crate::git::CommitIdentity,
    branch: &str,
    story: &str,
    capability: &str,
) -> Result<()> {
    let changed = crate::git::uncommitted(tree)?;
    if changed.is_empty() {
        return Ok(());
    }
    let discard = |why: String| -> anyhow::Error {
        match crate::git::discard_uncommitted(tree) {
            Ok(()) => anyhow!(
                "{why}; the record was discarded and no grant written; give the approval again"
            ),
            Err(error) => anyhow!(
                "{why}; the record could not be discarded ({error:#}) and no grant was written"
            ),
        }
    };
    if let Some(outside) = changed.iter().find(|path| !is_store_path(path)) {
        return Err(discard(format!(
            "recording the approval on {story} changed {outside}, outside the planning store"
        )));
    }
    crate::git::commit_paths(
        tree,
        identity,
        &changed,
        &approve_subject(story, capability),
    )
    .map_err(|error| {
        discard(format!(
            "the approval's record could not be committed on {branch}: {error:#}"
        ))
    })?;
    Ok(())
}

/// The adapter over the `aep` command line, run in the repository: `list` and `show` read the
/// store, `move` and `evidence` are the two writes, and the lifecycle the store enforces is the
/// store's own. Every call is a child held to [`Aep::limits`] with no credential in its
/// environment.
#[derive(Debug, Clone)]
pub struct Aep {
    /// The command, `aep` on the path by default.
    pub command: String,
    /// What each `aep` child is held to.
    pub limits: crate::forge::Limits,
}

impl Default for Aep {
    fn default() -> Self {
        Self {
            command: "aep".to_owned(),
            limits: crate::forge::Limits {
                timeout: std::time::Duration::from_secs(120),
                output: 16 << 20,
            },
        }
    }
}

impl Aep {
    /// Runs `aep plan artifact <args>` in `repo`, held to the limits and with no credential:
    /// what it printed on success, or what it printed on a non-zero exit, trimmed, as the
    /// inner error. A reply longer than the limit is an error, never read cut.
    fn plan(&self, repo: &Path, args: &[&str]) -> Result<std::result::Result<String, String>> {
        self.plan_as(repo, args, None)
    }

    /// [`Aep::plan`], the child told who acts (`AEP_ACTOR`) when `actor` names one, so the store
    /// attributes the write to it rather than to the person the process runs as.
    fn plan_as(
        &self,
        repo: &Path,
        args: &[&str],
        actor: Option<&str>,
    ) -> Result<std::result::Result<String, String>> {
        let mut command = Command::new(&self.command);
        command
            .args(["plan", "artifact"])
            .args(args)
            .current_dir(repo);
        crate::forge::without_credentials(&mut command, |_| false);
        if let Some(actor) = actor {
            command.env(ACTOR_VARIABLE, actor);
        }
        let output = crate::forge::run_child(&mut command, self.limits)
            .with_context(|| format!("run {} plan artifact {}", self.command, args.join(" ")))?;
        if output.stdout_cut {
            return Err(anyhow!(
                "{} plan artifact {} printed more than {} bytes; its reply was cut and is not read",
                self.command,
                args.join(" "),
                self.limits.output
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.success() {
            return Ok(Ok(stdout));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(Err(format!("{stdout}{stderr}").trim().to_owned()))
    }

    /// Refuses a repository without a planning store: `aep` would otherwise walk up from it
    /// and find a store above it.
    fn store(repo: &Path) -> Result<()> {
        if repo.join(".engineering/project.yaml").is_file() {
            Ok(())
        } else {
            Err(anyhow!(
                "the repository {} has no planning store: no .engineering/project.yaml",
                repo.display()
            ))
        }
    }

    /// Every artifact the store lists.
    pub fn list(&self, repo: &Path) -> Result<Vec<Listed>> {
        Self::store(repo)?;
        let json = self
            .plan(repo, &["list", "--format", "json"])?
            .map_err(|refusal| anyhow!("aep plan artifact list: {refusal}"))?;
        serde_json::from_str(&json).context("read the artifact list")
    }
}

impl Frontier for Aep {
    fn name(&self) -> &str {
        "aep"
    }

    /// `list`, the ready rule, id order.
    fn candidates(&self, repo: &Path) -> Result<Vec<String>> {
        let listed = self.list(repo)?;
        Ok(ready(&listed).into_iter().map(|a| a.id.clone()).collect())
    }

    /// `list`, the stories at [`TAKEN`], id order.
    fn active(&self, repo: &Path) -> Result<Vec<String>> {
        let mut active: Vec<String> = self
            .list(repo)?
            .into_iter()
            .filter(|a| a.kind == "story" && a.status == TAKEN)
            .map(|a| a.id)
            .collect();
        active.sort();
        Ok(active)
    }

    /// `show <id> --format json`; the repository's own store only, like `list`.
    fn show(&self, repo: &Path, id: &str) -> Result<Story> {
        #[derive(Deserialize)]
        struct Shown {
            id: String,
            title: String,
            #[serde(default)]
            body: String,
        }
        Self::store(repo)?;
        let json = self
            .plan(repo, &["show", id, "--format", "json"])?
            .map_err(|refusal| anyhow!("aep plan artifact show {id}: {refusal}"))?;
        let shown: Shown = serde_json::from_str(&json).with_context(|| format!("read {id}"))?;
        Ok(Story {
            id: shown.id,
            title: shown.title,
            body: shown.body,
        })
    }

    /// `move <id> --to active`, the first write; the repository's own store only, like `list`.
    fn take(&self, repo: &Path, id: &str) -> Result<Take> {
        Self::store(repo)?;
        Ok(
            match self.plan(
                repo,
                &["move", id, "--to", TAKEN, "--executor", "agent:takt"],
            )? {
                Ok(_) => Take::Taken,
                Err(refusal) => Take::Refused(refusal),
            },
        )
    }

    /// `evidence <story> --kind approval --source "takt approve <capability> --by <approver> at
    /// <revision>" --ref <case>/authority.yaml --at <granted at>`, written before the grant: the
    /// source names the command, the capability, the approver and the revision of the grant's
    /// subject the grant authorises, the reference the grant, the instant the grant's own; as
    /// the approver (`AEP_ACTOR`), so the record's actor is who granted; the repository's own
    /// store only, like `list`.
    fn record_approval(&self, repo: &Path, story: &str, approval: &Approval) -> Result<()> {
        Self::store(repo)?;
        let source = format!(
            "takt approve {} --by {} at {}",
            approval.capability, approval.approver, approval.revision
        );
        let reference = approval.reference();
        self.plan_as(
            repo,
            &[
                "evidence",
                story,
                "--kind",
                "approval",
                "--source",
                &source,
                "--ref",
                &reference,
                "--at",
                &approval.granted_at,
            ],
            Some(&approval.approver),
        )?
        .map(drop)
        .map_err(|refusal| anyhow!("aep plan artifact evidence {story}: {refusal}"))
    }

    /// `evidence <story> --kind test_result --source takt --ref <reference> --at <at>`, as
    /// [`ACTOR`]: the reference the record in the case, the instant the record's own, never one
    /// chosen here; the repository's own store only, like `list`.
    fn record_test_result(
        &self,
        repo: &Path,
        story: &str,
        reference: &str,
        at: &str,
    ) -> Result<Written> {
        Self::store(repo)?;
        Ok(
            match self.plan_as(
                repo,
                &[
                    "evidence",
                    story,
                    "--kind",
                    TEST_RESULT,
                    "--source",
                    SOURCE,
                    "--ref",
                    reference,
                    "--at",
                    at,
                ],
                Some(ACTOR),
            )? {
                Ok(_) => Written::Done,
                Err(refusal) => Written::Refused(refusal),
            },
        )
    }

    /// `move <story> --to implemented --executor agent:takt`, as [`ACTOR`], which the store's
    /// lifecycle allows once the story holds a `test_result` record; the repository's own store
    /// only, like `list`.
    fn implemented(&self, repo: &Path, story: &str) -> Result<Written> {
        Self::store(repo)?;
        Ok(
            match self.plan_as(
                repo,
                &["move", story, "--to", IMPLEMENTED, "--executor", ACTOR],
                Some(ACTOR),
            )? {
                Ok(_) => Written::Done,
                Err(refusal) => Written::Refused(refusal),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(id: &str, status: &str) -> Listed {
        Listed {
            id: id.to_owned(),
            kind: id.split(':').next().expect("a kind").to_owned(),
            status: status.to_owned(),
            title: String::new(),
            relations: vec![],
            blocked_by: vec![],
        }
    }

    fn depends(mut artifact: Listed, target: &str) -> Listed {
        artifact.relations.push(Relation {
            relation: "depends_on".to_owned(),
            target: target.to_owned(),
        });
        artifact
    }

    fn blocked(mut artifact: Listed, by: &str) -> Listed {
        artifact.blocked_by.push(Blocker {
            blocker: by.to_owned(),
        });
        artifact
    }

    fn ids(all: &[Listed]) -> Vec<&str> {
        ready(all).into_iter().map(|a| a.id.as_str()).collect()
    }

    #[test]
    fn a_proposed_story_nothing_blocks_or_precedes_is_ready() {
        let all = [listed("story:a", "proposed")];
        assert_eq!(ids(&all), ["story:a"]);
    }

    #[test]
    fn only_proposed_is_ready() {
        let all = [
            listed("story:a", "draft"),
            listed("story:b", "active"),
            listed("story:c", "implemented"),
            listed("story:d", "archived"),
            listed("story:e", "rejected"),
        ];
        assert!(ids(&all).is_empty());
    }

    #[test]
    fn a_blocked_story_is_not_ready() {
        let all = [
            blocked(listed("story:a", "proposed"), "decision-blocker:q"),
            listed("decision-blocker:q", "open"),
        ];
        assert!(ids(&all).is_empty());
    }

    #[test]
    fn a_dependency_is_met_only_once_it_is_done() {
        for (status, met) in [
            ("draft", false),
            ("proposed", false),
            ("active", false),
            ("implemented", true),
            ("archived", true),
        ] {
            let all = [
                depends(listed("story:a", "proposed"), "story:dep"),
                listed("story:dep", status),
            ];
            assert_eq!(
                ids(&all).contains(&"story:a"),
                met,
                "a dependency at {status}"
            );
        }
        let unlisted = [depends(listed("story:a", "proposed"), "story:gone")];
        assert!(
            ids(&unlisted).is_empty(),
            "an unlisted dependency is not done"
        );
        let two = [
            depends(
                depends(listed("story:a", "proposed"), "story:done"),
                "story:open",
            ),
            listed("story:done", "implemented"),
            listed("story:open", "proposed"),
        ];
        assert!(
            !ids(&two).contains(&"story:a"),
            "every dependency has to be done"
        );
    }

    #[test]
    fn the_first_in_id_order_is_the_unit() {
        let all = [
            listed("story:c", "proposed"),
            listed("story:a", "proposed"),
            blocked(listed("story:0", "proposed"), "decision-blocker:q"),
            listed("story:b", "proposed"),
        ];
        assert_eq!(ids(&all), ["story:a", "story:b", "story:c"]);
    }

    #[test]
    fn only_a_story_is_a_unit() {
        let all = [listed("epic:e", "proposed"), listed("task:t", "proposed")];
        assert!(ids(&all).is_empty());
    }

    #[test]
    fn the_intent_is_the_title_then_the_body() {
        let story = Story {
            id: "story:a".to_owned(),
            title: "Make it pass".to_owned(),
            body: "# Story: a\n\n## Outcome\n\nIt passes.\n".to_owned(),
        };
        assert_eq!(
            story.intent(),
            "Make it pass\n\n# Story: a\n\n## Outcome\n\nIt passes.\n"
        );
    }

    #[test]
    fn the_case_id_is_the_story_and_the_instant() {
        let story = Story {
            id: "story:ready-one".to_owned(),
            title: String::new(),
            body: String::new(),
        };
        assert_eq!(
            story.case_id("2026-10-18T12:00:00Z"),
            "story-ready-one-20261018T120000Z"
        );
    }

    #[test]
    fn the_listing_reads_as_aep_prints_it() {
        let json = r#"[{"id":"story:a","kind":"story","status":"proposed","title":"A",
            "path":"story/a.md","relations":[{"relation":"depends_on","target":"story:b"}],
            "refs":[],"blocked_by":[{"blocker":"decision-blocker:q","type":"decision"}]}]"#;
        let listed: Vec<Listed> = serde_json::from_str(json).expect("reads");
        assert_eq!(listed[0].relations[0].target, "story:b");
        assert_eq!(listed[0].blocked_by[0].blocker, "decision-blocker:q");
    }
}
