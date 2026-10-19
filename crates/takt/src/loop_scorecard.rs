//! The loop scorecard (design:recursive-self-improvement § Improving the loop): one row per
//! improvement turn, a `self.improvement` case under `.engineering/cases/`, read from the
//! case's own records, and figures computed from the rows each time they are asked for, never
//! kept apart from them. The metric definitions are data, [`DEFINITIONS`], read at a revision
//! through git and never from the working tree; the baseline's are snapshotted into the loop
//! case when it opens ([`snapshot_baseline`]), so a definition is judged as it stood at the
//! base. Three validators drive a `loop.improvement/1` case: [`LoopMeasure`]
//! (`loop.measure`), [`LoopDefinitionCheck`] (`loop.definition.check`) and [`LoopCompare`]
//! (`loop.compare`).
//!
//! A row's figures come from the turn's records only: its verdict, whether it was promoted, and
//! when its finding was verified from the decisions Canon wrote for it; its tokens and wall
//! time from its run record ([`crate::scorecard`]); its revisions from its snapshot; its
//! finding from the story it was opened for, and when that was filed from the commit that
//! added the finding to the store, read through git at the row's `finding` revision (else its
//! candidate's). What a record does not hold is absent, never guessed.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::case::{CASES_DIR, CaseDir};
use crate::decision::Decision;
use crate::evidence::Record;
use crate::validator::{Artifact, Observation, Validator};

/// The metric definitions of the loop scorecard, a path of the repository, read at a revision.
pub const DEFINITIONS: &str = "protocols/loop-improvement/metrics.yaml";

/// The code that computes the figures each definition names, chosen by the metric's id and
/// version: part of the definitions as far as `loop.definition.check` is concerned.
pub const FIGURES_CODE: &str = "crates/takt/src/loop_scorecard.rs";

/// The format of [`DEFINITIONS`].
pub const DEFINITIONS_FORMAT: &str = "takt-loop-metrics/1";

/// The format of the measurement and comparison `loop.measure` and `loop.compare` retain.
pub const MEASUREMENT_FORMAT: &str = "takt-loop-measurement/1";

/// The protocol of an improvement turn's case, as its `case.yaml` names it.
pub const TURN_PROTOCOL: &str = "self.improvement";

/// The major version of [`TURN_PROTOCOL`] a turn's case runs on, as its own `protocol.yaml`
/// names it: a case of another major is no turn of this scorecard.
pub const TURN_PROTOCOL_MAJOR: u64 = 1;

/// The turns a measurement or a side of a comparison needs at least.
pub const MIN_TURNS: usize = 3;

/// One improvement turn's row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopScorecardRow {
    /// The turn: its case id.
    pub turn: String,
    /// The finding's key: the store artifact the case was opened for, `story:<key>` or
    /// `decision-blocker:<key>` as `scripts/file-finding` files it; absent when it records none.
    pub finding: Option<String>,
    /// The `baseline` artifact's revision in the case's snapshot.
    pub baseline: String,
    /// The `candidate` artifact's revision in the case's snapshot.
    pub candidate: String,
    /// The outcome legitimate at the case's latest decision; absent while the turn is open.
    pub verdict: Option<String>,
    /// Whether the case's latest decision holds the claim `promoted`.
    pub promoted: bool,
    /// The tokens of the case's runs; absent unless every harness step of its run record
    /// reports a count, and one does.
    pub tokens: Option<u64>,
    /// The wall time of the case's runs, in milliseconds; absent when it has no run record.
    pub wall_ms: Option<u64>,
    /// When the finding was filed: the time of the commit that added its artifact to the store,
    /// read through git at the row's `finding` revision, or at its candidate while that names no
    /// commit (never at the baseline, which the filing comes after); absent when that cannot be
    /// read, or when it comes after `verified_at`.
    pub filed_at: Option<String>,
    /// When the finding was verified gone: the instant of the first decision of the last
    /// unbroken run of decisions holding `scenario.passes_after`, when the latest holds it;
    /// absent until then.
    pub verified_at: Option<String>,
}

/// The loop scorecard: one row per recorded improvement turn, in case order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoopScorecard {
    /// The rows.
    pub rows: Vec<LoopScorecardRow>,
}

/// A target: the figure at least, or at most, this value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// The figure meets the target at this value or above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_least: Option<f64>,
    /// The figure meets the target at this value or below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_most: Option<f64>,
}

impl Target {
    /// Whether `value` misses the target.
    #[must_use]
    pub fn missed_by(&self, value: f64) -> bool {
        self.at_least.is_some_and(|least| value < least)
            || self.at_most.is_some_and(|most| value > most)
    }

    /// Whether `after` is better than `before` in the target's direction.
    #[must_use]
    pub fn improves(&self, before: f64, after: f64) -> bool {
        if self.at_least.is_some() {
            after > before
        } else {
            after < before
        }
    }
}

/// One definition of a metric: the version the code computes it under, what it is, its target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// The version; a changed definition takes another.
    pub version: u32,
    /// What the figure is, in words.
    pub definition: String,
    /// The target; a metric without one is never below it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
}

/// A metric: its current definition and the previous ones whose figures print beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// The current definition's version.
    pub version: u32,
    /// The current definition, in words.
    pub definition: String,
    /// The current definition's target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// Previous definitions, each computed and printed beside the current one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous: Vec<Definition>,
}

impl Metric {
    /// The current definition.
    #[must_use]
    pub fn current(&self) -> Definition {
        Definition {
            version: self.version,
            definition: self.definition.clone(),
            target: self.target,
        }
    }
}

/// The metric definitions, [`DEFINITIONS`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definitions {
    /// [`DEFINITIONS_FORMAT`].
    pub format: String,
    /// Each metric by id.
    pub metrics: BTreeMap<String, Metric>,
}

/// What [`Definitions::at`] found at a revision.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// The definitions.
    Definitions(Definitions),
    /// The revision holds no file at [`DEFINITIONS`].
    Absent,
    /// The file does not read as definitions, for the reason given.
    Malformed(String),
    /// The revision is no commit of the repository (an artifact not yet set, `r0`).
    NoCommit,
}

/// Whether `revision` can be read through git as a commit id: 7 to 64 hexadecimal digits, so
/// it is never taken for an option.
fn is_commit_id(revision: &str) -> bool {
    (7..=64).contains(&revision.len()) && revision.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The full object id `revision` names, for a repository whose ids are `hex` digits long:
/// taken as given, lower-cased, never looked up by a git subprocess; none for a revision that
/// is no commit id (`r0`), or longer than the repository's ids. An abbreviated id is an error
/// naming it: resolving a prefix would take a lookup the reader's deadline does not hold.
fn object_id(hex: usize, revision: &str) -> Result<Option<String>> {
    if !is_commit_id(revision) || revision.len() > hex {
        return Ok(None);
    }
    if revision.len() < hex {
        bail!(
            "{revision} is an abbreviated commit id; takt reads a commit only by its full {hex}-digit id"
        );
    }
    Ok(Some(revision.to_ascii_lowercase()))
}

/// The most commits one walk of the history reads: a walk that would read more is refused,
/// naming the cap, rather than reading a history without end. The cap bounds the reads, not
/// the time: each read ends by its own 10 s, so a walk to the cap can outlast any budget; with
/// `--hours`, every read ends at the run's deadline, and so does the walk.
pub const MAX_WALK: usize = 100_000;

/// A repository's history read through one checked reader ([`crate::git::ObjectReader`]):
/// every commit, tree and blob checked against its id, capped and held to the reader's
/// deadline, and every commit's header read by git's own rule ([`crate::git::parse_commit`]).
/// Commits read once are kept, so one history serves every row of a validator's call. No
/// `git log`, `merge-base`, `rev-list` or `cat-file -e` answers any part of what it says.
pub struct History {
    tree: std::path::PathBuf,
    reader: crate::git::ObjectReader,
    hex: usize,
    commits: std::collections::HashMap<String, Option<crate::git::CommitHeader>>,
    shallow: Option<std::collections::HashSet<String>>,
}

/// What [`History::descends`] answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ancestry {
    /// The revision descends from the ancestor, or is it.
    Yes,
    /// It does not, the whole history walked.
    No,
    /// The walk reached a commit the repository is shallow at without finding the ancestor:
    /// the history that would say is not there.
    Unknown,
}

