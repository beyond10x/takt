//! An [`AgentExecutor`] of loom-sdk over a metaharness session: the session works in a scratch
//! copy of a workspace, never in the workspace itself, and the executor returns what it changed
//! there as a proposal ([`Proposal`]); it performs nothing. The embedder's edit effect applies
//! a proposal, or refuses it, under its own guard.
//!
//! The session is Claude Code through the metaharness library: `Kind::Claude`,
//! `HermeticMode::Strict`, Claude's native tools, the scratch copy as its working directory,
//! declared as the embedder's scratch copy (`with_cwd` and `with_scratch_cwd`). The child's
//! environment is metaharness's, constructed from an allowlist and never inherited, so no
//! variable of the embedder's reaches it but those metaharness names. Each tool call is decided
//! here (`DecisionMode::Ask`): the file tools ([`TOOLS`]) on paths inside the copy are allowed,
//! everything else is denied with the reason.
//!
//! This crate depends on no crate of the application that embeds it, so it can move into Loom
//! unchanged.

use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use loom_sdk::commission::model::json::Value as Json;
use loom_sdk::commission::model::responsibility::{
    Commission, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, Frontier,
    ProposedActionArguments, SuspensionReason, commission_state, frontier_state,
};
use loom_sdk::commission::ports::executor::AgentExecutor;
use metaharness::protocol::{
    Command, Decision, DecisionMode, Event, HermeticMode, HermeticRow, Kind, Severity, Verdict,
};
use metaharness::{ClaudeSeams, EventPoll, Input, Metaharness};

pub use metaharness;
pub use metaharness::ProcessRunner;
pub use metaharness::protocol::CredentialSource;

/// The native tools a session may use, on paths inside its copy: read, edit, write, list and
/// search the files, and nothing else.
pub const TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Glob", "Grep"];

/// One file a proposal changes: its path relative to the workspace, and its whole content after
/// the change, absent when the change removes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedFile {
    pub path: String,
    pub content: Option<String>,
}

/// What a session proposes: the one action it names, one line on what it did, and every file
/// of its copy it changed, added or removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub action: String,
    pub summary: String,
    pub files: Vec<ProposedFile>,
}

impl Proposal {
    /// The proposal as the executor outcome that carries it: `ProposedAction` with the action
    /// and, as its arguments, `{"summary": …, "files": [{"path": …, "content": …}]}`, a removed
    /// file's `content` null.
    #[must_use]
    pub fn to_outcome(&self) -> ExecutorOutcome {
        let files = self
            .files
            .iter()
            .map(|file| {
                Json::Object(vec![
                    ("path".to_owned(), Json::Text(file.path.clone())),
                    (
                        "content".to_owned(),
                        file.content.clone().map_or(Json::Null, Json::Text),
                    ),
                ])
            })
            .collect();
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: self.action.clone(),
            arguments: ProposedActionArguments(Json::Object(vec![
                ("summary".to_owned(), Json::Text(self.summary.clone())),
                ("files".to_owned(), Json::Array(files)),
            ])),
        })
    }

    /// The proposal an outcome carries: none for an outcome that proposes no action; an error
    /// naming what does not read for a `ProposedAction` whose arguments are not a proposal's.
    pub fn from_outcome(outcome: &ExecutorOutcome) -> Option<Result<Self, String>> {
        let ExecutorOutcome::ProposedAction(proposed) = outcome else {
            return None;
        };
        let arguments = &proposed.arguments.0;
        let read = || -> Result<Self, String> {
            let summary = match arguments.member("summary") {
                Some(Json::Text(text)) => text.clone(),
                None | Some(Json::Null) => String::new(),
                Some(other) => return Err(format!("summary is {}", other.describes())),
            };
            let Some(Json::Array(items)) = arguments.member("files") else {
                return Err("the proposal names no list of files".to_owned());
            };
            let mut files = Vec::new();
            for item in items {
                let Some(Json::Text(path)) = item.member("path") else {
                    return Err("a proposed file names no path".to_owned());
                };
                let content = match item.member("content") {
                    Some(Json::Text(text)) => Some(text.clone()),
                    None | Some(Json::Null) => None,
                    Some(other) => {
                        return Err(format!("the content of {path} is {}", other.describes()));
                    }
                };
                files.push(ProposedFile {
                    path: path.clone(),
                    content,
                });
            }
            Ok(Self {
                action: proposed.action.clone(),
                summary,
                files,
            })
        };
        Some(read())
    }
}

