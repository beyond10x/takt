//! The forge a run file names with `forge: connectors` and `forge_instance`: each action of the
//! [`Forge`] seam is one operation of a Connectors connector's instance. `forge_instance` names
//! the connector and, after one `/`, the instance, as `connectors invoke` takes its target:
//! `gitlab`, `github/work` ([`target_of`]). With the instance left out the connector's only
//! instance is used. It is optional and never a default: no run reaches it without both keys.
//! The push stays takt's own push over HTTPS to the project the `origin` remote names, or over
//! `http` to the loopback relay it names, port kept ([`RemoteTransport::LoopbackHttp`]): a line
//! to the relay goes through no proxy, and is refused when git's configuration would rewrite
//! the relay's URL to anything but a loopback host. This forge replaces the host's API calls
//! and nothing before them.
//!
//! **Two routes.** Without `forge_endpoint` each operation is one `connectors --output json
//! invoke <connector>[/<instance>] <operation> --schema <s> --revision <r> --input-json <json>`,
//! run through the forge [`Runner`] as an argument list, never a shell line, with the schema and
//! the revision `invoke … --describe` reported for the operation at the preflight. With
//! `forge_endpoint`, the URL of a served Connectors instance ([`check_endpoint`]), each operation
//! is one `POST /v1alpha2/invoke` there after one `GET /v1/describe`, and no `connectors` line
//! runs. No `Authorization` header and no credential of any kind goes with a served call: takt
//! holds none, the instance holds the provider's.
//!
//! The `gitlab` connector's operations take the project's path from `origin`:
//!
//! | action | operation | input | read from |
//! |---|---|---|---|
//! | open | `merge_request.create` | `id`, `sha` (the case branch's commit), `body` | `iid` |
//! | read | `merge_request.get` | `id`, `merge_request_iid` | `state`, `source_branch`, `target_branch`, `sha`, `head_pipeline` |
//! | checks | `pipeline.get` | `id`, `pipeline_id` | `sha`, `status` |
//! | merge | `merge_request.merge` | `id`, `merge_request_iid`, `pipeline_id`, `body.sha` | `state` |
//!
//! The `github` connector's take `owner` and `repo` from `origin`, which names `<owner>/<repo>`:
//!
//! | action | operation | input | read from |
//! |---|---|---|---|
//! | open | `pull_request.create` | `owner`, `repo`, `sha` (the case branch's commit), `body` (`head`, `base`, `title`, `body`) | `number` |
//! | read | `pull_request.get` | `owner`, `repo`, `pull_number` | `state`, `merged`, `head.ref`, `head.sha`, `base.ref` |
//! | checks | `check_runs.list`, `commit.status` | `owner`, `repo`, `ref` (the head's sha); `per_page`, `page` | `check_runs[].status`, `check_runs[].conclusion`, `total_count`; `state`, `statuses` |
//! | merge | `pull_request.merge` | `owner`, `repo`, `pull_number`, `body.sha` (the head) | `merged` |
//! | merge target | `branch.get` | `owner`, `repo`, `branch` | `commit.sha`, taken only when `git ls-remote` of the project `origin` names lists the branch at the same commit |
//!
//! **Preflight.** [`ConnectorsForge::for_tree`] describes every operation the connector's map
//! needs ([`Connector::required`]) before any case opens, and refuses naming each one the
//! instance does not describe: through `connectors invoke … --describe` on the CLI route, through
//! the one `GET /v1/describe` on the served route, whose `instance` must be the one
//! `forge_instance` names when it names one. What it found, the connector, the instance and the
//! descriptor revision, is [`ConnectorsForge::identity`].
//!
//! **Checks are bound to the head.** On gitlab the checks are the status of the pipeline the
//! change request's `head_pipeline` names, read only when that pipeline is the change request's
//! head's (its `sha` is the head); a pipeline of another head is not read as a result and the
//! checks stay pending, and the merge is invoked with the pipeline the checks were read on. On
//! github the check runs and the combined status are read for the head's sha, and their verdict
//! is [`github_checks`]; the merge sends the head's sha, which GitHub compares.
//!
//! **A merge that fails is no merge.** A merge that fails, a 409 among them, marks its pull
//! request, and before any further write (a merge, an open) every marked pull request is read
//! again; a read the caller makes counts. A marked pull request that is no longer open, or whose
//! head is no longer the one given, is not merged at all.
//!
//! **Keyed writes.** On the served route each write (the open, the merge) is sent first with an
//! `idempotency_key` ([`idempotency_key`]): `takt-` and the hex SHA-256 of the case's id, the
//! operation, the descriptor revision, its input and how many times that write at that revision
//! was not applied, so the same case and write give the same key and a write sent again after a
//! lost answer, or after `outcome_unknown`, replays its first outcome. A host with standing
//! grants takes the key; one without them refuses it as a member it does not take
//! (`unsupported`, HTTP 400, `request_id: null`, nothing run), and the forge then sends that
//! write, and every later one, with no key. An `idempotency_conflict`, `approval_required` or
//! `outcome_unknown` ends the write naming the code; after a merge's `outcome_unknown` the pull
//! request is read again before any further write. A write whose answer's
//! `mutation.classification` is `refused` or `not_attempted` takes a new key next time, so a
//! merge the provider refused is not replayed refused; `applied`, `unknown` or no
//! classification keeps the key. That count is kept in the case's artifacts directory
//! ([`ConnectorsForge::REFUSALS`]), so a resumed run reads it back; a descriptor revision
//! that moves gives a new key, never the host's `idempotency_conflict`.
//!
//! `connectors invoke` refuses a write without `--approval-file` (`approval_required`), and none
//! is given here: on the CLI route the open and the merge fail with that refusal. Standard error
//! of a failed line, and the error of a served call, are kept in the error, bounded; an input
//! never is.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::git::{NO_REDIRECT, RemoteTransport};
use crate::run_config::ForgeSettings;
use crate::scan::Scan;

use super::{
    Checks, Exit, Failed, Forge, Head, IdentityScan, Outgoing, PullRequest, PullRequestState,
    PullRequestView, Runner, commit_id, exact_ref, fresh, head_is_a_commit, is_object_id,
    may_publish, push_over_https_with, write_request,
};

/// The program every line of the CLI route runs.
pub const PROGRAM: &str = "connectors";

/// The operations the `gitlab` map needs: change request create, read and merge, and pipeline
/// read.
pub const REQUIRED: [&str; 4] = [
    "merge_request.create",
    "merge_request.get",
    "merge_request.merge",
    "pipeline.get",
];

/// The operations the `github` map needs: the merge target's branch, pull request create, read
/// and merge, and the check runs and the combined status of the head.
pub const GITHUB: [&str; 6] = [
    "branch.get",
    "pull_request.create",
    "pull_request.get",
    "pull_request.merge",
    "check_runs.list",
    "commit.status",
];

const CREATE: &str = REQUIRED[0];
const GET: &str = REQUIRED[1];
const MERGE: &str = REQUIRED[2];
const PIPELINE: &str = REQUIRED[3];

const BRANCH: &str = GITHUB[0];
const PULL_CREATE: &str = GITHUB[1];
const PULL_GET: &str = GITHUB[2];
const PULL_MERGE: &str = GITHUB[3];
const CHECK_RUNS: &str = GITHUB[4];
const STATUS: &str = GITHUB[5];

/// The path of the served wire's description.
pub const DESCRIBE_PATH: &str = "/v1/describe";

/// The path of the served wire's invocation.
pub const INVOKE_PATH: &str = "/v1alpha2/invoke";

/// The most pages of a listing read, so a listing that never ends is an error.
const PAGES: usize = 50;

/// The check runs one page of `check_runs.list` asks for.
const PER_PAGE: usize = 100;

/// The most bytes of a failed line's standard error, or of a served error's message, kept in
/// the error.
const STDERR_BYTES: usize = 600;

/// The most bytes of one served answer that are read.
const ANSWER_LIMIT: usize = 16 << 20;

/// How long a served call may take, whole.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a connection to the served instance may take to be made.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// The longest word of a target, as Connectors admits one.
const WORD_BYTES: usize = 128;

/// What every `idempotency_key` the forge sends begins with.
pub const KEY_PREFIX: &str = "takt-";

/// What the digest of an `idempotency_key` is taken over first, so a change to what goes into it
/// is a new name and never a key reused for another write.
const KEY_DOMAIN: &str = "takt.connectors.write/2";

