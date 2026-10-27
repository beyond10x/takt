//! The forge of a repository on a GitLab host, through `glab`: the case's commit pushed over
//! HTTPS under `glab`'s credential helper, the merge request opened, read and merged through
//! `glab api`, and, before anything leaves the machine, the run's scan adapter
//! ([`crate::scan::Scan`]) over the same text as the bot route scans: under `gates`, the gates
//! scan under `--repository <project>`; under a declared `none`, no scan. takt holds no token:
//! the only credential is `glab`'s own login for the host, which `glab auth status` must confirm
//! before a run opens its case.
//!
//! A second transport, `api` ([`Transport`]), calls the GitLab REST API over HTTPS directly under
//! a credential the run file names ([`CredentialSource`]): [`api`]. The default stays `glab`.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};

use crate::scan::Scan;

pub mod api;

use super::credential::CredentialSource;
use super::{
    BotRoute, Checks, Exit, Failed, Forge, Head, IdentityScan, Outgoing, Process, PullRequest,
    PullRequestState, PullRequestView, Runner, Scanner, exact_ref, fresh, head_is_a_commit,
    held_by, is_object_id, push_over_https_with, reading_of, succeed, without_userinfo,
};

/// The host and the full project path (`group/sub/repo`) an `origin` URL names, in the forms git
/// writes them: `git@host:a/b/c.git`, `ssh://git@host[:port]/a/b/c.git`, `https://host/a/b/c`
/// (with or without `.git`). An `https://` URL's port stays with the host (`host:8443`), since
/// the push and `glab api` go to that same HTTPS server; an SSH port says nothing of the HTTPS
/// one and is dropped. A local path, a URL with no host (`file://`), a path of one segment or a
/// segment that is empty, `.` or `..` names none.
#[must_use]
pub fn project_of(url: &str) -> Option<(String, String)> {
    let url = url.trim().trim_end_matches('/');
    let (scheme, (authority, path)) = if let Some((scheme, rest)) = url.split_once("://") {
        (Some(scheme), rest.split_once('/')?)
    } else {
        (None, url.split_once(':')?)
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.split_once(':') {
        Some((name, port))
            if scheme.is_some_and(|scheme| scheme.eq_ignore_ascii_case("https"))
                && !name.is_empty()
                && !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            host
        }
        Some((name, _)) => name,
        None => host,
    };
    let path = path.trim_start_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path.split('/').collect();
    let named = segments.len() >= 2
        && segments
            .iter()
            .all(|segment| !segment.is_empty() && *segment != "." && *segment != "..");
    (!host.is_empty() && !host.contains('/') && named).then(|| (host.to_owned(), path.to_owned()))
}

/// How the forge reaches the host, `transport` of the run file: `glab`, the default, through
/// `glab api` under glab's own login; `api`, direct HTTPS calls under a named credential.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "String")]
pub enum Transport {
    /// Through `glab`.
    #[default]
    Glab,
    /// Through the REST API directly.
    Api,
}

impl TryFrom<String> for Transport {
    type Error = String;

    fn try_from(name: String) -> std::result::Result<Self, String> {
        match name.as_str() {
            "glab" => Ok(Self::Glab),
            "api" => Ok(Self::Api),
            _ => Err("the transport is `glab` or `api`".to_owned()),
        }
    }
}