/// What the last session reported, each field absent when it did not state it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionReport {
    /// The harness program's version, as its opening record stated it.
    pub version: Option<String>,
    /// The models its terminal record named, joined with `+`, else the opening record's.
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub api_ms: Option<u64>,
    pub turns: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_creation: Option<u64>,
    /// The directory the session worked in, the scratch copy; removed when the call returns.
    pub cwd: Option<PathBuf>,
    /// Every event metaharness delivered, one JSON line each.
    pub transcript: Vec<u8>,
    /// Each gating row of metaharness's hermetic floor that was not `ok`, as
    /// `<row> <verdict>: <detail>`.
    pub floor: Vec<String>,
    /// Why the call proposed nothing, when it did not.
    pub error: Option<String>,
}

/// The executor: one call runs one session over a scratch copy of `workspace` holding the
/// files `files` names, with `brief` as its prompt, and proposes what it changed there.
pub struct SessionExecutor {
    /// The workspace the copy is made from; never written.
    pub workspace: PathBuf,
    /// The files copied, by their paths relative to the workspace.
    pub files: Vec<String>,
    /// The prompt the session is given, whole.
    pub brief: String,
    /// The model, when given.
    pub model: Option<String>,
    /// The most turns the session may take.
    pub max_turns: Option<u32>,
    /// Where the session's credential comes from: the operator's login by default.
    pub credentials: CredentialSource,
    /// The directory the scratch copy is made in, outside the workspace.
    pub scratch: PathBuf,
    /// How long the session may run; it is halted past it.
    pub timeout: Option<Duration>,
    /// The `claude` program the session runs, by path; none for the one metaharness finds on
    /// the `PATH` it gives its child.
    pub program: Option<PathBuf>,
    runner: RefCell<Option<Box<dyn ProcessRunner>>>,
    last: RefCell<Option<SessionReport>>,
}

impl std::fmt::Debug for SessionExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionExecutor")
            .field("workspace", &self.workspace)
            .field("files", &self.files.len())
            .field("model", &self.model)
            .field("scratch", &self.scratch)
            .finish_non_exhaustive()
    }
}

