//! The `bot` forge adapter, the organization's bot route through `b10x-gates`: the bot credential
//! never enters takt. `b10x-gates bot` pushes under the bot the commit scanned, by its id, to the
//! repository's HTTPS URL and over HTTPS only, and fetches through the same route;
//! `b10x-gates api` writes through JSON files and never a token in argv, and reads (`GET`, its
//! response written to a file in a directory of takt's own, `0700`) the pull request and the
//! checks on its head under the same App, so no `gh` and no login of the user's is needed; and
//! before any of it leaves the machine `b10x-gates scan-text` reads what a push would
//! publish and a pull request's title and body, as [`super`] says of every adapter that
//! publishes. It scans with `b10x-gates` whatever the run's scan adapter is
//! ([`crate::scan::Scan`]), since it is the gates adapter's own forge.
//!
//! Every line takt runs through `b10x-gates bot` and `b10x-gates api` is built here, the
//! scenario run's trial fetch and push included ([`BotRoute::trial_fetch_argv`],
//! [`BotRoute::trial_push_argv`]); the scan's line is the gates scan adapter's
//! ([`crate::scan::gates::outgoing_argv`]).

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};

use super::gitlab::encoded;
use super::{
    Checks, Exit, Failed, Forge, Head, IdentityScan, MergeMessage, Outgoing, Process, PullRequest,
    PullRequestState, PullRequestView, Runner, Scanner, exact_ref, fresh, head_is_a_commit,
    held_by, is_object_id, push_over_https, repository_of, succeed, without_userinfo,
    write_request,
};

/// The program every line of the bot route runs.
const PROGRAM: &str = crate::scan::gates::PROGRAM;

/// What `b10x-gates api` prints before the forge's own message for a refused write:
/// `b10x-gates: GitHub operation failed (<status>): <message>` ([`super::forge_message`]).
pub(super) const API_REFUSAL_PREFIX: &str = "b10x-gates: GitHub operation failed (";

/// The adapter over the organization's bot route: `b10x-gates scan-text` checks what a push and
/// a pull request would publish before any of it leaves the machine, `b10x-gates bot -- push`
/// pushes the branch over HTTPS only, `b10x-gates api` opens the pull request and merges it with
/// JSON files, and `b10x-gates api --method GET` reads its state and the checks on its head. The
/// scan and every API line name the repository and, when one is known, the policy file as
/// `--policy`; `b10x-gates` reads its own environment otherwise.
pub struct BotRoute {
    /// `<owner>/<repo>`, from the `origin` URL.
    pub repository: String,
    /// The `b10x-gates` policy file; none leaves it to the program's environment.
    pub policy: Option<PathBuf>,
    /// The remote whose tracking ref of the case's base says which commits it holds already.
    pub remote: String,
    /// The most bytes of commit messages and files one scan before a push is given; more is
    /// refused, never cut ([`Self::SCAN_LIMIT`] by default).
    pub scan_limit: usize,
    runner: Box<dyn Runner>,
}

impl fmt::Debug for BotRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BotRoute")
            .field("repository", &self.repository)
            .field("policy", &self.policy)
            .field("remote", &self.remote)
            .field("scan_limit", &self.scan_limit)
            .finish_non_exhaustive()
    }
}

impl BotRoute {
    /// The file under the directory `open_pull_request` is given that holds what was sent.
    pub const REQUEST: &str = "pull-request.request.json";
    /// The file under the directory `open_pull_request` is given that holds what came back.
    pub const RESPONSE: &str = "pull-request.response.json";
    /// The file under the directory `open_pull_request` is given that holds the title and the
    /// body as they are published, decoded: the text `scan-text` reads line by line.
    pub const PUBLISHED: &str = "pull-request.published.txt";
    /// The file under the directory `merge` is given that holds what was sent.
    pub const MERGE_REQUEST: &str = "pull-request.merge.request.json";
    /// The file under the directory `merge` is given that holds what came back.
    pub const MERGE_RESPONSE: &str = "pull-request.merge.response.json";
    /// The file under the directory `scan_outgoing` is given that holds, for each commit a push
    /// would publish, its id and its object as the push sends it: every header (author,
    /// committer, encoding, signature, any other) and the message, bytes never converted.
    pub const MESSAGES: &str = "push.messages.txt";
    /// The file under the directory `scan_outgoing` is given that holds every tree object a push
    /// would send, entry by entry (mode, name, id), every path its commits' trees hold in full
    /// from the root (`filename:<path>`), and every blob it would send, whole
    /// ([`outgoing_text`](super::outgoing_text)).
    pub const FILES: &str = "push.files.txt";
    /// The most bytes of messages and files one scan before a push is given by default.
    pub const SCAN_LIMIT: usize = 32 << 20;
    /// The file under the directory `scan_identity` is given that holds the commit identity as
    /// a commit object carries it ([`identity_headers`](super::identity_headers)).
    pub const IDENTITY: &str = "commit-identity.txt";
    /// What `b10x-gates` 0.1.21 says, exiting 1, when it finds no policy file, the one
    /// `--policy` names or its default: it then scanned nothing.
    pub const NO_POLICY: &str = crate::scan::gates::NO_POLICY;
    /// What a push's child is started with beyond every forge child's: git may push over HTTPS
    /// alone, whatever URL the remote or the repository's configuration names.
    pub const PUSH_ENV: [(&str, &str); 1] = [("GIT_ALLOW_PROTOCOL", "https")];