/// `text` percent-encoded as one segment of an API path: every byte but the unreserved ones
/// (`A-Z a-z 0-9 - . _ ~`), so `group/sub/repo` is `group%2Fsub%2Frepo`.
#[must_use]
pub fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// What `glab` printed, on one line: standard error, else standard output, each non-empty line
/// trimmed and joined by ` | `.
fn one_line(exit: &Exit) -> String {
    let said = if exit.stderr.trim().is_empty() {
        &exit.stdout
    } else {
        &exit.stderr
    };
    said.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// What `glab api` answered on the line `argv`, as JSON, once it exited 0. On an HTTP error
/// glab exits non-zero with the error's JSON body (`{"message": …}`) on standard output and the
/// status on standard error (`… (HTTP 401)`): that is an error naming the line, the body's
/// message and what glab said ([`failure_of`]), never a reading, so a request the host refused
/// or failed is never read as a pending pipeline or as no branch.
fn answer_of(argv: &[String], exit: &Exit) -> Result<serde_json::Value> {
    if !exit.success() {
        bail!("`{}` failed: {}", argv.join(" "), failure_of(exit));
    }
    reading_of(argv, exit)
}

/// What a `glab api` that did not exit 0 said, on one line: how it exited, the `message` (else
/// the `error`) of the JSON body on standard output, and standard error, which carries the HTTP
/// status; standard output itself when it is no such body and standard error is empty.
fn failure_of(exit: &Exit) -> String {
    let code = exit.code.map_or_else(
        || "it ended on a signal".to_owned(),
        |code| format!("exit {code}"),
    );
    let body = serde_json::from_str::<serde_json::Value>(exit.stdout.trim()).ok();
    let message = body.as_ref().and_then(|body| {
        ["message", "error"]
            .iter()
            .find_map(|key| match &body[*key] {
                serde_json::Value::Null => None,
                serde_json::Value::String(text) => Some(text.clone()),
                other => Some(other.to_string()),
            })
    });
    let stderr = exit
        .stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    match (message, stderr.is_empty()) {
        (Some(message), true) => format!("{code}, {message}"),
        (Some(message), false) => format!("{code}, {message} ({stderr})"),
        (None, false) => format!("{code}, {stderr}"),
        (None, true) if exit.stdout.trim().is_empty() => format!("{code}, nothing said"),
        (None, true) => format!("{code}, {}", one_line(exit)),
    }
}

/// The adapter over `glab` for a repository on a GitLab host. The push goes to the project's
/// HTTPS URL with `glab auth git-credential` as git's one credential helper; every API read and
/// write is `glab api --hostname <host>` on the project named by its full path, never a remote
/// of the tree, which a step may change; the scan before a push and a merge request is the
/// run's scan adapter's ([`Self::scan`]): under `gates` the one the bot route runs, under
/// `--repository <project>`; under a declared `none`, none.
pub struct Glab {
    /// The GitLab host, from the `origin` URL.
    pub host: String,
    /// The project's full path on the host, `group/sub/repo`.
    pub project: String,
    /// The run's scan adapter: under `gates`, its policy file, none leaving it to the scan
    /// program's environment; under `none`, nothing is scanned.
    pub scan: Scan,
    /// The remote whose tracking ref of the case's base says which commits it holds already.
    pub remote: String,
    /// The most bytes of commit messages and files one scan before a push is given; more is
    /// refused, never cut ([`BotRoute::SCAN_LIMIT`] by default).
    pub scan_limit: usize,
    runner: Box<dyn Runner>,
    api: Option<api::Api>,
}

impl fmt::Debug for Glab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Glab")
            .field("host", &self.host)
            .field("project", &self.project)
            .field("scan", &self.scan)
            .field("remote", &self.remote)
            .field("scan_limit", &self.scan_limit)
            .field("transport", &self.transport())
            .finish_non_exhaustive()
    }
}

impl Glab {
    /// The file under the directory `open_pull_request` is given that holds what was sent: the
    /// bot route's name, the one file of a case's artifacts `pr.open` may write for it.
    pub const REQUEST: &str = BotRoute::REQUEST;
    /// The file under the directory `open_pull_request` is given that holds what came back.
    pub const RESPONSE: &str = BotRoute::RESPONSE;
    /// The file under the directory `open_pull_request` is given that holds the title and the
    /// description as they are published, decoded: the text `scan-text` reads line by line.
    pub const PUBLISHED: &str = BotRoute::PUBLISHED;
    /// The file under the directory `merge` is given that holds the body of the merge's `PUT`,
    /// the head to match: the bot route's file name, the one `repository.merge` may write.
    pub const MERGE_BODY: &str = BotRoute::MERGE_REQUEST;
    /// The file under the directory `merge` is given that holds what came back.
    pub const MERGE_RESPONSE: &str = BotRoute::MERGE_RESPONSE;
    /// The credential helper of every push and fetch, after an empty one that clears whatever
    /// helpers git's configuration names: `glab`'s login for the host is the one credential.
    pub const CREDENTIAL_HELPER: &str = "credential.helper=!glab auth git-credential";

