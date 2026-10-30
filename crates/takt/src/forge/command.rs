//! The forge a run file configures by exact commands, `forge: command`: each action of the
//! [`Forge`] seam is the argument list the file's `forge_commands` section names, run without a
//! shell through the forge [`Runner`], so a title holding `;` or `$(...)` is one argument. The
//! push stays takt's own push over HTTPS to the project the `origin` remote names: this
//! forge replaces the host's API calls and nothing before them. The merge target's tip is read
//! through git, not through an entry ([`Forge::target_tip`]).
//!
//! | entry | runs | prints |
//! |---|---|---|
//! | `open` | to open the request: `{branch}`, `{base}`, `{title}`, `{body_file}` | its URL or number, read by `id_pattern` |
//! | `read` | to read it: `{number}` | JSON: `state`, `head`, `branch`, `base` |
//! | `checks` | to read the checks on its head: `{number}`, `{head}` | JSON: each check's `name` and `state` |
//! | `merge` | to merge it: `{number}`, `{head}` | nothing read |
//!
//! Exit 0 is ok; an exit in `refused_exit` is [`ForgeRefused`], any other exit [`ForgeFailed`];
//! what a command cannot be read from is [`ForgeOutput`]. Standard error of every command is
//! kept in a file under the state directory, never in the case.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use regex::Regex;

use crate::scan::Scan;

use super::{
    Checks, Exit, Failed, Forge, Head, IdentityScan, MERGE_REQUEST, MERGE_RESPONSE, Outgoing,
    PUSH_ENV, PullRequest, PullRequestState, PullRequestView, RESPONSE, Runner, fresh,
    head_is_a_commit, is_object_id, may_publish, push_over_https, write_request,
};

/// An action of the forge and the entry of `forge_commands` that performs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// `open`.
    Open,
    /// `read`.
    Read,
    /// `checks`.
    Checks,
    /// `merge`.
    Merge,
}

impl Entry {
    /// The entry's key in the run file.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Read => "read",
            Self::Checks => "checks",
            Self::Merge => "merge",
        }
    }

    /// The placeholders the entry's argument list may hold.
    #[must_use]
    pub fn placeholders(self) -> &'static [&'static str] {
        match self {
            Self::Open => &["branch", "base", "title", "body_file"],
            Self::Read => &["number"],
            Self::Checks | Self::Merge => &["number", "head"],
        }
    }
}

/// The names of the placeholder-shaped words of `element`: lowercase letters and `_` between
/// braces. A brace that holds anything else is text.
fn named_in(element: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = element;
    while let Some(at) = rest.find('{') {
        let after = &rest[at + 1..];
        if let Some(end) = after.find('}')
            && end > 0
            && after[..end]
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        {
            names.push(&after[..end]);
        }
        rest = after;
    }
    names
}

/// The `forge_commands` section of a run file: one argument list per action, never a shell line.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeCommands {
    /// Opens the request; prints its URL or number.
    pub open: Option<Vec<String>>,
    /// Reads the request; prints JSON with `state`, `head`, `branch` and `base`.
    pub read: Option<Vec<String>>,
    /// Reads the checks on the request's head; prints JSON.
    pub checks: Option<Vec<String>>,
    /// Merges the request at the head it is given.
    pub merge: Option<Vec<String>>,
    /// A regular expression whose first group is the request's number in what `open` printed.
    pub id_pattern: Option<String>,
    /// The exit codes that are a refusal and not a failure.
    pub refused_exit: Option<Vec<i32>>,
}

/// A command that exited in `refused_exit`: the forge refused the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeRefused {
    /// The entry that ran: `open`, `read`, `checks` or `merge`.
    pub command: String,
    /// Its exit code.
    pub exit: i32,
    /// The file its standard error was kept in.
    pub stderr: PathBuf,
}

impl fmt::Display for ForgeRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the forge command `{}` was refused with exit {}; its standard error is in {}",
            self.command,
            self.exit,
            self.stderr.display()
        )
    }
}

impl std::error::Error for ForgeRefused {}

/// A command that exited otherwise than 0 or a refusal, or ended on a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeFailed {
    /// The entry that ran.
    pub command: String,
    /// Its exit code; `None` when a signal ended it.
    pub exit: Option<i32>,
    /// The file its standard error was kept in.
    pub stderr: PathBuf,
}