/// The `idempotency_key` of the write `operation` on `input` at the descriptor revision
/// `revision` for the case `case`, after the provider refused that same write at that revision
/// `refused` times: [`KEY_PREFIX`] and the lowercase hex SHA-256 of a JSON array of
/// [`KEY_DOMAIN`], the case, the operation, the revision, the input and `refused`. The host holds
/// a key for the operation, revision and input it was first sent with, so a write sent at
/// another revision goes under another key, never meeting `idempotency_conflict`. 69 ASCII
/// bytes, inside the 1 to 256 bytes connectors admits; the same arguments always give the same
/// key, and any other argument another.
#[must_use]
pub fn idempotency_key(
    case: &str,
    operation: &str,
    revision: &str,
    input: &Value,
    refused: u32,
) -> String {
    use sha2::{Digest as _, Sha256};
    let named = json!([KEY_DOMAIN, case, operation, revision, input, refused]);
    let digest = Sha256::digest(named.to_string().as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{KEY_PREFIX}{hex}")
}

/// What the file of a case's refused writes ([`ConnectorsForge::REFUSALS`]) names itself by.
const REFUSALS_FORMAT: &str = "takt.connectors.refusals/1";

/// Whether a write's `mutation.classification` says it was not applied: the provider refused it
/// (`refused`), or its attempt never crossed the dispatch gate (`not_attempted`, an aborted
/// attempt's replay among them). Only then is the write sent again under a new key; `applied`,
/// `unknown` and an answer with no classification keep the key, so a write that may have been
/// made is replayed and never made twice.
fn not_applied(classification: Option<&str>) -> bool {
    matches!(classification, Some("refused" | "not_attempted"))
}

/// How many times the provider refused each write of the case whose artifacts directory is
/// `dir`, by the write's first key: what [`ConnectorsForge::REFUSALS`] holds there, empty when
/// it holds nothing. A file that is not one is an error, never a count of zero.
fn refusals_in(dir: &Path) -> Result<BTreeMap<String, u32>> {
    let path = dir.join(ConnectorsForge::REFUSALS);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    let document: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not JSON; nothing was sent", path.display()))?;
    if document["format"] != REFUSALS_FORMAT {
        bail!(
            "{} is no {REFUSALS_FORMAT} document; nothing was sent",
            path.display()
        );
    }
    document["refused"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, count)| {
            let count = count
                .as_u64()
                .and_then(|count| u32::try_from(count).ok())
                .ok_or_else(|| {
                    anyhow!(
                        "{} counts {key} as no number of refusals; nothing was sent",
                        path.display()
                    )
                })?;
            Ok((key.clone(), count))
        })
        .collect()
}

/// Writes `refused` into the case's artifacts directory `dir` ([`ConnectorsForge::REFUSALS`])
/// through a file beside it renamed over it, so a later process reads the old count or the new
/// one, never a part.
fn record_refusals(dir: &Path, refused: &BTreeMap<String, u32>) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(ConnectorsForge::REFUSALS);
    let partial = dir.join(format!(
        ".{}.{}.partial",
        ConnectorsForge::REFUSALS,
        std::process::id()
    ));
    let document = json!({"format": REFUSALS_FORMAT, "refused": refused});
    std::fs::write(&partial, serde_json::to_vec_pretty(&document)?)
        .with_context(|| format!("write {}", partial.display()))?;
    std::fs::rename(&partial, &path).with_context(|| format!("write {}", path.display()))
}

/// The case whose artifacts directory is `dir` (`<case directory>/artifacts`, as the validators
/// hand it to the forge): the case directory's name, its id; `dir` itself, as text, when it
/// is no such directory.
fn case_of(dir: &Path) -> String {
    dir.file_name()
        .filter(|name| *name == "artifacts")
        .and_then(|_| dir.parent()?.file_name())
        .map_or_else(
            || dir.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
}

/// The connectors this forge maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connector {
    /// `gitlab`: merge requests and pipelines.
    Gitlab,
    /// `github`: pull requests, check runs and the combined status.
    Github,
}

impl Connector {
    /// The connector `name` names, `None` for one this forge does not map.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "gitlab" => Some(Self::Gitlab),
            "github" => Some(Self::Github),
            _ => None,
        }
    }

    /// Its name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Gitlab => "gitlab",
            Self::Github => "github",
        }
    }

    /// The operations its map needs.
    #[must_use]
    pub fn required(self) -> &'static [&'static str] {
        match self {
            Self::Gitlab => &REQUIRED,
            Self::Github => &GITHUB,
        }
    }
}

/// Whether `name` is a plain word for an instance: letters, digits, `.`, `_` and `-`, not
/// starting with `-`, so it is one argument and never a flag.
#[must_use]
pub fn is_instance_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= WORD_BYTES
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// The connector and the instance `forge_instance` names: `<connector>` or
/// `<connector>/<instance>`, the connector one this forge maps and the instance a plain word
/// ([`is_instance_name`]); `None` for anything else.
#[must_use]
pub fn target_of(text: &str) -> Option<(Connector, Option<&str>)> {
    let (connector, instance) = match text.split_once('/') {
        Some((connector, instance)) => (connector, Some(instance)),
        None => (text, None),
    };
    if !instance.is_none_or(is_instance_name) {
        return None;
    }
    Some((Connector::named(connector)?, instance))
}

/// Whether `url` is an endpoint the forge calls, and if so whether its host is a loopback host:
/// `https://<host>[:<port>][/<prefix>]`, or `http://` to a loopback host alone (`localhost`,
/// `127.0.0.0/8`, `[::1]`), the host a name, an IPv4 address or a bracketed IPv6 address
/// ([`crate::harness::host_of`], [`crate::harness::loopback`]), with no userinfo, query,
/// fragment or white space. The refusal is the rule broken, worded after the key's name, never
/// the URL.
pub fn check_endpoint(url: &str) -> std::result::Result<bool, &'static str> {
    let Some((scheme, rest)) = url
        .split_once("://")
        .filter(|(scheme, _)| matches!(*scheme, "https" | "http"))
        .filter(|_| !url.chars().any(|c| c.is_whitespace() || c.is_control()))
    else {
        return Err(
            "is the URL of a served Connectors instance: `https://<host>[:<port>][/<prefix>]`, or `http://` to a loopback host, with no white space",
        );
    };
    let (authority, tail) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    if authority.contains('@') || tail.contains(['?', '#']) {
        return Err(
            "holds a userinfo, a query or a fragment, each of which can carry a credential; it is `https://<host>[:<port>][/<prefix>]`, or `http://` to a loopback host",
        );
    }
    let Some(host) = crate::harness::host_of(authority) else {
        return Err(
            "names no host takt reads: a name, an IPv4 address or a bracketed IPv6 address, with an optional `:<port>` of at most 65535",
        );
    };
    let loopback = crate::harness::loopback(host);
    if scheme == "http" && !loopback {
        return Err(
            "is `http://` only to a loopback host (`localhost`, `127.0.0.0/8`, `[::1]`); any other host takes `https://`",
        );
    }
    Ok(loopback)
}

/// The URL of the loopback relay `url`, an `origin` [`crate::git::remote_transport`] admitted
/// as [`RemoteTransport::LoopbackHttp`], read by the same parse: the authority ends at the
/// first `/`, `?` or `#`, the host is what follows its last `@`, and the userinfo is dropped.
/// A `?` or a `#` anywhere in it is refused, so no reader of the URL, git or curl, finds
/// another host in it than the one admitted. `takt run`'s lines and `takt scenario run`'s
/// trial lines to a loopback relay both go to this URL, so neither sends the userinfo.
pub fn relay_of(url: &str) -> Result<String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| anyhow!("the origin remote's URL is no URL of a loopback relay"))?;
    let (authority, tail) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_port)| host_port);
    if tail.contains(['?', '#']) {
        bail!(
            "the origin remote's URL to the loopback relay {scheme}://{host_port} holds a `?` or a `#`, after which a reader of the URL may find another host; nothing was opened"
        );
    }
    Ok(format!("{scheme}://{host_port}{tail}"))
}

/// What a git line to the loopback relay is given on its command line beside [`NO_REDIRECT`]:
/// no proxy, so the line goes to the relay itself and through nothing between.
const NO_PROXY_CONFIG: [&str; 2] = ["-c", "http.proxy="];

