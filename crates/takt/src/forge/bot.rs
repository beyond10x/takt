//! The `bot` forge adapter, the organization's bot route through `b10x-gates`: the bot credential
//! never enters takt. `b10x-gates bot` pushes under the bot the commit scanned, by its id, to the
//! repository's HTTPS URL and over HTTPS only, and fetches through the same route;
//! `b10x-gates api` writes through JSON files and never a token in argv; the reads go through
//! `gh`; and before any of it leaves the machine `b10x-gates scan-text` reads what a push would
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

use super::{
    Checks, Exit, Failed, Forge, Head, IdentityScan, MergeMessage, Outgoing, Process, PullRequest,
    PullRequestState, PullRequestView, Runner, Scanner, exact_ref, fresh, head_is_a_commit,
    held_by, is_object_id, push_over_https, reading_of, repository_of, succeed, without_userinfo,
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
/// JSON files, and read-only `gh pr view` and `gh pr checks` report its state and checks. The
/// scan and every API write name the repository and, when one is known, the policy file as
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

    /// The host the bot route serves: it pushes under a GitHub App and reads through `gh`.
    pub const HOST: &str = "github.com";

    /// What `gh pr checks` says on stderr, exiting 1 with nothing on stdout, for a head no
    /// check has been reported for yet: `gh` 2.100.0's message as read from its binary, not
    /// verified against a live pull request.
    pub const NO_CHECKS_REPORTED: &str = "no checks reported";

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
                "the origin remote {shown} is on {}, not {}: the bot route pushes under a GitHub App and reads through gh; give --forge none",
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

    /// The line that reads the state of pull request `number` of the repository: named, never
    /// taken from the tree's remotes, which a step may have changed.
    #[must_use]
    pub fn view_argv(&self, number: u64) -> Vec<String> {
        vec![
            "gh".to_owned(),
            "pr".to_owned(),
            "view".to_owned(),
            number.to_string(),
            "--json".to_owned(),
            "state,mergedAt,headRefName,headRefOid,baseRefName".to_owned(),
            "--repo".to_owned(),
            self.repository.clone(),
        ]
    }

    /// The line that reads the checks on the head of pull request `number` of the repository:
    /// named, never taken from the tree's remotes.
    #[must_use]
    pub fn checks_argv(&self, number: u64) -> Vec<String> {
        vec![
            "gh".to_owned(),
            "pr".to_owned(),
            "checks".to_owned(),
            number.to_string(),
            "--json".to_owned(),
            "bucket".to_owned(),
            "--repo".to_owned(),
            self.repository.clone(),
        ]
    }

    /// Runs `argv` in `tree`; a program that did not exit 0 is an error naming the line and
    /// what it said.
    fn succeed(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        succeed(&*self.runner, tree, argv)
    }

    /// Runs `argv` in `tree` and reads what it printed as JSON; anything else is an error
    /// naming the line and what the program said, never a reading.
    fn read_json(&self, tree: &Path, argv: &[String]) -> Result<serde_json::Value> {
        let exit = self.runner.run(tree, argv)?;
        reading_of(argv, &exit)
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

    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let argv = self.view_argv(number);
        let document = self.read_json(tree, &argv)?;
        let state = match document["state"].as_str() {
            Some("OPEN") => PullRequestState::Open,
            Some("MERGED") => PullRequestState::Merged,
            Some("CLOSED") => PullRequestState::Closed,
            other => bail!(
                "{} reported the state {other:?}, not OPEN, MERGED or CLOSED",
                argv.join(" ")
            ),
        };
        let field = |name: &str| -> Result<String> {
            document[name]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{} reported no {name}", argv.join(" ")))
        };
        Ok(PullRequestView {
            state,
            head: Head {
                branch: field("headRefName")?,
                sha: field("headRefOid")?,
            },
            base: field("baseRefName")?,
        })
    }

    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
        let argv = self.checks_argv(number);
        let exit = self.runner.run(tree, &argv)?;
        // A head no check has been reported for yet, the state of every pull request in the
        // seconds after its push: pending, not an error that ends the run. Anything else that
        // is not a reading stays an error.
        if !exit.success()
            && exit.stdout.trim().is_empty()
            && exit.stderr.contains(Self::NO_CHECKS_REPORTED)
        {
            return Ok(Checks::Pending);
        }
        let document = reading_of(&argv, &exit)?;
        let checks = document
            .as_array()
            .ok_or_else(|| anyhow!("{} printed no list of checks", argv.join(" ")))?;
        let mut pending = checks.is_empty();
        let mut failed = false;
        for check in checks {
            match check["bucket"].as_str() {
                Some("pass" | "skipping") => {}
                Some("fail" | "cancel") => failed = true,
                Some("pending") => pending = true,
                other => bail!(
                    "{} reported a check in the bucket {other:?}",
                    argv.join(" ")
                ),
            }
        }
        Ok(if failed {
            Checks::Fail
        } else if pending {
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
