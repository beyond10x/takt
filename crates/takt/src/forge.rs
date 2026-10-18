//! The `Forge` seam: the repository host behind a trait, one adapter that goes through the
//! organization's bot route, and an in-memory double for the tests. The bot credential never
//! enters takt: `b10x-gates bot` pushes under the bot, `b10x-gates api` writes through JSON
//! files and never a token in argv, the reads go through `gh`, and a pull-request body passes
//! `b10x-gates scan-text` before it leaves the machine. A validator turns what the forge
//! reports into evidence; the forge itself writes no record.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, anyhow, bail};

/// The environment variable that names the `b10x-gates` policy file.
pub const POLICY_VAR: &str = "B10X_GATES_POLICY";

/// What a pull request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PullRequestState {
    #[default]
    Open,
    Merged,
    Closed,
}

impl PullRequestState {
    /// The state as a record's result: `open`, `merged` or `closed`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

impl fmt::Display for PullRequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the checks on a pull request's head report: every check passed, one failed, or some
/// are still to come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Checks {
    Pass,
    Fail,
    #[default]
    Pending,
}

impl Checks {
    /// The checks as a record's result: `pass`, `fail` or `pending`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Pending => "pending",
        }
    }
}

impl fmt::Display for Checks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a pull request is opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// The head branch.
    pub branch: String,
    /// The base branch.
    pub base: String,
    /// The title.
    pub title: String,
    /// The body.
    pub body: String,
}

/// A pull request's head: the branch and the commit it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The head branch.
    pub branch: String,
    /// The commit the head branch names.
    pub sha: String,
}

/// What the forge reports about a pull request: its state and its head. A validator checks
/// the head against the case's branch and revision before it trusts a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestView {
    /// The state.
    pub state: PullRequestState,
    /// The head.
    pub head: Head,
}

/// What an `origin` URL names: the host and the `<owner>/<repo>` on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The host, `github.com` for the bot route.
    pub host: String,
    /// `<owner>/<repo>`.
    pub repository: String,
}

/// The repository host: pushes a branch, opens its pull request, and reports the pull
/// request's state and head and the checks on its head.
pub trait Forge {
    /// The adapter's name.
    fn name(&self) -> &str;
    /// Pushes `branch` of `tree` to the repository.
    fn push(&self, tree: &Path, branch: &str) -> Result<()>;
    /// Opens the pull request `request` describes; `dir` is where the adapter keeps what it
    /// sent and what came back, when it keeps anything. The pull request's number.
    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64>;
    /// The state and the head of pull request `number`.
    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView>;
    /// The checks on the head of pull request `number`.
    fn checks(&self, tree: &Path, number: u64) -> Result<Checks>;
}

/// What a program the adapter ran answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    /// The exit code; `None` when a signal ended the program.
    pub code: Option<i32>,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

impl Exit {
    /// Whether the program exited 0.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs one argument line, the program first, in a tree. The adapter builds every line it
/// runs and hands it here, so a test can read the lines without running them.
pub trait Runner {
    /// Runs `argv` in `tree`.
    fn run(&self, tree: &Path, argv: &[String]) -> Result<Exit>;
}

/// The runner that runs the line as a process, without a shell.
#[derive(Debug, Clone, Copy, Default)]
pub struct Process;

impl Runner for Process {
    fn run(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| anyhow!("an empty argument line"))?;
        let output = Command::new(program)
            .args(args)
            .current_dir(tree)
            .output()
            .with_context(|| format!("run {} in {}", argv.join(" "), tree.display()))?;
        Ok(Exit {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// The adapter over the organization's bot route: `b10x-gates bot -- push` pushes the branch,
/// `b10x-gates scan-text` checks the pull request before it leaves the machine, `b10x-gates
/// api` opens it with JSON files, and read-only `gh pr view` and `gh pr checks` report its
/// state and checks. The policy file is `--policy` when one is known; `b10x-gates` reads its
/// own environment otherwise.
pub struct BotRoute {
    /// `<owner>/<repo>`, from the `origin` URL.
    pub repository: String,
    /// The `b10x-gates` policy file; none leaves it to the program's environment.
    pub policy: Option<PathBuf>,
    /// The remote the branch is pushed to.
    pub remote: String,
    runner: Box<dyn Runner>,
}

impl fmt::Debug for BotRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BotRoute")
            .field("repository", &self.repository)
            .field("policy", &self.policy)
            .field("remote", &self.remote)
            .finish_non_exhaustive()
    }
}

impl BotRoute {
    /// The file under the directory `open_pull_request` is given that holds what was sent.
    pub const REQUEST: &str = "pull-request.request.json";
    /// The file under the directory `open_pull_request` is given that holds what came back.
    pub const RESPONSE: &str = "pull-request.response.json";

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
        let policy = std::env::var_os(POLICY_VAR).map(PathBuf::from);
        Ok(Self::new(origin.repository, policy, Box::new(Process)))
    }