impl SessionExecutor {
    /// An executor over `workspace`'s `files` with `brief` as the prompt, its copy made under
    /// the system's temporary directory, on the operator's login.
    #[must_use]
    pub fn new(
        workspace: impl Into<PathBuf>,
        files: Vec<String>,
        brief: impl Into<String>,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            files,
            brief: brief.into(),
            model: None,
            max_turns: None,
            credentials: CredentialSource::OperatorLogin,
            scratch: std::env::temp_dir(),
            timeout: None,
            program: None,
            runner: RefCell::new(None),
            last: RefCell::new(None),
        }
    }

    /// The same executor, whose session is started by `runner` instead of the real `claude`
    /// (metaharness's `ScriptedRunner`, or one that wraps it).
    #[must_use]
    pub fn with_runner(self, runner: Box<dyn ProcessRunner>) -> Self {
        *self.runner.borrow_mut() = Some(runner);
        self
    }

    /// What the last call's session reported; none before the first call.
    #[must_use]
    pub fn last_report(&self) -> Option<SessionReport> {
        self.last.borrow().clone()
    }

    /// One session: the copy made, the session run in it to its end, the proposal read.
    fn session(&self, report: &mut SessionReport) -> Result<Proposal, String> {
        let copy = ScratchCopy::make(&self.scratch, &self.workspace, &self.files)?;
        report.cwd = Some(copy.dir.clone());
        let mut builder = Metaharness::new(Kind::Claude)
            .with_hermetic(HermeticMode::Strict)
            .with_decisions(DecisionMode::Ask)
            .with_credentials(self.credentials)
            .with_cwd(&copy.dir)
            .with_scratch_cwd()
            .with_prompt(self.brief.clone());
        if let Some(model) = &self.model {
            builder = builder.with_model(model.clone());
        }
        if let Some(turns) = self.max_turns {
            builder = builder.with_max_turns(turns);
        }
        let started = match (self.runner.borrow_mut().as_mut(), &self.program) {
            (Some(runner), _) => {
                builder.start_with(Input::FromSpec, runner.as_mut(), &mut ClaudeSeams)
            }
            (None, Some(program)) => builder.start_with(
                Input::FromSpec,
                &mut Named {
                    program: program.display().to_string(),
                    inner: metaharness::SpawnRunner::new(),
                },
                &mut ClaudeSeams,
            ),
            (None, None) => builder.start(Input::FromSpec),
        };
        let mut run =
            started.map_err(|refusal| format!("metaharness refused the session: {refusal}"))?;
        let deadline = self
            .timeout
            .and_then(|left| Instant::now().checked_add(left));
        let mut said = Vec::new();
        let mut halted = false;
        let mut ended_in_error = None;
        // Whether every plugin the opening record names ships inside the vendor program
        // (`<name>@builtin`): none from a directory, a marketplace or a managed setting.
        let mut builtin_only = false;
        loop {
            if !halted && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                halted = true;
                let _ = run.send(Command::Halt {
                    reason: "the step's time ran out".to_owned(),
                });
            }
            let line = match run.poll_event() {
                Ok(EventPoll::Event(line)) => line,
                Ok(EventPoll::Ended) => break,
                Ok(EventPoll::Idle | EventPoll::Progress | EventPoll::DecisionPending) => continue,
                Err(error) => return Err(format!("the session's stream: {error}")),
            };
            if let Ok(text) = serde_json::to_string(&*line) {
                report.transcript.extend_from_slice(text.as_bytes());
                report.transcript.push(b'\n');
            }
            match &line.event {
                Event::SessionStarted {
                    harness_version,
                    model,
                    plugins,
                    ..
                } => {
                    report.version.clone_from(harness_version);
                    report.model.clone_from(model);
                    builtin_only = plugins.as_ref().is_some_and(|plugins| {
                        plugins.iter().all(|plugin| {
                            plugin
                                .source
                                .as_deref()
                                .is_some_and(|source| source.ends_with("@builtin"))
                        })
                    });
                }
                Event::Text { text, .. } => said.push(text.clone()),
                Event::ToolRequested {
                    call_id,
                    name,
                    input,
                    decision_required: true,
                    ..
                } => {
                    let decision = match decide(name, input, &copy.dir) {
                        Ok(()) => Decision::Allow,
                        Err(reason) => Decision::Deny { reason },
                    };
                    let _ = run.send(Command::ToolDecide {
                        call_id: call_id.clone(),
                        decision,
                    });
                }
                Event::SessionEnded {
                    is_error,
                    subtype,
                    num_turns,
                    duration_ms,
                    duration_api_ms,
                    total_cost_usd,
                    usage,
                    model_usage,
                    ..
                } => {
                    if *is_error == Some(true) {
                        ended_in_error = Some(subtype.clone().unwrap_or_default());
                    }
                    report.turns = *num_turns;
                    report.duration_ms = *duration_ms;
                    report.api_ms = *duration_api_ms;
                    report.cost_usd = *total_cost_usd;
                    if let Some(usage) = usage {
                        report.input_tokens = usage.input_tokens;
                        report.output_tokens = usage.output_tokens;
                        report.cache_read = usage.cache_read_input_tokens;
                        report.cache_creation = usage.cache_creation_input_tokens;
                    }
                    if let Some(models) = model_usage
                        && !models.is_empty()
                    {
                        report.model = Some(models.keys().cloned().collect::<Vec<_>>().join("+"));
                    }
                }
                _ => {}
            }
        }
        // Strict's floor: every gating row that is not `ok` is reported; a `gap` holds the
        // proposal back, but two facts about the program installed and not about what the
        // session was given: the vendor's version against the adapter's pin (H9), and plugins
        // the session loaded that metaharness did not declare (H1a) when every one of them ships
        // inside the vendor program (`<name>@builtin`, which a version newer than the pin adds).
        let mut held = Vec::new();
        for row in run.hermetic_floor() {
            if row.row.severity() != Severity::Gating || row.verdict == Verdict::Ok {
                continue;
            }
            let said = format!("{} {:?}: {}", row.row.id(), row.verdict, row.detail);
            let installed =
                row.row == HermeticRow::H9 || (row.row == HermeticRow::H1a && builtin_only);
            if row.verdict == Verdict::Gap && !installed {
                held.push(said.clone());
            }
            report.floor.push(said);
        }
        // What the program printed, as metaharness kept it: the transcript, in place of the
        // events, and the reply, the result record's `result`, where there is one.
        let raw = run
            .transcript()
            .path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .filter(|raw| !raw.is_empty());
        let mut replied = None;
        if let Some(raw) = raw {
            replied = result_of(&String::from_utf8_lossy(&raw));
            report.transcript = raw;
        }
        drop(run);
        if halted {
            return Err("the session was halted: the step's time ran out".to_owned());
        }
        if let Some(subtype) = ended_in_error {
            return Err(format!("the session ended in an error ({subtype})"));
        }
        if !held.is_empty() {
            return Err(format!(
                "the session's hermetic floor has a gating gap: {}",
                held.join("; ")
            ));
        }
        let (action, summary) = reply(&replied.unwrap_or_else(|| said.join("\n")))?;
        let files = copy.changes(&self.workspace, &self.files)?;
        Ok(Proposal {
            action,
            summary,
            files,
        })
    }
}