impl fmt::Display for ForgeFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the forge command `{}` failed, {}; its standard error is in {}",
            self.command,
            self.exit
                .map_or("it ended on a signal".to_owned(), |code| format!(
                    "exit {code}"
                )),
            self.stderr.display()
        )
    }
}

impl std::error::Error for ForgeFailed {}

/// Output of a command that cannot be read: the entry and the field that is missing or wrong
/// (`id_pattern`, `number`, `output`, `state`, `head`, `branch`, `base` or `name`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeOutput {
    /// The entry that ran.
    pub command: String,
    /// The field.
    pub field: String,
}

impl fmt::Display for ForgeOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the output of the forge command `{}` cannot be read: no usable `{}`",
            self.command, self.field
        )
    }
}

impl std::error::Error for ForgeOutput {}

/// An action the protocol needs whose entry the run file lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeEntryMissing {
    /// The entry.
    pub entry: String,
    /// The protocol's action that needs it.
    pub action: String,
}

impl fmt::Display for ForgeEntryMissing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the run file's `forge_commands` has no `{}` entry, which the protocol's action {} needs; nothing was opened",
            self.entry, self.action
        )
    }
}

impl std::error::Error for ForgeEntryMissing {}

/// The placeholders substituted once inside the argument that holds them; a value is never read
/// for another placeholder, and a brace that names none stays as written.
fn substitute(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let named = values.iter().find(|(name, _)| {
            after
                .strip_prefix('{')
                .and_then(|inner| inner.strip_prefix(name))
                .is_some_and(|inner| inner.starts_with('}'))
        });
        if let Some((name, value)) = named {
            out.push_str(value);
            rest = &after[name.len() + 2..];
        } else {
            out.push('{');
            rest = &after[1..];
        }
    }
    out.push_str(rest);
    out
}

impl ForgeCommands {
    /// The entries a protocol's `actions` need, each with the action that needs it: `pr.open`
    /// needs `open` and `read`, `ci.read` needs `read` and `checks`, `repository.merge` needs
    /// `read` and `merge`.
    #[must_use]
    pub fn needs(actions: &[String]) -> Vec<(Entry, String)> {
        let mut needs: Vec<(Entry, String)> = Vec::new();
        for (action, entries) in [
            ("pr.open", [Entry::Open, Entry::Read]),
            ("ci.read", [Entry::Read, Entry::Checks]),
            ("repository.merge", [Entry::Read, Entry::Merge]),
        ] {
            if actions.iter().any(|declared| declared == action) {
                for entry in entries {
                    if !needs.iter().any(|(held, _)| *held == entry) {
                        needs.push((entry, action.to_owned()));
                    }
                }
            }
        }
        needs
    }

    /// The argument list of `entry`, when the file names one.
    #[must_use]
    pub fn argv(&self, entry: Entry) -> Option<&[String]> {
        match entry {
            Entry::Open => &self.open,
            Entry::Read => &self.read,
            Entry::Checks => &self.checks,
            Entry::Merge => &self.merge,
        }
        .as_deref()
    }

    /// Refuses the first entry the protocol's `actions` need that the file lacks.
    pub fn require(&self, actions: &[String]) -> Result<(), ForgeEntryMissing> {
        for (entry, action) in Self::needs(actions) {
            if self.argv(entry).is_none() {
                return Err(ForgeEntryMissing {
                    entry: entry.name().to_owned(),
                    action,
                });
            }
        }
        Ok(())
    }