impl History {
    /// The history of the repository at `tree`, each read also ending by `until` (the run's
    /// deadline) when that comes before its own 10 s.
    pub fn open(tree: &Path, until: Option<std::time::Instant>) -> Result<Self> {
        let hex = match crate::git::ObjectFormat::of(tree)? {
            crate::git::ObjectFormat::Sha1 => 40,
            crate::git::ObjectFormat::Sha256 => 64,
        };
        Ok(Self {
            tree: tree.to_path_buf(),
            reader: crate::git::ObjectReader::open_until(tree, until)?,
            hex,
            commits: std::collections::HashMap::new(),
            shallow: None,
        })
    }

    /// The header of the commit `id` (a full object id); none when the repository holds no
    /// such object, or it is no commit.
    fn commit(&mut self, id: &str) -> Result<Option<crate::git::CommitHeader>> {
        if let Some(known) = self.commits.get(id) {
            return Ok(known.clone());
        }
        let header = match self.reader.read(id)? {
            Some((kind, bytes)) if kind == "commit" => {
                Some(crate::git::parse_commit(id, &bytes, self.hex)?)
            }
            _ => None,
        };
        self.commits.insert(id.to_owned(), header.clone());
        Ok(header)
    }

    /// The full id of the commit `revision` names, read through the checked reader; none when
    /// it names no commit. An abbreviated id is an error naming it ([`object_id`]).
    pub fn resolve(&mut self, revision: &str) -> Result<Option<String>> {
        let Some(id) = object_id(self.hex, revision)? else {
            return Ok(None);
        };
        Ok(self.commit(&id)?.map(|_| id))
    }

    /// The mode and the id of the entry at `path` under the tree `tree`; none when there is
    /// none, or a part of the way is no tree.
    fn entry(&mut self, tree: &str, path: &str) -> Result<Option<(Vec<u8>, String)>> {
        let mut object = tree.to_owned();
        let parts: Vec<&str> = path.split('/').collect();
        for (n, part) in parts.iter().enumerate() {
            let entries = match self.reader.read(&object)? {
                Some((kind, bytes)) if kind == "tree" => bytes,
                _ => bail!("the tree {object} does not read as a tree"),
            };
            let hash = self.hex / 2;
            let mut at = 0;
            let mut found = None;
            while at < entries.len() {
                let space = entries[at..]
                    .iter()
                    .position(|b| *b == b' ')
                    .ok_or_else(|| anyhow!("the tree {object} does not read"))?;
                let mode = &entries[at..at + space];
                let nul = entries[at + space..]
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or_else(|| anyhow!("the tree {object} does not read"))?;
                let name = &entries[at + space + 1..at + space + nul];
                let id_at = at + space + nul + 1;
                let id = entries
                    .get(id_at..id_at + hash)
                    .ok_or_else(|| anyhow!("the tree {object} does not read"))?;
                if name == part.as_bytes() {
                    let id: String = id.iter().map(|b| format!("{b:02x}")).collect();
                    found = Some((mode.to_vec(), id));
                    break;
                }
                at = id_at + hash;
            }
            let Some((mode, id)) = found else {
                return Ok(None);
            };
            if n + 1 == parts.len() {
                return Ok(Some((mode, id)));
            }
            if mode != b"40000" {
                return Ok(None);
            }
            object = id;
        }
        Ok(None)
    }

    /// The id of the file at `path` in the commit's tree `tree`; none when no file is there.
    fn file_id(&mut self, tree: &str, path: &str) -> Result<Option<String>> {
        Ok(self
            .entry(tree, path)?
            .filter(|(mode, _)| mode == b"100644" || mode == b"100755")
            .map(|(_, id)| id))
    }

    /// The bytes of the file at `path` in the commit `revision`; none when the commit holds no
    /// file there. A revision that names no commit is an error.
    pub fn file(&mut self, revision: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let commit = self
            .resolve(revision)?
            .ok_or_else(|| anyhow!("{revision} is no commit of {}", self.tree.display()))?;
        let header = self
            .commit(&commit)?
            .ok_or_else(|| anyhow!("{revision} is no commit of {}", self.tree.display()))?;
        let Some(blob) = self.file_id(&header.tree, path)? else {
            return Ok(None);
        };
        match self.reader.read(&blob)? {
            Some((kind, bytes)) if kind == "blob" => Ok(Some(bytes)),
            _ => bail!("the blob {blob} does not read as a blob"),
        }
    }

    /// The commits the repository is shallow at, its `shallow` file: commits whose parents it
    /// does not hold. Read once.
    fn shallow(&mut self) -> Result<&std::collections::HashSet<String>> {
        if self.shallow.is_none() {
            let path = crate::git::run(&self.tree, &["rev-parse", "--git-path", "shallow"])?;
            let path = self.tree.join(path);
            let listed = match std::fs::read_to_string(&path) {
                Ok(text) => text.lines().map(str::to_owned).collect(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::collections::HashSet::new()
                }
                Err(error) => {
                    return Err(error).with_context(|| format!("read {}", path.display()));
                }
            };
            self.shallow = Some(listed);
        }
        Ok(self
            .shallow
            .get_or_insert_with(std::collections::HashSet::new))
    }

    /// Whether the commit `revision` descends from the commit `ancestor`, or is it: walked from
    /// `revision` through the parents each commit's own checked header names, first parent
    /// first, every commit seen once (a cycle a forged commit makes ends the walk), at most
    /// [`MAX_WALK`] commits. A commit the repository is shallow at ends its branch of the walk:
    /// when the ancestor is not found, the answer is then [`Ancestry::Unknown`]. A commit on the
    /// way that cannot be read, or does not check out, is an error naming it, never `No`; so is
    /// a walk past the cap. `No` for a revision or an ancestor that names no commit.
    pub fn descends(&mut self, ancestor: &str, revision: &str) -> Result<Ancestry> {
        let Some(revision) = self.resolve(revision)? else {
            return Ok(Ancestry::No);
        };
        // An ancestor the repository does not hold may lie past a shallow boundary: a full
        // commit id is then walked toward, and the walk ends unknown at the boundary.
        let ancestor = match self.resolve(ancestor)? {
            Some(id) => id,
            None if ancestor.len() == self.hex
                && is_commit_id(ancestor)
                && !self.shallow()?.is_empty() =>
            {
                ancestor.to_ascii_lowercase()
            }
            None => return Ok(Ancestry::No),
        };
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut walk = vec![revision];
        let mut cut = false;
        while let Some(commit) = walk.pop() {
            if commit == ancestor {
                return Ok(Ancestry::Yes);
            }
            if !seen.insert(commit.clone()) {
                continue;
            }
            if seen.len() > MAX_WALK {
                bail!(
                    "walking the history for {ancestor} read more than {MAX_WALK} commits, the most takt reads in one walk"
                );
            }
            let header = self.commit(&commit)?.ok_or_else(|| {
                anyhow!(
                    "the commit {commit} on the way is not in the repository; the history cannot be walked"
                )
            })?;
            if self.shallow()?.contains(&commit) {
                cut = true;
                continue;
            }
            // The stack takes the first parent last, so it is walked first.
            walk.extend(header.parents.into_iter().rev());
        }
        Ok(if cut { Ancestry::Unknown } else { Ancestry::No })
    }