impl AgentExecutor for SessionExecutor {
    /// One session over the frontier the brief describes; the proposal as `ProposedAction`, or
    /// `Suspended` with `ExternalAvailability` naming why there is none.
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut report = SessionReport::default();
        let outcome = match self.session(&mut report) {
            Ok(proposal) => proposal.to_outcome(),
            Err(why) => {
                report.error = Some(why.clone());
                ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                    reason: SuspensionReason::ExternalAvailability(Json::Text(why)),
                })
            }
        };
        *self.last.borrow_mut() = Some(report);
        outcome
    }
}

/// metaharness's own spawn of the vendor program, the program named by path (the one the
/// embedder resolved, never another found first on the `PATH` metaharness gives its child),
/// offered the file tools [`TOOLS`] and no other.
struct Named {
    program: String,
    inner: metaharness::SpawnRunner,
}

impl ProcessRunner for Named {
    fn start(
        &mut self,
        plan: &metaharness::LaunchPlanView,
    ) -> std::io::Result<Box<dyn metaharness::HarnessProcess>> {
        // In a session of its own (`setsid`, which execs the program in place, waiting when
        // it had to fork), so whatever the program leaves running is in that session and an
        // embedder that sweeps what left its own session finds it; the program as is where
        // there is no `setsid`.
        let setsid = ["/usr/bin/setsid", "/bin/setsid"]
            .into_iter()
            .find(|path| Path::new(path).is_file());
        let mut args = Vec::with_capacity(plan.args.len() + 4);
        let program = match setsid {
            Some(setsid) => {
                args.push("--wait".to_owned());
                args.push(self.program.clone());
                setsid
            }
            None => self.program.as_str(),
        };
        args.extend(plan.args.iter().cloned());
        // Offered only the file tools, so no other tool costs its schema in every call: each
        // call of one is still decided at the seam ([`decide`]).
        args.extend(["--tools".to_owned(), TOOLS.join(",")]);
        let named = metaharness::LaunchPlanView {
            program,
            args: &args,
            ..*plan
        };
        self.inner.start(&named)
    }
}

/// The reply in what the program printed: the `result` of its last line that reads as a JSON
/// object whose `type` is `result`, else of its last line that is not blank, a reply printed
/// as one document; none when that names no string `result`.
fn result_of(printed: &str) -> Option<String> {
    let lines: Vec<&str> = printed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let docs = |line: &&str| serde_json::from_str::<serde_json::Value>(line).ok();
    lines
        .iter()
        .rev()
        .filter_map(docs)
        .find(|doc| doc["type"] == "result")
        .or_else(|| lines.last().and_then(docs))
        .and_then(|doc| doc["result"].as_str().map(str::to_owned))
}