/// What a git line to the loopback relay is given in its environment: no proxy for any host,
/// whatever the operator's environment or a URL-scoped `http.<url>.proxy` names.
const NO_PROXY_ENV: [(&str, &str); 2] = [("no_proxy", "*"), ("NO_PROXY", "*")];

/// Every URL git's configuration, `config`, could send a line given `url` on its command line
/// to: `url` itself, each `url` or `pushurl` of a remote named by `url`, and each of those
/// rewritten by every `url.<base>.insteadOf` and `url.<base>.pushInsteadOf` whose value begins
/// it. `config` is `git config --list -z`: `key\nvalue` entries, each ended by a NUL, the
/// section and the variable of a key in lower case. Every rewrite that matches is listed, not
/// only the one git picks, so the list holds git's choice whichever it is.
#[must_use]
pub fn reached_by(url: &str, config: &[u8]) -> Vec<String> {
    let entries: Vec<(String, String)> = config
        .split(|byte| *byte == 0)
        .filter_map(|entry| {
            let text = String::from_utf8_lossy(entry);
            let (key, value) = text.split_once('\n')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect();
    let named = [format!("remote.{url}.url"), format!("remote.{url}.pushurl")];
    let mut reached = vec![url.to_owned()];
    reached.extend(
        entries
            .iter()
            .filter(|(key, _)| named.contains(key))
            .map(|(_, value)| value.clone()),
    );
    let rewrites: Vec<(&str, &str)> = entries
        .iter()
        .filter_map(|(key, prefix)| {
            let base = key.strip_prefix("url.")?;
            let base = base
                .strip_suffix(".insteadof")
                .or_else(|| base.strip_suffix(".pushinsteadof"))?;
            Some((base, prefix.as_str()))
        })
        .collect();
    let mut rewritten = Vec::new();
    for target in &reached {
        for (base, prefix) in &rewrites {
            if let Some(rest) = target.strip_prefix(prefix) {
                rewritten.push(format!("{base}{rest}"));
            }
        }
    }
    reached.extend(rewritten);
    reached
}

/// The verdict of a GitHub head's checks: `Pass` when every check run is `completed` with
/// `success`, `neutral` or `skipped` and the combined status is `success`, or `pending` with no
/// status; `Fail` on any check run concluded `failure`, `cancelled` or `timed_out`, or a combined
/// status `failure` or `error`; else `Pending`. A head on which nothing has reported, no check
/// run and no status, is `Pending`: no check is never a pass. `runs` holds each check run's
/// `status` and `conclusion`; `state` is the combined status's and `statuses` how many it has.
/// A combined state other than `success`, `pending`, `failure` and `error` is an error.
pub fn github_checks(
    runs: &[(&str, Option<&str>)],
    state: &str,
    statuses: usize,
) -> Result<Checks> {
    if !matches!(state, "success" | "pending" | "failure" | "error") {
        bail!(
            "the combined status is {:?}, not success, pending, failure or error",
            state
        );
    }
    let failed = |conclusion: Option<&str>| {
        matches!(conclusion, Some("failure" | "cancelled" | "timed_out"))
    };
    if runs.iter().any(|(_, conclusion)| failed(*conclusion))
        || matches!(state, "failure" | "error")
    {
        return Ok(Checks::Fail);
    }
    let passed = runs.iter().all(|(status, conclusion)| {
        *status == "completed" && matches!(conclusion, Some("success" | "neutral" | "skipped"))
    });
    let combined = state == "success" || (state == "pending" && statuses == 0);
    if passed && combined && !(runs.is_empty() && statuses == 0) {
        Ok(Checks::Pass)
    } else {
        Ok(Checks::Pending)
    }
}

/// How the operations are reached.
enum Route {
    /// `connectors invoke <target>` through the runner.
    Cli,
    /// The served wire of the instance at `endpoint`.
    Served {
        /// The URL as given, without a trailing `/`.
        endpoint: String,
        /// Whether its host is a loopback host: then no proxy is asked.
        loopback: bool,
        /// The descriptor revision `GET /v1/describe` reported, which every invocation names.
        revision: String,
    },
}

/// A served call that did not succeed: the HTTP status and, when the answer says, its code and
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedRefused {
    /// What was called: the path, the operation and the endpoint, never the input.
    pub call: String,
    /// The HTTP status.
    pub status: u16,
    /// The answer's error code.
    pub code: Option<String>,
    /// The answer's message, bounded.
    pub message: String,
    /// Whether the answer named a request: its `request_id` is not null.
    pub echoed: bool,
    /// The answer's `mutation.classification`, when it carries one: what the instance recorded
    /// of a write's attempt.
    pub classification: Option<String>,
}

impl ServedRefused {
    /// Whether the instance answered that what was asked conflicts with what holds now: HTTP
    /// 409, or the code `conflict`; a key held for another request (`idempotency_conflict`) is
    /// not.
    #[must_use]
    pub fn is_conflict(&self) -> bool {
        (self.status == 409 && self.code.as_deref() != Some("idempotency_conflict"))
            || self.code.as_deref() == Some("conflict")
    }

    /// Whether the instance does not know whether the write was applied: `outcome_unknown`.
    #[must_use]
    pub fn is_outcome_unknown(&self) -> bool {
        self.code.as_deref() == Some("outcome_unknown")
    }

    /// Whether the instance refused the request for a member it does not take, before reading
    /// the rest of it: `unsupported`, HTTP 400, `request_id: null`, no attempt. A host without
    /// standing grants answers so to `idempotency_key`, having run nothing.
    #[must_use]
    pub fn refuses_a_member(&self) -> bool {
        self.status == 400
            && self.code.as_deref() == Some("unsupported")
            && !self.echoed
            && self.classification.is_none()
    }
}

impl fmt::Display for ServedRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} answered HTTP {}", self.call, self.status)?;
        if let Some(code) = &self.code {
            write!(f, " ({code})")?;
        }
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ServedRefused {}

/// `text` without control characters, at most [`STDERR_BYTES`] characters of it.
fn bounded(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .take(STDERR_BYTES)
        .collect()
}

/// The code of a `connectors` failure envelope, on standard error as `--output json` writes
/// one, or on standard output.
fn refusal_code(exit: &Exit) -> Option<String> {
    [&exit.stderr, &exit.stdout].into_iter().find_map(|text| {
        serde_json::from_str::<Value>(text.trim())
            .ok()
            .and_then(|document| document["error"]["code"].as_str().map(str::to_owned))
    })
}

/// The adapter over one instance of a Connectors connector, through `connectors invoke` or the
/// instance's served wire.
pub struct ConnectorsForge {
    connector: Connector,
    target: String,
    instance: Option<String>,
    route: Route,
    project: String,
    origin: (String, String),
    /// The `origin` URL without its userinfo when it is `http://` to a loopback relay
    /// ([`RemoteTransport::LoopbackHttp`], [`relay_of`]): the push and the fetch go there, port
    /// kept, and to no other host.
    relay: Option<String>,
    identity: String,
    /// The run's scan adapter.
    pub scan: Scan,
    runner: Box<dyn Runner>,
    described: RefCell<BTreeMap<&'static str, (String, String)>>,
    bound: RefCell<BTreeMap<u64, (String, u64)>>,
    reread: RefCell<BTreeSet<u64>>,
    calls: Cell<u64>,
    started: u128,
    /// Whether the served instance refused `idempotency_key` as a member it does not take: then
    /// every write goes with no key.
    unkeyed: Cell<bool>,
}

impl fmt::Debug for ConnectorsForge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorsForge")
            .field("target", &self.target)
            .field("origin", &self.origin)
            .field("scan", &self.scan)
            .finish_non_exhaustive()
    }
}

/// A change request as `merge_request.get` reports it.
struct Request {
    view: PullRequestView,
    head_pipeline: Option<(u64, Option<String>)>,
}

impl ConnectorsForge {
    /// The file under the case's artifacts directory that holds what the open answered: the
    /// one every forge writes, which `pr.open`'s validator declares.
    pub const RESPONSE: &'static str = super::RESPONSE;
    /// The file that holds what the merge answered, which `repository.merge`'s validator
    /// declares.
    pub const MERGE_RESPONSE: &'static str = super::MERGE_RESPONSE;
    /// The file under the case's artifacts directory that holds, for each keyed write by its
    /// first key, how many times the provider refused it: the count a resumed run reads back,
    /// so its write goes under the key after the last one refused and is not the refusal
    /// replayed.
    pub const REFUSALS: &'static str = "pull-request.refusals.json";