    /// When the file at `path` was added to the history of `revision`: the committer time of
    /// the commit, followed back along first parents from `revision`, whose tree holds a file
    /// there while its first parent's does not (or that has no parent); none when `revision`'s
    /// tree holds none, that commit names no committer time, or a first parent on the way is
    /// not in the repository (a shallow history). At most [`MAX_WALK`] commits.
    pub fn added(&mut self, revision: &str, path: &str) -> Result<Option<i64>> {
        let Some(commit) = self.resolve(revision)? else {
            return Ok(None);
        };
        let mut header = self
            .commit(&commit)?
            .ok_or_else(|| anyhow!("{commit} is no commit"))?;
        if self.file_id(&header.tree, path)?.is_none() {
            return Ok(None);
        }
        for _ in 0..MAX_WALK {
            let Some(parent) = header.parents.first().cloned() else {
                return Ok(header.committer_time);
            };
            // A first parent the repository does not hold (a shallow history) leaves the
            // filing unknown; one that does not check out is an error, from the reader.
            let Some(above) = self.commit(&parent)? else {
                return Ok(None);
            };
            if self.file_id(&above.tree, path)?.is_none() {
                return Ok(header.committer_time);
            }
            header = above;
        }
        bail!(
            "following {path} back from {revision} read more than {MAX_WALK} commits, the most takt reads in one walk"
        )
    }
}

/// The bytes of the file at `path` in the commit `revision`, read through one [`History`]: the
/// commit, each tree down the path and the blob, each checked against its id; none when the
/// commit holds no file there. An object whose bytes do not hash to its id is an error naming
/// it, so an object planted under another's id is never read as that one.
pub fn verified_file(tree: &Path, revision: &str, path: &str) -> Result<Option<Vec<u8>>> {
    verified_file_within(tree, revision, path, None)
}

/// [`verified_file`], each read also ending by `until` (the run's deadline) when that comes
/// before its own 10 s.
pub fn verified_file_within(
    tree: &Path,
    revision: &str,
    path: &str,
    until: Option<std::time::Instant>,
) -> Result<Option<Vec<u8>>> {
    History::open(tree, until)?.file(revision, path)
}

/// Whether the commit `revision` descends from the commit `ancestor`, or is it, by
/// [`History::descends`]: `true` only for [`Ancestry::Yes`], a shallow history's unknown included
/// in `false`.
pub fn verified_descends(tree: &Path, ancestor: &str, revision: &str) -> Result<bool> {
    verified_descends_within(tree, ancestor, revision, None)
}

/// [`verified_descends`], each read also ending by `until` (the run's deadline) when that
/// comes before its own 10 s.
pub fn verified_descends_within(
    tree: &Path,
    ancestor: &str,
    revision: &str,
    until: Option<std::time::Instant>,
) -> Result<bool> {
    Ok(History::open(tree, until)?.descends(ancestor, revision)? == Ancestry::Yes)
}

/// The first line, from one, on which the YAML parser reads an anchored node (`&name`) or an
/// alias (`*name`) in `text`, from its own events: what YAML reads as an anchor is one, and
/// what it reads as text (`rows *2`, `a 'b &c`) is not. The events are read one at a time and
/// dropped, before anything is deserialized, so no alias expands: one would expand without
/// bound once read, and a file well under its cap could expand to gigabytes. None when the
/// stream holds none up to its end, or up to where it does not parse (the deserializer, on the
/// same parser, then fails there before it builds anything). A parser that cannot be set up is
/// an error.
pub(crate) fn anchor_line(text: &str) -> Result<Option<usize>> {
    use unsafe_libyaml as unsafe_yaml;
    let size = u64::try_from(text.len()).context("the YAML text's size")?;
    let mut parser = Box::<unsafe_yaml::yaml_parser_t>::new_uninit();
    let parser = parser.as_mut_ptr();
    // SAFETY: the parser lives in its own allocation, never moved, initialized before any other
    // call and deleted after the last; the input outlives it. Each event is filled by
    // `yaml_parser_parse` before it is read, its union read only under the variant its type
    // names, and deleted before the next.
    unsafe {
        if unsafe_yaml::yaml_parser_initialize(parser).fail {
            bail!("the YAML parser could not be set up");
        }
        unsafe_yaml::yaml_parser_set_encoding(parser, unsafe_yaml::YAML_UTF8_ENCODING);
        unsafe_yaml::yaml_parser_set_input_string(parser, text.as_ptr(), size);
        let mut found = None;
        let mut event = std::mem::MaybeUninit::<unsafe_yaml::yaml_event_t>::uninit();
        loop {
            let event = event.as_mut_ptr();
            if unsafe_yaml::yaml_parser_parse(parser, event).fail {
                break;
            }
            let anchored = match (*event).type_ {
                unsafe_yaml::YAML_ALIAS_EVENT => true,
                unsafe_yaml::YAML_SCALAR_EVENT => !(*event).data.scalar.anchor.is_null(),
                unsafe_yaml::YAML_SEQUENCE_START_EVENT => {
                    !(*event).data.sequence_start.anchor.is_null()
                }
                unsafe_yaml::YAML_MAPPING_START_EVENT => {
                    !(*event).data.mapping_start.anchor.is_null()
                }
                _ => false,
            };
            let line = (*event).start_mark.line;
            let end = (*event).type_ == unsafe_yaml::YAML_STREAM_END_EVENT;
            unsafe_yaml::yaml_event_delete(event);
            if anchored {
                found = Some(
                    usize::try_from(line)
                        .unwrap_or(usize::MAX)
                        .saturating_add(1),
                );
                break;
            }
            if end {
                break;
            }
        }
        unsafe_yaml::yaml_parser_delete(parser);
        Ok(found)
    }
}

/// `text`, the file at `path`, read as YAML, of which a JSON record is one; one holding an
/// anchor or an alias is refused, naming the file and its line, before anything is
/// deserialized ([`anchor_line`]).
fn read_yaml<T: serde::de::DeserializeOwned>(path: &Path, text: &str) -> Result<T> {
    if let Some(line) = anchor_line(text)? {
        bail!(
            "{} holds a YAML anchor or alias on line {line}; it is read without them",
            path.display()
        );
    }
    serde_yaml_ng::from_str(text).with_context(|| format!("read {}", path.display()))
}