    /// Over `repository` with `policy`, running every line through `runner`.
    #[must_use]
    pub fn new(
        repository: impl Into<String>,
        policy: Option<PathBuf>,
        runner: Box<dyn Runner>,
    ) -> Self {
        Self {
            repository: repository.into(),
            policy,
            remote: "origin".to_owned(),
            scan_limit: Self::SCAN_LIMIT,
            runner,
        }
    }

    /// The host the bot route serves: it pushes, writes and reads under a GitHub App.
    pub const HOST: &str = "github.com";

    /// The most check runs, and the most statuses, one read of a head's checks lists
    /// (`per_page`, GitHub's own most); a head that has more is an error, never a reading of
    /// the first page.
    pub const CHECKS_PAGE: usize = 100;

    /// The most pull requests one read of the open pull requests for a head lists
    /// (`per_page`); a full page is an error, never a reading of the first page. GitHub holds
    /// one open pull request for a head and a base.
    pub const PULLS_PAGE: usize = 100;

    /// The file a read's response is written to, in a directory of the read's own.
    pub const READ_RESPONSE: &str = "response.json";

    /// Whether the bot route serves `host`; host names are case-insensitive.
    #[must_use]
    pub fn serves(host: &str) -> bool {
        host.eq_ignore_ascii_case(Self::HOST)
    }

    /// For `tree`: the repository from its `origin` URL, which must be on [`Self::HOST`], the
    /// policy from the environment, every line run as a process. A refusal names the URL
    /// without its userinfo: a credential in it never reaches what takt prints.
    pub fn for_tree(tree: &Path) -> Result<Self> {
        let url = crate::git::remote_url(tree, "origin")
            .context("the forge `bot` needs an origin remote")?;
        let shown = without_userinfo(&url);
        let origin = repository_of(&url).ok_or_else(|| {
            anyhow!(
                "the origin remote {shown} names no <owner>/<repo> on a host; give --forge none"
            )
        })?;
        if !Self::serves(&origin.host) {
            bail!(
                "the origin remote {shown} is on {}, not {}: the bot route pushes and reads under a GitHub App; give --forge none",
                origin.host,
                Self::HOST
            );
        }
        let policy = crate::scan::gates::policy_from_env();
        Ok(Self::new(
            origin.repository,
            policy,
            Box::new(Process::default()),
        ))
    }