    /// Checks the section's shape: each entry a non-empty argument list naming a program that is
    /// no placeholder, holding no placeholder the entry does not take (a word of lowercase
    /// letters and `_` in braces), `id_pattern` a regular expression with a group, `refused_exit` codes above 0 and below 256.
    /// The refusal names the key and never repeats a value.
    pub fn check(&self) -> Result<()> {
        for entry in [Entry::Open, Entry::Read, Entry::Checks, Entry::Merge] {
            if let Some(argv) = self.argv(entry)
                && argv.first().is_none_or(|program| program.trim().is_empty())
            {
                bail!(
                    "`forge_commands.{}` is an argument list whose first element is the program, and it names none",
                    entry.name()
                );
            }
            for element in self.argv(entry).unwrap_or_default() {
                if let Some(unknown) = named_in(element)
                    .into_iter()
                    .find(|name| !entry.placeholders().contains(name))
                {
                    bail!(
                        "`forge_commands.{}` holds the placeholder {{{unknown}}}, which that entry does not take: it takes {}",
                        entry.name(),
                        entry
                            .placeholders()
                            .iter()
                            .map(|name| format!("{{{name}}}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            if let Some(program) = self.argv(entry).and_then(<[String]>::first)
                && !named_in(program).is_empty()
            {
                bail!(
                    "`forge_commands.{}` names its program with a placeholder: the first element is a program of the file's own, never a substituted value",
                    entry.name()
                );
            }
        }
        self.pattern()?;
        if let Some(codes) = &self.refused_exit
            && codes.iter().any(|code| !(1..=255).contains(code))
        {
            bail!("`forge_commands.refused_exit` holds exit codes from 1 to 255; 0 is success");
        }
        Ok(())
    }

    /// `id_pattern`, compiled.
    fn pattern(&self) -> Result<Option<Regex>> {
        let Some(pattern) = &self.id_pattern else {
            return Ok(None);
        };
        let regex = Regex::new(pattern)
            .map_err(|_| anyhow!("`forge_commands.id_pattern` is not a regular expression"))?;
        if regex.captures_len() < 2 {
            bail!("`forge_commands.id_pattern` has no group to take the number from");
        }
        Ok(Some(regex))
    }
}

/// The adapter over the commands of `forge_commands`. The push and the scan adapter are the
/// run's, to the project `origin` names.
pub struct CommandForge {
    commands: ForgeCommands,
    id_pattern: Option<Regex>,
    state: PathBuf,
    origin: Option<(String, String)>,
    /// The run's scan adapter.
    pub scan: Scan,
    runner: Box<dyn Runner>,
}

impl fmt::Debug for CommandForge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandForge")
            .field("commands", &self.commands)
            .field("state", &self.state)
            .field("origin", &self.origin)
            .field("scan", &self.scan)
            .finish_non_exhaustive()
    }
}

impl CommandForge {
    /// The file under the case's artifacts directory that holds the body `{body_file}` names: the
    /// case's own, so no other run's `open` can change what this one publishes.
    pub const BODY: &str = "open.body";

    /// Over `commands`, refused when the section is misshapen or lacks an entry the protocol's
    /// `actions` need, naming it; `state` is the directory each command's
    /// standard error is kept in; the push goes to the project `tree`'s `origin`
    /// names, none when it names none (the push is then refused). Every line runs through
    /// `runner`.
    pub fn for_tree(
        tree: &Path,
        commands: ForgeCommands,
        actions: &[String],
        state: impl Into<PathBuf>,
        scan: Scan,
        runner: Box<dyn Runner>,
    ) -> Result<Self> {
        commands.check()?;
        commands.require(actions)?;
        let id_pattern = commands.pattern()?;
        let origin = crate::git::remote_url(tree, "origin")
            .ok()
            .and_then(|url| super::gitlab::project_of(&url));
        Ok(Self {
            commands,
            id_pattern,
            state: state.into(),
            origin,
            scan,
            runner,
        })
    }

    /// The project the push goes to.
    fn project(&self) -> Result<&(String, String)> {
        self.origin.as_ref().ok_or_else(|| {
            anyhow!(
                "the forge command pushes to the project the origin remote names, and it names none; nothing was pushed"
            )
        })
    }

    /// The push URL, `https://<host>/<project>.git`.
    fn push_url(&self) -> Result<String> {
        let (host, project) = self.project()?;
        Ok(format!("https://{host}/{project}.git"))
    }

    /// The line that pushes the commit `revision` as `branch`.
    fn push_argv(&self, branch: &str, revision: &str) -> Result<Vec<String>> {
        Ok([
            "git".to_owned(),
            "push".to_owned(),
            self.push_url()?,
            format!("{revision}:refs/heads/{branch}"),
            "--no-follow-tags".to_owned(),
            "--recurse-submodules=no".to_owned(),
        ]
        .to_vec())
    }

    /// Runs `entry` with `values` substituted, in `tree`: the argument list of the file, each
    /// element substituted on its own and never split. Standard error goes to the state
    /// directory.
    fn run(&self, entry: Entry, tree: &Path, values: &[(&str, &str)]) -> Result<Exit> {
        let template = self.commands.argv(entry).ok_or_else(|| ForgeEntryMissing {
            entry: entry.name().to_owned(),
            action: "the forge's".to_owned(),
        })?;
        let argv: Vec<String> = template
            .iter()
            .map(|element| substitute(element, values))
            .collect();
        std::fs::create_dir_all(&self.state)
            .with_context(|| format!("create {}", self.state.display()))?;
        let exit = self
            .runner
            .run(tree, &argv)
            .with_context(|| format!("the forge command `{}` could not be run", entry.name()))?;
        let stderr = self.state.join(format!("{}.stderr", entry.name()));
        std::fs::write(&stderr, &exit.stderr)
            .with_context(|| format!("write {}", stderr.display()))?;
        match exit.code {
            Some(0) => Ok(exit),
            Some(code)
                if self
                    .commands
                    .refused_exit
                    .as_deref()
                    .is_some_and(|codes| codes.contains(&code)) =>
            {
                Err(ForgeRefused {
                    command: entry.name().to_owned(),
                    exit: code,
                    stderr,
                }
                .into())
            }
            code => Err(ForgeFailed {
                command: entry.name().to_owned(),
                exit: code,
                stderr,
            }
            .into()),
        }
    }

    /// The request's number in what `open` printed: `id_pattern`'s first group, last match; else
    /// the number of the request path segment of the last URL that has one
    /// ([`request_number_of`]), else a lone number on the last line.
    fn number_of(&self, stdout: &str) -> Result<u64> {
        let unreadable = || ForgeOutput {
            command: Entry::Open.name().to_owned(),
            field: "id_pattern".to_owned(),
        };
        let digits = if let Some(pattern) = &self.id_pattern {
            pattern
                .captures_iter(stdout)
                .last()
                .and_then(|found| found.get(1))
                .map(|group| group.as_str().to_owned())
        } else {
            stdout
                .split_whitespace()
                .rev()
                .filter(|word| word.contains("://"))
                .find_map(request_number_of)
                .map(str::to_owned)
                .or_else(|| {
                    let line = stdout.lines().rfind(|line| !line.trim().is_empty())?.trim();
                    let line = line.trim_start_matches(['#', '!']);
                    (!line.is_empty() && line.bytes().all(|b| b.is_ascii_digit()))
                        .then(|| line.to_owned())
                })
        };
        digits
            .and_then(|digits| digits.parse().ok())
            .ok_or_else(|| unreadable().into())
    }

    /// `stdout` of `entry` as a JSON document.
    fn json(entry: Entry, exit: &Exit) -> Result<serde_json::Value> {
        serde_json::from_str(&exit.stdout).map_err(|_| {
            ForgeOutput {
                command: entry.name().to_owned(),
                field: "output".to_owned(),
            }
            .into()
        })
    }
}

/// The path segments after which a request's number stands: GitLab's `merge_requests` and
/// GitHub's `pull` and `pulls`.
const REQUEST_SEGMENTS: [&str; 3] = ["merge_requests", "pull", "pulls"];

/// The request number in the URL `word`: the digits that make up the whole path segment after a
/// segment of [`REQUEST_SEGMENTS`], the last such pair of the path. Nothing else of a URL is a
/// number: not a digit of the host, its port, the project's name or a segment that is no number
/// (`.../merge_requests/new`), and not the query or the fragment.
fn request_number_of(word: &str) -> Option<&str> {
    let word = word.trim_end_matches(['.', ',', ';', ')', ']', '>', '"', '\'']);
    let rest = word.split_once("://")?.1;
    let rest = rest.split(['?', '#']).next()?;
    let path = &rest[rest.find('/')?..];
    let segments: Vec<&str> = path.split('/').collect();
    segments
        .windows(2)
        .rev()
        .find(|pair| {
            REQUEST_SEGMENTS.contains(&pair[0])
                && !pair[1].is_empty()
                && pair[1].bytes().all(|byte| byte.is_ascii_digit())
        })
        .map(|pair| pair[1])
}

/// The commit id in the output of `git ls-remote` for the reference `reference`: the one line
/// that names exactly that reference (a bare id is the line of a pattern that matched one),
/// never another reference the pattern also matches. `None` when no line names it.
fn tip_of<'a>(stdout: &'a str, reference: &str) -> Option<&'a str> {
    let mut named = stdout.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let id = fields.next()?;
        match fields.next() {
            None => Some(id),
            Some(name) if name == reference && fields.next().is_none() => Some(id),
            Some(_) => None,
        }
    });
    let tip = named.next()?;
    named.next().is_none().then_some(tip)
}