impl Definitions {
    /// Reads definitions from `text`: [`DEFINITIONS_FORMAT`], each target with exactly one
    /// bound, a finite number its metric's figures can take (a share's 0 to 1, any other's 0
    /// and above), and no version of a metric defined twice. Definitions holding a YAML anchor
    /// or alias are refused as the parser reads it, naming its line, before anything is
    /// deserialized ([`anchor_line`]).
    pub fn parse(text: &str) -> Result<Self> {
        if let Some(line) = anchor_line(text)? {
            bail!(
                "the loop metric definitions hold a YAML anchor or alias on line {line}; they are read without them"
            );
        }
        let definitions: Self =
            serde_yaml_ng::from_str(text).context("read the loop metric definitions")?;
        if definitions.format != DEFINITIONS_FORMAT {
            bail!(
                "the loop metric definitions are {}, not {DEFINITIONS_FORMAT}",
                definitions.format
            );
        }
        for (id, metric) in &definitions.metrics {
            let mut versions = vec![metric.version];
            for definition in std::iter::once(metric.current()).chain(metric.previous.clone()) {
                if let Some(target) = definition.target {
                    if target.at_least.is_some() == target.at_most.is_some() {
                        bail!(
                            "{id} v{}: a target is at_least or at_most",
                            definition.version
                        );
                    }
                    // A bound that is no finite number (`.nan`, `.inf`) is never missed or
                    // never met: no target at all.
                    let bound = target.at_least.or(target.at_most).unwrap_or_default();
                    if !bound.is_finite() {
                        bail!("{id} v{}: a target is a finite number", definition.version);
                    }
                    // A target no figure of the metric can take is meaningless: a share is 0 to
                    // 1 however its id is spelled (`ACCEPTED_SHARE`, `promoted_shares`), and a
                    // count, a token figure or a time is never below 0.
                    let share = id.to_ascii_lowercase().contains("share");
                    if bound < 0.0 || (share && bound > 1.0) {
                        bail!(
                            "{id} v{}: a target of {bound} is outside {}",
                            definition.version,
                            if share { "0 to 1" } else { "0 and above" }
                        );
                    }
                }
            }
            for previous in &metric.previous {
                if versions.contains(&previous.version) {
                    bail!("{id} v{} is defined twice", previous.version);
                }
                versions.push(previous.version);
            }
        }
        Ok(definitions)
    }

    /// The definitions at `revision` of `tree`, read through git, never from the working tree.
    pub fn at(tree: &Path, revision: &str) -> Result<Found> {
        Self::at_within(tree, revision, None)
    }

    /// [`Self::at`], each read also ending by `until` (the run's deadline) when that comes
    /// before its own 10 s.
    pub fn at_within(
        tree: &Path,
        revision: &str,
        until: Option<std::time::Instant>,
    ) -> Result<Found> {
        // Whether it names a commit is read through the checked reader, so a commit planted
        // under another's id is refused there, naming it.
        let mut history = History::open(tree, until)?;
        let Some(commit) = history.resolve(revision)? else {
            return Ok(Found::NoCommit);
        };
        let Some(bytes) = history.file(&commit, DEFINITIONS)? else {
            return Ok(Found::Absent);
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return Ok(Found::Malformed("not UTF-8".to_owned()));
        };
        Ok(match Self::parse(&text) {
            Ok(definitions) => Found::Definitions(definitions),
            Err(error) => Found::Malformed(format!("{error:#}")),
        })
    }
}

/// One figure of the loop scorecard, computed from the rows it names.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Figure {
    /// The metric.
    pub metric: String,
    /// The version of the definition it was computed under.
    pub version: u32,
    /// Whether that definition is the metric's current one; a previous one prints beside it.
    pub current: bool,
    /// The figure; absent when no row counts towards it.
    pub value: Option<f64>,
    /// The figure as printed: a share as `<accepted>/<all>`.
    pub shown: String,
    /// The definition's target.
    pub target: Option<Target>,
    /// Whether the figure misses the target, or has a target and no value (no data fails
    /// closed).
    pub below_target: bool,
    /// The turns it was computed from, by case id.
    pub cases: Vec<String>,
}

/// The median of `values`, none for none.
fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some(f64::midpoint(sorted[n / 2 - 1], sorted[n / 2])),
    }
}

/// A figure as printed: whole numbers without a fraction, others to three places.
fn shown(value: Option<f64>) -> String {
    match value {
        None => "-".to_owned(),
        Some(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Some(value) => format!("{value:.3}"),
    }
}

/// An instant as Canon writes one, `YYYY-MM-DDTHH:MM:SS[.f...]Z`, in nanoseconds since the
/// epoch; none for any other text.
fn instant(at: &str) -> Option<i128> {
    let at = at.strip_suffix('Z')?;
    let (whole, fraction) = at.split_once('.').unwrap_or((at, ""));
    let bytes = whole.as_bytes();
    if bytes.len() != 19
        || [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')]
            .iter()
            .any(|(at, byte)| bytes[*at] != *byte)
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number = |from: usize, to: usize| whole.get(from..to)?.parse::<u32>().ok();
    let date = time::Date::from_calendar_date(
        i32::try_from(number(0, 4)?).ok()?,
        time::Month::try_from(u8::try_from(number(5, 7)?).ok()?).ok()?,
        u8::try_from(number(8, 10)?).ok()?,
    )
    .ok()?;
    let time = time::Time::from_hms(
        u8::try_from(number(11, 13)?).ok()?,
        u8::try_from(number(14, 16)?).ok()?,
        u8::try_from(number(17, 19)?).ok()?,
    )
    .ok()?;
    let nanos: String = fraction
        .chars()
        .chain("000000000".chars())
        .take(9)
        .collect();
    Some(
        time::PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp_nanos()
            + nanos.parse::<i128>().ok()?,
    )
}

/// The figure of `metric` under `version` over `rows`: its value, as printed, and the turns
/// it was computed from. A version the code does not compute is an error naming it.
fn compute(
    metric: &str,
    version: u32,
    rows: &[&LoopScorecardRow],
) -> Result<(Option<f64>, String, Vec<String>)> {
    let all: Vec<String> = rows.iter().map(|row| row.turn.clone()).collect();
    let share = |counted: usize| {
        let value = (!rows.is_empty()).then(|| counted as f64 / rows.len() as f64);
        (value, format!("{counted}/{}", rows.len()), all.clone())
    };
    Ok(match (metric, version) {
        ("turns", 1) => (Some(rows.len() as f64), rows.len().to_string(), all),
        ("accepted_share", 1) => share(
            rows.iter()
                .filter(|row| row.verdict.as_deref() == Some("accepted"))
                .count(),
        ),
        ("accepted_share", 2) => {
            let closed: Vec<&&LoopScorecardRow> =
                rows.iter().filter(|row| row.verdict.is_some()).collect();
            let counted = closed
                .iter()
                .filter(|row| row.verdict.as_deref() == Some("accepted"))
                .count();
            (
                (!closed.is_empty()).then(|| counted as f64 / closed.len() as f64),
                format!("{counted}/{}", closed.len()),
                closed.iter().map(|row| row.turn.clone()).collect(),
            )
        }
        ("promoted_share", 1) => share(rows.iter().filter(|row| row.promoted).count()),
        ("median_tokens_per_accepted_turn", 1) => {
            let counted: Vec<&&LoopScorecardRow> = rows
                .iter()
                .filter(|row| row.verdict.as_deref() == Some("accepted") && row.tokens.is_some())
                .collect();
            let values: Vec<f64> = counted
                .iter()
                .filter_map(|row| row.tokens.map(|tokens| tokens as f64))
                .collect();
            let value = median(&values);
            (
                value,
                shown(value),
                counted.iter().map(|row| row.turn.clone()).collect(),
            )
        }
        ("finding_to_verified", 1) => {
            let mut values = Vec::new();
            let mut cases = Vec::new();
            for row in rows {
                if let (Some(filed), Some(verified)) = (
                    row.filed_at.as_deref().and_then(instant),
                    row.verified_at.as_deref().and_then(instant),
                ) {
                    values.push((verified - filed) as f64 / 1e9);
                    cases.push(row.turn.clone());
                }
            }
            let value = median(&values);
            (value, shown(value), cases)
        }
        _ => bail!("the loop scorecard computes no {metric} v{version}"),
    })
}

/// The decisions under `case_dir`, `decisions/<n>.json` in the order of `n`, each with the
/// instant it was computed at, when it names one.
fn decisions(case_dir: &Path) -> Result<Vec<(Decision, Option<String>)>> {
    let dir = case_dir.join("decisions");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut numbered: Vec<(u64, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        if let Some(n) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<u64>().ok())
        {
            numbered.push((n, path));
        }
    }
    numbered.sort();
    numbered
        .into_iter()
        .map(|(_, path)| {
            let json = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let at = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|doc| {
                    doc["explanation"]["computed_from"]["at"]
                        .as_str()
                        .map(str::to_owned)
                });
            let decision =
                Decision::from_json(&json).with_context(|| format!("read {}", path.display()))?;
            Ok((decision, at))
        })
        .collect()
}

