//! Validators: programs that turn an observation into evidence: `tests.run`, `gate.run`,
//! `pr.open`, `ci.read`, `repository.merge` and `change.read`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{Context as _, Result, anyhow, bail};

use crate::case::{BRANCH_FILE, Branch, current};
use crate::evidence::Record;
use crate::forge::{BotRoute, Checks, Forge, Limits, PullRequest, PullRequestState};

/// What a validator observed, and the records it wrote from it.
#[derive(Debug)]
pub struct Observation {
    /// One line on what was observed.
    pub summary: String,
    /// The evidence records the observation yields.
    pub records: Vec<Record>,
    /// What the validator retains beside its records, when it keeps something: the
    /// observation as its provider wrote it, kept under the case as `artifacts/<name>`.
    pub artifact: Option<Artifact>,
}

/// One file a validator retains with an observation.
#[derive(Clone)]
pub struct Artifact {
    /// The file name under the case's `artifacts/`.
    pub name: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Artifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Artifact")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// A program that performs one deterministic action and answers evidence.
pub trait Validator {
    /// The validator's name, the producer of its records.
    fn name(&self) -> &str;
    /// The protocol action it performs.
    fn action(&self) -> &str;
    /// The evidence kind it produces.
    fn kind(&self) -> &str;
    /// The artifact its records are about.
    fn subject(&self) -> &str;
    /// Performs the action in `tree` about `revision`; `seq` makes record ids unique.
    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation>;
    /// Observes as [`Self::observe`] does, and says whether an observation that wrote no record
    /// waits on the forge: on a state outside the tree that moves without a commit (checks
    /// still running, a pull request's head not yet at the head pushed, a merge held for a
    /// required check that has not reported), so the same observation made again after a while
    /// may write one. The loop then observes again within its budget and offers no judgement
    /// step meanwhile. Not waiting by default: an observation without a record is final at its
    /// revision.
    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        Ok((self.observe(tree, revision, seq)?, false))
    }
    /// Observes as [`Self::observe_waiting`] does, a child it runs held to `deadline` (the
    /// instant the run's hour budget ends, the run's own; `None` for a run without one) as well
    /// as its own timeout. By default `deadline` is not read: the validator runs no child of its
    /// own.
    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let _ = deadline;
        self.observe_waiting(tree, revision, seq)
    }
    /// The files under the case's directory the validator itself writes while it observes,
    /// relative to it. Nothing else under the cases directory may change during an
    /// observation: the loop puts back whatever else changed and refuses the observation,
    /// because what a validator runs (a test command, a gate's commands) may be code a model
    /// wrote. None by default.
    fn writes(&self) -> Vec<PathBuf> {
        Vec::new()
    }
    /// The words of the command the validator runs from the tree, the test command's: what
    /// names the command's own script on the run's judge surface
    /// ([`crate::run::JudgeSurface`]), which no judgement step changes. None by default.
    fn runs(&self) -> Vec<String> {
        Vec::new()
    }
}

/// What the case's branch did to the judge surface between its base and a revision.
enum Judged {
    /// Nothing on it differs.
    Unchanged,
    /// These paths differ from the case's base, shown as the summary names it.
    Changed { base: String, paths: Vec<String> },
    /// The case pins no base commit to compare with.
    NoBase,
}

/// Whether the case's branch changed what decides `tests.run` and `ci.read` between the case's
/// base at `dir` and `revision`: the run's judge surface ([`crate::run::JudgeSurface`]) but the
/// planning store, a link on it or on the way to a script at either commit, and the entries of a
/// cargo manifest that decide how cargo builds or runs the tests
/// ([`crate::run::manifest_build_changes`]) and of `pyproject.toml`'s pytest tables, the surface
/// extended by the repository's own list ([`crate::run::JUDGE_LIST`]) read at the base, which
/// records nothing when it does not parse. A change no step
/// could make, because the step guard refuses it, but one that reached the branch by another
/// route (a commit made outside the loop) would decide the verdict all the same. The store is
/// guarded at the step too, but it decides no test and no check, and takt itself writes it on
/// the case's branch (the take, an accepted case's test result and move): it is not compared.
fn judged(tree: &Path, dir: &Path, revision: &str, runs: &[String]) -> Result<Judged> {
    let Some((base, at)) = base_revision(tree, dir)? else {
        return Ok(Judged::NoBase);
    };
    // The repository's own list, read at the base: one that does not parse records nothing.
    let mut paths: Vec<String> = Vec::new();
    let globs = match crate::run::judge_list_at(tree, &at)? {
        Ok(globs) => globs,
        Err(why) => {
            paths.push(why);
            Vec::new()
        }
    };
    // The case's module names, and the top-level modules the base held: a name the base held
    // in an import directory is the project's own, one the branch added there is not.
    let at_base = crate::git::run(
        tree,
        &["ls-tree", "-r", "-z", "--name-only", "--full-tree", &at],
    )?;
    let surface = crate::run::JudgeSurface::new(runs, tree)
        .without_store()
        .with_globs(globs)
        .with_modules(crate::run::Modules::read(dir))
        .provided_by(at_base.split('\0').filter(|path| !path.is_empty()));
    // A link is never followed: one the base or the revision holds on the surface, or on the
    // way to a script, is named, and nothing is recorded while it is there.
    for commit in [at.as_str(), revision] {
        let listed = crate::git::run(tree, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
        for entry in listed.split('\0') {
            let Some((meta, path)) = entry.split_once('\t') else {
                continue;
            };
            if meta.starts_with("120000 ") && surface.reads_through(path) {
                let named = format!("{path} (a link)");
                if !paths.contains(&named) {
                    paths.push(named);
                }
            }
        }
    }
    let diff = crate::git::run(
        tree,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--name-only",
            &at,
            revision,
            "--",
        ],
    )?;
    for path in diff.split('\0').filter(|path| !path.is_empty()) {
        // While a line of the test command's script fails closed every program is on the
        // surface: one the base held is as much one as the tree's now.
        // In a directory on `PATH` an entry the base held executable is a program a lookup
        // finds, as one executable now is.
        if surface.contains(path)
            || (surface.guards_programs() && program_at(tree, &at, path)?)
            || (surface.in_a_path_dir(path) && executable_at(tree, &at, path)?)
        {
            paths.push(path.to_owned());
        } else if crate::run::is_manifest(path) || crate::run::is_pyproject(path) {
            let before = crate::run::manifest_at(tree, &at, path)?;
            let after = crate::run::manifest_at(tree, revision, path)?;
            let compared = if crate::run::is_manifest(path) {
                crate::run::manifest_build_changes(&before, &after)
            } else {
                crate::run::pytest_config_changes(&before, &after)
            };
            match compared {
                Ok(keys) => paths.extend(keys.into_iter().map(|key| format!("{path}: {key}"))),
                Err(line) => paths.push(format!(
                    "{path}: a line the manifest scanner cannot read with certainty, {line}"
                )),
            }
        }
    }
    // Where the module names came from, when one of the paths is such a module.
    let note = surface.modules_note(&paths);
    if let Some(last) = paths.last_mut() {
        last.push_str(&note);
    }
    Ok(if paths.is_empty() {
        Judged::Unchanged
    } else {
        Judged::Changed { base, paths }
    })
}