    /// Whether a run can reach the bot route at all, checked before its case opens, so a run
    /// that cannot is refused before any step instead of at `pr.open`: `b10x-gates` must be an
    /// executable file on takt's `PATH`, and the policy ([`crate::scan::gates::POLICY_VAR`])
    /// must be given and name a file that can be read. `gh` is not looked for: the route runs
    /// none.
    ///
    /// # Errors
    /// Naming `b10x-gates` when it is on no directory of `PATH`, or naming
    /// [`crate::scan::gates::POLICY_VAR`] when no policy is given or it names no readable file.
    pub fn preflight(&self) -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;
        let runnable = std::env::var_os("PATH")
            .iter()
            .flat_map(std::env::split_paths)
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join(PROGRAM))
            .any(|path| {
                std::fs::metadata(path)
                    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            });
        if !runnable {
            bail!(
                "the forge `bot` needs {PROGRAM} on PATH, and no directory of PATH holds it; install it, or give --forge none; no case was opened"
            );
        }
        let var = crate::scan::gates::POLICY_VAR;
        let Some(policy) = self.policy.as_deref() else {
            bail!(
                "the forge `bot` needs {var} naming the {PROGRAM} policy file, and it is not set; set it, or give --forge none; no case was opened"
            );
        };
        let shown = crate::git::shown(policy.as_os_str());
        let readable = std::fs::File::open(policy).and_then(|file| {
            if file.metadata()?.is_file() {
                Ok(())
            } else {
                Err(std::io::Error::other("not a regular file"))
            }
        });
        if let Err(error) = readable {
            bail!(
                "the forge `bot` needs {var} naming the {PROGRAM} policy file, and `{shown}` is no file that can be read ({error}); set it, or give --forge none; no case was opened"
            );
        }
        Ok(())
    }

    /// The URL every push goes to: the repository's on [`Self::HOST`], over HTTPS. A URL, not
    /// the remote's name, so a push moves no tracking ref: those are guarded from a step and a
    /// validator alike, and the scan reads them as what the repository holds.
    #[must_use]
    pub fn push_url(&self) -> String {
        format!("https://{}/{}.git", Self::HOST, self.repository)
    }

    /// The line that pushes the commit `revision` as `branch`: `<revision>:refs/heads/<branch>`,
    /// both made by takt (a commit id and the case's branch), never read from a file a step can
    /// write, and never the branch's name, which git would resolve again when it pushes; no tag
    /// and no submodule's commit with it (`--no-follow-tags`, `--recurse-submodules=no`,
    /// whatever `push.*` says), the options last, after the URL and the refspec.
    #[must_use]
    pub fn push_revision_argv(&self, branch: &str, revision: &str) -> Vec<String> {
        let url = self.push_url();
        let refspec = format!("{revision}:refs/heads/{branch}");
        [
            PROGRAM,
            "bot",
            "--",
            "push",
            &url,
            &refspec,
            "--no-follow-tags",
            "--recurse-submodules=no",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The line that reads, through the bot route, which commit the repository's `base` branch
    /// is at, writing nothing: a dry-run fetch of it onto [`Self::remote`]'s tracking ref, its
    /// porcelain report on standard output (`<flag> <old> <new> <ref>`), no tag and no
    /// submodule, the options last as on a push.
    #[must_use]
    pub fn remote_base_argv(&self, base: &str) -> Vec<String> {
        let url = self.push_url();
        let refspec = format!("refs/heads/{base}:refs/remotes/{}/{base}", self.remote);
        [
            PROGRAM,
            "bot",
            "--",
            "fetch",
            &url,
            &refspec,
            "--porcelain",
            "--dry-run",
            "--verbose",
            "--no-tags",
            "--recurse-submodules=no",
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The commit the repository's `base` branch is at, read through the bot route
    /// ([`Self::remote_base_argv`]) over HTTPS alone; `None` when the repository has no such
    /// branch. A fetch that fails otherwise, or reports nothing for the branch, is an error: what
    /// the repository holds is then not known.
    fn remote_base(&self, tree: &Path, base: &str) -> Result<Option<String>> {
        let argv = self.remote_base_argv(base);
        let exit = self.runner.run_env(tree, &argv, &Self::PUSH_ENV)?;
        if !exit.success() {
            if exit.stderr.contains("couldn't find remote ref") {
                return Ok(None);
            }
            return Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: exit.stderr.trim().to_owned(),
            }
            .into());
        }
        let target = format!("refs/remotes/{}/{base}", self.remote);
        exit.stdout
            .lines()
            .find_map(|line| {
                let fields: Vec<&str> = line.get(2..)?.split_whitespace().collect();
                match fields[..] {
                    [_, new, reference] if reference == target => Some(new.to_owned()),
                    _ => None,
                }
            })
            .map(Some)
            .ok_or_else(|| {
                anyhow!(
                    "{} reported nothing for {target}: what the repository holds of {base} is not known; nothing was pushed",
                    argv.join(" ")
                )
            })
    }

    /// The commit the repository holds as the case's `base`, what the scan need not read again:
    /// the tracking ref `refs/remotes/<remote>/<base>`, read by its full name, but only while the
    /// repository's own `base`, read through the bot route ([`Self::remote_base`]), is at the
    /// same commit. A tracking ref at another commit (stale, or written by hand) refuses the
    /// push, naming both; a repository with no `base` holds nothing of it; no tracking ref says
    /// nothing. `None` has every commit of the head scanned.
    fn held(&self, tree: &Path, base: &str) -> Result<Option<String>> {
        held_by(tree, &self.remote, base, || self.remote_base(tree, base))
    }

    /// The line that fetches the commit `object` and what it reaches through the bot route into
    /// no ref: the object id alone as the refspec, so no branch, tag or tracking ref moves, and
    /// no `FETCH_HEAD` (`--no-write-fetch-head`), no tag and no submodule, the options last as
    /// on a push. `object` is an id the repository reported, never a name git would resolve.
    #[must_use]
    pub fn fetch_object_argv(&self, object: &str) -> Vec<String> {
        let url = self.push_url();
        [
            PROGRAM,
            "bot",
            "--",
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

    /// The line a scenario run fetches the repository's trial branches with: from
    /// [`Self::push_url`], `+refs/heads/trial/*` onto [`Self::remote`]'s remote-tracking refs
    /// `refs/remotes/<remote>/trial/*`.
    #[must_use]
    pub fn trial_fetch_argv(&self) -> Vec<String> {
        let url = self.push_url();
        let refspec = format!("+refs/heads/trial/*:refs/remotes/{}/trial/*", self.remote);
        [PROGRAM, "bot", "--", "fetch", &url, &refspec]
            .map(str::to_owned)
            .to_vec()
    }

    /// The line a scenario run creates the repository's branch `trial` with, from the local
    /// one, to [`Self::remote`], reporting each ref on standard output (`--porcelain`); the
    /// lease (`--force-with-lease=refs/heads/<trial>:`) refuses it when the repository has the
    /// branch already.
    #[must_use]
    pub fn trial_push_argv(&self, trial: &str) -> Vec<String> {
        let lease = format!("--force-with-lease=refs/heads/{trial}:");
        let refspec = format!("refs/heads/{trial}:refs/heads/{trial}");
        [
            PROGRAM,
            "bot",
            "--",
            "push",
            "--porcelain",
            &lease,
            &self.remote,
            &refspec,
        ]
        .map(str::to_owned)
        .to_vec()
    }

    /// The scan before a push and a pull request, under this route's repository and policy.
    fn scanner(&self) -> Scanner<'_> {
        Scanner {
            runner: &*self.runner,
            repository: &self.repository,
            policy: self.policy.as_deref(),
        }
    }

    /// The options every `b10x-gates` line that reads the policy carries, the scan and the API
    /// writes alike: `--repository`, and `--policy` when the policy file is known.
    fn scope(&self) -> Vec<String> {
        crate::scan::gates::scope(&self.repository, self.policy.as_deref())
    }

    /// The line that scans `path` before it leaves the machine, under the same repository and
    /// policy as the API write that would send it; the path comes last.
    #[must_use]
    pub fn scan_argv(&self, path: &Path) -> Vec<String> {
        self.scan_paths_argv(&[path])
    }

    /// The line that scans a pull request before it is opened: the JSON `request` the API write
    /// sends and, last, the `published` title and body, decoded, as the forge shows them. The
    /// scanner's rules match line by line, and the JSON holds the body as one escaped line.
    #[must_use]
    pub fn scan_request_argv(&self, request: &Path, published: &Path) -> Vec<String> {
        self.scan_paths_argv(&[request, published])
    }

    fn scan_paths_argv(&self, paths: &[&Path]) -> Vec<String> {
        self.scanner().argv(paths)
    }

    /// The line of an API write: `method` on `path`, the document read from `input`, the
    /// response written to `output`.
    fn api_argv(&self, method: &str, path: String, input: &Path, output: &Path) -> Vec<String> {
        let mut argv = vec![
            PROGRAM.to_owned(),
            "api".to_owned(),
            "--method".to_owned(),
            method.to_owned(),
            "--path".to_owned(),
            path,
            "--input".to_owned(),
            input.display().to_string(),
            "--output".to_owned(),
            output.display().to_string(),
        ];
        argv.extend(self.scope());
        argv
    }

    /// The line that opens the pull request `input` describes, the response written to
    /// `output`.
    #[must_use]
    pub fn open_argv(&self, input: &Path, output: &Path) -> Vec<String> {
        self.api_argv(
            "POST",
            format!("/repos/{}/pulls", self.repository),
            input,
            output,
        )
    }

    /// The line that merges pull request `number`, the head to match read from `input`, the
    /// response written to `output`.
    #[must_use]
    pub fn merge_argv(&self, number: u64, input: &Path, output: &Path) -> Vec<String> {
        self.api_argv(
            "PUT",
            format!("/repos/{}/pulls/{number}/merge", self.repository),
            input,
            output,
        )
    }

    /// The line of an API read: `GET` on `path`, no input, the response written to `output`,
    /// under the same repository and policy as the writes.
    #[must_use]
    pub fn read_argv(&self, path: &str, output: &Path) -> Vec<String> {
        let mut argv = vec![
            PROGRAM.to_owned(),
            "api".to_owned(),
            "--method".to_owned(),
            "GET".to_owned(),
            "--path".to_owned(),
            path.to_owned(),
            "--output".to_owned(),
            output.display().to_string(),
        ];
        argv.extend(self.scope());
        argv
    }

    /// The line that reads pull request `number` of the repository
    /// (`GET /repos/<owner>/<repo>/pulls/<number>`): named, never taken from the tree's
    /// remotes, which a step may have changed.
    #[must_use]
    pub fn view_argv(&self, number: u64, output: &Path) -> Vec<String> {
        self.read_argv(
            &format!("/repos/{}/pulls/{number}", self.repository),
            output,
        )
    }

    /// The line that lists the open pull requests of the repository whose head is `branch` of
    /// the repository's owner and whose base is `base`
    /// (`GET /repos/<owner>/<repo>/pulls?state=open&head=<owner>:<branch>&base=<base>`),
    /// [`Self::PULLS_PAGE`] of them; the owner, the branch and the base percent-encoded.
    #[must_use]
    pub fn open_pulls_argv(&self, branch: &str, base: &str, output: &Path) -> Vec<String> {
        let owner = self.repository.split('/').next().unwrap_or_default();
        self.read_argv(
            &format!(
                "/repos/{}/pulls?state=open&head={}:{}&base={}&per_page={}",
                self.repository,
                encoded(owner),
                encoded(branch),
                encoded(base),
                Self::PULLS_PAGE
            ),
            output,
        )
    }

    /// The line that reads the check runs on the commit `head` of the repository
    /// (`GET /repos/<owner>/<repo>/commits/<head>/check-runs`), [`Self::CHECKS_PAGE`] of them.
    #[must_use]
    pub fn check_runs_argv(&self, head: &str, output: &Path) -> Vec<String> {
        self.read_argv(
            &format!(
                "/repos/{}/commits/{head}/check-runs?per_page={}",
                self.repository,
                Self::CHECKS_PAGE
            ),
            output,
        )
    }

    /// The line that reads the combined status of the commit `head` of the repository
    /// (`GET /repos/<owner>/<repo>/commits/<head>/status`), [`Self::CHECKS_PAGE`] statuses.
    #[must_use]
    pub fn status_argv(&self, head: &str, output: &Path) -> Vec<String> {
        self.read_argv(
            &format!(
                "/repos/{}/commits/{head}/status?per_page={}",
                self.repository,
                Self::CHECKS_PAGE
            ),
            output,
        )
    }

    /// Runs the read `argv_of(output)` in `tree`, `output` [`Self::READ_RESPONSE`] in a new
    /// directory of its own under the temporary directory, readable by its owner alone
    /// (`b10x-gates` refuses to write into a directory another user can write) and removed
    /// after; the response read as JSON. A read that does not exit 0, or writes no JSON, is an
    /// error naming the line, never a reading.
    fn read(
        &self,
        tree: &Path,
        argv_of: impl FnOnce(&Path) -> Vec<String>,
    ) -> Result<(Vec<String>, serde_json::Value)> {
        let dir = ReadDir::new()?;
        let output = dir.0.join(Self::READ_RESPONSE);
        let argv = argv_of(&output);
        self.succeed(tree, &argv)?;
        let text = std::fs::read_to_string(&output)
            .with_context(|| format!("{} wrote no response", argv.join(" ")))?;
        let document = serde_json::from_str(&text)
            .with_context(|| format!("{} wrote no JSON", argv.join(" ")))?;
        Ok((argv, document))
    }

    /// What each check run the check runs `document` lists says, read as [`Checks`].
    fn check_runs(argv: &[String], document: &serde_json::Value) -> Result<Vec<Checks>> {
        let runs = listed(argv, document, "check_runs")?;
        runs.iter()
            .map(|run| {
                let status = run["status"].as_str();
                Ok(match status {
                    Some("completed") => match run["conclusion"].as_str() {
                        Some("success" | "neutral" | "skipped") => Checks::Pass,
                        Some(
                            "failure" | "cancelled" | "timed_out" | "action_required"
                            | "startup_failure" | "stale",
                        ) => Checks::Fail,
                        other => bail!(
                            "{} reported a completed check run concluding {other:?}",
                            argv.join(" ")
                        ),
                    },
                    Some("queued" | "in_progress" | "waiting" | "requested" | "pending") => {
                        Checks::Pending
                    }
                    other => bail!(
                        "{} reported a check run in the status {other:?}",
                        argv.join(" ")
                    ),
                })
            })
            .collect()
    }

    /// What the statuses of the combined status `document` say, read as [`Checks`]; its
    /// overall `state` is not read, since it is `pending` for a head with no status at all.
    fn statuses(argv: &[String], document: &serde_json::Value) -> Result<Vec<Checks>> {
        let statuses = listed(argv, document, "statuses")?;
        statuses
            .iter()
            .map(|status| {
                Ok(match status["state"].as_str() {
                    Some("success") => Checks::Pass,
                    Some("failure" | "error") => Checks::Fail,
                    Some("pending") => Checks::Pending,
                    other => bail!(
                        "{} reported a status in the state {other:?}",
                        argv.join(" ")
                    ),
                })
            })
            .collect()
    }

    /// Runs `argv` in `tree`; a program that did not exit 0 is an error naming the line and
    /// what it said.
    fn succeed(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        succeed(&*self.runner, tree, argv)
    }

    /// Merges pull request `number` at `head` through the API ([`Self::merge_argv`]), the
    /// request kept as [`Self::MERGE_REQUEST`] and the response as [`Self::MERGE_RESPONSE`]
    /// under `dir`; with `message`, the merge commit's `commit_title` and `commit_message`. No
    /// `merge_method` is sent: the forge's default, a merge commit.
    fn merge_saying(
        &self,
        tree: &Path,
        dir: &Path,
        number: u64,
        head: &str,
        message: Option<&MergeMessage>,
    ) -> Result<()> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let input = dir.join(Self::MERGE_REQUEST);
        let output = fresh(dir.join(Self::MERGE_RESPONSE))?;
        // The head to match: the API refuses the merge when the pull request's head is no longer
        // this commit, so what is merged is what was read and recorded, never a later push.
        let mut document = serde_json::json!({ "sha": head });
        if let Some(message) = message {
            document["commit_title"] = message.title.as_str().into();
            document["commit_message"] = message.body.as_str().into();
        }
        let bytes = serde_json::to_vec_pretty(&document).context("serialize the merge request")?;
        std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
        let argv = self.merge_argv(number, &input, &output);
        self.succeed(tree, &argv)?;
        let text = std::fs::read_to_string(&output)
            .with_context(|| format!("read the response {}", output.display()))?;
        let response: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("read the response {}", output.display()))?;
        if response["merged"] != true {
            bail!(
                "{} did not merge pull request #{number}: {}",
                argv.join(" "),
                response["message"]
                    .as_str()
                    .unwrap_or("the response does not say `merged: true`")
            );
        }
        Ok(())
    }

    /// Runs the push line `argv` in `tree` over HTTPS alone ([`Self::PUSH_ENV`]). A push git
    /// refuses because a URL rewrite in the repository's configuration (`insteadOf`,
    /// `pushInsteadOf`) would send it over another transport is refused naming the URL pushed to
    /// and that transport.
    fn push_line(&self, tree: &Path, argv: &[String]) -> Result<()> {
        push_over_https(&*self.runner, tree, argv, &self.push_url(), "the bot route")
    }
}