    /// The forge `forge` names for a run: the adapter over the connector `settings.instance`
    /// names, at `settings.endpoint` when it names one, when `forge` is `connectors`, after the
    /// settings are checked; `None` for every other forge and for none, and then nothing is
    /// called. This is the one place the adapter is chosen, so a run file without
    /// `forge: connectors` never reaches `connectors` or an instance.
    pub fn select(
        tree: &Path,
        forge: Option<&str>,
        settings: &ForgeSettings,
        scan: Scan,
        runner: Box<dyn Runner>,
    ) -> Result<Option<Self>> {
        settings.check_for(forge)?;
        if forge != Some("connectors") {
            return Ok(None);
        }
        let target = settings
            .instance
            .as_deref()
            .ok_or_else(|| anyhow!("`forge: connectors` needs `forge_instance`"))?;
        Self::for_tree(tree, target, settings.endpoint.as_deref(), scan, runner).map(Some)
    }

    /// Over the instance `target` names (`<connector>[/<instance>]`, [`target_of`]), reached at
    /// `endpoint` when one is given and through `connectors` otherwise, after its preflight: the
    /// project `tree`'s `origin` names, and every operation the connector's map needs described,
    /// else an error naming each one that is not, before anything is opened. The push goes to
    /// the project `origin` names. Every line runs through `runner`.
    pub fn for_tree(
        tree: &Path,
        target: &str,
        endpoint: Option<&str>,
        scan: Scan,
        runner: Box<dyn Runner>,
    ) -> Result<Self> {
        let Some((connector, instance)) = target_of(target) else {
            bail!(
                "`forge_instance` is `<connector>` or `<connector>/<instance>`: the connector gitlab or github, the instance a word of letters, digits, `.`, `_` and `-`, not starting with `-`"
            );
        };
        let route = match endpoint {
            None => Route::Cli,
            Some(url) => {
                let loopback = check_endpoint(url)
                    .map_err(|rule| anyhow!("`forge_endpoint` {rule}; nothing was opened"))?;
                Route::Served {
                    endpoint: url.trim_end_matches('/').to_owned(),
                    loopback,
                    revision: String::new(),
                }
            }
        };
        let url = crate::git::remote_url(tree, "origin").ok();
        let relay = match url
            .as_deref()
            .map(crate::git::remote_transport)
            .transpose()?
        {
            Some(RemoteTransport::LoopbackHttp) => url.as_deref().map(relay_of).transpose()?,
            _ => None,
        };
        let origin = url
            .as_deref()
            .and_then(super::gitlab::project_of)
            .ok_or_else(|| {
                anyhow!(
                    "the forge connectors names the project by the origin remote, and it names none; nothing was opened"
                )
            })?;
        if connector == Connector::Github && origin.1.split('/').count() != 2 {
            bail!(
                "the github connector names a repository by `<owner>/<repo>`, and the origin remote names {}; nothing was opened",
                origin.1
            );
        }
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let mut forge = Self {
            connector,
            target: target.to_owned(),
            instance: instance.map(str::to_owned),
            route,
            project: origin.1.clone(),
            origin,
            relay,
            identity: String::new(),
            scan,
            runner,
            described: RefCell::new(BTreeMap::new()),
            bound: RefCell::new(BTreeMap::new()),
            reread: RefCell::new(BTreeSet::new()),
            calls: Cell::new(0),
            started,
            unkeyed: Cell::new(false),
        };
        forge.preflight(tree)?;
        Ok(forge)
    }

    /// The instance the preflight found: its connector, its name and the revision of its
    /// descriptor, and on the served route the endpoint.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// What each action goes through: `connectors invoke <target>`, or `POST /v1alpha2/invoke`
    /// at the endpoint.
    #[must_use]
    pub fn route(&self) -> String {
        match &self.route {
            Route::Cli => format!("{PROGRAM} invoke {}", self.target),
            Route::Served { endpoint, .. } => format!("POST {INVOKE_PATH} at {endpoint}"),
        }
    }

    /// Describes every operation the connector's map needs and refuses the ones the instance
    /// does not describe.
    fn preflight(&mut self, tree: &Path) -> Result<()> {
        let required = self.connector.required();
        let (missing, found) = match &self.route {
            Route::Cli => self.preflight_cli(tree, required)?,
            Route::Served { .. } => self.preflight_served(required)?,
        };
        if !missing.is_empty() {
            bail!(
                "{} does not describe {}, which `forge: connectors` needs on the {} connector, and a write is described only when the instance admits it; nothing was opened",
                self.reached(),
                missing.join(", "),
                self.connector.name()
            );
        }
        self.identity = found;
        Ok(())
    }

    /// The instance as an error names it.
    fn reached(&self) -> String {
        match &self.route {
            Route::Cli => format!("the connectors target {}", self.target),
            Route::Served { endpoint, .. } => format!("the instance served at {endpoint}"),
        }
    }