/// An empty home made for one run of a test command or a gate's commands, outside the tree,
/// removed when dropped. The command runs with `HOME` there and `PYTHONNOUSERSITE=1`, so
/// nothing a step wrote in the caller's home (Python's user site, its start-up files) runs
/// with it; cargo and rustup keep the caller's homes ([`FreshHome::vars`]).
pub(crate) struct FreshHome(PathBuf);

impl FreshHome {
    /// A new empty directory under the system's temporary directory, readable by its owner
    /// alone.
    pub(crate) fn new() -> Result<Self> {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos());
        let dir =
            std::env::temp_dir().join(format!("takt-home-{}-{made}-{nanos}", std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&dir)
            .with_context(|| format!("create a home for the command at {}", dir.display()))?;
        Ok(Self(dir))
    }

    /// What the command runs with, given what it would inherit (`inherited`): `HOME` this
    /// directory, `PYTHONNOUSERSITE=1`, and `CARGO_HOME` and `RUSTUP_HOME`, when it would
    /// inherit none, the caller's (its own variable, or `.cargo` and `.rustup` in its home).
    fn vars(&self, inherited: impl Fn(&str) -> bool) -> Vec<(&'static str, std::ffi::OsString)> {
        let caller_home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
            .or_else(std::env::home_dir);
        let mut vars = vec![
            ("HOME", self.0.clone().into_os_string()),
            ("PYTHONNOUSERSITE", "1".into()),
        ];
        for (name, under) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
            if inherited(name) {
                continue;
            }
            let caller = std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .or_else(|| caller_home.as_ref().map(|home| home.join(under).into()));
            vars.extend(caller.map(|value| (name, value)));
        }
        vars
    }

    /// The assertions project file `project` with every command it admits, and every
    /// provider's, run in this home ([`FreshHome::vars`]): the gate's commands inherit nothing,
    /// so `CARGO_HOME` and `RUSTUP_HOME` are set unless the binding sets them.
    fn admitting(&self, project: &str) -> Result<String> {
        let mut doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(project).context("read the project file")?;
        let Some(top) = doc.as_mapping_mut() else {
            bail!("the project file is not a mapping");
        };
        let mut bindings: Vec<&mut serde_yaml_ng::Mapping> = Vec::new();
        for (key, value) in top.iter_mut() {
            let Some(entries) = value.as_mapping_mut() else {
                continue;
            };
            match key.as_str() {
                Some("commands") => bindings.extend(
                    entries
                        .values_mut()
                        .filter_map(serde_yaml_ng::Value::as_mapping_mut),
                ),
                Some("providers") => {
                    bindings.extend(entries.values_mut().filter_map(|provider| {
                        provider
                            .get_mut("command")
                            .and_then(serde_yaml_ng::Value::as_mapping_mut)
                    }));
                }
                _ => {}
            }
        }
        for binding in bindings {
            let env = binding
                .entry("env".into())
                .or_insert_with(|| serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()));
            let Some(env) = env.as_mapping_mut() else {
                bail!("the project file binds an `env` that is not a mapping");
            };
            let set: Vec<String> = env
                .keys()
                .filter_map(|key| key.as_str().map(str::to_owned))
                .collect();
            for (name, value) in self.vars(|name| set.iter().any(|key| key == name)) {
                env.insert(name.into(), value.to_string_lossy().into_owned().into());
            }
        }
        serde_yaml_ng::to_string(&doc).context("write the project file")
    }

    /// The directory.
    pub(crate) fn dir(&self) -> &Path {
        &self.0
    }

    /// Runs `command` in this home ([`FreshHome::vars`]).
    pub(crate) fn apply(&self, command: &mut Command) {
        for (name, value) in self.vars(|name| std::env::var_os(name).is_some()) {
            command.env(name, value);
        }
    }
}

impl Drop for FreshHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Whether the commit `at` holds an executable entry at `path`: a file with an exec bit, or a
/// link.
fn executable_at(tree: &Path, at: &str, path: &str) -> Result<bool> {
    Ok(object_at(tree, at, path)?
        .is_some_and(|object| object.starts_with("100755 ") || object.starts_with("120000 ")))
}

/// Whether the commit `at` holds a program at `path`: a file with an exec bit, or one whose
/// first two bytes are `#!`.
fn program_at(tree: &Path, at: &str, path: &str) -> Result<bool> {
    let Some(object) = object_at(tree, at, path)? else {
        return Ok(false);
    };
    let mut fields = object.split_whitespace();
    let (mode, kind, id) = (fields.next(), fields.next(), fields.next());
    if mode == Some("100755") {
        return Ok(true);
    }
    match (kind, id) {
        (Some("blob"), Some(id)) => Ok(crate::git::blob(tree, id)?.starts_with(b"#!")),
        _ => Ok(false),
    }
}

/// An observation that recorded nothing, its summary `summary`.
fn nothing_recorded(summary: String) -> Observation {
    Observation {
        summary,
        records: Vec::new(),
        artifact: None,
    }
}

/// `tests.run`: runs the test command without a shell; exit 0 is `pass`, anything else `fail`.
/// The command is code from the tree: it gets no credential, no standard input, a timeout past
/// which it is killed with what it started, and only a bounded tail of its output is kept.
/// What decides it besides the code under test is on the run's judge surface, which no step
/// changes: the cargo configuration, the toolchain, build scripts and the command's own script
/// ([`crate::run::JudgeSurface`]). In a case, nothing is run and no record written while the
/// case's branch changed one of those since its base, or when the case pins no base to compare
/// with; the summary names the paths.
#[derive(Debug, Clone)]
pub struct TestsRun {
    /// The command and its arguments.
    pub command: Vec<String>,
    /// What the command is held to.
    pub limits: Limits,
}

impl TestsRun {
    /// The action it performs.
    pub const ACTION: &str = "tests.run";
    /// The evidence kind it writes.
    pub const KIND: &str = "test_result";

    /// Splits `command` at white space.
    pub fn new(command: &str) -> Result<Self> {
        let command: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        if command.is_empty() {
            bail!("the test command is empty");
        }
        Ok(Self {
            command,
            limits: Self::LIMITS,
        })
    }

    /// The limits a test command is held to by default: thirty minutes, 64 KiB of output.
    pub const LIMITS: Limits = Limits {
        timeout: std::time::Duration::from_secs(1800),
        output: 64 << 10,
    };