/// Whether a tool call is allowed: one of [`TOOLS`], every path it names inside `copy`; else
/// the reason the model is told.
fn decide(name: &str, input: &serde_json::Value, copy: &Path) -> Result<(), String> {
    if !TOOLS.contains(&name) {
        return Err(format!(
            "{name} is not offered here: use only the file tools {} on the files of this directory",
            TOOLS.join(", ")
        ));
    }
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(path) = input.get(key).and_then(serde_json::Value::as_str)
            && !inside(copy, path)
        {
            return Err(format!(
                "{path} is outside this directory; work only on its files"
            ));
        }
    }
    if let Some(pattern) = input.get("pattern").and_then(serde_json::Value::as_str)
        && name == "Glob"
        && (pattern.starts_with('/') || pattern.starts_with('~'))
        && !inside(copy, pattern)
    {
        return Err(format!(
            "{pattern} is outside this directory; work only on its files"
        ));
    }
    Ok(())
}

/// Whether `path`, relative to `copy` or absolute, names a place inside it, read lexically:
/// a `..` that leaves it, a `~`, or an absolute path elsewhere does not.
fn inside(copy: &Path, path: &str) -> bool {
    if path.starts_with('~') {
        return false;
    }
    let path = Path::new(path);
    let rest = if path.is_absolute() {
        match path.strip_prefix(copy) {
            Ok(rest) => rest,
            Err(_) => return false,
        }
    } else {
        path
    };
    let mut depth = 0usize;
    for part in rest.components() {
        match part {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// The action and summary of a reply: its last line that opens a JSON object naming `action`.
fn reply(text: &str) -> Result<(String, String), String> {
    for line in text.lines().rev() {
        let line = line.trim();
        if line.starts_with('{') && line.contains("\"action\"") {
            let doc: serde_json::Value = serde_json::from_str(line)
                .map_err(|error| format!("the session's JSON line does not read: {error}"))?;
            let action = doc["action"]
                .as_str()
                .ok_or("the session's JSON line names no action")?
                .to_owned();
            let summary = doc["summary"].as_str().unwrap_or("").to_owned();
            return Ok((action, summary));
        }
    }
    Err("the session ended without the JSON line naming its action".to_owned())
}

/// Whether `path` is one plain path relative to a root: no root, no `.` or `..` part, no part
/// named `.git`, not empty.
#[must_use]
pub fn plain_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(name) if name != ".git"))
}

/// The scratch copy a session works in: `<scratch>/session-<pid>-<n>/copy`, removed when
/// dropped.
struct ScratchCopy {
    root: PathBuf,
    dir: PathBuf,
}

impl ScratchCopy {
    /// Makes the copy of `files` of `workspace` under `scratch`: each regular file with its
    /// bytes and its executable bit; a link, a directory or a path that is not plain is left out.
    fn make(scratch: &Path, workspace: &Path, files: &[String]) -> Result<Self, String> {
        use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let root = scratch.join(format!("session-{}-{stamp}-{n}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|error| format!("make the scratch copy {}: {error}", root.display()))?;
        let mut copy = Self {
            dir: root.join("copy"),
            root,
        };
        std::fs::create_dir(&copy.dir)
            .map_err(|error| format!("make {}: {error}", copy.dir.display()))?;
        // Resolved, as metaharness resolves the working directory it is given, so a path the
        // session names under it reads as inside it.
        copy.dir = std::fs::canonicalize(&copy.dir)
            .map_err(|error| format!("resolve {}: {error}", copy.dir.display()))?;
        for file in files.iter().filter(|file| plain_relative(file)) {
            let from = workspace.join(file);
            let Ok(meta) = std::fs::symlink_metadata(&from) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let to = copy.dir.join(file);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("make {}: {error}", parent.display()))?;
            }
            std::fs::copy(&from, &to).map_err(|error| {
                format!("copy {} into the scratch copy: {error}", from.display())
            })?;
            let mode = if meta.permissions().mode() & 0o111 != 0 {
                0o755
            } else {
                0o644
            };
            std::fs::set_permissions(&to, std::fs::Permissions::from_mode(mode))
                .map_err(|error| format!("set the mode of {}: {error}", to.display()))?;
        }
        Ok(copy)
    }

