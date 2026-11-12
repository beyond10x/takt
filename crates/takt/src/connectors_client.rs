//! The one client of a served Connectors instance: `GET /v1/describe` ([`Client::describe`]),
//! `POST /v1alpha2/invoke` of an operation named at run time ([`Client::invoke`],
//! [`Client::answer`]), the `idempotency_key` of a write ([`idempotency_key`]) and the record of
//! a case's refused writes under a file name its caller gives ([`refusals_in`],
//! [`record_refusals`]). The forge `connectors` runs on it
//! ([`crate::forge::connectors::ConnectorsForge`]), and so does every other caller of a served
//! instance, each with its own record file, so two callers in one case never share a count.
//!
//! **No credential.** No `Authorization` header and no credential of any kind goes with a call:
//! takt holds none, the instance holds the provider's. The endpoint is `https://`, or `http://`
//! to a loopback host ([`check_endpoint`]), and a call to a loopback host asks no proxy. A
//! redirect is never followed. A call is bounded: [`CONNECT_TIMEOUT`] to connect,
//! [`CALL_TIMEOUT`] whole, at most [`ANSWER_LIMIT`] bytes of an answer read.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::{Value, json};

/// The path of the served wire's description.
pub const DESCRIBE_PATH: &str = "/v1/describe";

/// The path of the served wire's invocation.
pub const INVOKE_PATH: &str = "/v1alpha2/invoke";

/// The most bytes of a served error's message, or of a failed line's standard error, kept in
/// the error.
const MESSAGE_BYTES: usize = 600;

/// The most bytes of one served answer that are read.
const ANSWER_LIMIT: usize = 16 << 20;

/// How long a served call may take, whole.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a connection to the served instance may take to be made.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// What every `idempotency_key` takt sends begins with.
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

/// Whether `text` has the form of a key [`idempotency_key`] makes: `takt-` and 64 lowercase
/// hexadecimal digits.
fn is_key(text: &str) -> bool {
    text.strip_prefix("takt-").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Whether a write's `mutation.classification` says it was not applied: the provider refused it
/// (`refused`), or its attempt never crossed the dispatch gate (`not_attempted`, an aborted
/// attempt's replay among them). Only then is the write sent again under a new key; `applied`,
/// `unknown` and an answer with no classification keep the key, so a write that may have been
/// made is replayed and never made twice.
#[must_use]
pub fn not_applied(classification: Option<&str>) -> bool {
    matches!(classification, Some("refused" | "not_attempted"))
}

/// What a record of refused writes names itself by.
const REFUSALS_FORMAT: &str = "takt.connectors.refusals/1";

/// The most bytes of a record of refused writes that are read.
const REFUSALS_LIMIT: u64 = 64 << 10;

/// What a record of refused writes holds, the record of one caller in one case.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Record {
    /// How many times each write at a revision was not applied, by its first key.
    pub refused: BTreeMap<String, u32>,
    /// Each key a write was answered `outcome_unknown` under, with the fresh key taken after
    /// the caller settled that it was not applied, `None` until one is.
    pub unknown: BTreeMap<String, Option<String>>,
}

/// `file` when it is one plain name a record can have in its directory: not empty, no path
/// separator, not `.` or `..` and not beginning with `.`, which the record's partial file
/// begins with.
fn record_name(file: &str) -> Result<&str> {
    if file.is_empty() || file.starts_with('.') || file.contains(['/', '\0']) {
        bail!(
            "{file:?} is no file name of a refusal record: one plain name, not beginning with `.`"
        );
    }
    Ok(file)
}