    /// Held to `limits`.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

impl Validator for TestsRun {
    fn name(&self) -> &str {
        "takt.tests-run"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn runs(&self) -> Vec<String> {
        self.command.clone()
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_by(tree, revision, seq, None)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq, deadline);
        Ok((ended_by(deadline, self.action(), observed)?, false))
    }
}

/// What an observation held to `deadline` answers: an error once it returned at or after it,
/// whatever it observed, so the budget that ended it records nothing; else `observed`.
fn ended_by(
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

impl TestsRun {
    /// Observes, the test command held to the smaller of its own timeout and what is left
    /// before `deadline` when it starts.
    fn observe_by(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Observation> {
        let shown = self.command.join(" ");
        // A tree on no case's branch is no case's observation and has no base to compare with.
        if crate::git::current_branch(tree)?
            .is_some_and(|branch| branch.starts_with(crate::case::BRANCH_PREFIX))
        {
            let dir = current(tree)?;
            match judged(tree, &dir, revision, &self.command)? {
                Judged::Unchanged => {}
                Judged::NoBase => {
                    return Ok(nothing_recorded(format!(
                        "the test command `{shown}`: not run; the case pins no base commit ({BRANCH_FILE} has no base_commit) to compare its judge surface with; no record, the claim stays unknown; abandon the case and open it again"
                    )));
                }
                Judged::Changed { base, paths } => {
                    return Ok(nothing_recorded(format!(
                        "the test command `{shown}`: not run; the case's branch at {} changed the judge surface since {base}: {}; no record, the claim stays unknown",
                        short(revision),
                        paths.join(", ")
                    )));
                }
            }
        }
        let mut command = Command::new(&self.command[0]);
        command.args(&self.command[1..]).current_dir(tree);
        crate::forge::without_credentials(&mut command, |_| false);
        let home = FreshHome::new()?;
        home.apply(&mut command);
        let mut limits = self.limits;
        if let Some(deadline) = deadline {
            limits.timeout = limits
                .timeout
                .min(deadline.saturating_duration_since(std::time::Instant::now()));
        }
        let output = crate::forge::run_child(&mut command, limits)
            .with_context(|| format!("run {}", self.command.join(" ")))?;
        drop(home);
        let result = if output.success() { "pass" } else { "fail" };
        let tail =
            String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
        let tail: String = tail
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        let record = Record::new(
            format!("test-result-{seq}"),
            self.kind(),
            Some(result.to_owned()),
            self.subject(),
            revision,
            self.name(),
        );
        Ok(Observation {
            summary: format!(
                "the test command exited {}: {result}; {}",
                output.code.map_or("signal".to_owned(), |c| c.to_string()),
                tail.chars().take(300).collect::<String>()
            ),
            records: vec![record],
            artifact: None,
        })
    }
}

/// The project file a gates document is evaluated with when the validator is given none and
/// the tree has it: `engineering-assertions-project/1`, at the path the assertions guide names.
pub const DEFAULT_PROJECT: &str = ".engineering/assertions-project.yaml";

/// `gate.run`: evaluates the repository's `engineering-gates/1` document through the
/// engineering assertions library, as `canon-engineering gates run` does. Every gate true is
/// `pass`, any gate false is `fail`; an unknown evaluation writes no record, because a claim
/// that could not be observed stays unknown; an invalid document or provider datum is an error.
/// The project file admits the commands and providers the gates bind. Every observation
/// retains the bundle the library evaluated, `engineering-gate-evidence/1`, as
/// `<record-id>.bundle.json`.
///
/// The gates document and the project file are read from the case's base revision, the tip of
/// the branch in `branch.yaml`, never from the tree: the tree is the case's branch, which a
/// model edits, and a gate it rewrote would decide its own verdict. What the gates observe is
/// the tree. The two files are copied for the evaluation into the case's directory, which the
/// tree's source identity leaves out, and removed again before the observation ends.
#[derive(Debug, Clone)]
pub struct GateRun {
    /// The gates document, relative to the tree.
    pub gates: PathBuf,
    /// The `engineering-assertions-project/1` file that admits the gates' commands and
    /// providers, relative to the tree; `None` takes [`DEFAULT_PROJECT`] when the base
    /// revision has it.
    pub project: Option<PathBuf>,
}

/// The directory under the case's that holds the base revision's gate inputs while the gate
/// validator evaluates them.
const GATE_INPUTS: &str = "gate-inputs";

/// The case's base revision: the commit pinned in `branch.yaml` when the case opened, shown
/// with the base branch's name, and the commit. `None` for a case that pins none (one opened
/// before the pin was recorded): no branch name stands in for it, because a test command can
/// move any branch, even onto `HEAD`, without touching the tree.
fn base_revision(tree: &Path, case_dir: &Path) -> Result<Option<(String, String)>> {
    let branch = Branch::read(case_dir)?;
    let shown = branch.base.as_deref().unwrap_or("a detached HEAD");
    let Some(pinned) = &branch.base_commit else {
        return Ok(None);
    };
    let at = crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{pinned}^{{commit}}"),
        ],
    )
    .with_context(|| format!("resolve the case's base commit {pinned}"))?;
    Ok(Some((
        format!("{shown} at {}, pinned at open", short(&at)),
        at,
    )))
}

/// The files a project file names that decide what a gate runs: the executables of the
/// commands and providers it admits, and the catalogs it extends the functions with; those
/// relative to the tree, which a model can edit, as they are named.
fn admitted_files(project: &str) -> Result<Vec<String>> {
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(project).context("read the project file")?;
    let mut paths = Vec::new();
    let named = |value: &serde_yaml_ng::Value| value.as_str().map(str::to_owned);
    if let Some(commands) = doc["commands"].as_mapping() {
        paths.extend(commands.values().filter_map(|c| named(&c["executable"])));
    }
    if let Some(providers) = doc["providers"].as_mapping() {
        paths.extend(
            providers
                .values()
                .filter_map(|p| named(&p["command"]["executable"])),
        );
    }
    if let Some(catalogs) = doc["catalogs"].as_sequence() {
        paths.extend(catalogs.iter().filter_map(named));
    }
    // Relative to the tree and normalised; one that leads out of the tree is not the tree's.
    let mut inside: Vec<String> = paths
        .iter()
        .filter(|path| Path::new(path).is_relative())
        .filter_map(|path| {
            let mut parts: Vec<&str> = Vec::new();
            for part in path.split('/') {
                match part {
                    "" | "." => {}
                    ".." => {
                        parts.pop()?;
                    }
                    part => parts.push(part),
                }
            }
            (!parts.is_empty()).then(|| parts.join("/"))
        })
        .collect();
    inside.sort();
    inside.dedup();
    Ok(inside)
}

/// What git stores at `path` in the commit `at`: its mode and object, or `None` when the commit
/// has no such path.
fn object_at(tree: &Path, at: &str, path: &str) -> Result<Option<String>> {
    let listed = crate::git::run(tree, &["ls-tree", "--full-tree", at, "--", path])?;
    Ok(listed
        .split('\t')
        .next()
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned))
}

/// The text of `path`, relative to the tree, at the commit `at`; `None` when it has no such
/// file.
fn file_at(tree: &Path, at: &str, path: &Path) -> Result<Option<String>> {
    let relative = path
        .to_str()
        .filter(|path| path.split('/').all(|part| part != ".."))
        .filter(|_| path.is_relative())
        .ok_or_else(|| anyhow!("{} is not a path inside the tree", path.display()))?;
    let spec = format!("{at}:./{relative}");
    if crate::git::run(tree, &["cat-file", "-e", &spec]).is_err() {
        return Ok(None);
    }
    crate::git::run(tree, &["show", &spec]).map(|text| Some(format!("{text}\n")))
}

impl GateRun {
    /// The action it performs.
    pub const ACTION: &str = "gate.run";
    /// The evidence kind it writes.
    pub const KIND: &str = "gate_result";

    /// Over the gates document at `gates`, relative to the tree.
    #[must_use]
    pub fn new(gates: impl Into<PathBuf>) -> Self {
        Self {
            gates: gates.into(),
            project: None,
        }
    }

    /// With the project file at `project`, relative to the tree.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<PathBuf>) -> Self {
        self.project = Some(project.into());
        self
    }
}

impl Validator for GateRun {
    fn name(&self) -> &str {
        "takt.gate-run"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_by(tree, revision, seq, None)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq, deadline);
        Ok((ended_by(deadline, self.action(), observed)?, false))
    }
}