/// The revision of `artifact` in the snapshot `doc`.
fn revision_in(doc: &serde_yaml_ng::Value, artifact: &str) -> Option<String> {
    doc["artifacts"][artifact]["revision"]
        .as_str()
        .map(str::to_owned)
}

/// The snapshot of the case at `case_dir`.
fn snapshot(case_dir: &Path) -> Result<serde_yaml_ng::Value> {
    let path = case_dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    read_yaml(&path, &text)
}

/// The finding's key and its file in the planning store, for the store artifact `artifact` a
/// case was opened for: `story:<key>` or `decision-blocker:<key>` (`blocker:<key>` as
/// `scripts/file-finding` names the kind), the key lower-case letters and digits in words joined
/// by `-`; none for anything else.
fn store_path(artifact: &str) -> Option<(String, String)> {
    let (kind, key) = artifact.split_once(':')?;
    let dir = match kind {
        "story" => "story",
        "blocker" | "decision-blocker" => "decision-blocker",
        _ => return None,
    };
    let is_key = key.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    is_key.then(|| {
        (
            key.to_owned(),
            format!(".engineering/planning/{dir}/{key}.md"),
        )
    })
}

/// `seconds` since the epoch as Canon writes an instant.
fn instant_of(seconds: i64) -> Option<String> {
    time::OffsetDateTime::from_unix_timestamp(seconds)
        .ok()?
        .format(&time::macros::format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
        ))
        .ok()
}

/// Whether the case at `case_dir` runs [`TURN_PROTOCOL`] at [`TURN_PROTOCOL_MAJOR`], as its own
/// `protocol.yaml` names them; a case without one runs on no protocol this scorecard reads.
fn runs_turn_major(case_dir: &Path) -> Result<bool> {
    let path = case_dir.join("protocol.yaml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    let doc: serde_yaml_ng::Value = read_yaml(&path, &text)?;
    Ok(doc["protocol"]["id"].as_str() == Some(TURN_PROTOCOL)
        && doc["protocol"]["revision"].as_u64() == Some(TURN_PROTOCOL_MAJOR))
}

impl LoopScorecard {
    /// The rows as [`Self::read_at`] reads them with no base: no `filed_at`.
    pub fn read(repo: &Path) -> Result<Self> {
        Self::read_at(repo, None)
    }

    /// The rows of the `self.improvement` cases under `repo`'s cases directory, in name order;
    /// none when the directory is absent. A case of another protocol is no turn. With `base`
    /// given (the revision the caller works at), each row's `filed_at` is read through git at
    /// that row's `finding` revision, else its candidate's, never at `base`; with none, no row
    /// has one. A snapshot, a decision or a run record that does not read is an error naming it.
    pub fn read_at(repo: &Path, base: Option<&str>) -> Result<Self> {
        Self::read_within(repo, base, None)
    }

    /// [`Self::read_at`], each object read also ending by `until` (the run's deadline) when
    /// that comes before its own 10 s. The history is read through one [`History`] for every
    /// row: each commit checked against its id, a filing found by following first parents and
    /// comparing trees, never by `git log`.
    pub fn read_within(
        repo: &Path,
        base: Option<&str>,
        until: Option<std::time::Instant>,
    ) -> Result<Self> {
        let mut history = match base {
            Some(_) => Some(History::open(repo, until)?),
            None => None,
        };
        let cases = repo.join(CASES_DIR);
        let mut names: Vec<String> = Vec::new();
        match std::fs::read_dir(&cases) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.with_context(|| format!("read {}", cases.display()))?;
                    if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                        names.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("read {}", cases.display())),
        }
        names.sort();
        let spent: BTreeMap<String, crate::scorecard::Case> =
            crate::scorecard::Scorecard::read(repo)?
                .cases()
                .into_iter()
                .map(|case| (case.case.clone(), case))
                .collect();
        let mut rows = Vec::new();
        for name in names {
            let dir = cases.join(&name);
            if !dir.join("case.yaml").is_file() {
                continue;
            }
            let doc = snapshot(&dir)?;
            if doc["protocol"].as_str() != Some(TURN_PROTOCOL) || !runs_turn_major(&dir)? {
                continue;
            }
            let revision = |artifact: &str| {
                revision_in(&doc, artifact).ok_or_else(|| {
                    anyhow!(
                        "{}: the turn names no {artifact} revision",
                        dir.join("case.yaml").display()
                    )
                })
            };
            let decided = decisions(&dir)?;
            let latest = decided.last().map(|(decision, _)| decision);
            let verified_at = decided
                .iter()
                .rev()
                .take_while(|(decision, _)| decision.holds("scenario.passes_after"))
                .last()
                .and_then(|(_, at)| at.clone());
            let artifact = crate::case::story_of(&dir)?;
            let stored = artifact.as_deref().and_then(store_path);
            let baseline = revision("baseline")?;
            let candidate = revision("candidate")?;
            // The filing is read in the history the turn's `finding` artifact was opened at,
            // the store as it stood when the finding was filed, or, while that names no commit
            // of the repository (`r0`), in the candidate's: a finding is filed after the
            // baseline it was observed on, so the baseline's history never holds it. A filing
            // later than the verification is no filing time: the store was committed after the
            // run, and the time from finding to verified would come out negative.
            let filed_at = match (history.as_mut(), &stored) {
                (Some(history), Some((_, path))) => {
                    let source = match revision_in(&doc, "finding") {
                        Some(finding) if history.resolve(&finding)?.is_some() => finding,
                        _ => candidate.clone(),
                    };
                    history.added(&source, path)?.and_then(instant_of)
                }
                _ => None,
            }
            .filter(|filed| {
                match (instant(filed), verified_at.as_deref().and_then(instant)) {
                    (Some(filed), Some(verified)) => filed <= verified,
                    _ => true,
                }
            });
            // A step reports its tokens only when its harness counted them: the turn's tokens
            // are known only when every harness step of its record (every step but a
            // validator's) reported a count, and at least one did; a partial sum, or a record
            // with no count at all, is no count.
            let lines = crate::events::Events::new(&dir).read()?;
            let steps: Vec<&serde_json::Value> = lines
                .iter()
                .filter(|line| line["event"] == "step" && line["by"] != "validator")
                .collect();
            let counted =
                !steps.is_empty() && steps.iter().all(|line| line["tokens"].as_u64().is_some());
            let spend = spent.get(&name);
            rows.push(LoopScorecardRow {
                turn: name.clone(),
                finding: stored.map(|(key, _)| key),
                baseline,
                candidate,
                verdict: latest.and_then(|decision| decision.legitimate().into_iter().next()),
                promoted: latest.is_some_and(|decision| decision.holds("promoted")),
                tokens: spend.filter(|_| counted).map(|case| case.tokens),
                wall_ms: spend.map(|case| case.wall_ms),
                filed_at,
                verified_at,
            });
        }
        Ok(Self { rows })
    }

    /// The figures of every metric `definitions` defines over every row: the current
    /// definition's, then each previous one's, metrics in id order.
    pub fn figures(&self, definitions: &Definitions) -> Result<Vec<Figure>> {
        figures_of(&self.rows.iter().collect::<Vec<_>>(), definitions)
    }
}