/// How many times the provider refused each write of one caller in the case whose artifacts
/// directory is `dir`, by the write's first key: what the record `file` holds there, empty when
/// nothing is there. A file that is not one is an error, never a count of zero: the record is
/// opened without following a link at it (`O_NOFOLLOW`) or waiting on it (`O_NONBLOCK`), must be
/// a regular file of at most [`REFUSALS_LIMIT`] bytes, its `refused` must be an object of
/// non-negative integer counts, and its `unknown`, when there, an object of keys each naming a
/// fresh key or null ([`Record::unknown`]).
pub fn refusals_in(dir: &Path, file: &str) -> Result<Record> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let path = dir.join(record_name(file)?);
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Record::default()),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => bail!(
            "{} is a link, and a refusal record is never read through one; nothing was sent",
            path.display()
        ),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read {}; nothing was sent", path.display()));
        }
    };
    let meta = file
        .metadata()
        .with_context(|| format!("read {}; nothing was sent", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a regular file; nothing was sent", path.display());
    }
    let mut bytes = Vec::new();
    file.take(REFUSALS_LIMIT + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}; nothing was sent", path.display()))?;
    if bytes.len() as u64 > REFUSALS_LIMIT {
        bail!(
            "{} is longer than {REFUSALS_LIMIT} bytes; nothing was sent",
            path.display()
        );
    }
    let document: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("{} is not JSON; nothing was sent", path.display()))?;
    if document["format"] != REFUSALS_FORMAT {
        bail!(
            "{} is no {REFUSALS_FORMAT} document; nothing was sent",
            path.display()
        );
    }
    let Some(refused) = document["refused"].as_object() else {
        bail!(
            "{} holds no object of counts at `refused`; nothing was sent",
            path.display()
        );
    };
    let refused = refused
        .iter()
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
        .collect::<Result<_>>()?;
    let unknown = match document.get("unknown") {
        None => BTreeMap::new(),
        Some(Value::Object(unknown)) => unknown
            .iter()
            .map(|(key, fresh)| match fresh {
                _ if !is_key(key) => Err(anyhow!(
                    "{} names {key:?} at `unknown`, which is no key takt takes; nothing was sent",
                    path.display()
                )),
                Value::Null => Ok((key.clone(), None)),
                Value::String(fresh) if is_key(fresh) => Ok((key.clone(), Some(fresh.clone()))),
                _ => Err(anyhow!(
                    "{} names no fresh key or null for {key} at `unknown`; nothing was sent",
                    path.display()
                )),
            })
            .collect::<Result<_>>()?,
        Some(_) => bail!(
            "{} holds no object of keys at `unknown`; nothing was sent",
            path.display()
        ),
    };
    Ok(Record { refused, unknown })
}

/// Writes `record` as the record `file` in the case's artifacts directory `dir` through a file
/// beside it renamed over it, so a later process reads the old count or the new one, never a
/// part. That file is made new (`O_EXCL`) without following a link at its name
/// (`O_NOFOLLOW`); whatever a stale one left there, a link among them, is removed first, never
/// written through.
pub fn record_refusals(dir: &Path, file: &str, record: &Record) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = record_name(file)?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(file);
    let partial = dir.join(format!(".{file}.{}.partial", std::process::id()));
    let mut document = json!({"format": REFUSALS_FORMAT, "refused": record.refused});
    if !record.unknown.is_empty() {
        document["unknown"] = json!(record.unknown);
    }
    let bytes = serde_json::to_vec_pretty(&document)?;
    let create = || {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&partial)
    };
    let mut file = match create() {
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(&partial)
                .with_context(|| format!("remove the stale {}", partial.display()))?;
            create()
        }
        opened => opened,
    }
    .with_context(|| format!("create {}", partial.display()))?;
    let written = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(error).with_context(|| format!("write {}", partial.display()));
    }
    std::fs::rename(&partial, &path).with_context(|| format!("write {}", path.display()))
}