impl Forge for BotRoute {
    fn name(&self) -> &str {
        "bot"
    }

    /// Pushes the commit `branch` is at, by its id ([`Self::push_revision`]).
    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let at = exact_ref(tree, &format!("refs/heads/{branch}"))
            .ok_or_else(|| anyhow!("{branch} names no commit; nothing was pushed"))?;
        self.push_revision(tree, branch, &at)
    }

    /// Pushes the commit `revision` as `branch`, always by its id, the one scanned
    /// ([`Self::push_revision_argv`]).
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        self.push_line(tree, &self.push_revision_argv(branch, revision))
    }

    /// One `scan-text` line over everything pushing `head` would publish:
    /// the pull request, when one is opened (the JSON request and the title and body as
    /// published), and the objects the push sends that [`Self::remote`] does not hold
    /// ([`published_commits`](super::published_commits),
    /// [`outgoing_text`](super::outgoing_text)): every commit object raw ([`Self::MESSAGES`]),
    /// every tree's entry names, every path in full and every blob ([`Self::FILES`]), all written
    /// under `dir`. A head that is not a commit is refused first ([`head_is_a_commit`]). Once git
    /// prints more than [`Self::scan_limit`] bytes for them, it is read no further and the push
    /// is refused, nothing written or scanned. The bot route scans so whatever the run's scan
    /// adapter is ([`crate::scan::Scan`]): it is the gates adapter's own forge.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        head_is_a_commit(tree, outgoing.branch, outgoing.head)?;
        let held = self.held(tree, outgoing.base)?;
        self.scanner().outgoing(
            tree,
            dir,
            outgoing,
            held.as_deref(),
            self.scan_limit,
            write_request,
            "pull request",
        )
    }

    /// One `scan-text` line, under the repository and policy of the scan before a push
    /// ([`Self::scan_argv`]), over [`Self::IDENTITY`] written under `dir`: the identity as the
    /// `author` and `committer` headers of a commit object
    /// ([`identity_headers`](super::identity_headers)). A `b10x-gates` that is not on the
    /// program path, or that finds no policy file
    /// ([`Self::NO_POLICY`]), scanned nothing: [`IdentityScan::NotScanned`], naming what it said.
    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &crate::git::CommitIdentity,
    ) -> Result<IdentityScan> {
        self.scanner()
            .identity(tree, dir, identity, "the bot route")
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::RESPONSE))?;
        let (input, published) = write_request(dir, request)?;
        self.succeed(tree, &self.scan_request_argv(&input, &published))
            .with_context(|| {
                format!(
                    "the pull request for {branch} did not pass scan-text over {} and was not opened; its title and body stayed on this machine, but the branch {branch} was pushed before this scan",
                    published.display(),
                    branch = request.branch
                )
            })?;
        self.succeed(tree, &self.open_argv(&input, &output))?;
        let text = std::fs::read_to_string(&output)
            .with_context(|| format!("read the response {}", output.display()))?;
        let response: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("read the response {}", output.display()))?;
        response["number"]
            .as_u64()
            .ok_or_else(|| anyhow!("the response {} has no number", output.display()))
    }

    /// `GET /repos/<owner>/<repo>/pulls` for the open pull requests of `branch` into `base`
    /// ([`Self::open_pulls_argv`]). Only an entry the answer itself reports open, with the head
    /// `branch` and the base `base`, and of this repository when it names the head's repository,
    /// is taken: the query is the forge's to honour, and its answer is checked. An answer that is
    /// not a list, a full page, an entry taken with no number, or two different numbers taken,
    /// is an error, never none.
    fn find_open_pull_request(&self, tree: &Path, branch: &str, base: &str) -> Result<Option<u64>> {
        let (argv, document) =
            self.read(tree, |output| self.open_pulls_argv(branch, base, output))?;
        let listed = document
            .as_array()
            .ok_or_else(|| anyhow!("{} wrote no list of pull requests", argv.join(" ")))?;
        if listed.len() >= Self::PULLS_PAGE {
            bail!(
                "{} lists {} pull requests, a full page: the rest are not read, so whether {branch} has one open is not known",
                argv.join(" "),
                listed.len()
            );
        }
        let text = |pull: &serde_json::Value, pointer: &str| {
            pull.pointer(pointer)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let mut found = None;
        for pull in listed {
            let ours = text(pull, "/state").as_deref() == Some("open")
                && text(pull, "/head/ref").as_deref() == Some(branch)
                && text(pull, "/base/ref").as_deref() == Some(base)
                && text(pull, "/head/repo/full_name")
                    .is_none_or(|full| full.eq_ignore_ascii_case(&self.repository));
            if !ours {
                continue;
            }
            let number = pull["number"].as_u64().ok_or_else(|| {
                anyhow!(
                    "{} lists an open pull request of {branch} with no number",
                    argv.join(" ")
                )
            })?;
            if let Some(other) = found.replace(number)
                && other != number
            {
                bail!(
                    "{} lists two open pull requests of {branch} into {base}, #{other} and #{number}",
                    argv.join(" ")
                );
            }
        }
        Ok(found)
    }

    /// `GET /repos/<owner>/<repo>/pulls/<number>` ([`Self::view_argv`]): `open` is open, `closed`
    /// is merged when `merged` is true and closed otherwise; the head's branch and commit and
    /// the base's branch must all be there.
    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let (argv, document) = self.read(tree, |output| self.view_argv(number, output))?;
        let merged = document["merged"].as_bool();
        let state = match (document["state"].as_str(), merged) {
            (Some("open"), _) => PullRequestState::Open,
            (Some("closed"), Some(true)) => PullRequestState::Merged,
            (Some("closed"), Some(false)) => PullRequestState::Closed,
            (state, merged) => bail!(
                "{} reported the state {state:?} with merged {merged:?}, not open, or closed and merged or not",
                argv.join(" ")
            ),
        };
        let field = |pointer: &str| -> Result<String> {
            document
                .pointer(pointer)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{} reported no {pointer}", argv.join(" ")))
        };
        Ok(PullRequestView {
            state,
            head: Head {
                branch: field("/head/ref")?,
                sha: field("/head/sha")?,
            },
            base: field("/base/ref")?,
        })
    }

    /// The checks on the head of pull request `number`, as the pull request reports it
    /// ([`Self::pull_request`]): its check runs ([`Self::check_runs_argv`]) and its statuses
    /// ([`Self::status_argv`]). One failed, cancelled or timed-out run, or one failing status,
    /// fails; else one queued or running run, or one pending status, or none of either, is
    /// pending; else every one passed. A head that is no object id is not read.
    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
        let head = self.pull_request(tree, number)?.head.sha;
        if !is_object_id(&head) {
            bail!(
                "pull request #{number} of {} reports its head at `{}`, which is no object id; its checks were not read",
                self.repository,
                head.escape_debug()
            );
        }
        let (argv, runs) = self.read(tree, |output| self.check_runs_argv(&head, output))?;
        let mut checks = Self::check_runs(&argv, &runs)?;
        let (argv, status) = self.read(tree, |output| self.status_argv(&head, output))?;
        checks.extend(Self::statuses(&argv, &status)?);
        Ok(if checks.contains(&Checks::Fail) {
            Checks::Fail
        } else if checks.is_empty() || checks.contains(&Checks::Pending) {
            Checks::Pending
        } else {
            Checks::Pass
        })
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        self.merge_saying(tree, dir, number, head, None)
    }

    /// Merges as [`Self::merge`] does, the merge commit's title and body `message`, sent as the
    /// request's `commit_title` and `commit_message`; the merge method stays the forge's
    /// default, a merge commit.
    fn merge_with(
        &self,
        tree: &Path,
        dir: &Path,
        number: u64,
        head: &str,
        message: &MergeMessage,
    ) -> Result<()> {
        self.merge_saying(tree, dir, number, head, Some(message))
    }

    /// The repository's own `base`, read through the bot route's dry-run fetch
    /// ([`Self::remote_base`]), then its commit fetched into no ref
    /// ([`Self::fetch_object_argv`]): a tip newer than anything the tree's repository holds is
    /// then held, so the allow-list and the delegation record at it can be read, and no ref a
    /// step or the tree sees has moved. A repository with no such branch, an answer that is no
    /// object id, or a fetch that fails is the error.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        let tip = self.remote_base(tree, base)?.ok_or_else(|| {
            anyhow!(
                "the repository {} has no branch {base} to merge into",
                self.repository
            )
        })?;
        if !is_object_id(&tip) {
            bail!(
                "the repository {} reports {base} at `{}`, which is no object id",
                self.repository,
                tip.escape_debug()
            );
        }
        let argv = self.fetch_object_argv(&tip);
        let exit = self.runner.run_env(tree, &argv, &Self::PUSH_ENV)?;
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