    /// Every file of the copy that differs from `workspace`: a file `files` named whose bytes
    /// differ or that is gone, and any file the session added. Content that is not UTF-8, or a
    /// link the session left, is an error: a proposal carries text files only.
    fn changes(&self, workspace: &Path, files: &[String]) -> Result<Vec<ProposedFile>, String> {
        let mut found = Vec::new();
        walk(&self.dir, &self.dir, &mut found)?;
        let listed: std::collections::BTreeSet<&str> = files
            .iter()
            .filter(|file| plain_relative(file))
            .map(String::as_str)
            .collect();
        let mut changes = Vec::new();
        for path in &found {
            let now = std::fs::read(self.dir.join(path))
                .map_err(|error| format!("read {path} in the scratch copy: {error}"))?;
            let was = if listed.contains(path.as_str()) {
                std::fs::read(workspace.join(path)).ok()
            } else {
                None
            };
            if was.as_deref() == Some(now.as_slice()) {
                continue;
            }
            let content = String::from_utf8(now).map_err(|_| {
                format!("{path} is not UTF-8 text; a proposal carries text files only")
            })?;
            changes.push(ProposedFile {
                path: path.clone(),
                content: Some(content),
            });
        }
        let present: std::collections::BTreeSet<&str> = found.iter().map(String::as_str).collect();
        for path in &listed {
            let copied =
                std::fs::symlink_metadata(workspace.join(path)).is_ok_and(|meta| meta.is_file());
            if copied && !present.contains(path) {
                changes.push(ProposedFile {
                    path: (*path).to_owned(),
                    content: None,
                });
            }
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
    }
}

impl Drop for ScratchCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Every regular file under `dir`, by its path relative to `root`; a link is an error.
fn walk(root: &Path, dir: &Path, into: &mut Vec<String>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("read {}: {error}", dir.display()))?
            .path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("{} is outside the copy", path.display()))?
            .to_string_lossy()
            .into_owned();
        if meta.file_type().is_symlink() {
            return Err(format!(
                "the session left a link at {relative}; a proposal carries files only"
            ));
        }
        if meta.is_dir() {
            walk(root, &path, into)?;
        } else if meta.is_file() {
            into.push(relative);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_inside_the_copy_only_lexically_under_it() {
        let copy = Path::new("/s/copy");
        for path in ["src/lib.rs", "./a", "a/../b", "/s/copy/x", "/s/copy"] {
            assert!(inside(copy, path), "{path}");
        }
        for path in [
            "../x",
            "/etc/passwd",
            "a/../../b",
            "~/.ssh/id",
            "/s/copy/../x",
        ] {
            assert!(!inside(copy, path), "{path}");
        }
    }

    #[test]
    fn a_proposal_reads_back_from_its_outcome() {
        let proposal = Proposal {
            action: "implementation.edit".to_owned(),
            summary: "fixed add".to_owned(),
            files: vec![
                ProposedFile {
                    path: "src/lib.rs".to_owned(),
                    content: Some("x\n".to_owned()),
                },
                ProposedFile {
                    path: "old.rs".to_owned(),
                    content: None,
                },
            ],
        };
        let back = Proposal::from_outcome(&proposal.to_outcome());
        assert_eq!(back, Some(Ok(proposal)));
    }

    #[test]
    fn only_file_tools_inside_the_copy_are_allowed() {
        let copy = Path::new("/s/copy");
        let input = serde_json::json!({"file_path": "/s/copy/a.rs"});
        assert!(decide("Edit", &input, copy).is_ok());
        assert!(decide("Bash", &serde_json::json!({"command": "ls"}), copy).is_err());
        let outside = serde_json::json!({"file_path": "/home/u/.bashrc"});
        assert!(decide("Write", &outside, copy).is_err());
    }
}