/// Whether `url` is an endpoint takt calls, and if so whether its host is a loopback host:
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
    /// Whether the answer is a v1alpha2 error envelope ([`is_error_envelope`]), and not only
    /// something that names a code.
    pub envelope: bool,
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
    /// the rest of it: a v1alpha2 error envelope ([`is_error_envelope`]) with the code
    /// `unsupported`, HTTP 400, `request_id: null`, no attempt. A host without standing grants
    /// answers so to `idempotency_key`, having run nothing. Any other answer, one that only names
    /// the code among them, is no such refusal.
    #[must_use]
    pub fn refuses_a_member(&self) -> bool {
        self.envelope
            && self.status == 400
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

/// `text` without control characters, at most [`MESSAGE_BYTES`] characters of it.
pub(crate) fn bounded(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .take(MESSAGE_BYTES)
        .collect()
}

/// Whether `answer` is an error envelope of the v1alpha2 binding, as compatibility §5 closes
/// it: an object of `version: v1alpha2`, `request_id` (a string or null), `status: error`, an
/// `error` of a string `code`, a string `message` and an optional positive integer
/// `retry_after_seconds` and nothing else, no `result`, `audit_ref` and `audit_status`
/// consistent (`complete` and `incomplete` with a reference, `unavailable` and `not_required`
/// with null), and no member besides those and `source_audit` and `mutation`.
#[must_use]
pub fn is_error_envelope(answer: &Value) -> bool {
    const MEMBERS: [&str; 8] = [
        "version",
        "request_id",
        "status",
        "error",
        "audit_ref",
        "audit_status",
        "source_audit",
        "mutation",
    ];
    let Some(members) = answer.as_object() else {
        return false;
    };
    let Some(error) = answer["error"].as_object() else {
        return false;
    };
    let audited = match answer["audit_status"].as_str() {
        Some("complete" | "incomplete") => answer["audit_ref"].is_string(),
        Some("unavailable" | "not_required") => {
            members.contains_key("audit_ref") && answer["audit_ref"].is_null()
        }
        _ => false,
    };
    members.keys().all(|name| MEMBERS.contains(&name.as_str()))
        && answer["version"] == "v1alpha2"
        && members
            .get("request_id")
            .is_some_and(|id| id.is_null() || id.is_string())
        && answer["status"] == "error"
        && audited
        && error
            .keys()
            .all(|name| matches!(name.as_str(), "code" | "message" | "retry_after_seconds"))
        && error.get("code").is_some_and(Value::is_string)
        && error.get("message").is_some_and(Value::is_string)
        && error
            .get("retry_after_seconds")
            .is_none_or(|seconds| seconds.as_u64().is_some_and(|seconds| seconds > 0))
}

/// The error of a served call that did not succeed, from its HTTP status and its answer. The
/// code and the message of an answer that is no envelope are read from its top level, for the
/// message alone: such an answer is never taken for the refusal of a member
/// ([`ServedRefused::refuses_a_member`]).
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
        envelope: is_error_envelope(&answer),
    }
}

/// What `GET /v1/describe` reported: the instance, the descriptor revision every invocation
/// names, and the ids of the operations it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// The instance's name.
    pub instance: String,
    /// The descriptor revision.
    pub revision: String,
    /// The ids of the operations described, in the order reported.
    pub operations: Vec<String>,
}

