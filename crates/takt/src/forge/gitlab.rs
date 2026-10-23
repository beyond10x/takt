//! The forge of a repository on a GitLab host, through `glab`: the case's commit pushed over
//! HTTPS under `glab`'s credential helper, the merge request opened, read and merged through
//! `glab api`, and, before anything leaves the machine, the same `b10x-gates scan-text` over the
//! same text as the bot route's. takt holds no token: the only credential is `glab`'s own login
//! for the host, which `glab auth status` must confirm before a run opens its case.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};

use super::{
    BotRoute, Checks, Exit, Failed, Forge, Head, IdentityScan, Outgoing, POLICY_VAR, Process,
    PullRequest, PullRequestState, PullRequestView, Runner, Scanner, exact_ref, fresh,
    head_is_a_commit, held_by, is_object_id, push_over_https, reading_of, succeed,
    without_userinfo,
};

/// The host and the full project path (`group/sub/repo`) an `origin` URL names, in the forms git
/// writes them: `git@host:a/b/c.git`, `ssh://git@host[:port]/a/b/c.git`, `https://host/a/b/c`
/// (with or without `.git`). A local path, a URL with no host (`file://`), a path of one segment
/// or a segment that is empty, `.` or `..` names none.
#[must_use]
pub fn project_of(url: &str) -> Option<(String, String)> {
    let url = url.trim().trim_end_matches('/');
    let (authority, path) = if let Some((_, rest)) = url.split_once("://") {
        rest.split_once('/')?
    } else {
        url.split_once(':')?
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split_once(':').map_or(host, |(host, _)| host);
    let path = path.trim_start_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path.split('/').collect();
    let named = segments.len() >= 2
        && segments
            .iter()
            .all(|segment| !segment.is_empty() && *segment != "." && *segment != "..");
    (!host.is_empty() && !host.contains('/') && named).then(|| (host.to_owned(), path.to_owned()))
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

/// The adapter over `glab` for a repository on a GitLab host. The push goes to the project's
/// HTTPS URL with `glab auth git-credential` as git's one credential helper; every API read and
/// write is `glab api --hostname <host>` on the project named by its full path, never a remote
/// of the tree, which a step may change; the scan before a push and a merge request is the bot
/// route's `b10x-gates scan-text`, under `--repository <project>`.
pub struct Glab {
    /// The GitLab host, from the `origin` URL.
    pub host: String,
    /// The project's full path on the host, `group/sub/repo`.
    pub project: String,
    /// The `b10x-gates` policy file; none leaves it to the program's environment.
    pub policy: Option<PathBuf>,
    /// The remote whose tracking ref of the case's base says which commits it holds already.
    pub remote: String,
    /// The most bytes of commit messages and files one scan before a push is given; more is
    /// refused, never cut ([`BotRoute::SCAN_LIMIT`] by default).
    pub scan_limit: usize,
    runner: Box<dyn Runner>,
}

impl fmt::Debug for Glab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Glab")
            .field("host", &self.host)
            .field("project", &self.project)
            .field("policy", &self.policy)
            .field("remote", &self.remote)
            .field("scan_limit", &self.scan_limit)
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
    /// The file under the directory `merge` is given that holds what was sent.
    pub const MERGE_REQUEST: &str = BotRoute::MERGE_REQUEST;
    /// The file under the directory `merge` is given that holds what came back.
    pub const MERGE_RESPONSE: &str = BotRoute::MERGE_RESPONSE;
    /// The credential helper of every push and fetch, after an empty one that clears whatever
    /// helpers git's configuration names: `glab`'s login for the host is the one credential.
    pub const CREDENTIAL_HELPER: &str = "credential.helper=!glab auth git-credential";

    /// Over `project` on `host` with `policy`, running every line through `runner`.
    #[must_use]
    pub fn new(
        host: impl Into<String>,
        project: impl Into<String>,
        policy: Option<PathBuf>,
        runner: Box<dyn Runner>,
    ) -> Self {
        Self {
            host: host.into(),
            project: project.into(),
            policy,
            remote: "origin".to_owned(),
            scan_limit: BotRoute::SCAN_LIMIT,
            runner,
        }
    }

    /// For `tree`: the host and the project from its `origin` URL, which must not be on
    /// [`BotRoute::HOST`], the policy from the environment, every line run as a process; then
    /// [`Self::logged_in`]. A refusal names the URL without its userinfo.
    pub fn for_tree(tree: &Path) -> Result<Self> {
        let url = crate::git::remote_url(tree, "origin")
            .context("the forge `gitlab` needs an origin remote")?;
        let shown = without_userinfo(&url);
        let (host, project) = project_of(&url).ok_or_else(|| {
            anyhow!(
                "the origin remote {shown} names no <namespace>/<project> on a host; give --forge none"
            )
        })?;
        if BotRoute::serves(&host) {
            bail!(
                "the origin remote {shown} is on {host}, which the bot route serves: give --forge bot, not gitlab"
            );
        }
        let policy = std::env::var_os(POLICY_VAR).map(PathBuf::from);
        let glab = Self::new(host, project, policy, Box::new(Process::default()));
        glab.logged_in(tree)?;
        Ok(glab)
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
            Self::CREDENTIAL_HELPER,
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
            Self::CREDENTIAL_HELPER,
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
        self.api_get_argv(&format!(
            "{}/repository/branches/{}",
            self.api_project(),
            encoded(base)
        ))
    }

    /// The line that opens the merge request `input` describes.
    #[must_use]
    pub fn open_argv(&self, input: &Path) -> Vec<String> {
        self.api_write_argv(
            "POST",
            &format!("{}/merge_requests", self.api_project()),
            input,
        )
    }

    /// The line that reads merge request `iid`: its state, head and base, and its head pipeline.
    #[must_use]
    pub fn view_argv(&self, iid: u64) -> Vec<String> {
        self.api_get_argv(&format!("{}/merge_requests/{iid}", self.api_project()))
    }

    /// The line that merges merge request `iid`, the head to match read from `input`.
    #[must_use]
    pub fn merge_argv(&self, iid: u64, input: &Path) -> Vec<String> {
        self.api_write_argv(
            "PUT",
            &format!("{}/merge_requests/{iid}/merge", self.api_project()),
            input,
        )
    }

    /// The scan before a push and a merge request, under `--repository <project>`.
    fn scanner(&self) -> Scanner<'_> {
        Scanner {
            runner: &*self.runner,
            repository: &self.project,
            policy: self.policy.as_deref(),
        }
    }

    /// The line that scans the merge request before it is opened: the JSON `request` and, last,
    /// the `published` title and description.
    #[must_use]
    pub fn scan_request_argv(&self, request: &Path, published: &Path) -> Vec<String> {
        self.scanner().argv(&[request, published])
    }

    /// The commit the project's branch `base` is at, read through `glab api`; `None` when the
    /// project has no such branch (`glab` says `HTTP 404`). Anything else that is not a commit id
    /// in `.commit.id` is an error.
    fn base_tip(&self, tree: &Path, base: &str) -> Result<Option<String>> {
        let argv = self.branch_argv(base);
        let exit = self.runner.run(tree, &argv)?;
        if !exit.success() && exit.stderr.contains("HTTP 404") {
            return Ok(None);
        }
        let document = reading_of(&argv, &exit)?;
        document["commit"]["id"]
            .as_str()
            .map(|id| Some(id.to_owned()))
            .ok_or_else(|| anyhow!("{} reported no commit.id", argv.join(" ")))
    }

    /// The merge request `iid` as `glab api` reports it.
    fn read(&self, tree: &Path, iid: u64) -> Result<(Vec<String>, serde_json::Value)> {
        let argv = self.view_argv(iid);
        let exit = self.runner.run(tree, &argv)?;
        let document = reading_of(&argv, &exit)?;
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

/// The checks on a merge request's head, from its `head_pipeline`: none yet, or one of an older
/// push (its `sha` not the merge request's), is pending; `success` and `skipped` pass, `failed`
/// and `canceled` fail, `created`, `waiting_for_resource`, `preparing`, `pending`, `running`,
/// `manual` and `scheduled` are pending; any other status is an error naming it.
fn checks_of(argv: &[String], document: &serde_json::Value) -> Result<Checks> {
    let pipeline = &document["head_pipeline"];
    if pipeline.is_null() {
        return Ok(Checks::Pending);
    }
    let head = document["sha"]
        .as_str()
        .ok_or_else(|| anyhow!("{} reported no sha", argv.join(" ")))?;
    if pipeline["sha"].as_str() != Some(head) {
        return Ok(Checks::Pending);
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
        push_over_https(
            &*self.runner,
            tree,
            &self.push_revision_argv(branch, revision),
            &self.push_url(),
            "the forge gitlab",
        )
    }

    /// The bot route's scan over the same text: the merge request, when one is opened, and the
    /// objects the push sends that the project does not hold, its `base` read through `glab api`.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        head_is_a_commit(tree, outgoing.branch, outgoing.head)?;
        let held = held_by(tree, &self.remote, outgoing.base, || {
            self.base_tip(tree, outgoing.base)
        })?;
        self.scanner().outgoing(
            tree,
            dir,
            outgoing,
            held.as_deref(),
            self.scan_limit,
            Self::write_request,
            "merge request",
        )
    }

    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &crate::git::CommitIdentity,
    ) -> Result<IdentityScan> {
        self.scanner()
            .identity(tree, dir, identity, "the forge gitlab")
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::RESPONSE))?;
        let (input, published) = Self::write_request(dir, request)?;
        succeed(
            &*self.runner,
            tree,
            &self.scan_request_argv(&input, &published),
        )
        .with_context(|| {
            format!(
                "the merge request for {branch} did not pass scan-text over {} and was not opened; its title and description stayed on this machine, but the branch {branch} was pushed before this scan",
                published.display(),
                branch = request.branch
            )
        })?;
        let argv = self.open_argv(&input);
        let exit = succeed(&*self.runner, tree, &argv)?;
        std::fs::write(&output, &exit.stdout)
            .with_context(|| format!("write the response {}", output.display()))?;
        let response = reading_of(&argv, &exit)?;
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
        let input = dir.join(Self::MERGE_REQUEST);
        let output = fresh(dir.join(Self::MERGE_RESPONSE))?;
        // The head to match: GitLab refuses the merge when the merge request's head is no longer
        // this commit, so what is merged is what was read and recorded, never a later push.
        let document = serde_json::json!({ "sha": head });
        let bytes = serde_json::to_vec_pretty(&document).context("serialize the merge request")?;
        std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
        let argv = self.merge_argv(number, &input);
        let exit = succeed(&*self.runner, tree, &argv)?;
        std::fs::write(&output, &exit.stdout)
            .with_context(|| format!("write the response {}", output.display()))?;
        let response = reading_of(&argv, &exit)?;
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
        let exit = self.runner.run_env(tree, &argv, &BotRoute::PUSH_ENV)?;
        if !exit.success() {
            return Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: exit.stderr.trim().to_owned(),
            }
            .into());
        }
        Ok(tip)
    }
}