    /// Over `project` on `host` under the scan adapter `scan`, running every line through
    /// `runner`.
    #[must_use]
    pub fn new(
        host: impl Into<String>,
        project: impl Into<String>,
        scan: Scan,
        runner: Box<dyn Runner>,
    ) -> Self {
        Self {
            host: host.into(),
            project: project.into(),
            scan,
            remote: "origin".to_owned(),
            scan_limit: BotRoute::SCAN_LIMIT,
            runner,
            api: None,
        }
    }

    /// For `tree`: the host and the project from its `origin` URL, which must not be on
    /// [`BotRoute::HOST`], under the run's scan adapter `scan`, every line run as a process;
    /// then [`Self::logged_in`]. A refusal names the URL without its userinfo.
    pub fn for_tree(tree: &Path, scan: &Scan) -> Result<Self> {
        Self::for_tree_through(tree, scan, Box::new(Process::default()))
    }

    /// [`Self::for_tree`], every line, the login check first, run through `runner`.
    pub fn for_tree_through(tree: &Path, scan: &Scan, runner: Box<dyn Runner>) -> Result<Self> {
        let (host, project) = Self::origin_of(tree)?;
        let glab = Self::new(host, project, scan.clone(), runner);
        glab.logged_in(tree)?;
        Ok(glab)
    }

    /// The host and the project `tree`'s `origin` names; refused when it names none or the bot
    /// route's host.
    fn origin_of(tree: &Path) -> Result<(String, String)> {
        let url = crate::git::remote_url(tree, "origin")
            .context("the forge `gitlab` needs an origin remote")?;
        let shown = without_userinfo(&url);
        let (host, project) = project_of(&url).ok_or_else(|| {
            anyhow!(
                "the origin remote {shown} names no <namespace>/<project> on a host; give --forge none"
            )
        })?;
        if BotRoute::serves(host.split_once(':').map_or(host.as_str(), |(name, _)| name)) {
            bail!(
                "the origin remote {shown} is on {host}, which the bot route serves: give --forge bot, not gitlab"
            );
        }
        Ok((host, project))
    }

    /// For `tree` under the `api` transport: [`Self::for_tree`]'s host and project, and the
    /// credential `source` resolved before anything is opened (an unset, open or misplaced
    /// source is refused naming it, never its value); no `glab` is asked for a login.
    pub fn for_tree_api(tree: &Path, scan: &Scan, source: &CredentialSource) -> Result<Self> {
        Self::for_tree_api_through(tree, scan, source, Box::new(Process::default()))
    }

    /// [`Self::for_tree_api`], the push and the scan run through `runner`.
    pub fn for_tree_api_through(
        tree: &Path,
        scan: &Scan,
        source: &CredentialSource,
        runner: Box<dyn Runner>,
    ) -> Result<Self> {
        let (host, project) = Self::origin_of(tree)?;
        let secret = source.resolve(tree)?;
        let mut glab = Self::new(host.clone(), project, scan.clone(), runner);
        glab.api = Some(api::Api::new(&host, secret));
        Ok(glab)
    }

    /// The transport this forge reaches the host through.
    #[must_use]
    pub fn transport(&self) -> Transport {
        if self.api.is_some() {
            Transport::Api
        } else {
            Transport::Glab
        }
    }

    /// One call of the host's API: under `glab` the line [`Self::api_get_argv`] or
    /// [`Self::api_write_argv`] runs; under `api` the HTTPS call. The line that names the call,
    /// for what is said of it (never the token), and what the host answered, as an [`Exit`].
    fn exchange(
        &self,
        tree: &Path,
        method: &str,
        path: &str,
        input: Option<&Path>,
    ) -> Result<(Vec<String>, Exit)> {
        if let Some(api) = &self.api {
            let body = input
                .map(|input| {
                    std::fs::read(input).with_context(|| format!("read {}", input.display()))
                })
                .transpose()?;
            let exit = api.send(method, path, body)?;
            return Ok((vec![method.to_owned(), api.url(path)], exit));
        }
        let argv = match input {
            Some(input) => self.api_write_argv(method, path, input),
            None => self.api_get_argv(path),
        };
        let exit = self.runner.run(tree, &argv)?;
        Ok((argv, exit))
    }