/// The list `key` of the response `document` the read `argv` wrote, every entry of it: a
/// `total_count` above the entries listed (a head with more than one page) is an error, never
/// a reading of part of them.
fn listed<'a>(
    argv: &[String],
    document: &'a serde_json::Value,
    key: &str,
) -> Result<&'a [serde_json::Value]> {
    let entries = document[key]
        .as_array()
        .ok_or_else(|| anyhow!("{} wrote no list {key}", argv.join(" ")))?;
    let total = document["total_count"]
        .as_u64()
        .ok_or_else(|| anyhow!("{} wrote no total_count", argv.join(" ")))?;
    if usize::try_from(total).map_or(true, |total| total > entries.len()) {
        bail!(
            "{} counts {total} {key} and lists {}: the rest are not read, so the checks are not known",
            argv.join(" "),
            entries.len()
        );
    }
    Ok(entries)
}

/// A directory one read's response is written into: new, under the temporary directory,
/// `0700`; removed on drop.
struct ReadDir(PathBuf);

impl ReadDir {
    fn new() -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static MADE: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos());
        let dir = std::env::temp_dir().join(format!(
            "takt-bot-read-{}-{}-{nanos}",
            std::process::id(),
            MADE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&dir)
            .with_context(|| format!("create a directory for a read at {}", dir.display()))?;
        Ok(Self(dir))
    }
}