    /// The line that pushes `branch`.
    #[must_use]
    pub fn push_argv(&self, branch: &str) -> Vec<String> {
        ["b10x-gates", "bot", "--", "push", &self.remote, branch]
            .map(str::to_owned)
            .to_vec()
    }

    /// The line that scans `path` before it leaves the machine.
    #[must_use]
    pub fn scan_argv(&self, path: &Path) -> Vec<String> {
        vec![
            "b10x-gates".to_owned(),
            "scan-text".to_owned(),
            path.display().to_string(),
        ]
    }

    /// The line that opens the pull request `input` describes, the response written to
    /// `output`.
    #[must_use]
    pub fn open_argv(&self, input: &Path, output: &Path) -> Vec<String> {
        let mut argv = vec![
            "b10x-gates".to_owned(),
            "api".to_owned(),
            "--method".to_owned(),
            "POST".to_owned(),
            "--path".to_owned(),
            format!("/repos/{}/pulls", self.repository),
            "--input".to_owned(),
            input.display().to_string(),
            "--output".to_owned(),
            output.display().to_string(),
            "--repository".to_owned(),
            self.repository.clone(),
        ];
        if let Some(policy) = &self.policy {
            argv.push("--policy".to_owned());
            argv.push(policy.display().to_string());
        }
        argv
    }

    /// The line that reads the state of pull request `number`.
    #[must_use]
    pub fn view_argv(&self, number: u64) -> Vec<String> {
        vec![
            "gh".to_owned(),
            "pr".to_owned(),
            "view".to_owned(),
            number.to_string(),
            "--json".to_owned(),
            "state,mergedAt,headRefName,headRefOid".to_owned(),
        ]
    }

    /// The line that reads the checks on the head of pull request `number`.
    #[must_use]
    pub fn checks_argv(&self, number: u64) -> Vec<String> {
        vec![
            "gh".to_owned(),
            "pr".to_owned(),
            "checks".to_owned(),
            number.to_string(),
            "--json".to_owned(),
            "bucket".to_owned(),
        ]
    }

    /// Runs `argv` in `tree`; a program that did not exit 0 is an error naming the line and
    /// what it said.
    fn succeed(&self, tree: &Path, argv: &[String]) -> Result<Exit> {
        let exit = self.runner.run(tree, argv)?;
        if !exit.success() {
            bail!(
                "{} exited {}: {}",
                argv.join(" "),
                exit.code
                    .map_or("on a signal".to_owned(), |code| code.to_string()),
                exit.stderr.trim()
            );
        }
        Ok(exit)
    }