    /// The preflight of the CLI route: `invoke <target> <operation> --describe` for each
    /// operation, its schema and revision kept for the run.
    fn preflight_cli(
        &self,
        tree: &Path,
        required: &[&'static str],
    ) -> Result<(Vec<&'static str>, String)> {
        let mut missing = Vec::new();
        let mut found = None;
        for operation in required {
            let (line, exit) = self.cli_exit(tree, operation, &["--describe"])?;
            if !exit.success() && refusal_code(&exit).as_deref() == Some("not_found") {
                missing.push(*operation);
                continue;
            }
            let answer = Self::cli_answer(line, &exit)?;
            let text = |name: &str| {
                answer[name]
                    .as_str()
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        anyhow!(
                            "`{PROGRAM} invoke {} {operation} --describe` reported no {name}",
                            self.target
                        )
                    })
            };
            let held = (text("schema")?, text("revision")?);
            found.get_or_insert_with(|| {
                let word = |name: &str| answer[name].as_str().unwrap_or("unnamed").to_owned();
                format!(
                    "connector {}, instance {}, descriptor {}",
                    word("connector"),
                    word("instance"),
                    held.1
                )
            });
            self.described.borrow_mut().insert(operation, held);
        }
        if missing.len() == required.len() {
            bail!(
                "the connectors target {} describes none of {}: it names no configured instance, or one that provides none of them; nothing was opened",
                self.target,
                required.join(", ")
            );
        }
        Ok((missing, found.unwrap_or_default()))
    }

    /// The preflight of the served route: one `GET /v1/describe`, its instance the one
    /// `forge_instance` names when it names one, its revision kept for every invocation.
    fn preflight_served(
        &mut self,
        required: &[&'static str],
    ) -> Result<(Vec<&'static str>, String)> {
        let Route::Served {
            endpoint, loopback, ..
        } = &self.route
        else {
            unreachable!("the served preflight runs on the served route");
        };
        let call = format!("GET {DESCRIBE_PATH} at {endpoint}");
        let (status, bytes) = exchange(
            endpoint,
            *loopback,
            reqwest::Method::GET,
            DESCRIBE_PATH,
            None,
        )
        .with_context(|| format!("{call} did not answer; nothing was opened"))?;
        let answer: Option<Value> = serde_json::from_slice(&bytes).ok();
        let Some(answer) = answer.filter(|_| status == 200) else {
            return Err(anyhow::Error::new(refused(call, status, &bytes)))
                .context("nothing was opened");
        };
        let text = |name: &str| {
            answer[name]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{call} reported no {name}; nothing was opened"))
        };
        let revision = text("revision")?;
        let served = text("instance")?;
        if let Some(instance) = &self.instance
            && *instance != served
        {
            bail!(
                "{call} describes the instance {served}, not {instance}, which `forge_instance` names; nothing was opened"
            );
        }
        let listed: Vec<&str> = answer["operations"]
            .as_array()
            .ok_or_else(|| anyhow!("{call} reported no operations; nothing was opened"))?
            .iter()
            .filter_map(|operation| operation["id"].as_str())
            .collect();
        let missing = required
            .iter()
            .copied()
            .filter(|name| !listed.contains(name))
            .collect();
        let found = format!(
            "connector {}, instance {served}, descriptor {revision}, served at {endpoint}",
            self.connector.name()
        );
        if let Route::Served { revision: held, .. } = &mut self.route {
            *held = revision;
        }
        Ok((missing, found))
    }

    /// Runs `connectors --output json invoke <target> <operation> <extra>`: the line as an error
    /// names it, never its input, and what the program answered.
    fn cli_exit(&self, tree: &Path, operation: &str, extra: &[&str]) -> Result<(String, Exit)> {
        let mut argv: Vec<String> = [
            PROGRAM,
            "--output",
            "json",
            "invoke",
            &self.target,
            operation,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        argv.extend(extra.iter().map(|word| (*word).to_owned()));
        let line = format!("{PROGRAM} invoke {} {operation}", self.target);
        let exit = self
            .runner
            .run(tree, &argv)
            .with_context(|| format!("`{line}` could not be run"))?;
        Ok((line, exit))
    }

    /// The `result` of an answer that is `ok`, else the line's failure with the refusal's code.
    fn cli_answer(line: String, exit: &Exit) -> Result<Value> {
        if !exit.success() {
            let code = refusal_code(exit);
            let hint = if code.as_deref() == Some("approval_required") {
                "; `connectors invoke` refuses a write without `--approval-file`, and the forge gives none"
            } else {
                ""
            };
            let coded = code.map(|code| format!("{code}: ")).unwrap_or_default();
            return Err(Failed {
                line,
                code: exit.code,
                stderr: format!("{coded}{}{hint}", bounded(&exit.stderr)),
            }
            .into());
        }
        let document: Option<Value> = serde_json::from_str(exit.stdout.trim()).ok();
        match document {
            Some(document) if document["ok"] == true && !document["result"].is_null() => {
                Ok(document["result"].clone())
            }
            _ => bail!("`{line}` reported no result: its output is not an ok answer"),
        }
    }

    /// The schema and the revision of `operation` on the CLI route, described once and kept.
    fn described(&self, tree: &Path, operation: &'static str) -> Result<(String, String)> {
        if let Some(held) = self.described.borrow().get(operation) {
            return Ok(held.clone());
        }
        let (line, exit) = self.cli_exit(tree, operation, &["--describe"])?;
        let answer = Self::cli_answer(line, &exit)?;
        let text = |name: &str| {
            answer[name]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    anyhow!(
                        "`{PROGRAM} invoke {} {operation} --describe` reported no {name}",
                        self.target
                    )
                })
        };
        let held = (text("schema")?, text("revision")?);
        self.described.borrow_mut().insert(operation, held.clone());
        Ok(held)
    }

    /// What an error names the invocation of `operation` by.
    fn named(&self, operation: &str) -> String {
        match &self.route {
            Route::Cli => format!("`{PROGRAM} invoke {} {operation}`", self.target),
            Route::Served { endpoint, .. } => {
                format!("`POST {INVOKE_PATH}` {operation} at {endpoint}")
            }
        }
    }

    /// Invokes `operation` once on `input`: the provider's `body`.
    fn invoke(&self, tree: &Path, operation: &'static str, input: &Value) -> Result<Value> {
        let outcome = self.outcome(tree, operation, input)?;
        self.body_of(operation, &outcome)
    }

    /// The provider's `body` of `outcome`, an operation's result; a provider status outside
    /// 2xx is the error.
    fn body_of(&self, operation: &str, outcome: &Value) -> Result<Value> {
        match outcome["status"].as_u64() {
            Some(200..=299) | None => {}
            Some(status) => bail!(
                "{} reported the provider's status {status}",
                self.named(operation)
            ),
        }
        Ok(outcome["body"].clone())
    }

    /// Invokes the write `operation` once on `input` for the case whose artifacts directory is
    /// `dir`: the provider's `body`. On the served route it carries its `idempotency_key`
    /// ([`idempotency_key`]) until the instance refuses the member, and is then sent with none.
    /// The key counts the refusals of that write at that revision recorded in `dir`
    /// ([`Self::REFUSALS`]); an answer whose `mutation.classification` says the write was not
    /// applied ([`not_applied`]) is recorded there before the answer is returned, so the next
    /// attempt, in this process or a resumed one, goes under a new key.
    fn write(
        &self,
        tree: &Path,
        dir: &Path,
        operation: &'static str,
        input: &Value,
    ) -> Result<Value> {
        let Route::Served {
            endpoint,
            loopback,
            revision,
        } = &self.route
        else {
            return self.invoke(tree, operation, input);
        };
        if !self.unkeyed.get() {
            let case = case_of(dir);
            let first = idempotency_key(&case, operation, revision, input, 0);
            let mut refusals = refusals_in(dir)?;
            let refused = refusals.get(&first).copied().unwrap_or(0);
            let key = idempotency_key(&case, operation, revision, input, refused);
            let outcome =
                self.served_answer(endpoint, *loopback, revision, operation, input, Some(&key));
            let refusal = outcome
                .as_ref()
                .err()
                .and_then(|error| error.downcast_ref::<ServedRefused>());
            if refusal.is_some_and(ServedRefused::refuses_a_member) {
                self.unkeyed.set(true);
            } else {
                let classification = match &outcome {
                    Ok(answer) => answer["mutation"]["classification"].as_str(),
                    Err(_) => refusal.and_then(|refusal| refusal.classification.as_deref()),
                };
                if not_applied(classification) {
                    refusals.insert(first, refused.saturating_add(1));
                    record_refusals(dir, &refusals).with_context(|| {
                        format!(
                            "{} was not applied, and its refusal could not be recorded for the next attempt",
                            self.named(operation)
                        )
                    })?;
                }
                return self.body_of(operation, &outcome?["result"]);
            }
        }
        let outcome = self.served_invoke(endpoint, *loopback, revision, operation, input, None)?;
        self.body_of(operation, &outcome)
    }

    /// Invokes `operation` once on `input`, unkeyed: the operation's result.
    fn outcome(&self, tree: &Path, operation: &'static str, input: &Value) -> Result<Value> {
        let outcome = match &self.route {
            Route::Cli => {
                let (schema, revision) = self.described(tree, operation)?;
                let input = input.to_string();
                let (line, exit) = self.cli_exit(
                    tree,
                    operation,
                    &[
                        "--schema",
                        &schema,
                        "--revision",
                        &revision,
                        "--input-json",
                        &input,
                    ],
                )?;
                Self::cli_answer(line, &exit)?["result"].clone()
            }
            Route::Served {
                endpoint,
                loopback,
                revision,
            } => self.served_invoke(endpoint, *loopback, revision, operation, input, None)?,
        };
        Ok(outcome)
    }

    /// One `POST /v1alpha2/invoke` of `operation` on `input`, carrying `key` as its
    /// `idempotency_key` when one is given: the operation's result. A replay of a key
    /// (`mutation.replayed: true`) is read as the original outcome it is.
    fn served_invoke(
        &self,
        endpoint: &str,
        loopback: bool,
        revision: &str,
        operation: &'static str,
        input: &Value,
        key: Option<&str>,
    ) -> Result<Value> {
        self.served_answer(endpoint, loopback, revision, operation, input, key)
            .map(|answer| answer["result"].clone())
    }

    /// [`Self::served_invoke`]'s call, its whole success answer: the `result` and, for a write,
    /// the `mutation` the instance recorded.
    fn served_answer(
        &self,
        endpoint: &str,
        loopback: bool,
        revision: &str,
        operation: &'static str,
        input: &Value,
        key: Option<&str>,
    ) -> Result<Value> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        let request_id = format!("takt-{}-{}-{call}", std::process::id(), self.started);
        let mut envelope = json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "operation": operation,
            "revision": revision,
            "input": input,
        });
        if let Some(key) = key {
            envelope["idempotency_key"] = json!(key);
        }
        let named = self.named(operation);
        let (status, bytes) = exchange(
            endpoint,
            loopback,
            reqwest::Method::POST,
            INVOKE_PATH,
            Some(envelope.to_string().into_bytes()),
        )
        .with_context(|| format!("{named} did not answer"))?;
        let answer: Option<Value> = serde_json::from_slice(&bytes).ok();
        match answer {
            Some(answer) if (200..300).contains(&status) && answer["status"] == "success" => {
                if answer["request_id"] != request_id.as_str() {
                    bail!("{named} answered another request than the one sent");
                }
                Ok(answer)
            }
            _ => Err(refused(named, status, &bytes).into()),
        }
    }

    /// The pull request `number` as the connector reports it; it is read again, so a merge
    /// that failed on it no longer holds a write back.
    fn view(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let view = match self.connector {
            Connector::Gitlab => self.read(tree, number)?.view,
            Connector::Github => self.read_pull(tree, number)?,
        };
        self.reread.borrow_mut().remove(&number);
        Ok(view)
    }

    /// Reads again every pull request a merge failed on since it was last read, before any
    /// write. `merging` is the pull request about to be merged and the head given: refused with
    /// no merge when it is marked and no longer open at that head.
    fn read_again(&self, tree: &Path, merging: Option<(u64, &str)>) -> Result<()> {
        let marked: Vec<u64> = self.reread.borrow().iter().copied().collect();
        for number in marked {
            let view = self.view(tree, number)?;
            let Some((merged, head)) = merging.filter(|(merged, _)| *merged == number) else {
                continue;
            };
            if view.state != PullRequestState::Open {
                bail!(
                    "pull request {merged} is {:?} since a merge of it failed, read again; nothing was merged",
                    view.state
                );
            }
            if view.head.sha != head {
                bail!(
                    "the head of pull request {merged} is {} since a merge of it failed, read again, not {head}; nothing was merged",
                    view.head.sha
                );
            }
        }
        Ok(())
    }

    /// A merge of `number` that failed: no merge, and the pull request is read again before any
    /// further write. A merge the instance answered `outcome_unknown` may have been made, and
    /// is read again before any further write all the same.
    fn merge_failed(&self, number: u64, error: anyhow::Error) -> anyhow::Error {
        self.reread.borrow_mut().insert(number);
        if error
            .downcast_ref::<ServedRefused>()
            .is_some_and(ServedRefused::is_outcome_unknown)
        {
            return error.context(format!(
                "whether pull request {number} was merged is not known (outcome_unknown): it is read again before any further write, and a merge sent again carries the same key"
            ));
        }
        let conflict = error
            .downcast_ref::<ServedRefused>()
            .is_some_and(ServedRefused::is_conflict)
            || error
                .downcast_ref::<Failed>()
                .is_some_and(|failed| failed.stderr.starts_with("conflict:"));
        let why = if conflict {
            "the instance answered 409, a conflict: what was read no longer holds"
        } else {
            "the merge failed"
        };
        error.context(format!(
            "pull request {number} was not merged: {why}; nothing was merged, and it is read again before any further write"
        ))
    }

    /// `owner` and `repo` of the github connector's input, from `origin`.
    fn repository(&self) -> (&str, &str) {
        self.project.split_once('/').unwrap_or((&self.project, ""))
    }

    /// The change request `number`, with the pipeline its head names.
    fn read(&self, tree: &Path, number: u64) -> Result<Request> {
        let body = self.invoke(
            tree,
            GET,
            &json!({"id": self.project, "merge_request_iid": number}),
        )?;
        let named = self.named(GET);
        let field = |name: &str| -> Result<String> {
            body[name]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{named} reported no {name}"))
        };
        let state = match body["state"].as_str() {
            Some("opened" | "locked") => PullRequestState::Open,
            Some("merged") => PullRequestState::Merged,
            Some("closed") => PullRequestState::Closed,
            other => {
                bail!("{named} reported the state {other:?}, not opened, locked, merged or closed")
            }
        };
        let head_pipeline = body["head_pipeline"]
            .as_object()
            .and_then(|pipeline| pipeline.get("id")?.as_u64())
            .map(|id| (id, body["head_pipeline"]["sha"].as_str().map(str::to_owned)));
        Ok(Request {
            view: PullRequestView {
                state,
                head: Head {
                    branch: field("source_branch")?,
                    sha: field("sha")?,
                },
                base: field("target_branch")?,
            },
            head_pipeline,
        })
    }

    /// The pull request `number` as `pull_request.get` reports it.
    fn read_pull(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        let (owner, repo) = self.repository();
        let body = self.invoke(
            tree,
            PULL_GET,
            &json!({"owner": owner, "repo": repo, "pull_number": number}),
        )?;
        let named = self.named(PULL_GET);
        let field = |pointer: &str| -> Result<String> {
            body.pointer(pointer)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{named} reported no {}", pointer[1..].replace('/', ".")))
        };
        let state = match (body["state"].as_str(), body["merged"].as_bool()) {
            (Some("open"), _) => PullRequestState::Open,
            (Some("closed"), Some(true)) => PullRequestState::Merged,
            (Some("closed"), Some(false)) => PullRequestState::Closed,
            (state, merged) => bail!(
                "{named} reported the state {state:?} with merged {merged:?}, not open, or closed with merged true or false"
            ),
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

    /// The checks on the head of change request `number`, and the pipeline they were read
    /// on, `None` while no pipeline of that head says anything.
    fn read_checks(&self, tree: &Path, number: u64) -> Result<(Checks, String, Option<u64>)> {
        let request = self.read(tree, number)?;
        self.reread.borrow_mut().remove(&number);
        let head = request.view.head.sha;
        let Some((id, sha)) = request.head_pipeline else {
            return Ok((Checks::Pending, head, None));
        };
        if sha.as_deref() != Some(head.as_str()) {
            return Ok((Checks::Pending, head, None));
        }
        let pipeline = self.invoke(
            tree,
            PIPELINE,
            &json!({"id": self.project, "pipeline_id": id}),
        )?;
        if pipeline["sha"].as_str() != Some(head.as_str()) {
            return Ok((Checks::Pending, head, None));
        }
        let checks = match pipeline["status"].as_str() {
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
                "{} reported a pipeline in the status {other:?}",
                self.named(PIPELINE)
            ),
        };
        Ok((checks, head, Some(id)))
    }

    /// The checks on the head of pull request `number` on github: every page of its check
    /// runs and its combined status, for the head's sha ([`github_checks`]). The walk ends once
    /// the runs read reach the `total_count` a page reports, whatever size the instance serves
    /// a page at; a page that reports none ends it when it is shorter than asked. An empty page
    /// before the count is reached is an error, never a reading of part of them.
    fn github_checks_of(&self, tree: &Path, number: u64) -> Result<Checks> {
        let head = self.view(tree, number)?.head.sha;
        let (owner, repo) = self.repository();
        let mut runs: Vec<(String, Option<String>)> = Vec::new();
        let mut complete = false;
        for page in 1..=PAGES {
            let body = self.invoke(
                tree,
                CHECK_RUNS,
                &json!({
                    "owner": owner,
                    "repo": repo,
                    "ref": head,
                    "per_page": PER_PAGE,
                    "page": page,
                }),
            )?;
            let listed = body["check_runs"]
                .as_array()
                .ok_or_else(|| anyhow!("{} reported no check_runs", self.named(CHECK_RUNS)))?;
            for run in listed {
                let status = run["status"].as_str().ok_or_else(|| {
                    anyhow!(
                        "{} reported a check run with no status",
                        self.named(CHECK_RUNS)
                    )
                })?;
                runs.push((
                    status.to_owned(),
                    run["conclusion"].as_str().map(str::to_owned),
                ));
            }
            let total = body["total_count"].as_u64().map(|total| total as usize);
            let read_all = match total {
                Some(total) => runs.len() >= total,
                None => listed.len() < PER_PAGE,
            };
            if read_all {
                complete = true;
                break;
            }
            if listed.is_empty() {
                bail!(
                    "{} counts {} check runs and lists {} before an empty page: the rest are not read, so the checks are not known",
                    self.named(CHECK_RUNS),
                    total.unwrap_or_default(),
                    runs.len()
                );
            }
        }
        if !complete {
            bail!(
                "{} has more than {PAGES} pages of check runs",
                self.named(CHECK_RUNS)
            );
        }
        let status = self.invoke(
            tree,
            STATUS,
            &json!({"owner": owner, "repo": repo, "ref": head}),
        )?;
        let state = status["state"]
            .as_str()
            .ok_or_else(|| anyhow!("{} reported no state", self.named(STATUS)))?;
        let statuses = status["statuses"]
            .as_array()
            .map_or(0, Vec::len)
            .max(status["total_count"].as_u64().unwrap_or(0) as usize);
        let runs: Vec<(&str, Option<&str>)> = runs
            .iter()
            .map(|(status, conclusion)| (status.as_str(), conclusion.as_deref()))
            .collect();
        github_checks(&runs, state, statuses).with_context(|| self.named(STATUS))
    }

    /// The URL of the project the push and the fetch go to: the loopback relay `origin` names,
    /// else the project over HTTPS.
    fn push_url(&self) -> String {
        self.relay
            .clone()
            .unwrap_or_else(|| format!("https://{}/{}.git", self.origin.0, self.origin.1))
    }

    /// How a line to [`Self::push_url`] goes: over HTTPS, or to the loopback relay over `http`
    /// and HTTPS alone.
    fn transport(&self) -> RemoteTransport {
        if self.relay.is_some() {
            RemoteTransport::LoopbackHttp
        } else {
            RemoteTransport::Https
        }
    }

    /// What a git line to [`Self::push_url`] is given on its command line after `git`: to the
    /// loopback relay, no redirect followed ([`NO_REDIRECT`]) and no proxy
    /// ([`NO_PROXY_CONFIG`]); over HTTPS, nothing.
    fn relay_options(&self) -> Vec<String> {
        if self.relay.is_none() {
            return Vec::new();
        }
        NO_REDIRECT
            .iter()
            .chain(&NO_PROXY_CONFIG)
            .map(|word| (*word).to_owned())
            .collect()
    }

    /// `git` and what a line that reads from [`Self::push_url`] (`ls-remote`, `fetch`) is given
    /// before its verb: no hook, no redirect followed ([`NO_REDIRECT`]) and, to the loopback
    /// relay, no proxy ([`Self::relay_options`]).
    fn read_prefix(&self) -> Vec<String> {
        let mut argv: Vec<String> = ["git", "-c", "core.hooksPath=/dev/null"]
            .into_iter()
            .chain(NO_REDIRECT)
            .map(str::to_owned)
            .collect();
        if self.relay.is_some() {
            argv.extend(NO_PROXY_CONFIG.map(str::to_owned));
        }
        argv
    }

    /// The environment of a git line to [`Self::push_url`]: the protocols its transport allows
    /// and, to the loopback relay, no proxy for any host ([`NO_PROXY_ENV`]).
    fn line_env(&self) -> Vec<(&'static str, &'static str)> {
        let mut env = vec![("GIT_ALLOW_PROTOCOL", self.transport().allowed_protocols())];
        if self.relay.is_some() {
            env.extend(NO_PROXY_ENV);
        }
        env
    }

    /// Refuses a line to the loopback relay that git's configuration would carry anywhere else:
    /// each URL git reaches the relay's URL at, as it reads them for a fetch (`insteadOf`) and
    /// for a push (`pushInsteadOf`, and a `pushurl` or `url` of a remote named by that URL),
    /// must be `http://` to a loopback host ([`RemoteTransport::LoopbackHttp`]). Every such URL
    /// is read off the tree's configuration ([`reached_by`]), as git reads it for this tree,
    /// with the configuration the environment hands git included, as the push keeps it.
    /// Over HTTPS, nothing is read: `GIT_ALLOW_PROTOCOL` holds that line to HTTPS.
    fn relay_reached(&self, tree: &Path) -> Result<()> {
        let Some(relay) = &self.relay else {
            return Ok(());
        };
        let config = crate::git::bytes_as_remote_git(tree, &["config", "--list", "-z"])
            .context("read git's configuration of the tree before a line to the loopback relay; nothing was sent")?;
        for url in reached_by(relay, &config) {
            if !matches!(
                crate::git::remote_transport(&url),
                Ok(RemoteTransport::LoopbackHttp)
            ) {
                bail!(
                    "a line to the loopback relay {relay} would go to {}, no loopback host, as git's configuration rewrites it (`url.<base>.insteadOf`, `url.<base>.pushInsteadOf`, a remote's `pushurl`): the relay's lines go to the relay alone, so nothing was sent",
                    super::without_userinfo(&url)
                );
            }
        }
        Ok(())
    }

    /// The commit `origin`'s project holds `base` at, read through git from the URL every line
    /// goes to ([`Self::push_url`]): `git ls-remote --refs <url> refs/heads/<base>`, run
    /// through the runner. A branch it does not list, or a line that fails, is the error.
    fn origin_tip(&self, tree: &Path, base: &str) -> Result<String> {
        self.relay_reached(tree)?;
        let url = self.push_url();
        let reference = format!("refs/heads/{base}");
        let mut argv = self.read_prefix();
        argv.extend([
            "ls-remote".to_owned(),
            "--refs".to_owned(),
            url.clone(),
            reference.clone(),
        ]);
        let exit = self.runner.run_env(tree, &argv, &self.line_env())?;
        if !exit.success() {
            return Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: bounded(&exit.stderr),
            }
            .into());
        }
        exit.stdout
            .lines()
            .find_map(|line| {
                let (object, name) = line.split_once('\t')?;
                (name.trim_end() == reference).then(|| object.trim().to_owned())
            })
            .filter(|object| is_object_id(object))
            .ok_or_else(|| {
                anyhow!(
                    "`git ls-remote` of {url} lists no commit for {reference}: what the project holds of {base} is not known; nothing is merged"
                )
            })
    }

    /// Merges change request `number` at `head` with the pipeline the checks were read on: the
    /// reading kept by [`Forge::checks`] when it is of `head`, else a reading made now, which
    /// must be of `head` too; a head with none is refused with no merge invoked.
    fn merge_gitlab(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<Value> {
        let held = self
            .bound
            .borrow()
            .get(&number)
            .filter(|(read_on, _)| read_on == head)
            .map(|(_, id)| *id);
        let pipeline = match held {
            Some(id) => id,
            None => {
                let (_, read_on, pipeline) = self.read_checks(tree, number)?;
                match pipeline {
                    Some(id) if read_on == head => id,
                    _ => bail!(
                        "no checks were read on the head {head} of change request {number}, whose head is {read_on}; nothing was merged"
                    ),
                }
            }
        };
        let body = self
            .write(
                tree,
                dir,
                MERGE,
                &json!({
                    "id": self.project,
                    "merge_request_iid": number,
                    "pipeline_id": pipeline,
                    "body": {"sha": head},
                }),
            )
            .map_err(|error| self.merge_failed(number, error))?;
        if let Some(state) = body["state"].as_str()
            && state != "merged"
        {
            self.reread.borrow_mut().insert(number);
            bail!(
                "{} did not merge change request {number}: its state is {state}",
                self.named(MERGE)
            );
        }
        Ok(body)
    }

    /// Merges pull request `number` at `head`: `pull_request.merge` sends the head's sha, which
    /// GitHub compares with the pull request's head.
    fn merge_github(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<Value> {
        let (owner, repo) = self.repository();
        let body = self
            .write(
                tree,
                dir,
                PULL_MERGE,
                &json!({
                    "owner": owner,
                    "repo": repo,
                    "pull_number": number,
                    "body": {"sha": head},
                }),
            )
            .map_err(|error| self.merge_failed(number, error))?;
        if body["merged"] != true {
            self.reread.borrow_mut().insert(number);
            bail!(
                "{} did not merge pull request {number}: {}",
                self.named(PULL_MERGE),
                body["message"].as_str().map_or_else(
                    || "the answer does not say `merged: true`".to_owned(),
                    bounded
                )
            );
        }
        Ok(body)
    }
}

/// The error of a served call that did not succeed, from its HTTP status and its answer.
fn refused(call: String, status: u16, bytes: &[u8]) -> ServedRefused {
    let answer: Value = serde_json::from_slice(bytes).unwrap_or_default();
    let error = if answer["error"].is_object() {
        &answer["error"]
    } else {
        &answer
    };
    ServedRefused {
        call,
        status,
        code: error["code"].as_str().map(bounded),
        message: error["message"].as_str().map(bounded).unwrap_or_default(),
        echoed: !answer["request_id"].is_null(),
        classification: answer["mutation"]["classification"].as_str().map(bounded),
    }
}

/// One served call, `method` on `path` of `endpoint`, with `body`, a JSON document, when there
/// is one; on a runtime of its own in a thread of its own, so a caller inside a runtime is no
/// obstacle to it. No credential goes with it and a redirect is never followed. The status and
/// at most [`ANSWER_LIMIT`] bytes of the answer.
fn exchange(
    endpoint: &str,
    loopback: bool,
    method: reqwest::Method,
    path: &str,
    body: Option<Vec<u8>>,
) -> Result<(u16, Vec<u8>)> {
    let url = format!("{endpoint}{path}");
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async {
                    let mut client = reqwest::Client::builder()
                        .redirect(reqwest::redirect::Policy::none())
                        .connect_timeout(CONNECT_TIMEOUT)
                        .timeout(CALL_TIMEOUT)
                        .user_agent(concat!("takt/", env!("CARGO_PKG_VERSION")));
                    if loopback {
                        client = client.no_proxy();
                    }
                    let mut request = client
                        .build()?
                        .request(method.clone(), &url)
                        .header(reqwest::header::ACCEPT, "application/json");
                    if let Some(body) = body {
                        request = request
                            .header(reqwest::header::CONTENT_TYPE, "application/json")
                            .body(body);
                    }
                    let mut response = request.send().await?;
                    let status = response.status().as_u16();
                    let mut answer = Vec::new();
                    while let Some(chunk) = response.chunk().await? {
                        if answer.len() + chunk.len() > ANSWER_LIMIT {
                            bail!("the answer is longer than {ANSWER_LIMIT} bytes");
                        }
                        answer.extend_from_slice(&chunk);
                    }
                    Ok((status, answer))
                })
            })
            .join()
            .map_err(|_| anyhow!("the call to {url} panicked"))?
    })
}