impl Drop for ReadDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use anyhow::Result;

    use super::{BotRoute, Exit, Forge as _, IdentityScan, Runner};
    use crate::git::CommitIdentity;

    /// What the scan double answers.
    #[derive(Clone, Copy)]
    enum Answer {
        Pass,
        Refuse,
        NoPolicy,
        NotInstalled,
    }

    /// Records each line and the text of the file it names last, and answers `answer`.
    struct Scan {
        answer: Answer,
        seen: Rc<RefCell<Vec<(String, String)>>>,
    }

    impl Runner for Scan {
        fn run(&self, _: &Path, argv: &[String]) -> Result<Exit> {
            let read = argv
                .last()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .unwrap_or_default();
            self.seen.borrow_mut().push((argv.join(" "), read));
            let exit = |code, stderr: &str| Exit {
                code: Some(code),
                stdout: String::new(),
                stderr: stderr.to_owned(),
            };
            match self.answer {
                Answer::Pass => Ok(exit(0, "")),
                Answer::Refuse => Ok(exit(
                    1,
                    "b10x-gates: commit-identity.txt breaks 1 private rule(s): line 1 person-name; private details remain local\n",
                )),
                Answer::NoPolicy => Ok(exit(
                    1,
                    "b10x-gates: protected file /var/empty/.config/b10x/gates/policy.json unavailable: not found\n",
                )),
                Answer::NotInstalled => Err(anyhow::Error::new(std::io::Error::from(
                    std::io::ErrorKind::NotFound,
                ))
                .context("run b10x-gates")),
            }
        }

        fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> Result<Exit> {
            self.run(tree, argv)
        }
    }

    /// A scratch directory of this test's own, removed on drop.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scan(name: &str, answer: Answer) -> (Result<IdentityScan>, Vec<(String, String)>, Scratch) {
        let dir = std::env::temp_dir().join(format!("takt-forge-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let route = BotRoute::new(
            "owner/repo",
            Some(PathBuf::from("/policies/gates.json")),
            Box::new(Scan {
                answer,
                seen: Rc::clone(&seen),
            }),
        );
        let identity = CommitIdentity::new("Some One", "one@example.invalid").expect("an identity");
        let scanned = route.scan_identity(Path::new("."), &dir, &identity);
        let seen = seen.borrow().clone();
        (scanned, seen, Scratch(dir))
    }

    /// The identity is scanned by one `scan-text` line under the push scan's repository and
    /// policy, over the identity as a commit object's `author` and `committer` headers; a scan
    /// that exits 0 passes it.
    #[test]
    fn an_identity_is_scanned_as_a_commit_carries_it() {
        let (scanned, seen, scratch) = scan("pass", Answer::Pass);
        assert_eq!(scanned.expect("scanned"), IdentityScan::Passed);
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(
            seen[0].0,
            format!(
                "b10x-gates scan-text --repository owner/repo --policy /policies/gates.json {}",
                scratch.0.join(BotRoute::IDENTITY).display()
            )
        );
        assert_eq!(
            seen[0].1,
            "author Some One <one@example.invalid> 0 +0000\ncommitter Some One <one@example.invalid> 0 +0000\n"
        );
    }

    /// A refusal is an error naming the identity and the scan's own words.
    #[test]
    fn a_refused_identity_is_an_error_naming_the_scans_refusal() {
        let (scanned, _, _scratch) = scan("refuse", Answer::Refuse);
        let error = format!("{:#}", scanned.expect_err("refused"));
        for named in [
            "Some One <one@example.invalid>",
            "did not pass scan-text",
            "breaks 1 private rule(s): line 1 person-name",
        ] {
            assert!(error.contains(named), "{named}: {error}");
        }
    }

    /// Answers a read with `{"state":"open",…}`, recording the mode of the directory `--output`
    /// is in and that directory.
    struct Reads {
        seen: Rc<RefCell<Vec<(String, u32, PathBuf)>>>,
    }

    impl Runner for Reads {
        fn run(&self, _: &Path, argv: &[String]) -> Result<Exit> {
            use std::os::unix::fs::PermissionsExt as _;
            let at = argv.iter().position(|a| a == "--output").expect("--output");
            let output = PathBuf::from(&argv[at + 1]);
            let dir = output.parent().expect("a directory").to_path_buf();
            let mode = std::fs::metadata(&dir)
                .expect("the directory")
                .permissions()
                .mode();
            self.seen
                .borrow_mut()
                .push((argv.join(" "), mode & 0o7777, dir));
            std::fs::write(
                &output,
                r#"{"state":"open","merged":false,"head":{"ref":"b","sha":"c"},"base":{"ref":"main"}}"#,
            )?;
            Ok(Exit {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> Result<Exit> {
            self.run(tree, argv)
        }
    }

    /// A read's response is written into a new directory of its own, `0700`, which
    /// `b10x-gates` requires, and the directory is gone once the response is read.
    #[test]
    fn a_read_writes_into_a_private_directory_removed_after() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let route = BotRoute::new(
            "owner/repo",
            None,
            Box::new(Reads {
                seen: Rc::clone(&seen),
            }),
        );
        let view = route.pull_request(Path::new("."), 7).expect("reads");
        assert_eq!(view.base, "main");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1, "{seen:?}");
        let (line, mode, dir) = &seen[0];
        assert!(
            line.starts_with(
                "b10x-gates api --method GET --path /repos/owner/repo/pulls/7 --output "
            ),
            "{line}"
        );
        assert!(line.ends_with(" --repository owner/repo"), "{line}");
        assert_eq!(*mode, 0o700, "{}", dir.display());
        assert!(dir.starts_with(std::env::temp_dir()), "{}", dir.display());
        assert!(!dir.exists(), "{} is removed after the read", dir.display());
    }

    /// No policy file, or no `b10x-gates` to run, scanned nothing: not scanned, never passed.
    #[test]
    fn no_policy_or_no_scanner_is_not_scanned() {
        for (name, answer, said) in [
            ("no-policy", Answer::NoPolicy, "unavailable: not found"),
            ("not-installed", Answer::NotInstalled, "run b10x-gates"),
        ] {
            let (scanned, _, _scratch) = scan(name, answer);
            match scanned.expect(name) {
                IdentityScan::NotScanned(why) => assert!(why.contains(said), "{name}: {why}"),
                IdentityScan::Passed => panic!("{name}: reported as passed"),
            }
        }
    }
}