/// The client of one served Connectors instance. Each invocation carries a `request_id` of its
/// own, `takt-<process id>-<the client's start in nanoseconds>-<call>`, and an answer for
/// another request is refused.
pub struct Client {
    /// The URL as given, without a trailing `/`.
    endpoint: String,
    /// Whether its host is a loopback host: then no proxy is asked.
    loopback: bool,
    calls: Cell<u64>,
    started: u128,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// The client of the instance served at `url`, refused with the rule it breaks when it is no
    /// endpoint takt calls ([`check_endpoint`]); nothing is called.
    pub fn at(url: &str) -> std::result::Result<Self, &'static str> {
        let loopback = check_endpoint(url)?;
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        Ok(Self {
            endpoint: url.trim_end_matches('/').to_owned(),
            loopback,
            calls: Cell::new(0),
            started,
        })
    }

    /// The endpoint, without a trailing `/`.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// What an error names the invocation of `operation` by.
    #[must_use]
    pub fn named(&self, operation: &str) -> String {
        format!("`POST {INVOKE_PATH}` {operation} at {}", self.endpoint)
    }

    /// One `GET /v1/describe`: what the instance describes. Its `instance` must be `instance`
    /// when one is given, which the caller's key `named_by` names. Every error ends with
    /// `consequence`, what the caller did not do because of it.
    pub fn describe(
        &self,
        instance: Option<&str>,
        named_by: &str,
        consequence: &str,
    ) -> Result<Description> {
        let endpoint = &self.endpoint;
        let call = format!("GET {DESCRIBE_PATH} at {endpoint}");
        let (status, bytes) = exchange(
            endpoint,
            self.loopback,
            reqwest::Method::GET,
            DESCRIBE_PATH,
            None,
        )
        .with_context(|| format!("{call} did not answer; {consequence}"))?;
        let answer: Option<Value> = serde_json::from_slice(&bytes).ok();
        let Some(answer) = answer.filter(|_| status == 200) else {
            return Err(anyhow::Error::new(refused(call, status, &bytes)))
                .context(consequence.to_owned());
        };
        let text = |name: &str| {
            answer[name]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{call} reported no {name}; {consequence}"))
        };
        let revision = text("revision")?;
        let served = text("instance")?;
        if let Some(instance) = instance
            && instance != served
        {
            bail!(
                "{call} describes the instance {served}, not {instance}, which {named_by} names; {consequence}"
            );
        }
        let operations = answer["operations"]
            .as_array()
            .ok_or_else(|| anyhow!("{call} reported no operations; {consequence}"))?
            .iter()
            .filter_map(|operation| operation["id"].as_str().map(str::to_owned))
            .collect();
        Ok(Description {
            instance: served,
            revision,
            operations,
        })
    }

    /// One `POST /v1alpha2/invoke` of `operation` on `input` at the descriptor revision
    /// `revision`, carrying `key` as its `idempotency_key` when one is given: the operation's
    /// result. A replay of a key (`mutation.replayed: true`) is read as the original outcome it
    /// is. An answer that is no success is a [`ServedRefused`], naming its code.
    pub fn invoke(
        &self,
        revision: &str,
        operation: &str,
        input: &Value,
        key: Option<&str>,
    ) -> Result<Value> {
        self.answer(revision, operation, input, key)
            .map(|answer| answer["result"].clone())
    }

    /// [`Self::invoke`]'s call, its whole success answer: the `result` and, for a write, the
    /// `mutation` the instance recorded.
    pub fn answer(
        &self,
        revision: &str,
        operation: &str,
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
            &self.endpoint,
            self.loopback,
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

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread::JoinHandle;

    use serde_json::json;

    use super::*;

    /// One request the fake instance read: its request line, its header lines as sent and its
    /// body.
    struct Seen {
        line: String,
        headers: Vec<String>,
        body: Value,
    }

    /// A served instance on 127.0.0.1 that answers `calls` requests, one connection each, with
    /// what `answer` makes of the path and the body: the URL and, once it has answered them all,
    /// what it read.
    fn serve(
        calls: usize,
        answer: impl Fn(&str, &Value) -> (u16, Value) + Send + 'static,
    ) -> (String, JoinHandle<Vec<Seen>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let served = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for _ in 0..calls {
                let (stream, _) = listener.accept().expect("a connection");
                let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
                let mut line = String::new();
                reader.read_line(&mut line).expect("the request line");
                let mut headers = Vec::new();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).expect("a header");
                    let header = header.trim_end().to_owned();
                    if header.is_empty() {
                        break;
                    }
                    headers.push(header);
                }
                let length = headers
                    .iter()
                    .find_map(|header| {
                        let (name, value) = header.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).expect("the body");
                let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                let path = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                let (status, reply) = answer(&path, &body);
                let reply = reply.to_string();
                let mut stream = stream;
                write!(
                    stream,
                    "HTTP/1.1 {status} Answer\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                )
                .expect("the answer");
                seen.push(Seen {
                    line: line.trim_end().to_owned(),
                    headers,
                    body,
                });
            }
            seen
        });
        (url, served)
    }

    /// The description the fake instance gives: the instance `team` at revision `r1`.
    fn described() -> (u16, Value) {
        (
            200,
            json!({
                "instance": "team",
                "revision": "r1",
                "operations": [{"id": "conversations.replies"}, {"id": "chat.postMessage"}],
            }),
        )
    }

    /// A directory of its own under the temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "takt-connectors-client-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("the scratch directory");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An operation named only at run time is described and invoked, and its result returned;
    /// no request carries an `Authorization` header.
    #[test]
    fn client_invokes_a_runtime_operation() {
        let (url, served) = serve(2, |path, body| match path {
            DESCRIBE_PATH => described(),
            _ => (
                200,
                json!({
                    "version": "v1alpha2",
                    "request_id": body["request_id"],
                    "status": "success",
                    "result": {"messages": [{"ts": "1700000000.000100", "text": "hello"}]},
                }),
            ),
        });
        let operation: String = ["conversations", "replies"].join(".");
        let client = Client::at(&url).expect("a loopback endpoint");
        let description = client
            .describe(Some("team"), "`connectors.instance`", "nothing was sent")
            .expect("the instance describes itself");
        assert_eq!(description.instance, "team");
        assert_eq!(description.revision, "r1");
        assert!(description.operations.contains(&operation));
        let input = json!({"channel": "C0000000", "ts": "1700000000.000100"});
        let result = client
            .invoke(&description.revision, &operation, &input, None)
            .expect("the operation answers");
        assert_eq!(result["messages"][0]["text"], "hello");
        let seen = served.join().expect("the instance");
        assert_eq!(seen[0].line, format!("GET {DESCRIBE_PATH} HTTP/1.1"));
        assert_eq!(seen[1].line, format!("POST {INVOKE_PATH} HTTP/1.1"));
        let sent = &seen[1].body;
        assert_eq!(sent["version"], "v1alpha2");
        assert_eq!(sent["operation"], operation.as_str());
        assert_eq!(sent["revision"], "r1");
        assert_eq!(sent["input"], input);
        assert!(sent.get("idempotency_key").is_none(), "{sent}");
        for request in &seen {
            assert!(
                !request.headers.iter().any(|header| header
                    .split_once(':')
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case("authorization"))),
                "{}: {:?}",
                request.line,
                request.headers
            );
        }
    }

    /// An answer `approval_required` is an error naming that code, its status kept; the key
    /// given went with the call.
    #[test]
    fn client_names_a_refusal() {
        let (url, served) = serve(2, |path, body| match path {
            DESCRIBE_PATH => described(),
            _ => (
                403,
                json!({
                    "version": "v1alpha2",
                    "request_id": body["request_id"],
                    "status": "error",
                    "error": {"code": "approval_required", "message": "no standing grant covers it"},
                    "audit_ref": null,
                    "audit_status": "not_required",
                }),
            ),
        });
        let operation = String::from("chat.postMessage");
        let client = Client::at(&url).expect("a loopback endpoint");
        let description = client
            .describe(None, "`connectors.instance`", "nothing was sent")
            .expect("the instance describes itself");
        let input = json!({"channel": "C0000000", "text": "status"});
        let key = idempotency_key("case-1", &operation, &description.revision, &input, 0);
        let error = client
            .invoke(&description.revision, &operation, &input, Some(&key))
            .expect_err("the write is refused");
        let said = format!("{error:#}");
        assert!(said.contains("approval_required"), "{said}");
        assert!(said.contains(&operation), "{said}");
        let refusal = error
            .downcast_ref::<ServedRefused>()
            .expect("a served refusal");
        assert_eq!(refusal.status, 403);
        assert_eq!(refusal.code.as_deref(), Some("approval_required"));
        assert!(refusal.envelope);
        let seen = served.join().expect("the instance");
        assert_eq!(seen[1].body["idempotency_key"], key.as_str());
    }

    /// Two callers in one case, under two file names, keep two records: writing one leaves the
    /// other as it was.
    #[test]
    fn client_refusal_records_are_per_caller() {
        let scratch = Scratch::new("per-caller");
        let dir = scratch.0.join("case-1").join("artifacts");
        let (forge, action) = ("pull-request.refusals.json", "status.publish.refusals.json");
        let input = json!({"a": 1});
        let first = idempotency_key("case-1", "pull_request.merge", "r1", &input, 0);
        let other = idempotency_key("case-1", "chat.postMessage", "r1", &input, 0);
        assert!(
            refusals_in(&dir, forge)
                .expect("none yet")
                .refused
                .is_empty()
        );
        let mut merges = Record::default();
        merges.refused.insert(first.clone(), 1);
        merges.unknown.insert(first.clone(), None);
        record_refusals(&dir, forge, &merges).expect("the forge's record");
        let mut posts = Record::default();
        posts.refused.insert(other.clone(), 2);
        record_refusals(&dir, action, &posts).expect("the action's record");
        posts.refused.insert(other.clone(), 3);
        record_refusals(&dir, action, &posts).expect("the action's record again");
        let read = refusals_in(&dir, forge).expect("the forge's record");
        assert_eq!(read.refused, BTreeMap::from([(first.clone(), 1)]));
        assert_eq!(read.unknown, BTreeMap::from([(first, None)]));
        let read = refusals_in(&dir, action).expect("the action's record");
        assert_eq!(read.refused, BTreeMap::from([(other, 3)]));
        assert!(read.unknown.is_empty());
        for file in [forge, action] {
            let text = std::fs::read_to_string(dir.join(file)).expect("a record");
            let document: Value = serde_json::from_str(&text).expect("JSON");
            assert_eq!(document["format"], "takt.connectors.refusals/1");
        }
        for bad in ["", "a/b.json", "../b.json", "."] {
            assert!(
                record_refusals(&dir, bad, &posts).is_err(),
                "{bad:?} is no file name of a record"
            );
        }
    }
}