/// A field of `command`'s output that cannot be read.
fn unreadable(entry: Entry, field: &str) -> anyhow::Error {
    ForgeOutput {
        command: entry.name().to_owned(),
        field: field.to_owned(),
    }
    .into()
}

/// A request's view as `read` printed it.
fn view_of(document: &serde_json::Value) -> Result<PullRequestView> {
    let text = |value: &serde_json::Value| {
        value
            .as_str()
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let state = match document["state"].as_str() {
        Some("open" | "opened" | "locked") => PullRequestState::Open,
        Some("merged") => PullRequestState::Merged,
        Some("closed") => PullRequestState::Closed,
        _ => return Err(unreadable(Entry::Read, "state")),
    };
    let (sha, in_head) = match &document["head"] {
        serde_json::Value::String(sha) if !sha.is_empty() => (sha.clone(), None),
        serde_json::Value::Object(head) => (
            head.get("sha")
                .and_then(text)
                .ok_or_else(|| unreadable(Entry::Read, "head"))?,
            head.get("branch").and_then(text),
        ),
        _ => return Err(unreadable(Entry::Read, "head")),
    };
    let branch = in_head
        .or_else(|| text(&document["branch"]))
        .ok_or_else(|| unreadable(Entry::Read, "branch"))?;
    let base = text(&document["base"]).ok_or_else(|| unreadable(Entry::Read, "base"))?;
    Ok(PullRequestView {
        state,
        head: Head { branch, sha },
        base,
    })
}

/// The checks `checks` printed, as one: a failed check fails, else a pending one, or none yet,
/// leaves it pending, else it passes.
fn checks_of(document: &serde_json::Value) -> Result<Checks> {
    let list = document
        .as_array()
        .or_else(|| document["checks"].as_array())
        .ok_or_else(|| unreadable(Entry::Checks, "output"))?;
    let mut result = if list.is_empty() {
        Checks::Pending
    } else {
        Checks::Pass
    };
    for check in list {
        if check["name"].as_str().is_none_or(str::is_empty) {
            return Err(unreadable(Entry::Checks, "name"));
        }
        let one = match check["state"]
            .as_str()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("success" | "passed" | "pass" | "succeeded" | "skipped" | "neutral") => {
                Checks::Pass
            }
            Some(
                "failure" | "failed" | "fail" | "error" | "errored" | "cancelled" | "canceled"
                | "timed_out" | "action_required",
            ) => Checks::Fail,
            Some(
                "pending"
                | "queued"
                | "in_progress"
                | "running"
                | "created"
                | "waiting"
                | "expected"
                | "scheduled"
                | "manual"
                | "preparing"
                | "waiting_for_resource",
            ) => Checks::Pending,
            _ => return Err(unreadable(Entry::Checks, "state")),
        };
        result = match (result, one) {
            (Checks::Fail, _) | (_, Checks::Fail) => Checks::Fail,
            (Checks::Pending, _) | (_, Checks::Pending) => Checks::Pending,
            _ => Checks::Pass,
        };
    }
    Ok(result)
}