impl GateRun {
    /// Observes, each command the gate runs held to the smaller of the project file's timeout
    /// and what is left before `deadline` when the gate's commands are collected.
    fn observe_by(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Observation> {
        let dir = current(tree)?;
        let Some((base, at)) = base_revision(tree, &dir)? else {
            return Ok(Observation {
                summary: format!(
                    "the gates document {}: not run; the case pins no base commit ({BRANCH_FILE} has no base_commit; it was opened before takt recorded one), and the gate is read from nothing else; no record, the claim stays unknown; abandon the case and open it again",
                    self.gates.display()
                ),
                records: Vec::new(),
                artifact: None,
            });
        };
        let document = format!("{} from {base}", self.gates.display());
        let gates = file_at(tree, &at, &self.gates)?
            .ok_or_else(|| anyhow!("the gates document {document} is not in the base revision"))?;
        let project = match &self.project {
            Some(path) => Some((
                path.clone(),
                file_at(tree, &at, path)?.ok_or_else(|| {
                    anyhow!(
                        "the project file {} is not in the base revision, {base}",
                        path.display()
                    )
                })?,
            )),
            None => file_at(tree, &at, Path::new(DEFAULT_PROJECT))?
                .map(|text| (PathBuf::from(DEFAULT_PROJECT), text)),
        };
        // What the project file admits runs from the tree: an executable or a catalog the case's
        // branch changed would let a model's edit decide the gate the base revision defines.
        // Nothing is run and no record written while one differs from the base revision.
        if let Some((path, text)) = &project {
            let mut changed = Vec::new();
            for file in admitted_files(text)
                .with_context(|| format!("read the project file {}", path.display()))?
            {
                if object_at(tree, &at, &file)? != object_at(tree, revision, &file)? {
                    changed.push(file);
                }
            }
            if !changed.is_empty() {
                return Ok(Observation {
                    summary: format!(
                        "the gates document {document}: not run; what the project file {} admits differs from the base revision at {}: {}; no record, the claim stays unknown",
                        path.display(),
                        short(revision),
                        changed.join(", ")
                    ),
                    records: Vec::new(),
                    artifact: None,
                });
            }
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        // The case directory, which the loop writes every pass, stays out of the bundle's
        // source identity: that identity is the repository tree's. The base revision's two
        // files are evaluated from there, the paths relative to the tree as the library takes
        // them, and are gone again before the observation ends.
        let cases = Path::new(crate::case::CASES_DIR);
        let inputs = dir.join(GATE_INPUTS);
        let relative = inputs
            .strip_prefix(tree)
            .with_context(|| format!("{} is not under the tree", inputs.display()))?
            .to_path_buf();
        let copy = |kind: &str, path: &Path, text: &str| -> Result<PathBuf> {
            let name = path
                .file_name()
                .ok_or_else(|| anyhow!("{} names no file", path.display()))?;
            let at = inputs.join(kind);
            std::fs::create_dir_all(&at).with_context(|| format!("create {}", at.display()))?;
            std::fs::write(at.join(name), text)
                .with_context(|| format!("write {}", at.join(name).display()))?;
            Ok(relative.join(kind).join(name))
        };
        let clear = || match std::fs::remove_dir_all(&inputs) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("remove {}", inputs.display()))
            }
            _ => Ok(()),
        };
        clear()?;
        // The gate's commands run in a home of their own, as the test command does.
        let home = FreshHome::new()?;
        let collected = (|| -> Result<_> {
            let gates = copy("gates", &self.gates, &gates)?;
            let copied = project
                .as_ref()
                .map(|(path, text)| copy("project", path, &home.admitting(text)?))
                .transpose()?;
            let mut prepared = canon_engineering_assertions::prepare(
                tree,
                &gates,
                copied.as_deref(),
                Some(cases),
            )
            .with_context(|| match &project {
                Some((project, _)) => format!(
                    "prepare the gates document {document} with the project file {} from {base}",
                    project.display()
                ),
                None => format!("prepare the gates document {document}"),
            })?;
            if let Some(deadline) = deadline {
                prepared.context.timeout = prepared
                    .context
                    .timeout
                    .min(deadline.saturating_duration_since(std::time::Instant::now()));
            }
            canon_engineering_assertions::collect(prepared, now)
                .with_context(|| format!("collect the observations of {document}"))
        })();
        drop(home);
        let cleared = clear();
        let bundle = collected?;
        cleared?;
        let report = canon_engineering_assertions::evaluate(&bundle, now, None, None)
            .with_context(|| format!("evaluate {document}"))?;
        // The report holds the gates' assertions in declaration order; the gate that decided
        // is the first whose assertion shares the report's truth.
        let decided = bundle
            .document
            .gates
            .iter()
            .flat_map(|gate| std::iter::repeat_n(gate.id.as_str(), gate.assertions.len()))
            .zip(&report.assertions)
            .find(|(_, evaluation)| evaluation.truth == report.truth)
            .map(|(gate, evaluation)| {
                (
                    gate,
                    evaluation.expression.as_str(),
                    evaluation.reasons.join("; "),
                )
            });
        let decided = |truth: &str| match &decided {
            Some((gate, expression, reasons)) if reasons.is_empty() => {
                format!("gate {gate} {truth}: {expression}")
            }
            Some((gate, expression, reasons)) => format!(
                "gate {gate} {truth}: {expression} ({})",
                reasons.chars().take(300).collect::<String>()
            ),
            None => format!("no assertion is {truth}"),
        };
        let artifact = format!("gate-result-{seq}.bundle.json");
        let (result, summary) = match report.truth {
            canon_expr::Truth::True => {
                let gates: Vec<&str> = bundle
                    .document
                    .gates
                    .iter()
                    .map(|gate| gate.id.as_str())
                    .collect();
                (
                    Some("pass"),
                    format!(
                        "the gates document {document}: pass; every gate true: {}; retained as {artifact}",
                        gates.join(", ")
                    ),
                )
            }
            canon_expr::Truth::False => (
                Some("fail"),
                format!(
                    "the gates document {document}: fail; {}; retained as {artifact}",
                    decided("false")
                ),
            ),
            canon_expr::Truth::Unknown => (
                None,
                format!(
                    "the gates document {document}: unknown; {}; no record, the claim stays unknown; retained as {artifact}",
                    decided("unknown")
                ),
            ),
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("gate-result-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        let bytes = serde_json::to_vec_pretty(&bundle).context("serialize the evaluated bundle")?;
        Ok(Observation {
            summary,
            records,
            artifact: Some(Artifact {
                name: artifact,
                bytes,
            }),
        })
    }
}

/// The first eight characters of a revision, as the loop prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// Whether the pull request head `head` carries the implementation revision `revision` of the
/// case at `case_dir`: it is the revision, or it reaches the revision only through takt's
/// riders for the story ([`crate::case::approvals_only`]: the approval's commits, the
/// `test_result` and the move to `implemented`, each file byte-equal to the record the case
/// keeps), which are how those records reach the base with the pull request. A head that is
/// not a commit of the repository carries nothing.
fn carries(tree: &Path, case_dir: &Path, revision: &str, head: &str) -> Result<bool> {
    if head == revision {
        return Ok(true);
    }
    if head.len() < 7
        || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
        || crate::git::run(tree, &["cat-file", "-e", &format!("{head}^{{commit}}")]).is_err()
    {
        return Ok(false);
    }
    crate::case::approvals_only(tree, revision, head, case_dir)
}

/// How a summary names a head that carries takt's riders above the revision; nothing
/// for a head that is the revision.
fn above(revision: &str, head: &str) -> String {
    if head == revision {
        String::new()
    } else {
        format!("; its head {} carries takt's records above it", short(head))
    }
}

/// `pr.open`: pushes the case's branch through the forge and opens its one pull request, the
/// head at the implementation revision; writes one `pull_request` record with the state the
/// forge reports, `open` for a pull request just opened. At a later revision the branch is
/// pushed again and the pull request's state read: one pull request per case. The tree's
/// `HEAD` must be the revision, or the revision with only takt's riders for the story above it
/// ([`crate::case::approvals_only`]), which are pushed with it so the store's records reach the
/// base with the pull request; anything else would not be what the claim is about, and nothing
/// is pushed. A number the case directory carries is trusted only once the forge reports the pull
/// request's head as the case's branch at the head pushed, else no record is written. A case
/// with no base is refused before anything is pushed. The title is the intent's first line,
/// the body the intent and the claims that hold; the forge passes both through `scan-text`
/// before they leave the machine.
pub struct PrOpen {
    /// The forge the push and the pull request go through.
    pub forge: Rc<dyn Forge>,
}

impl PrOpen {
    /// The action it performs.
    pub const ACTION: &str = "pr.open";
    /// The evidence kind it writes.
    pub const KIND: &str = "pull_request";

    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

impl std::fmt::Debug for PrOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrOpen")
            .field("forge", &self.forge.name())
            .finish()
    }
}