/// The figures of every metric `definitions` defines over `rows`.
fn figures_of(rows: &[&LoopScorecardRow], definitions: &Definitions) -> Result<Vec<Figure>> {
    let mut figures = Vec::new();
    for (id, metric) in &definitions.metrics {
        let current = std::iter::once((metric.current(), true));
        let previous = metric.previous.iter().cloned().map(|d| (d, false));
        for (definition, is_current) in current.chain(previous) {
            let (value, shown, cases) = compute(id, definition.version, rows)?;
            figures.push(Figure {
                metric: id.clone(),
                version: definition.version,
                current: is_current,
                // A target with no figure to hold to it fails closed: no data is below target.
                below_target: match (value, definition.target) {
                    (Some(value), Some(target)) => target.missed_by(value),
                    (None, Some(_)) => true,
                    (_, None) => false,
                },
                value,
                shown,
                target: definition.target,
                cases,
            });
        }
    }
    Ok(figures)
}

/// Each figure as `<metric> v<n> <shown>`, a missed target marked.
fn line(figures: &[Figure]) -> String {
    figures
        .iter()
        .map(|figure| {
            format!(
                "{} v{}{} {}{}",
                figure.metric,
                figure.version,
                if figure.current { "" } else { " (previous)" },
                figure.shown,
                if figure.below_target {
                    " below target"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first eight characters of a revision.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// An observation that recorded nothing.
fn nothing(summary: String) -> Observation {
    Observation {
        summary,
        records: Vec::new(),
        artifact: None,
    }
}

/// The revision of `artifact` in the case whose branch `tree` is on.
fn case_revision(tree: &Path, artifact: &str) -> Result<String> {
    let dir = crate::case::current(tree)?;
    revision_in(&snapshot(&dir)?, artifact)
        .ok_or_else(|| anyhow!("the case at {} names no {artifact} revision", dir.display()))
}

/// The file under a loop case's directory holding the baseline's metric definitions as they
/// were read when the case opened ([`snapshot_baseline`]).
pub const BASELINE_SNAPSHOT: &str = "loop-baseline.json";

/// The format of [`BASELINE_SNAPSHOT`].
pub const BASELINE_SNAPSHOT_FORMAT: &str = "takt-loop-baseline/1";

/// Snapshots the metric definitions at the loop case `case`'s `loop_baseline` revision into
/// the case's directory as [`BASELINE_SNAPSHOT`]: the revision as the case names it, the
/// commit it is, and the bytes of [`DEFINITIONS`] there (none when it holds no such file).
/// Called when the case opens, before any step: a read of the base after a step ran trusts an
/// object store the step could write, while the case directory is guarded around every step.
/// Answers whether it wrote one: nothing is written while `loop_baseline` names no commit.
pub fn snapshot_baseline(tree: &Path, case: &CaseDir) -> Result<bool> {
    let Some(revision) = case.revision("loop_baseline") else {
        return Ok(false);
    };
    let mut history = History::open(tree, None)?;
    let Some(commit) = history.resolve(&revision)? else {
        return Ok(false);
    };
    let definitions = history
        .file(&commit, DEFINITIONS)?
        .map(|bytes| {
            String::from_utf8(bytes)
                .map_err(|_| anyhow!("{DEFINITIONS} at {} is not UTF-8", short(&commit)))
        })
        .transpose()?;
    let snapshot = serde_json::json!({
        "format": BASELINE_SNAPSHOT_FORMAT,
        "revision": revision,
        "commit": commit,
        "definitions": definitions,
    });
    let path = case.dir.join(BASELINE_SNAPSHOT);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&snapshot).context("serialize the baseline snapshot")?,
    )
    .with_context(|| format!("write {}", path.display()))?;
    Ok(true)
}

/// The baseline's definitions from the snapshot in the case whose branch `tree` is on, taken
/// when it opened ([`snapshot_baseline`]), never read from the object store again; or what to
/// say when there is no snapshot of `base`, or no definitions in it.
fn snapshotted(tree: &Path, base: &str) -> Result<Result<Definitions, String>> {
    let path = crate::case::current(tree)?.join(BASELINE_SNAPSHOT);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Err(format!(
            "no snapshot of its definitions was taken when the case opened ({BASELINE_SNAPSHOT})"
        )));
    };
    let snapshot: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    if snapshot["format"] != BASELINE_SNAPSHOT_FORMAT || snapshot["revision"] != base {
        return Ok(Err(format!(
            "the snapshot {BASELINE_SNAPSHOT} is not of {}",
            short(base)
        )));
    }
    let Some(definitions) = snapshot["definitions"].as_str() else {
        return Ok(Err(format!("{} holds no {DEFINITIONS}", short(base))));
    };
    Ok(Definitions::parse(definitions)
        .map_err(|error| format!("{DEFINITIONS} at {}: {error:#}", short(base))))
}

/// What a loop validator's observation held to `deadline` (the run's, from its hour budget)
/// answers: an error once it returned at or after it, whatever it observed, so the budget that
/// ended it records nothing; else `observed`. Each read the observation makes is held to the
/// same deadline as well as its own 10 s ([`crate::git::ObjectReader::open_until`]).
fn held_to(
    deadline: Option<std::time::Instant>,
    action: &str,
    observed: Result<Observation>,
) -> Result<Observation> {
    if !deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
        return observed;
    }
    match observed {
        Err(error) => Err(error),
        Ok(_) => {
            bail!("{action} returned at or after the run's end; what it observed is not recorded")
        }
    }
}

/// `loop.measure`: the loop scorecard over the recorded turns (at least [`MIN_TURNS`]), its
/// figures under the baseline's definitions as the case snapshotted them when it opened
/// ([`snapshot_baseline`]), never read from the object store after a step ran; writes one
/// `loop_measurement` record about `loop_baseline`: `below_target` when a current figure misses
/// its target, or has a target and no value, else `at_target`. No snapshot of `loop_baseline`
/// records nothing. The figures, each naming the turns it was computed from, are retained as
/// `loop-measurement-<seq>.json`: an observation; the rows stay the source.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopMeasure;

impl LoopMeasure {
    /// The action it performs.
    pub const ACTION: &str = "loop.measure";
    /// The evidence kind it writes.
    pub const KIND: &str = "loop_measurement";
}

impl Validator for LoopMeasure {
    fn name(&self) -> &str {
        "takt.loop-measure"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "loop_baseline"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_within(tree, revision, seq, None)
            .map(|(observation, _)| observation)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = (|| -> Result<Observation> {
            let definitions = match snapshotted(tree, revision)? {
                Ok(definitions) => definitions,
                Err(why) => return Ok(nothing(format!("the baseline: {why}; nothing recorded"))),
            };
            let card = LoopScorecard::read_within(tree, Some(revision), deadline)?;
            if card.rows.len() < MIN_TURNS {
                return Ok(nothing(format!(
                    "{} turns recorded under {CASES_DIR}, at least {MIN_TURNS} needed; nothing recorded",
                    card.rows.len()
                )));
            }
            let figures = match card.figures(&definitions) {
                Ok(figures) => figures,
                Err(error) => return Ok(nothing(format!("{error:#}; nothing recorded"))),
            };
            let result = if figures
                .iter()
                .any(|figure| figure.current && figure.below_target)
            {
                "below_target"
            } else {
                "at_target"
            };
            let turns: Vec<String> = card.rows.iter().map(|row| row.turn.clone()).collect();
            let artifact = format!("loop-measurement-{seq}.json");
            let snapshot = serde_json::json!({
                "format": MEASUREMENT_FORMAT,
                "definitions_revision": revision,
                "turns": turns,
                "result": result,
                "figures": figures,
                "rows": card.rows,
            });
            Ok(Observation {
                summary: format!(
                    "the loop scorecard over {} turns ({}) under the definitions at {}: {result}; {}; retained as {artifact}",
                    turns.len(),
                    turns.join(", "),
                    short(revision),
                    line(&figures)
                ),
                records: vec![Record::new(
                    format!("loop-measurement-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )],
                artifact: Some(Artifact {
                    name: artifact,
                    bytes: serde_json::to_vec_pretty(&snapshot)
                        .context("serialize the loop measurement")?,
                }),
            })
        })();
        Ok((held_to(deadline, self.action(), observed)?, false))
    }
}

/// How the candidate's metric definitions stand against the baseline's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefinitionCheck {
    /// Every definition of the baseline is the candidate's current one.
    Unchanged,
    /// A definition changed, took another version, kept its target, and the baseline's is kept
    /// as a previous one beside every previous one the baseline prints, so the figures of both
    /// print.
    DualReported,
    /// A definition changed and only the new one prints: it kept its version, dropped the
    /// baseline's or a previous one the baseline prints, or the metric is gone; or its target
    /// changed.
    Redefined,
}

impl DefinitionCheck {
    /// The result as the protocol names it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::DualReported => "dual_reported",
            Self::Redefined => "redefined",
        }
    }
}