impl Forge for ConnectorsForge {
    fn name(&self) -> &str {
        "connectors"
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
        let at = exact_ref(tree, &format!("refs/heads/{branch}"))
            .ok_or_else(|| anyhow!("{branch} names no commit; nothing was pushed"))?;
        self.push_revision(tree, branch, &at)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        self.relay_reached(tree)?;
        let mut argv = vec!["git".to_owned()];
        argv.extend(self.relay_options());
        argv.extend([
            "push".to_owned(),
            self.push_url(),
            format!("{revision}:refs/heads/{branch}"),
            "--no-follow-tags".to_owned(),
            "--recurse-submodules=no".to_owned(),
        ]);
        push_over_https_with(
            &*self.runner,
            tree,
            &argv,
            &self.push_url(),
            "the forge connectors",
            &self.line_env(),
        )
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        may_publish(&self.scan, &request.branch)?;
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::RESPONSE))?;
        write_request(dir, request)?;
        let head = exact_ref(tree, &format!("refs/heads/{}", request.branch)).ok_or_else(|| {
            anyhow!(
                "{} names no commit; no pull request was opened",
                request.branch
            )
        })?;
        self.read_again(tree, None)?;
        let (operation, input, number) = match self.connector {
            Connector::Gitlab => (
                CREATE,
                json!({
                    "id": self.project,
                    "sha": head,
                    "body": {
                        "source_branch": request.branch,
                        "target_branch": request.base,
                        "title": request.title,
                        "description": request.body,
                    },
                }),
                "iid",
            ),
            Connector::Github => {
                let (owner, repo) = self.repository();
                (
                    PULL_CREATE,
                    json!({
                        "owner": owner,
                        "repo": repo,
                        "sha": head,
                        "body": {
                            "head": request.branch,
                            "base": request.base,
                            "title": request.title,
                            "body": request.body,
                        },
                    }),
                    "number",
                )
            }
        };
        let body = self
            .write(tree, dir, operation, &input)
            .map_err(|error| {
                if error
                    .downcast_ref::<ServedRefused>()
                    .is_some_and(ServedRefused::is_outcome_unknown)
                {
                    error.context(format!(
                        "whether a pull request for {} was opened is not known (outcome_unknown): an open sent again carries the same key",
                        request.branch
                    ))
                } else {
                    error
                }
            })?;
        std::fs::write(&output, serde_json::to_vec_pretty(&body)?)
            .with_context(|| format!("write the response {}", output.display()))?;
        body[number].as_u64().ok_or_else(|| {
            anyhow!(
                "{} reported no {number}: no pull request number",
                self.named(operation)
            )
        })
    }

    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        self.view(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
        if self.connector == Connector::Github {
            return self.github_checks_of(tree, number);
        }
        let (checks, head, pipeline) = self.read_checks(tree, number)?;
        match pipeline {
            Some(id) => {
                self.bound.borrow_mut().insert(number, (head, id));
            }
            None => {
                self.bound.borrow_mut().remove(&number);
            }
        }
        Ok(checks)
    }

    /// Merges at `head`, after every pull request a merge failed on is read again: on gitlab
    /// with the pipeline the checks were read on, on github with the head's sha. A merge that
    /// fails is no merge.
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::MERGE_RESPONSE))?;
        self.read_again(tree, Some((number, head)))?;
        let body = match self.connector {
            Connector::Gitlab => self.merge_gitlab(tree, dir, number, head)?,
            Connector::Github => self.merge_github(tree, dir, number, head)?,
        };
        std::fs::write(&output, serde_json::to_vec_pretty(&body)?)
            .with_context(|| format!("write the response {}", output.display()))?;
        Ok(())
    }

    /// On github, the commit `branch.get` reports the base at, when the project `origin` names
    /// holds the base at that same commit, read through git ([`Self::origin_tip`]); a served
    /// answer that differs is refused, so no instance's answer alone names the commit the judge
    /// guard reads. The commit is fetched from that project into no ref when the tree does not
    /// hold it. On gitlab, refused as the seam's default is: nothing is merged through it.
    fn target_tip(&self, tree: &Path, base: &str) -> Result<String> {
        if self.connector == Connector::Gitlab {
            bail!(
                "the forge connectors cannot report the tip of {base}, the merge target, on the gitlab connector; nothing is merged through it"
            );
        }
        let (owner, repo) = self.repository();
        let body = self.invoke(
            tree,
            BRANCH,
            &json!({"owner": owner, "repo": repo, "branch": base}),
        )?;
        let tip = body["commit"]["sha"].as_str().unwrap_or_default();
        if !is_object_id(tip) {
            bail!(
                "{} reports {base} at `{}`, which is no object id",
                self.named(BRANCH),
                tip.escape_debug()
            );
        }
        let held = self.origin_tip(tree, base)?;
        if held != tip {
            bail!(
                "{} reports {base} at {tip}, and {} holds it at {held}: the tip of the merge target is not known; nothing is merged",
                self.named(BRANCH),
                self.push_url()
            );
        }
        if commit_id(tree, tip).is_ok() {
            return Ok(tip.to_owned());
        }
        let url = self.push_url();
        let mut argv = self.read_prefix();
        argv.extend(
            [
                "fetch",
                &url,
                tip,
                "--no-write-fetch-head",
                "--no-tags",
                "--recurse-submodules=no",
            ]
            .map(str::to_owned),
        );
        let exit = self.runner.run_env(tree, &argv, &self.line_env())?;
        if !exit.success() {
            return Err(Failed {
                line: argv.join(" "),
                code: exit.code,
                stderr: bounded(&exit.stderr),
            }
            .into());
        }
        Ok(tip.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instance_is_one_plain_word() {
        for good in ["forge", "a.b_c-d", "x1"] {
            assert!(is_instance_name(good), "{good}");
        }
        for bad in ["", "-x", "--flag", "a b", "a/b", "$(id)", "a;b"] {
            assert!(!is_instance_name(bad), "{bad:?}");
        }
        assert!(!is_instance_name(&"x".repeat(WORD_BYTES + 1)));
    }

    #[test]
    fn a_target_is_a_mapped_connector_and_an_optional_instance() {
        assert_eq!(target_of("gitlab"), Some((Connector::Gitlab, None)));
        assert_eq!(
            target_of("github/work"),
            Some((Connector::Github, Some("work")))
        );
        for bad in [
            "",
            "forge",
            "gitlab/",
            "gitlab/-x",
            "github/a/b",
            "/work",
            "gitea/x",
        ] {
            assert_eq!(target_of(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn an_endpoint_is_https_or_http_to_a_loopback_host() {
        assert_eq!(check_endpoint("http://127.0.0.1:8402"), Ok(true));
        assert_eq!(check_endpoint("http://[::1]:1/x"), Ok(true));
        assert_eq!(check_endpoint("https://connectors.example/x/"), Ok(false));
        assert_eq!(check_endpoint("https://localhost"), Ok(true));
        for bad in [
            "http://example.com",
            "http://127.0.0.1.example",
            "https://u@connectors.example",
            "https://connectors.example?a",
            "https://connectors.example/#a",
            "https://:1",
            "file:///x",
            "https://a b",
        ] {
            assert!(check_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_github_checks_rule() {
        let ok = ("completed", Some("success"));
        assert_eq!(github_checks(&[ok], "success", 1).ok(), Some(Checks::Pass));
        assert_eq!(github_checks(&[ok], "pending", 0).ok(), Some(Checks::Pass));
        assert_eq!(github_checks(&[], "pending", 0).ok(), Some(Checks::Pending));
        assert_eq!(github_checks(&[ok], "error", 1).ok(), Some(Checks::Fail));
        assert!(github_checks(&[ok], "unknown", 0).is_err());
    }
}