/// The pull request's title: the intent's first line.
fn title_of(intent: &str) -> String {
    intent
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("takt")
        .chars()
        .take(120)
        .collect()
}

/// The claims the case's latest decision holds, in id order; none before a decision.
fn claims_held(case_dir: &Path) -> Result<Vec<String>> {
    Ok(crate::case::latest_decision(case_dir)?
        .map(|decision| {
            decision
                .claims
                .iter()
                .filter(|(_, value)| value.as_str() == "true")
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default())
}

/// The pull request's body: the intent, then the case, the revision and the claims that hold.
fn body_of(case_dir: &Path, intent: &str, revision: &str) -> Result<String> {
    let case = case_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let claims = claims_held(case_dir)?;
    let claims = if claims.is_empty() {
        "none yet".to_owned()
    } else {
        claims.join(", ")
    };
    Ok(format!(
        "{}\n\n---\n\nOpened by takt for the case `{case}` at `{revision}`. Claims that hold: {claims}.\n",
        intent.trim()
    ))
}

/// What `pr.open` does after the push: opens the case's pull request, or reads the one it has.
enum Next {
    Open(PullRequest),
    Read(u64),
}

/// The case's branch, from the case directory's name: what the validators push and open for,
/// never the `name` a file under the case directory carries, which a harness can write.
fn branch_of(case_dir: &Path) -> Result<String> {
    let id = case_dir
        .file_name()
        .ok_or_else(|| anyhow!("the case directory {} has no name", case_dir.display()))?;
    Ok(Branch::name_of(&id.to_string_lossy()))
}

/// The pull request a case opens: `name` against `base`, the intent's first line as the
/// title, the intent and the claims that hold as the body. A case with no base is refused.
fn request_of(
    case_dir: &Path,
    name: &str,
    base: Option<&str>,
    revision: &str,
) -> Result<PullRequest> {
    let base = base.ok_or_else(|| {
        anyhow!(
            "the case has no base branch: it was opened from a detached HEAD; open it from the branch its pull request is against"
        )
    })?;
    let intent = case_dir.join("intent.md");
    let intent =
        std::fs::read_to_string(&intent).with_context(|| format!("read {}", intent.display()))?;
    Ok(PullRequest {
        branch: name.to_owned(),
        base: base.to_owned(),
        title: title_of(&intent),
        body: body_of(case_dir, &intent, revision)?,
    })
}

impl Validator for PrOpen {
    fn name(&self) -> &str {
        "takt.pr-open"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    /// The case's `branch.yaml`, and what the bot route keeps of the request it sent, the text it
    /// scanned as published and the response it read.
    fn writes(&self) -> Vec<PathBuf> {
        let artifacts = Path::new("artifacts");
        vec![
            PathBuf::from(BRANCH_FILE),
            artifacts.join(BotRoute::REQUEST),
            artifacts.join(BotRoute::RESPONSE),
            artifacts.join(BotRoute::PUBLISHED),
        ]
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    /// Waits on the forge when the pull request's head, on the case's branch, is not yet at
    /// the head pushed: the forge moves it after the push, without a commit.
    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        let dir = current(tree)?;
        // `HEAD` is the implementation revision, or takt's riders on top of it
        // (`approvals_only`); `HEAD` is what is pushed, so the pull request carries the
        // store's records to the base, and the record written is about the revision.
        let head = crate::git::head(tree)?;
        if head != revision && !crate::case::approvals_only(tree, revision, &head, &dir)? {
            bail!(
                "the tree's HEAD {head} is not the implementation revision {revision}; the pull request's head would not be the revision the claim is about"
            );
        }
        let mut branch = Branch::read(&dir)?;
        let forge = self.forge.name();
        // The branch pushed and opened for is the case's, from the case id: the `name` in
        // `branch.yaml` is in a file a harness can write, and a refspec there must never reach
        // the forge. A `name` that differs is set right, and the summary says so.
        let name = branch_of(&dir)?;
        let repaired = if branch.name == name {
            String::new()
        } else {
            let was = std::mem::replace(&mut branch.name, name.clone());
            format!("{BRANCH_FILE} named {was}, not the case's branch; set right; ")
        };
        // What follows the push is settled before it: a case with no pull request yet needs its
        // base and its request, and one without a base is refused with nothing pushed.
        let next = match branch.pull_request {
            Some(number) => Next::Read(number),
            None => Next::Open(request_of(&dir, &name, branch.base.as_deref(), revision)?),
        };
        // A pull request needs a commit on the case's branch beyond its base: at the base itself
        // the forge has nothing to compare, and opening one would be refused there. The take
        // committed when the case opened (`takt: take <story>`) is not such a commit: it moves
        // the story's status, not the work. Nothing is pushed and no record written; a commit
        // moves the revision and makes `pr.open` due again.
        if let Next::Open(request) = &next {
            let ahead = crate::git::run(
                tree,
                &[
                    "rev-list",
                    "--count",
                    "--invert-grep",
                    "--extended-regexp",
                    "--grep=^takt: take [^ ]+$",
                    &format!("{}..{revision}", request.base),
                ],
            )?;
            if ahead.trim() == "0" {
                return Ok((
                    Observation {
                        summary: format!(
                            "{name} at {} has no commit beyond {}; nothing pushed, no pull request; the claim stays unknown",
                            short(revision),
                            request.base
                        ),
                        records: Vec::new(),
                        artifact: None,
                    },
                    false,
                ));
            }
        }
        self.forge
            .push_revision(tree, &name, &head)
            .with_context(|| format!("push {name} through the forge {forge}"))?;
        branch.pushed = Some(head.clone());
        let carried = above(revision, &head);
        let mut waits = false;
        let (state, summary) = match next {
            Next::Read(number) => {
                let view = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge}")
                })?;
                if view.head.branch == name && view.head.sha == head {
                    (
                        Some(view.state),
                        format!(
                            "{repaired}pushed {name} at {} through {forge}{carried}; pull request #{number} is {}",
                            short(revision),
                            view.state
                        ),
                    )
                } else {
                    // The case's branch at another commit: the forge has not moved the pull
                    // request to the push yet. Another branch is not a lag, and not waited on.
                    waits = view.head.branch == name;
                    (
                        None,
                        format!(
                            "{repaired}pushed {name} at {} through {forge}; pull request #{number} has the head {} at {}, not the case's branch at the revision; no record, the claim stays unknown",
                            short(revision),
                            view.head.branch,
                            short(&view.head.sha)
                        ),
                    )
                }
            }
            Next::Open(request) => {
                let number = self
                    .forge
                    .open_pull_request(tree, &dir.join("artifacts"), &request)
                    .with_context(|| {
                        format!("open the pull request for {name} through the forge {forge}")
                    })?;
                branch.pull_request = Some(number);
                (
                    Some(PullRequestState::Open),
                    format!(
                        "{repaired}pushed {name} at {} through {forge}{carried} and opened pull request #{number}: open",
                        short(revision)
                    ),
                )
            }
        };
        branch.write(&dir)?;
        let records = state
            .map(|state| {
                vec![Record::new(
                    format!("pull-request-{seq}"),
                    self.kind(),
                    Some(state.as_str().to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        Ok((
            Observation {
                summary,
                records,
                artifact: None,
            },
            waits,
        ))
    }
}

/// `ci.read`: reads the checks on the head of the case's pull request through the forge and
/// writes one `ci_result` record, `pass` or `fail`. Pending checks write no record, because a
/// claim that could not be observed stays unknown; so does a case with no pull request yet, or
/// one whose pull request the forge does not report as the case's branch at the head pushed,
/// the revision or the revision with only takt's riders above it: a number the
/// case directory carries is not trusted on its own. Nothing but the pull request head is read.
/// Pending checks, and a pull request whose head on the case's branch is not yet at the head
/// pushed, wait on the forge. What the checks run is decided by the run's judge surface (the CI
/// workflows under `.github/`, the cargo configuration, the toolchain, build scripts, the test
/// command's own script; [`crate::run::JudgeSurface`]), which no step changes: nothing is read
/// and no record written while the case's branch changed it since its base, or when the case
/// pins no base to compare with; the summary names the paths.
pub struct CiRead {
    /// The forge the checks are read through.
    pub forge: Rc<dyn Forge>,
    /// The words of the commands the run's validators run from the tree ([`Validator::runs`]):
    /// what names the test command's own script on the judge surface.
    pub runs: Vec<String>,
}

impl CiRead {
    /// The action it performs.
    pub const ACTION: &str = "ci.read";
    /// The evidence kind it writes.
    pub const KIND: &str = "ci_result";

    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self {
            forge,
            runs: Vec::new(),
        }
    }

    /// With the run's validators running `runs` from the tree ([`Validator::runs`]).
    #[must_use]
    pub fn with_runs(mut self, runs: Vec<String>) -> Self {
        self.runs = runs;
        self
    }
}

impl std::fmt::Debug for CiRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CiRead")
            .field("forge", &self.forge.name())
            .finish()
    }
}

impl Validator for CiRead {
    fn name(&self) -> &str {
        "takt.ci-read"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        let dir = current(tree)?;
        let branch = Branch::read(&dir)?;
        // The case's branch, from the case id: not the `name` of a file a harness can write.
        let name = branch_of(&dir)?;
        let nothing = |summary: String, waits: bool| {
            (
                Observation {
                    summary,
                    records: Vec::new(),
                    artifact: None,
                },
                waits,
            )
        };
        match judged(tree, &dir, revision, &self.runs)? {
            Judged::Unchanged => {}
            Judged::NoBase => {
                return Ok(nothing(
                    format!(
                        "the case pins no base commit ({BRANCH_FILE} has no base_commit) to compare its judge surface with; nothing read, the claim stays unknown; abandon the case and open it again"
                    ),
                    false,
                ));
            }
            Judged::Changed { base, paths } => {
                return Ok(nothing(
                    format!(
                        "the case's branch at {} changed the judge surface since {base}: {}; nothing read, the claim stays unknown",
                        short(revision),
                        paths.join(", ")
                    ),
                    false,
                ));
            }
        }
        let Some(number) = branch.pull_request else {
            return Ok(nothing(
                "no pull request is open for the case; nothing to read, the claim stays unknown"
                    .to_owned(),
                false,
            ));
        };
        let pushed = match branch.pushed {
            Some(pushed) if carries(tree, &dir, revision, &pushed)? => pushed,
            pushed => {
                return Ok(nothing(
                    format!(
                        "the head of pull request #{number} is {}, not the implementation revision {}; nothing to read, the claim stays unknown",
                        pushed.as_deref().map_or("not pushed", short),
                        short(revision)
                    ),
                    false,
                ));
            }
        };
        let forge = self.forge.name();
        let view = self
            .forge
            .pull_request(tree, number)
            .with_context(|| format!("read pull request #{number} through the forge {forge}"))?;
        if view.head.branch != name || view.head.sha != pushed {
            return Ok(nothing(
                format!(
                    "pull request #{number} has the head {} at {}, not {name} at {}; nothing to read, the claim stays unknown",
                    view.head.branch,
                    short(&view.head.sha),
                    short(revision)
                ),
                view.head.branch == name,
            ));
        }
        let checks = self.forge.checks(tree, number).with_context(|| {
            format!("read the checks on pull request #{number} through the forge {forge}")
        })?;
        let result = match checks {
            Checks::Pass => Some("pass"),
            Checks::Fail => Some("fail"),
            Checks::Pending => None,
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("ci-result-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        let summary = match result {
            Some(result) => format!(
                "checks on pull request #{number} at {} through {forge}: {result}",
                short(revision)
            ),
            None => format!(
                "checks on pull request #{number} at {} through {forge}: pending; no record, the claim stays unknown",
                short(revision)
            ),
        };
        Ok((
            Observation {
                summary,
                records,
                artifact: None,
            },
            checks == Checks::Pending,
        ))
    }
}

/// `repository.merge`: merges the case's pull request through the forge, the one deterministic
/// step with a write effect, and writes one `merge_observation` record with the state the forge
/// reports afterwards: `merged` once it is, `closed` for a pull request closed unmerged. The
/// loop takes the step only when Canon admits the action, and Canon admits it only under a
/// recorded grant of `repository.merge` read from the case's `authority.yaml`: a connection to
/// the forge is never authority. The grant authorises the implementation revision it was given
/// at and no other, and the step checks that again itself: an open pull request at a revision
/// the case holds no grant for is not merged. Nothing is merged unless the forge reports the
/// pull request's head as the case's branch at the head last pushed, or at the revision, and
/// that head carries the revision: it is the revision, or the revision with only takt's riders
/// for the story above it ([`crate::case::approvals_only`]), the store's records of the grant,
/// the test result and the move to `implemented`, which so reach the base with the merge. Any
/// other head is not merged and not replaced. When the case's branch holds riders above the
/// pull request's head that it descends from and that carry the revision, the branch is pushed
/// first, named in `branch.yaml` before the push, so the head merged carries the records and a
/// push the forge took but reported failed is merged by the next run. A head carrying riders
/// is one CI was not read on: its checks are read before the merge, and pending or failing checks merge nothing and
/// write no record, the summary saying which; a later run merges it. Pending checks, a head
/// pushed that the pull request does not show yet, and a merge the forge refuses for a required
/// check that has not reported ([`crate::forge::refused_for_a_pending_check`]) merge nothing,
/// write no record and wait on the forge. The merge names
/// the head, so a head that moved meanwhile is refused by the forge rather than merged. The
/// record is about the revision the grant authorises; the head merged is retained beside it as
/// `artifacts/merged-head-<seq>.json`. A pull request already merged is recorded without a
/// second merge; one the forge still reports open after the merge writes no record, because a
/// claim that could not be observed stays unknown.
pub struct PrMerged {
    /// The forge the merge goes through and the state is read from.
    pub forge: Rc<dyn Forge>,
}

impl PrMerged {
    /// The action it performs.
    pub const ACTION: &str = "repository.merge";
    /// The evidence kind it writes.
    pub const KIND: &str = "merge_observation";

    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

impl std::fmt::Debug for PrMerged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrMerged")
            .field("forge", &self.forge.name())
            .finish()
    }
}

impl Validator for PrMerged {
    fn name(&self) -> &str {
        "takt.pr-merged"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    /// The case's `branch.yaml`, when takt's riders are pushed before the merge, and
    /// what the bot route keeps of the merge request it sent and the response it read.
    fn writes(&self) -> Vec<PathBuf> {
        let artifacts = Path::new("artifacts");
        vec![
            PathBuf::from(BRANCH_FILE),
            artifacts.join(BotRoute::MERGE_REQUEST),
            artifacts.join(BotRoute::MERGE_RESPONSE),
        ]
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        let dir = current(tree)?;
        let mut branch = Branch::read(&dir)?;
        // The case's branch, from the case id: not the `name` of a file a harness can write.
        let name = branch_of(&dir)?;
        let forge = self.forge.name();
        let nothing = |summary: String| Observation {
            summary,
            records: Vec::new(),
            artifact: None,
        };
        let Some(number) = branch.pull_request else {
            return Ok((
                nothing(
                    "no pull request is open for the case; nothing to merge, the claim stays unknown"
                        .to_owned(),
                ),
                false,
            ));
        };
        let pushed = match branch.pushed.clone() {
            Some(pushed) if carries(tree, &dir, revision, &pushed)? => pushed,
            pushed => {
                return Ok((
                    nothing(format!(
                        "the head of pull request #{number} is {}, not the implementation revision {}; nothing to merge, the claim stays unknown",
                        pushed.as_deref().map_or("not pushed", short),
                        short(revision)
                    )),
                    false,
                ));
            }
        };
        let mut view = self
            .forge
            .pull_request(tree, number)
            .with_context(|| format!("read pull request #{number} through the forge {forge}"))?;
        // The head last pushed, or the revision itself: `branch.yaml` names the head before it
        // is pushed, so a push the forge never took leaves the pull request at the revision,
        // and the push below is made again.
        if view.head.branch != name || (view.head.sha != pushed && view.head.sha != revision) {
            return Ok((
                nothing(format!(
                    "pull request #{number} has the head {} at {}, not {name} at {}; nothing to merge, the claim stays unknown",
                    view.head.branch,
                    short(&view.head.sha),
                    short(&pushed)
                )),
                false,
            ));
        }
        let (state, merged_now) = match view.state {
            PullRequestState::Open => {
                // The grant authorises the revision the case reached the gate at and no other:
                // checked here as well as in the evaluation that admitted the step.
                let authority = crate::case::Authority::read(&dir)?.unwrap_or_default();
                if !authority.grants_at(self.action(), revision) {
                    let granted = authority
                        .decision(self.action())
                        .filter(|decision| decision.decision == crate::case::Grant::Granted)
                        .map_or_else(
                            || "the case holds no grant of it".to_owned(),
                            |decision| {
                                format!(
                                    "the case's grant is for {}",
                                    decision.revision.as_deref().map_or("no revision", short)
                                )
                            },
                        );
                    return Ok((
                        nothing(format!(
                            "pull request #{number} at {} is open, and {} is not granted at that revision ({granted}); nothing merged, the claim stays unknown",
                            short(revision),
                            self.action()
                        )),
                        false,
                    ));
                }
                // takt's riders above the revision go with the pull request, so the store's
                // records reach the base with the merge: the head is brought up to the
                // branch's `HEAD` when that carries the revision and descends from the head.
                let local = crate::git::head(tree)?;
                if local != view.head.sha
                    && carries(tree, &dir, revision, &local)?
                    && crate::git::run(
                        tree,
                        &["merge-base", "--is-ancestor", &view.head.sha, &local],
                    )
                    .is_ok()
                {
                    // Named before the push: a push the forge took but reported failed leaves
                    // the head it took recorded, and the next run merges it.
                    branch.pushed = Some(local.clone());
                    branch.write(&dir)?;
                    self.forge
                        .push_revision(tree, &name, &local)
                        .with_context(|| format!("push {name} through the forge {forge}"))?;
                    view = self.forge.pull_request(tree, number).with_context(|| {
                        format!("read pull request #{number} through the forge {forge}")
                    })?;
                    if view.head.branch != name || view.head.sha != local {
                        // The case's branch at another commit: the forge has not moved the
                        // pull request to the push yet, and is waited on.
                        return Ok((
                            nothing(format!(
                                "pushed {name} at {} through {forge}, and pull request #{number} has the head {} at {}; nothing merged, the claim stays unknown",
                                short(&local),
                                view.head.branch,
                                short(&view.head.sha)
                            )),
                            view.head.branch == name,
                        ));
                    }
                }
                // CI was read on the revision; a head carrying takt's riders above it
                // is a new head, and a forge that requires checks merges it only once they
                // pass. Its checks are read first: pending or failing, nothing is merged and no
                // record written; pending checks are waited on, and a later run merges once
                // failing ones pass.
                if view.head.sha != revision {
                    let checks = self.forge.checks(tree, number).with_context(|| {
                        format!(
                            "read the checks on pull request #{number} through the forge {forge}"
                        )
                    })?;
                    if checks != Checks::Pass {
                        return Ok((
                            nothing(format!(
                                "checks on pull request #{number} at its head {}, takt's riders above {}, through {forge}: {checks}; nothing merged, the claim stays unknown",
                                short(&view.head.sha),
                                short(revision)
                            )),
                            checks == Checks::Pending,
                        ));
                    }
                }
                // A read of the checks can answer for another head than the one merged (a
                // forge whose checks lag the push): the forge then refuses the merge for its
                // required check, and that refusal is nothing merged, waited on like pending
                // checks. Every other failure of the merge stays an error.
                if let Err(error) =
                    self.forge
                        .merge(tree, &dir.join("artifacts"), number, &view.head.sha)
                {
                    if crate::forge::refused_for_a_pending_check(&error) {
                        return Ok((
                            nothing(format!(
                                "the forge {forge} refused the merge of pull request #{number} at its head {} for a required check that has not reported ({}); nothing merged, the claim stays unknown",
                                short(&view.head.sha),
                                format!("{error:#}").replace('\n', " ")
                            )),
                            true,
                        ));
                    }
                    return Err(error.context(format!(
                        "merge pull request #{number} through the forge {forge}"
                    )));
                }
                let after = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge} after the merge")
                })?;
                (after.state, true)
            }
            state => (state, false),
        };
        let head = view.head.sha.clone();
        let carried = above(revision, &head);
        let result = match state {
            PullRequestState::Merged => Some("merged"),
            PullRequestState::Closed => Some("closed"),
            PullRequestState::Open => None,
        };
        let summary = match (state, merged_now) {
            (PullRequestState::Merged, true) => format!(
                "merged pull request #{number} at {} through {forge}: merged{carried}",
                short(revision)
            ),
            (PullRequestState::Merged, false) => format!(
                "pull request #{number} at {} is already merged, read through {forge}: merged{carried}",
                short(revision)
            ),
            (PullRequestState::Closed, _) => format!(
                "pull request #{number} at {} is closed unmerged, read through {forge}: closed",
                short(revision)
            ),
            (PullRequestState::Open, _) => format!(
                "merged pull request #{number} at {} through {forge}, which still reports it open; no record, the claim stays unknown",
                short(revision)
            ),
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("merge-observation-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        // The record is about the revision the grant authorises; which head was merged, the
        // revision or the revision with takt's riders above it, is kept beside it.
        let artifact = if state == PullRequestState::Merged {
            let kept = serde_json::json!({
                "pull_request": number,
                "granted": revision,
                "merged_head": head,
            });
            Some(Artifact {
                name: format!("merged-head-{seq}.json"),
                bytes: serde_json::to_vec_pretty(&kept).context("serialize the merged head")?,
            })
        } else {
            None
        };
        Ok((
            Observation {
                summary,
                records,
                artifact,
            },
            false,
        ))
    }
}

/// A built-in validator: its type's name, the action it performs and the evidence kind it
/// writes, each read from the type's own `ACTION` and `KIND`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltIn {
    /// The type's name, as `impl Validator for` names it.
    pub validator: &'static str,
    /// The action it performs.
    pub action: &'static str,
    /// The evidence kind it writes.
    pub kind: &'static str,
}

/// takt's built-in validators, whether or not a run has them configured. An action one of them
/// performs is the run loop's to take, never the harness's, and a kind one of them writes is an
/// observation, never a choice the step choice records (`crate::run`): a protocol in which an
/// action no built-in validator performs may produce one with `effect: read` is refused, and so
/// is one where an outcome or an obligation holds on choice records alone.
pub const BUILT_IN: [BuiltIn; 6] = [
    BuiltIn {
        validator: "TestsRun",
        action: TestsRun::ACTION,
        kind: TestsRun::KIND,
    },
    BuiltIn {
        validator: "GateRun",
        action: GateRun::ACTION,
        kind: GateRun::KIND,
    },
    BuiltIn {
        validator: "PrOpen",
        action: PrOpen::ACTION,
        kind: PrOpen::KIND,
    },
    BuiltIn {
        validator: "CiRead",
        action: CiRead::ACTION,
        kind: CiRead::KIND,
    },
    BuiltIn {
        validator: "PrMerged",
        action: PrMerged::ACTION,
        kind: PrMerged::KIND,
    },
    BuiltIn {
        validator: "ChangeRead",
        action: ChangeRead::ACTION,
        kind: ChangeRead::KIND,
    },
];

/// Whether a built-in validator performs `action`.
#[must_use]
pub fn is_built_in_action(action: &str) -> bool {
    BUILT_IN.iter().any(|built_in| built_in.action == action)
}

/// Whether a built-in validator writes `kind`.
#[must_use]
pub fn is_built_in_kind(kind: &str) -> bool {
    BUILT_IN.iter().any(|built_in| built_in.kind == kind)
}

/// `change.read`: observes whether the implementation revision's tree differs from the case's
/// base outside `.engineering/` (the planning store and the cases, which takt's own take,
/// approval and records change), by content, not by its commits or their messages, and writes
/// one `change_record`: `ahead` when merging the revision into the tip, now, of the base branch
/// `branch.yaml` names (`git merge-tree`, nothing checked out) would change that tip's tree
/// outside `.engineering/`, `none` when it would not: the same content landed on the base by
/// another commit, with or without more beside it, is no change to publish. When the base
/// branch is not there, the revision's tree is compared with the commit pinned when the case
/// opened instead; a case with neither writes no record. It reads git only, with no code from
/// configuration: nothing is run from the tree and nothing leaves the machine.
#[derive(Debug, Clone, Default)]
pub struct ChangeRead;

impl ChangeRead {
    /// The action it performs.
    pub const ACTION: &str = "change.read";
    /// The evidence kind it writes.
    pub const KIND: &str = "change_record";
}

impl Validator for ChangeRead {
    fn name(&self) -> &str {
        "takt.change-read"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let dir = current(tree)?;
        let pinned = base_revision(tree, &dir)?;
        let tip = Branch::read(&dir)?.base.and_then(|name| {
            crate::git::run(
                tree,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &format!("refs/heads/{name}^{{commit}}"),
                ],
            )
            .ok()
            .filter(|tip| !tip.is_empty())
            .map(|tip| (name, tip))
        });
        let (differs, summary) = if let Some((name, tip)) = tip {
            let merged = crate::git::merged_tree(tree, &tip, revision)?;
            let differs = crate::git::trees_differ_outside(tree, &tip, &merged, ".engineering")?;
            (
                differs,
                format!(
                    "merging {} into {name} at {}, its tip now, {} its tree outside .engineering/",
                    short(revision),
                    short(&tip),
                    if differs {
                        "changes"
                    } else {
                        "leaves as it is"
                    }
                ),
            )
        } else if let Some((shown, base)) = pinned {
            let differs = crate::git::trees_differ_outside(tree, &base, revision, ".engineering")?;
            (
                differs,
                format!(
                    "the base branch is not there to merge into; the tree of {} {} the tree of {shown} outside .engineering/",
                    short(revision),
                    if differs { "differs from" } else { "is" }
                ),
            )
        } else {
            return Ok(Observation {
                summary: "the case records no base; nothing to compare, the claim stays unknown"
                    .to_owned(),
                records: Vec::new(),
                artifact: None,
            });
        };
        let result = if differs { "ahead" } else { "none" };
        Ok(Observation {
            summary: format!("{summary}: {result}"),
            records: vec![Record::new(
                format!("change-record-{seq}"),
                self.kind(),
                Some(result.to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed() -> Result<Observation> {
        Ok(Observation {
            summary: "observed".to_owned(),
            records: Vec::new(),
            artifact: None,
        })
    }

    /// An observation that returns at the deadline it was given, the run's own, records
    /// nothing, whatever it observed; one before it, or without one, is kept.
    #[test]
    fn an_observation_returning_at_the_deadline_is_not_recorded() {
        let now = std::time::Instant::now();
        let ended = ended_by(Some(now), "tests.run", observed());
        let error = format!(
            "{:#}",
            ended.expect_err("an observation at the deadline ends")
        );
        assert!(error.contains("run's end"), "{error}");
        let later = now + std::time::Duration::from_secs(60);
        assert!(ended_by(Some(later), "tests.run", observed()).is_ok());
        assert!(ended_by(None, "tests.run", observed()).is_ok());
    }
}