/// Each metric of `base` with how `candidate` stands against it, and the result over all: the
/// worst of them. No candidate definitions (absent or unreadable) redefines every metric. A
/// metric whose target changed (its value or its direction) is redefined whatever its version
/// and its previous definitions say: targets are the owner's, and no candidate changes one
/// through this check. So is one that drops or changes a previous definition the base prints.
#[must_use]
pub fn check(
    base: &Definitions,
    candidate: Option<&Definitions>,
) -> (DefinitionCheck, Vec<(String, DefinitionCheck)>) {
    let each: Vec<(String, DefinitionCheck)> = base
        .metrics
        .iter()
        .map(|(id, metric)| {
            let was = metric.current();
            let stands = match candidate.and_then(|c| c.metrics.get(id)) {
                None => DefinitionCheck::Redefined,
                Some(now) if now.target != was.target => DefinitionCheck::Redefined,
                Some(now)
                    if metric
                        .previous
                        .iter()
                        .any(|printed| !now.previous.contains(printed)) =>
                {
                    DefinitionCheck::Redefined
                }
                Some(now) if now.current() == was => DefinitionCheck::Unchanged,
                Some(now) if now.version != was.version && now.previous.contains(&was) => {
                    DefinitionCheck::DualReported
                }
                Some(_) => DefinitionCheck::Redefined,
            };
            (id.clone(), stands)
        })
        .collect();
    let overall = each
        .iter()
        .map(|(_, stands)| *stands)
        .max()
        .unwrap_or(DefinitionCheck::Unchanged);
    (overall, each)
}

/// The metrics of `definitions` some figure of which the code does not compute over the rows of
/// `card`, current or previous, each with why.
fn uncomputed(card: &LoopScorecard, definitions: &Definitions) -> Vec<(String, String)> {
    let rows: Vec<&LoopScorecardRow> = card.rows.iter().collect();
    definitions
        .metrics
        .iter()
        .filter_map(|(id, metric)| {
            std::iter::once(metric.current())
                .chain(metric.previous.iter().cloned())
                .find_map(|definition| compute(id, definition.version, &rows).err())
                .map(|error| (id.clone(), format!("{error:#}")))
        })
        .collect()
}

/// `loop.definition.check`: the metric definitions at the candidate revision, read through git,
/// against the baseline's as the case snapshotted them when it opened ([`snapshot_baseline`]),
/// never read from the object store after a step ran; writes one `definition_check` record
/// about `loop_candidate`, its result [`DefinitionCheck`]. The figures the candidate's
/// definitions name are then computed over the recorded turns: a metric any of whose figures
/// the code does not compute is `redefined`, whatever [`check`] found, since its figure cannot
/// print; so is every metric when the turns do not read. No snapshot of the case's current
/// `loop_baseline`, or no definitions in it, records nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopDefinitionCheck;

impl LoopDefinitionCheck {
    /// The action it performs.
    pub const ACTION: &str = "loop.definition.check";
    /// The evidence kind it writes.
    pub const KIND: &str = "definition_check";
}

impl Validator for LoopDefinitionCheck {
    fn name(&self) -> &str {
        "takt.loop-definition-check"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "loop_candidate"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_within(tree, revision, seq, None)
            .map(|(observation, _)| observation)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = (|| -> Result<Observation> {
            let base = case_revision(tree, "loop_baseline")?;
            let base_definitions = match snapshotted(tree, &base)? {
                Ok(definitions) => definitions,
                Err(why) => return Ok(nothing(format!("the baseline: {why}; nothing recorded"))),
            };
            let (candidate, why) = match Definitions::at_within(tree, revision, deadline)? {
                Found::Definitions(definitions) => (Some(definitions), String::new()),
                Found::Absent => (
                    None,
                    format!("; {} holds no {DEFINITIONS}", short(revision)),
                ),
                Found::Malformed(why) => (None, format!("; {DEFINITIONS}: {why}")),
                Found::NoCommit => {
                    return Ok(nothing(format!(
                        "the candidate {revision} is no commit; nothing recorded"
                    )));
                }
            };
            let (mut result, mut each) = check(&base_definitions, candidate.as_ref());
            let mut why = why;
            if let Some(candidate) = &candidate {
                let failed: Vec<(String, String)> =
                    match LoopScorecard::read_within(tree, Some(&base), deadline) {
                        Ok(card) => uncomputed(&card, candidate),
                        Err(error) => candidate
                            .metrics
                            .keys()
                            .map(|id| (id.clone(), format!("{error:#}")))
                            .collect(),
                    };
                for (id, reason) in &failed {
                    match each.iter_mut().find(|(each_id, _)| each_id == id) {
                        Some((_, stands)) => *stands = DefinitionCheck::Redefined,
                        None => each.push((id.clone(), DefinitionCheck::Redefined)),
                    }
                    why.push_str(&format!("; {id} does not print: {reason}"));
                }
                if !failed.is_empty() {
                    result = DefinitionCheck::Redefined;
                }
            }
            // The figures are computed by code chosen by each metric's id and version, so the
            // code counts as part of the definitions: read at the baseline and at the candidate
            // through the checked history, any difference redefines every metric.
            let mut history = History::open(tree, deadline)?;
            let code_was = history.file(&base, FIGURES_CODE)?;
            let code_now = history.file(revision, FIGURES_CODE)?;
            if code_was != code_now {
                for (_, stands) in &mut each {
                    *stands = DefinitionCheck::Redefined;
                }
                if let Some(candidate) = &candidate {
                    for id in candidate.metrics.keys() {
                        if !each.iter().any(|(each_id, _)| each_id == id) {
                            each.push((id.clone(), DefinitionCheck::Redefined));
                        }
                    }
                }
                result = DefinitionCheck::Redefined;
                why.push_str(&format!(
                    "; {FIGURES_CODE}, the code that computes the figures, differs from the baseline's"
                ));
            }
            let changed: Vec<String> = each
                .iter()
                .filter(|(_, stands)| *stands != DefinitionCheck::Unchanged)
                .map(|(id, stands)| format!("{id} {}", stands.as_str()))
                .collect();
            Ok(Observation {
                summary: format!(
                    "the metric definitions at {} against the baseline's at {}: {}{}{why}",
                    short(revision),
                    short(&base),
                    result.as_str(),
                    if changed.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", changed.join(", "))
                    }
                ),
                records: vec![Record::new(
                    format!("definition-check-{seq}"),
                    self.kind(),
                    Some(result.as_str().to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )],
                artifact: None,
            })
        })();
        Ok((held_to(deadline, self.action(), observed)?, false))
    }
}