impl Forge for CommandForge {
    fn name(&self) -> &str {
        "command"
    }

    /// A head that is not a commit is refused under any adapter; then what pushing would publish
    /// leaves the machine only under a declared `none` ([`may_publish`]).
    fn scan_outgoing(&self, tree: &Path, _dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        head_is_a_commit(tree, outgoing.branch, outgoing.head)?;
        may_publish(&self.scan, outgoing.branch)
    }

    /// No scan reads the commit identity: [`IdentityScan::NotScanned`] naming why
    /// ([`Scan::not_scanned`]), no line run.
    fn scan_identity(
        &self,
        _tree: &Path,
        _dir: &Path,
        _identity: &crate::git::CommitIdentity,
    ) -> Result<IdentityScan> {
        Ok(IdentityScan::NotScanned(self.scan.not_scanned()))
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let at = super::exact_ref(tree, &format!("refs/heads/{branch}"))
            .ok_or_else(|| anyhow!("{branch} names no commit; nothing was pushed"))?;
        self.push_revision(tree, branch, &at)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        push_over_https(
            &*self.runner,
            tree,
            &self.push_argv(branch, revision)?,
            &self.push_url()?,
            "the forge command",
        )
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        may_publish(&self.scan, &request.branch)?;
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(RESPONSE))?;
        write_request(dir, request)?;
        let body = dir.join(Self::BODY);
        std::fs::write(&body, &request.body)
            .with_context(|| format!("write {}", body.display()))?;
        let body = body.display().to_string();
        let exit = self.run(
            Entry::Open,
            tree,
            &[
                ("branch", &request.branch),
                ("base", &request.base),
                ("title", &request.title),
                ("body_file", &body),
            ],
        )?;
        std::fs::write(&output, &exit.stdout)
            .with_context(|| format!("write the response {}", output.display()))?;
        let number = self.number_of(&exit.stdout)?;
        let view = self.pull_request(tree, number).with_context(|| {
            format!(
                "the number {number} read from what `open` printed was not read back through `read`; the request may be open under another number, and none is recorded"
            )
        })?;
        if view.head.branch != request.branch || view.state != PullRequestState::Open {
            return Err(ForgeOutput {
                command: Entry::Open.name().to_owned(),
                field: "number".to_owned(),
            })
            .with_context(|| {
                format!(
                    "`read` reports request {number} as {} with the head {}, not an open request for {}; none is recorded",
                    view.state, view.head.branch, request.branch
                )
            });
        }
        Ok(number)
    }

    /// The commit the branch `base`, the merge target, is at in the project `origin` names:
    /// `git ls-remote` of `refs/heads/<base>` over HTTPS alone, as the push goes, then the
    /// commit fetched into no ref, so the tree's repository holds it and no branch, tag or
    /// tracking ref a step can see has moved. No `forge_commands` entry reports it: `read`
    /// describes one request, not a branch. A project with no such branch, an answer that is no
    /// object id, or a command that fails is the error.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        let url = self.push_url()?;
        let reference = format!("refs/heads/{base}");
        let guarded = |verb: &[&str]| -> Vec<String> {
            [
                "git",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "http.followRedirects=false",
            ]
            .into_iter()
            .chain(verb.iter().copied())
            .map(str::to_owned)
            .collect()
        };
        let run = |argv: &[String]| -> Result<Exit> {
            let exit = self.runner.run_env(tree, argv, &PUSH_ENV)?;
            if exit.success() {
                return Ok(exit);
            }
            Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: exit.stderr.trim().to_owned(),
            }
            .into())
        };
        let listed = run(&guarded(&["ls-remote", &url, &reference]))?;
        let tip = tip_of(&listed.stdout, &reference).ok_or_else(|| {
            anyhow!(
                "the project {url} has no single branch {base} to merge into: `git ls-remote` named none"
            )
        })?;
        if !is_object_id(tip) {
            bail!(
                "the project {url} reports {base} at `{}`, which is no object id",
                tip.escape_debug()
            );
        }
        run(&guarded(&[
            "fetch",
            &url,
            tip,
            "--no-write-fetch-head",
            "--no-tags",
            "--recurse-submodules=no",
        ]))?;
        Ok(tip.to_owned())
    }

    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let exit = self.run(Entry::Read, tree, &[("number", &number.to_string())])?;
        view_of(&Self::json(Entry::Read, &exit)?)
    }

    /// The checks on the head `read` reports for the request, the head the command is given.
    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
        let head = self.pull_request(tree, number)?.head.sha;
        let exit = self.run(
            Entry::Checks,
            tree,
            &[("number", &number.to_string()), ("head", &head)],
        )?;
        checks_of(&Self::json(Entry::Checks, &exit)?)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let input = dir.join(MERGE_REQUEST);
        let output = fresh(dir.join(MERGE_RESPONSE))?;
        let document = serde_json::json!({ "number": number, "sha": head });
        std::fs::write(&input, serde_json::to_vec_pretty(&document)?)
            .with_context(|| format!("write {}", input.display()))?;
        let exit = self.run(
            Entry::Merge,
            tree,
            &[("number", &number.to_string()), ("head", head)],
        )?;
        std::fs::write(&output, &exit.stdout)
            .with_context(|| format!("write the response {}", output.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placeholder_is_substituted_once_inside_its_element() {
        let values = [("title", "{body_file} $(id)"), ("body_file", "/x")];
        assert_eq!(
            substitute("--title={title}", &values),
            "--title={body_file} $(id)"
        );
        assert_eq!(substitute("{nothing}", &values), "{nothing}");
        assert_eq!(substitute("{\"a\":{body_file}", &values), "{\"a\":/x");
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn an_unknown_placeholder_is_refused_naming_the_entry() {
        for (commands, entry) in [
            (
                ForgeCommands {
                    open: Some(argv(&["tool", "open", "{branch}", "{titel}"])),
                    ..Default::default()
                },
                "open",
            ),
            (
                ForgeCommands {
                    read: Some(argv(&["tool", "read", "{head}"])),
                    ..Default::default()
                },
                "read",
            ),
            (
                ForgeCommands {
                    merge: Some(argv(&["tool", "merge", "--into={base}"])),
                    ..Default::default()
                },
                "merge",
            ),
        ] {
            let error = commands.check().expect_err("refused").to_string();
            assert!(
                error.contains(&format!("forge_commands.{entry}")),
                "{error}"
            );
            assert!(error.contains("placeholder"), "{error}");
        }
        ForgeCommands {
            open: Some(argv(&[
                "tool",
                "open",
                "{\"a\": 1}",
                "{",
                "}",
                "{Branch}",
                "{branch}",
            ])),
            ..Default::default()
        }
        .check()
        .expect("braces that name no placeholder are text");
    }

    #[test]
    fn a_placeholder_is_no_program() {
        for program in ["{branch}", "{number}", "/bin/{title}", "{nothing}"] {
            let commands = ForgeCommands {
                checks: Some(argv(&[program, "{number}"])),
                ..Default::default()
            };
            let error = commands.check().expect_err("refused").to_string();
            assert!(error.contains("forge_commands.checks"), "{error}");
        }
    }

    #[test]
    fn a_number_is_only_the_request_path_segment() {
        for (url, number) in [
            ("https://forge.example/g/p/-/merge_requests/17", Some("17")),
            ("https://forge.example/g/p/-/merge_requests/17/", Some("17")),
            (
                "https://forge.example/g/p/-/merge_requests/17/diffs?x=3#n4",
                Some("17"),
            ),
            ("https://forge.example/g/p/pull/9", Some("9")),
            ("https://forge.example/g/p/pulls/9.", Some("9")),
            ("https://forge.example:8443/g/p/pull/9", Some("9")),
            ("https://forge.example/g/p/-/merge_requests/new", None),
            ("https://forge.example/g/project-2/-/merge_requests", None),
            ("https://forge.example/g/p/-/merge_requests/2x", None),
            ("https://forge.example:8443/g/project", None),
            ("https://forge.example:8443", None),
            ("https://10.0.0.1/g/p", None),
            ("https://forge.example/g/p?merge_requests/5", None),
            ("https://forge.example/g/p#pull/5", None),
            ("https://forge.example/g/p/issues/12", None),
            (
                "https://forge.example/g/merge_requests/5x/pull/6",
                Some("6"),
            ),
        ] {
            assert_eq!(request_number_of(url), number, "{url}");
        }
    }

    #[test]
    fn a_tip_is_the_line_that_names_exactly_the_reference() {
        let id = "0123456789abcdef0123456789abcdef01234567";
        let other = "89abcdef0123456789abcdef0123456789abcdef";
        let main = "refs/heads/main";
        assert_eq!(tip_of(&format!("{id}\t{main}\n"), main), Some(id));
        assert_eq!(tip_of(&format!("{id}\n"), main), Some(id));
        assert_eq!(
            tip_of(
                &format!("{other}\trefs/heads/x/{main}\n{id}\t{main}\n"),
                main
            ),
            Some(id)
        );
        assert_eq!(tip_of("", main), None);
        assert_eq!(tip_of(&format!("{id}\trefs/heads/x/{main}\n"), main), None);
        assert_eq!(
            tip_of(&format!("{id}\t{main}\n{other}\t{main}\n"), main),
            None
        );
    }

    #[test]
    fn needs_follow_the_actions() {
        let actions = ["ci.read".to_owned(), "tests.run".to_owned()];
        let names: Vec<&str> = ForgeCommands::needs(&actions)
            .iter()
            .map(|(entry, _)| entry.name())
            .collect();
        assert_eq!(names, ["read", "checks"]);
    }
}