    /// Runs `argv` in `tree` and reads what it printed as JSON; anything else is an error
    /// naming the line and what the program said, never a reading.
    fn read_json(&self, tree: &Path, argv: &[String]) -> Result<serde_json::Value> {
        let exit = self.runner.run(tree, argv)?;
        reading_of(argv, &exit)
    }
}

/// What `argv` printed, as JSON; anything else is an error naming the line and what the
/// program said, never a reading.
fn reading_of(argv: &[String], exit: &Exit) -> Result<serde_json::Value> {
    serde_json::from_str(&exit.stdout).map_err(|error| {
        anyhow!(
            "{} exited {} and printed no reading ({error}): {}",
            argv.join(" "),
            exit.code
                .map_or("on a signal".to_owned(), |code| code.to_string()),
            exit.stderr.trim()
        )
    })
}

impl Forge for BotRoute {
    fn name(&self) -> &str {
        "bot"
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        self.succeed(tree, &self.push_argv(branch))?;
        Ok(())
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let input = dir.join(Self::REQUEST);
        let output = dir.join(Self::RESPONSE);
        let document = serde_json::json!({
            "title": request.title,
            "head": request.branch,
            "base": request.base,
            "body": request.body,
        });
        let bytes = serde_json::to_vec_pretty(&document).context("serialize the pull request")?;
        std::fs::write(&input, bytes).with_context(|| format!("write {}", input.display()))?;
        self.succeed(tree, &self.scan_argv(&input))
            .context("the pull request did not pass scan-text; nothing left the machine")?;
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
}

/// The host and the `<owner>/<repo>` an `origin` URL names, in the forms git writes them:
/// `git@host:owner/repo.git`, `ssh://git@host[:port]/owner/repo.git`, `https://host/owner/repo`.
/// A local path, or a URL with no host (`file://`), names none.
#[must_use]
pub fn repository_of(url: &str) -> Option<Origin> {
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
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.rsplit('/');
    let repo = parts.next()?;
    let owner = parts.next()?;
    (!host.is_empty() && !owner.is_empty() && !repo.is_empty()).then(|| Origin {
        host: host.to_owned(),
        repository: format!("{owner}/{repo}"),
    })
}

/// `url` with its userinfo (`user:password@`) removed, what takt prints of a remote URL: a
/// credential a checkout carries in its origin never reaches an error or a line of output.
#[must_use]
pub fn without_userinfo(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, None), |(authority, path)| (authority, Some(path)));
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        match path {
            Some(path) => format!("{scheme}://{host}/{path}"),
            None => format!("{scheme}://{host}"),
        }
    } else {
        match url.split_once(':') {
            Some((authority, path)) if authority.contains('@') => {
                let host = authority
                    .rsplit_once('@')
                    .map_or(authority, |(_, host)| host);
                format!("{host}:{path}")
            }
            _ => url.to_owned(),
        }
    }
}

/// The in-memory double: answers what it is set to and logs every call, for the tests. A pull
/// request it opened has the head it was opened for, at the revision last pushed for that
/// branch (the tree's `HEAD` at the push), unless `head` says otherwise.
#[derive(Debug, Default)]
pub struct Memory {
    /// What `checks` answers for a pull request it opened.
    pub checks: Checks,
    /// The state `pull_request` answers for a pull request it opened.
    pub state: PullRequestState,
    /// The head `pull_request` answers instead of the one it was opened for, when set.
    pub head: Option<Head>,
    /// Every call, in order: `push <branch>`, `open <branch> #<n>`, `view #<n>`, `checks #<n>`.
    pub calls: RefCell<Vec<String>>,
    /// The pull requests opened, numbered from 1 in this order.
    pub opened: RefCell<Vec<PullRequest>>,
    /// The revision last pushed, by branch.
    pub pushed: RefCell<BTreeMap<String, String>>,
}

impl Memory {
    /// The index of pull request `number` among the opened ones.
    fn index(&self, number: u64) -> Result<usize> {
        let opened = self.opened.borrow().len();
        match usize::try_from(number) {
            Ok(n) if n >= 1 && n <= opened => Ok(n - 1),
            _ => bail!("no pull request #{number} was opened through the double"),
        }
    }
}

impl Forge for Memory {
    fn name(&self) -> &str {
        "memory"
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let sha = crate::git::head(tree)?;
        self.pushed.borrow_mut().insert(branch.to_owned(), sha);
        self.calls.borrow_mut().push(format!("push {branch}"));
        Ok(())
    }

    fn open_pull_request(&self, _: &Path, _: &Path, request: &PullRequest) -> Result<u64> {
        let mut opened = self.opened.borrow_mut();
        opened.push(request.clone());
        let number = opened.len() as u64;
        self.calls
            .borrow_mut()
            .push(format!("open {} #{number}", request.branch));
        Ok(number)
    }

    fn pull_request(&self, _: &Path, number: u64) -> Result<PullRequestView> {
        let index = self.index(number)?;
        self.calls.borrow_mut().push(format!("view #{number}"));
        let head = self.head.clone().unwrap_or_else(|| {
            let branch = self.opened.borrow()[index].branch.clone();
            let sha = self
                .pushed
                .borrow()
                .get(&branch)
                .cloned()
                .unwrap_or_default();
            Head { branch, sha }
        });
        Ok(PullRequestView {
            state: self.state,
            head,
        })
    }

    fn checks(&self, _: &Path, number: u64) -> Result<Checks> {
        self.index(number)?;
        self.calls.borrow_mut().push(format!("checks #{number}"));
        Ok(self.checks)
    }
}