    /// The credential helper of every push and fetch: under `glab`, its own; under `api`, the one
    /// that answers from the token in the environment of that one line.
    fn helper(&self) -> &'static str {
        if self.api.is_some() {
            api::CREDENTIAL_HELPER
        } else {
            Self::CREDENTIAL_HELPER
        }
    }

    /// What a push or a fetch is run with: git held to HTTPS, and under `api` the token in the
    /// variable the helper reads, for that one line.
    fn push_env(&self) -> Vec<(&str, &str)> {
        let mut env = BotRoute::PUSH_ENV.to_vec();
        if let Some(api) = &self.api {
            env.push((api::HELPER_VARIABLE, api.secret().expose()));
        }
        env
    }

    /// `text` with the token replaced, when there is one.
    fn redacted(&self, text: &str) -> String {
        match &self.api {
            Some(api) => api.secret().redact(text),
            None => text.to_owned(),
        }
    }

    /// An error with the token replaced, when there is one.
    fn without_token(&self, error: anyhow::Error) -> anyhow::Error {
        match &self.api {
            Some(api) => anyhow!("{}", api.secret().redact(&format!("{error:#}"))),
            None => error,
        }
    }

    /// Whether `glab` is logged in to the host: [`Self::auth_status_argv`] exits 0. A `glab`
    /// that cannot be run, or that exits otherwise, is an error naming `glab`, the host and, on
    /// one line, what it said.
    pub fn logged_in(&self, tree: &Path) -> Result<()> {
        let argv = self.auth_status_argv();
        let exit = self.runner.run(tree, &argv).map_err(|error| {
            anyhow!(
                "glab could not be run to check its login to {}: {error:#}; install glab and run `glab auth login --hostname {}`",
                self.host,
                self.host
            )
        })?;
        if !exit.success() {
            bail!(
                "glab is not logged in to {}: `{}` exited {}: {}",
                self.host,
                argv.join(" "),
                exit.code
                    .map_or("on a signal".to_owned(), |code| code.to_string()),
                one_line(&exit)
            );
        }
        Ok(())
    }

    /// The line that checks `glab`'s login to the host.
    #[must_use]
    pub fn auth_status_argv(&self) -> Vec<String> {
        ["glab", "auth", "status", "--hostname", &self.host]
            .map(str::to_owned)
            .to_vec()
    }

    /// The project's API path, `projects/<project percent-encoded>`.
    fn api_project(&self) -> String {
        format!("projects/{}", encoded(&self.project))
    }

    /// The line that reads `path` of the API on the host.
    #[must_use]
    pub fn api_get_argv(&self, path: &str) -> Vec<String> {
        ["glab", "api", "--hostname", &self.host, path]
            .map(str::to_owned)
            .to_vec()
    }

    /// The line that sends the JSON document `input` to `path` of the API on the host with
    /// `method`.
    #[must_use]
    pub fn api_write_argv(&self, method: &str, path: &str, input: &Path) -> Vec<String> {
        let input = input.display().to_string();
        [
            "glab",
            "api",
            "--hostname",
            &self.host,
            "--method",
            method,
            "--header",
            "Content-Type: application/json",
            path,
            "--input",
            &input,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The URL every push and fetch goes to: the project's on the host, over HTTPS. A URL, not
    /// the remote's name, so a push moves no tracking ref.
    #[must_use]
    pub fn push_url(&self) -> String {
        format!("https://{}/{}.git", self.host, self.project)
    }

    /// The line that pushes the commit `revision` as `branch`, `<revision>:refs/heads/<branch>`,
    /// with `glab`'s credential helper the only one, no tag and no submodule's commit with it,
    /// the options last.
    #[must_use]
    pub fn push_revision_argv(&self, branch: &str, revision: &str) -> Vec<String> {
        let url = self.push_url();
        let refspec = format!("{revision}:refs/heads/{branch}");
        [
            "git",
            "-c",
            "credential.helper=",
            "-c",
            self.helper(),
            "push",
            &url,
            &refspec,
            "--no-follow-tags",
            "--recurse-submodules=no",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The line that fetches the commit `object` and what it reaches into no ref, as the push
    /// authenticates: no `FETCH_HEAD`, no tag and no submodule.
    #[must_use]
    pub fn fetch_object_argv(&self, object: &str) -> Vec<String> {
        let url = self.push_url();
        [
            "git",
            "-c",
            "credential.helper=",
            "-c",
            self.helper(),
            "fetch",
            &url,
            object,
            "--no-write-fetch-head",
            "--no-tags",
            "--recurse-submodules=no",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The line that reads the project's branch `base`.
    #[must_use]
    pub fn branch_argv(&self, base: &str) -> Vec<String> {
        self.api_get_argv(&self.branch_path(base))
    }

    /// The API path of the project's branch `base`.
    fn branch_path(&self, base: &str) -> String {
        format!(
            "{}/repository/branches/{}",
            self.api_project(),
            encoded(base)
        )
    }

    /// The line that opens the merge request `input` describes.
    #[must_use]
    pub fn open_argv(&self, input: &Path) -> Vec<String> {
        self.api_write_argv("POST", &self.open_path(), input)
    }

    /// The API path merge requests are opened at.
    fn open_path(&self) -> String {
        format!("{}/merge_requests", self.api_project())
    }

    /// The line that reads merge request `iid`: its state, head and base, and its head pipeline.
    #[must_use]
    pub fn view_argv(&self, iid: u64) -> Vec<String> {
        self.api_get_argv(&self.view_path(iid))
    }

    /// The API path of merge request `iid`.
    fn view_path(&self, iid: u64) -> String {
        format!("{}/merge_requests/{iid}", self.api_project())
    }

    /// The line that merges merge request `iid`, the head to match read from `input`.
    #[must_use]
    pub fn merge_argv(&self, iid: u64, input: &Path) -> Vec<String> {
        self.api_write_argv("PUT", &self.merge_path(iid), input)
    }

    /// The API path that merges merge request `iid`.
    fn merge_path(&self, iid: u64) -> String {
        format!("{}/merge_requests/{iid}/merge", self.api_project())
    }

    /// The scan before a push and a merge request, the run's scan adapter's: under `gates`, the
    /// gates scan under `--repository <project>`; none under a declared `none`.
    fn scanner(&self) -> Option<Scanner<'_>> {
        match &self.scan {
            Scan::Gates { policy } => Some(Scanner {
                runner: &*self.runner,
                repository: &self.project,
                policy: policy.as_deref(),
            }),
            Scan::None { .. } => None,
        }
    }

    /// The line that scans the merge request before it is opened: the JSON `request` and, last,
    /// the `published` title and description; none under a declared `none`, which scans nothing.
    #[must_use]
    pub fn scan_request_argv(&self, request: &Path, published: &Path) -> Option<Vec<String>> {
        self.scanner()
            .map(|scanner| scanner.argv(&[request, published]))
    }

    /// The commit the project's branch `base` is at, read through `glab api`; `None` when the
    /// project has no such branch (`glab` says `HTTP 404`). Anything else that is not a commit id
    /// in `.commit.id` is an error.
    fn base_tip(&self, tree: &Path, base: &str) -> Result<Option<String>> {
        let (argv, exit) = self.exchange(tree, "GET", &self.branch_path(base), None)?;
        if !exit.success() && exit.stderr.contains("HTTP 404") {
            return Ok(None);
        }
        let document = answer_of(&argv, &exit)?;
        document["commit"]["id"]
            .as_str()
            .map(|id| Some(id.to_owned()))
            .ok_or_else(|| anyhow!("{} reported no commit.id", argv.join(" ")))
    }

    /// The merge request `iid` as `glab api` reports it.
    fn read(&self, tree: &Path, iid: u64) -> Result<(Vec<String>, serde_json::Value)> {
        let (argv, exit) = self.exchange(tree, "GET", &self.view_path(iid), None)?;
        let document = answer_of(&argv, &exit)?;
        Ok((argv, document))
    }

    /// Writes the merge request `request` into `dir`: the JSON sent ([`Self::REQUEST`]) and the
    /// title and description as published, decoded ([`Self::PUBLISHED`]). Both paths.
    fn write_request(dir: &Path, request: &PullRequest) -> Result<(PathBuf, PathBuf)> {
        let input = dir.join(Self::REQUEST);
        let document = serde_json::json!({
            "source_branch": request.branch,
            "target_branch": request.base,
            "title": request.title,
            "description": request.body,
        });
        let bytes = serde_json::to_vec_pretty(&document).context("serialize the merge request")?;
        std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
        let published = dir.join(Self::PUBLISHED);
        std::fs::write(&published, format!("{}\n\n{}", request.title, request.body))
            .with_context(|| format!("write {}", published.display()))?;
        Ok((input, published))
    }
}

/// A merge request's `state` as one of the three: `opened` and `locked` (a merge in progress,
/// which the next read reports `merged`) are open; anything but these four is an error.
fn state_of(argv: &[String], document: &serde_json::Value) -> Result<PullRequestState> {
    Ok(match document["state"].as_str() {
        Some("opened" | "locked") => PullRequestState::Open,
        Some("merged") => PullRequestState::Merged,
        Some("closed") => PullRequestState::Closed,
        other => bail!(
            "{} reported the state {other:?}, not opened, locked, merged or closed",
            argv.join(" ")
        ),
    })
}

/// The checks on a merge request's head, from its `head_pipeline`: none yet, one of an older
/// push (its `sha` not the merge request's) or one with no `sha`, is pending; `success` and
/// `skipped` pass, `failed` and `canceled` fail, `created`, `waiting_for_resource`,
/// `preparing`, `pending`, `running`, `manual` and `scheduled` are pending; any other status is
/// an error naming it.
fn checks_of(argv: &[String], document: &serde_json::Value) -> Result<Checks> {
    let pipeline = &document["head_pipeline"];
    if pipeline.is_null() {
        return Ok(Checks::Pending);
    }
    let head = document["sha"]
        .as_str()
        .ok_or_else(|| anyhow!("{} reported no sha", argv.join(" ")))?;
    // Only a pipeline that names the head as its commit says anything of the head: one of an
    // older push, or one that names no commit, is pending.
    match pipeline["sha"].as_str() {
        Some(sha) if sha == head => {}
        Some(_) | None => return Ok(Checks::Pending),
    }
    Ok(match pipeline["status"].as_str() {
        Some("success" | "skipped") => Checks::Pass,
        Some("failed" | "canceled") => Checks::Fail,
        Some(
            "created"
            | "waiting_for_resource"
            | "preparing"
            | "pending"
            | "running"
            | "manual"
            | "scheduled",
        ) => Checks::Pending,
        other => bail!(
            "{} reported a head pipeline in the status {other:?}",
            argv.join(" ")
        ),
    })
}

impl Forge for Glab {
    fn name(&self) -> &str {
        "gitlab"
    }

    /// Pushes the commit `branch` is at, by its id ([`Self::push_revision`]).
    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let at = exact_ref(tree, &format!("refs/heads/{branch}"))
            .ok_or_else(|| anyhow!("{branch} names no commit; nothing was pushed"))?;
        self.push_revision(tree, branch, &at)
    }

    /// Pushes the commit `revision` as `branch`, by its id ([`Self::push_revision_argv`]), over
    /// HTTPS alone.
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        push_over_https_with(
            &*self.runner,
            tree,
            &self.push_revision_argv(branch, revision),
            &self.push_url(),
            "the forge gitlab",
            &self.push_env(),
        )
        .map_err(|error| self.without_token(error))
    }

    /// The run's scan adapter over the text the bot route scans: the merge request, when one
    /// is opened, and the objects the push sends that the project does not hold, its `base` read
    /// through `glab api`. A head that is not a commit is refused first, under any adapter;
    /// under a declared `none` nothing else is read and no scan runs.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        head_is_a_commit(tree, outgoing.branch, outgoing.head)?;
        let Some(scanner) = self.scanner() else {
            return Ok(());
        };
        let held = held_by(tree, &self.remote, outgoing.base, || {
            self.base_tip(tree, outgoing.base)
        })?;
        scanner.outgoing(
            tree,
            dir,
            outgoing,
            held.as_deref(),
            self.scan_limit,
            Self::write_request,
            "merge request",
        )
    }

    /// The commit identity under the run's scan adapter, as the scan before a push reads it;
    /// under a declared `none`, [`IdentityScan::NotScanned`] naming the declaration
    /// ([`Scan::not_scanned`]), no line run.
    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &crate::git::CommitIdentity,
    ) -> Result<IdentityScan> {
        match self.scanner() {
            Some(scanner) => scanner.identity(tree, dir, identity, "the forge gitlab"),
            None => Ok(IdentityScan::NotScanned(
                self.scan.not_scanned().unwrap_or_default(),
            )),
        }
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::RESPONSE))?;
        let (input, published) = Self::write_request(dir, request)?;
        if let Some(scan) = self.scan_request_argv(&input, &published) {
            succeed(&*self.runner, tree, &scan).with_context(|| {
                format!(
                    "the merge request for {branch} did not pass scan-text over {} and was not opened; its title and description stayed on this machine, but the branch {branch} was pushed before this scan",
                    published.display(),
                    branch = request.branch
                )
            })?;
        }
        let (argv, exit) = self.exchange(tree, "POST", &self.open_path(), Some(&input))?;
        std::fs::write(&output, &exit.stdout)
            .with_context(|| format!("write the response {}", output.display()))?;
        let response = answer_of(&argv, &exit)?;
        response["iid"].as_u64().ok_or_else(|| {
            anyhow!(
                "the response {} has no iid: no merge request number",
                output.display()
            )
        })
    }

    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let (argv, document) = self.read(tree, number)?;
        let state = state_of(&argv, &document)?;
        let field = |name: &str| -> Result<String> {
            document[name]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{} reported no {name}", argv.join(" ")))
        };
        Ok(PullRequestView {
            state,
            head: Head {
                branch: field("source_branch")?,
                sha: field("sha")?,
            },
            base: field("target_branch")?,
        })
    }

    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
        let (argv, document) = self.read(tree, number)?;
        checks_of(&argv, &document)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let input = dir.join(Self::MERGE_BODY);
        let output = fresh(dir.join(Self::MERGE_RESPONSE))?;
        // The head to match: GitLab refuses the merge when the merge request's head is no longer
        // this commit, so what is merged is what was read and recorded, never a later push.
        let document = serde_json::json!({ "sha": head });
        let bytes = serde_json::to_vec_pretty(&document).context("serialize the merge request")?;
        std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
        let (argv, exit) = self.exchange(tree, "PUT", &self.merge_path(number), Some(&input))?;
        std::fs::write(&output, &exit.stdout)
            .with_context(|| format!("write the response {}", output.display()))?;
        let response = answer_of(&argv, &exit)?;
        if response["state"] != "merged" {
            bail!(
                "{} did not merge merge request !{number}: {}",
                argv.join(" "),
                response["message"]
                    .as_str()
                    .unwrap_or("the response does not say `state: merged`")
            );
        }
        Ok(())
    }

    /// The project's own `base`, read through `glab api` ([`Self::branch_argv`]), then its commit
    /// fetched into no ref ([`Self::fetch_object_argv`]). A project with no such branch, an
    /// answer that is no object id, or a fetch that fails is the error.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        let tip = self.base_tip(tree, base)?.ok_or_else(|| {
            anyhow!(
                "the project {} on {} has no branch {base} to merge into",
                self.project,
                self.host
            )
        })?;
        if !is_object_id(&tip) {
            bail!(
                "the project {} on {} reports {base} at `{}`, which is no object id",
                self.project,
                self.host,
                tip.escape_debug()
            );
        }
        let argv = self.fetch_object_argv(&tip);
        let exit = self
            .runner
            .run_env(tree, &argv, &self.push_env())
            .map_err(|error| self.without_token(error))?;
        if !exit.success() {
            return Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: self.redacted(exit.stderr.trim()),
            }
            .into());
        }
        Ok(tip)
    }
}