/// The evidence records of the case whose branch `tree` is on, in name order (the order they
/// were written), each read as YAML, of which a JSON record is one.
fn case_records(tree: &Path) -> Result<Vec<serde_yaml_ng::Value>> {
    let dir = crate::case::current(tree)?.join("evidence");
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json" || extension == "yaml")
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            read_yaml(path, &text)
        })
        .collect()
}

/// Whether `record` is of `kind` with `result` (any when none) about `subject` at `revision`.
fn is_record(
    record: &serde_yaml_ng::Value,
    kind: &str,
    result: Option<&str>,
    subject: &str,
    revision: &str,
) -> bool {
    record["kind"].as_str() == Some(kind)
        && result.is_none_or(|result| record["result"].as_str() == Some(result))
        && record["subject"].as_str() == Some(subject)
        && record["subject_revision"].as_str() == Some(revision)
}

/// The turns the case's latest `loop_measurement` about `loop_baseline` at `base` read, as the
/// measurement it retained names them; none without such a measurement or its retained file.
fn measured_rows(tree: &Path, base: &str) -> Result<Option<Vec<LoopScorecardRow>>> {
    let Some(id) = case_records(tree)?
        .iter()
        .rev()
        .find(|record| is_record(record, LoopMeasure::KIND, None, "loop_baseline", base))
        .and_then(|record| record["id"].as_str().map(str::to_owned))
    else {
        return Ok(None);
    };
    let path = crate::case::current(tree)?
        .join("artifacts")
        .join(format!("{id}.json"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let retained: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    // The rows the measurement read, as it retained them; one that retained none is no
    // measurement a comparison can stand on.
    Ok(serde_json::from_value(retained["rows"].clone()).ok())
}

/// `loop.compare`: due once the case holds a `merge_observation` of the loop candidate (the
/// protocol's precondition; nothing is recorded without one). The turns whose baseline revision
/// descends from the merged candidate, at least [`MIN_TURNS`] of them with a verdict, against
/// the turns the case's `loop_measurement` of the baseline read, under the baseline's definitions
/// as the case snapshotted them when it opened ([`snapshot_baseline`]); no snapshot records
/// nothing. The metrics compared are those whose current figure missed
/// its target before; `improved` when each of them is better after, in its target's direction,
/// else `not_improved`. Writes one `loop_comparison` record about `loop_candidate` and retains
/// both sides' figures as `loop-comparison-<seq>.json`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopCompare;

impl LoopCompare {
    /// The action it performs.
    pub const ACTION: &str = "loop.compare";
    /// The evidence kind it writes.
    pub const KIND: &str = "loop_comparison";
}

impl Validator for LoopCompare {
    fn name(&self) -> &str {
        "takt.loop-compare"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "loop_candidate"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_within(tree, revision, seq, None)
            .map(|(observation, _)| observation)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = (|| -> Result<Observation> {
            let base = case_revision(tree, "loop_baseline")?;
            let definitions = match snapshotted(tree, &base)? {
                Ok(definitions) => definitions,
                Err(why) => return Ok(nothing(format!("the baseline: {why}; nothing recorded"))),
            };
            let records = case_records(tree)?;
            if !records.iter().any(|record| {
                is_record(
                    record,
                    "merge_observation",
                    Some("merged"),
                    "loop_candidate",
                    revision,
                )
            }) {
                return Ok(nothing(format!(
                    "no merge of the loop candidate {} is recorded; nothing recorded",
                    short(revision)
                )));
            }
            let Some(measured) = measured_rows(tree, &base)? else {
                return Ok(nothing(format!(
                    "no loop measurement of the baseline {} is retained in the case; nothing recorded",
                    short(&base)
                )));
            };
            let card = LoopScorecard::read_within(tree, Some(&base), deadline)?;
            // The before side is the rows the measurement read, as it retained them, never
            // recomputed from today's records; a measured turn gone from the cases is refused.
            let missing: Vec<&str> = measured
                .iter()
                .filter(|row| !card.rows.iter().any(|now| now.turn == row.turn))
                .map(|row| row.turn.as_str())
                .collect();
            if !missing.is_empty() {
                return Ok(nothing(format!(
                    "the measured turns {} are no longer recorded; nothing recorded",
                    missing.join(", ")
                )));
            }
            let before: Vec<&LoopScorecardRow> = measured.iter().collect();
            // One history serves every row: each commit is read and checked once. A row whose
            // ancestry the shallow history cannot say is left out, named, and not recorded.
            let mut history = History::open(tree, deadline)?;
            let mut after: Vec<&LoopScorecardRow> = Vec::new();
            let mut unknown: Vec<&str> = Vec::new();
            for row in &card.rows {
                match history.descends(revision, &row.baseline)? {
                    Ancestry::Yes => after.push(row),
                    Ancestry::No => {}
                    Ancestry::Unknown => unknown.push(&row.turn),
                }
            }
            let ran = after.iter().filter(|row| row.verdict.is_some()).count();
            if ran < MIN_TURNS {
                return Ok(nothing(format!(
                    "{ran} turns with a verdict on a baseline that holds the candidate {}, at least {MIN_TURNS} needed; nothing recorded",
                    short(revision)
                )));
            }
            let (was, now) = match (
                figures_of(&before, &definitions),
                figures_of(&after, &definitions),
            ) {
                (Ok(was), Ok(now)) => (was, now),
                (Err(error), _) | (_, Err(error)) => {
                    return Ok(nothing(format!("{error:#}; nothing recorded")));
                }
            };
            let mut compared = Vec::new();
            let mut improved = true;
            for figure in was.iter().filter(|f| f.current && f.below_target) {
                compared.push(figure.metric.clone());
                let later = now
                    .iter()
                    .find(|f| f.current && f.metric == figure.metric)
                    .and_then(|f| f.value);
                // A metric below target for want of data before improved once it has a figure
                // after that meets its target.
                improved &= match (figure.value, later, figure.target) {
                    (Some(before), Some(after), Some(target)) => target.improves(before, after),
                    (None, Some(after), Some(target)) => !target.missed_by(after),
                    _ => false,
                };
            }
            let result = if improved && !compared.is_empty() {
                "improved"
            } else {
                "not_improved"
            };
            let artifact = format!("loop-comparison-{seq}.json");
            let retained = serde_json::json!({
                "format": MEASUREMENT_FORMAT,
                "definitions_revision": base,
                "candidate": revision,
                "compared": compared,
                "result": result,
                "before": { "turns": before.iter().map(|row| &row.turn).collect::<Vec<_>>(), "figures": was },
                "after": { "turns": after.iter().map(|row| &row.turn).collect::<Vec<_>>(), "figures": now },
                "unknown": unknown,
            });
            Ok(Observation {
                summary: format!(
                    "{} turns before the merge of {} against {} after, on {}: {result}; before: {}; after: {}{}; retained as {artifact}",
                    before.len(),
                    short(revision),
                    after.len(),
                    if compared.is_empty() {
                        "no metric below its target before".to_owned()
                    } else {
                        compared.join(", ")
                    },
                    line(&was),
                    line(&now),
                    if unknown.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "; unknown in the shallow history, not recorded: {}",
                            unknown.join(", ")
                        )
                    }
                ),
                records: vec![Record::new(
                    format!("loop-comparison-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )],
                artifact: Some(Artifact {
                    name: artifact,
                    bytes: serde_json::to_vec_pretty(&retained)
                        .context("serialize the loop comparison")?,
                }),
            })
        })();
        Ok((held_to(deadline, self.action(), observed)?, false))
    }
}
