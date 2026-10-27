//! The forge a run file names with `forge: connectors` and `forge_instance`: each action of the
//! [`Forge`] seam is one `connectors operations invoke` on that instance, run through the forge
//! [`Runner`] as an argument list, never a shell line. It is optional and never a default: no run
//! reaches it without both keys. The push stays takt's own, scanned push over HTTPS to the project
//! the `origin` remote names; this forge replaces the host's API calls and nothing before them.
//!
//! The operations are the instance's `gitlab` connector's, and each takes the project's path
//! from `origin`:
//!
//! | action | operation | input | read from |
//! |---|---|---|---|
//! | open | `merge_request.create` | `id`, `sha` (the case branch's commit), `body` | `body.iid` |
//! | read | `merge_request.get` | `id`, `merge_request_iid` | `state`, `source_branch`, `target_branch`, `sha`, `head_pipeline` |
//! | checks | `pipeline.get` | `id`, `pipeline_id` | `sha`, `status` |
//! | merge | `merge_request.merge` | `id`, `merge_request_iid`, `pipeline_id`, `body.sha` | `state` |
//!
//! `operations invoke` takes the schema and the revision of the operation, which `operations
//! describe` reports once per operation and this forge keeps for the run.
//!
//! **Preflight.** [`ConnectorsForge::for_tree`] lists the instance's operations
//! (`connectors operations list --instance <name> --output json`, every page) before any case
//! opens, and refuses naming each of [`REQUIRED`] the instance does not list: change request
//! create, read and merge, and pipeline read. An instance lists an operation only when its
//! permissions admit it, so a write the instance does not admit is a missing operation. What it
//! found, the connector, the instance and the descriptor revision, is [`ConnectorsForge::identity`].
//!
//! **Checks are bound to the head.** The checks are the status of the pipeline the change
//! request's `head_pipeline` names, read only when that pipeline is the change request's head's
//! (its `sha` is the head): a pipeline of another head, from the change request or from
//! `pipeline.get`, is not read as a result and the checks stay pending. The merge is invoked at
//! the head the caller gives, with the pipeline the checks were read on; a head with no such
//! reading is refused with no invocation, and the instance's own guard refuses a head that moved.
//!
//! A write that `connectors` holds for an approval fails with its own refusal: no approval file is
//! given here. Standard error of a failed line is kept in the error, bounded, and an input never
//! is.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::run_config::ForgeSettings;
use crate::scan::Scan;

use super::{
    BotRoute, Checks, Failed, Forge, Head, IdentityScan, Outgoing, PullRequest, PullRequestState,
    PullRequestView, Runner, Scanner, exact_ref, fresh, head_is_a_commit, held_by, push_over_https,
    succeed, write_request,
};

/// The program every line runs.
pub const PROGRAM: &str = "connectors";

/// The operations an instance must list: change request create, read and merge, and pipeline
/// read.
pub const REQUIRED: [&str; 4] = [
    "merge_request.create",
    "merge_request.get",
    "merge_request.merge",
    "pipeline.get",
];

const CREATE: &str = REQUIRED[0];
const GET: &str = REQUIRED[1];
const MERGE: &str = REQUIRED[2];
const PIPELINE: &str = REQUIRED[3];

/// The most pages of a listing read, so a listing that never ends is an error.
const PAGES: usize = 50;

/// The most bytes of a failed line's standard error kept in the error.
const STDERR_BYTES: usize = 600;

/// Whether `name` is a plain word for an instance: letters, digits, `.`, `_` and `-`, not
/// starting with `-`, so it is one argument and never a flag.
#[must_use]
pub fn is_instance_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// The adapter over `connectors operations invoke` on one instance.
pub struct ConnectorsForge {
    instance: String,
    project: String,
    origin: (String, String),
    identity: String,
    /// The run's scan adapter.
    pub scan: Scan,
    /// The most bytes of commit messages and files one scan before a push is given.
    pub scan_limit: usize,
    runner: Box<dyn Runner>,
    described: RefCell<BTreeMap<&'static str, (String, String)>>,
    bound: RefCell<BTreeMap<u64, (String, u64)>>,
}

impl fmt::Debug for ConnectorsForge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorsForge")
            .field("instance", &self.instance)
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
    /// The file under the case's artifacts directory that holds what `merge_request.create`
    /// answered.
    pub const RESPONSE: &'static str = "connectors-create.json";
    /// The file that holds what `merge_request.merge` answered.
    pub const MERGE_RESPONSE: &'static str = "connectors-merge.json";

    /// The forge `forge` names for a run: the adapter over `settings.instance` when `forge` is
    /// `connectors`, after the settings are checked; `None` for every other forge and for none,
    /// and then no line is run. This is the one place the adapter is chosen, so a run file
    /// without `forge: connectors` never reaches `connectors`.
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
        let instance = settings
            .instance
            .as_deref()
            .ok_or_else(|| anyhow!("`forge: connectors` needs `forge_instance`"))?;
        Self::for_tree(tree, instance, scan, runner).map(Some)
    }

    /// Over the instance `instance`, after its preflight: the project `tree`'s `origin` names,
    /// and the operations the instance lists, [`REQUIRED`] among them, else an error naming
    /// every one it does not, before anything is opened. The push and the scan go to the
    /// project `origin` names. Every line runs through `runner`.
    pub fn for_tree(
        tree: &Path,
        instance: &str,
        scan: Scan,
        runner: Box<dyn Runner>,
    ) -> Result<Self> {
        if !is_instance_name(instance) {
            bail!(
                "`forge_instance` is one word of letters, digits, `.`, `_` and `-`, not starting with `-`"
            );
        }
        let origin = crate::git::remote_url(tree, "origin")
            .ok()
            .and_then(|url| super::gitlab::project_of(&url))
            .ok_or_else(|| {
                anyhow!(
                    "the forge connectors names the project by the origin remote, and it names none; nothing was opened"
                )
            })?;
        let mut forge = Self {
            instance: instance.to_owned(),
            project: origin.1.clone(),
            origin,
            identity: String::new(),
            scan,
            scan_limit: BotRoute::SCAN_LIMIT,
            runner,
            described: RefCell::new(BTreeMap::new()),
            bound: RefCell::new(BTreeMap::new()),
        };
        forge.preflight(tree)?;
        Ok(forge)
    }

    /// The instance the preflight found: its connector, its name and the revision of its
    /// descriptor.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Lists every page of the instance's operations and refuses the ones [`REQUIRED`] that are
    /// not among them.
    fn preflight(&mut self, tree: &Path) -> Result<()> {
        let mut listed: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        let mut first: Option<Value> = None;
        for _ in 0..PAGES {
            let mut args = vec!["list"];
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
            }
            let page = self.connectors(tree, &args, "operations list")?;
            listed.extend(
                page["operations"]
                    .as_array()
                    .ok_or_else(|| {
                        anyhow!(
                            "`connectors operations list` on the instance {} reported no operations",
                            self.instance
                        )
                    })?
                    .iter()
                    .filter_map(|operation| operation["id"].as_str().map(str::to_owned)),
            );
            cursor = page["next_cursor"]
                .as_str()
                .filter(|next| !next.is_empty())
                .map(str::to_owned);
            first.get_or_insert(page);
            if cursor.is_none() {
                break;
            }
        }
        if cursor.is_some() {
            bail!(
                "`connectors operations list` on the instance {} has more than {PAGES} pages",
                self.instance
            );
        }
        let missing: Vec<&str> = REQUIRED
            .into_iter()
            .filter(|name| !listed.iter().any(|listed| listed == name))
            .collect();
        if !missing.is_empty() {
            bail!(
                "the instance {} does not list {}, which `forge: connectors` needs, and a write is listed only when the instance admits it; nothing was opened",
                self.instance,
                missing.join(", ")
            );
        }
        let first = first.unwrap_or_default();
        let word = |name: &str| first[name].as_str().unwrap_or("unnamed").to_owned();
        self.identity = format!(
            "connector {}, instance {}, descriptor {}",
            word("connector"),
            word("instance"),
            word("revision")
        );
        Ok(())
    }

    /// Runs `connectors operations <args> --instance <name> --output json`: the `result` of an
    /// answer that is `ok`. `label` is what an error names the line by, never its input.
    fn connectors(&self, tree: &Path, args: &[&str], label: &str) -> Result<Value> {
        let mut argv: Vec<String> = [PROGRAM, "operations"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        argv.extend(args.iter().map(|word| (*word).to_owned()));
        argv.extend(
            ["--instance", &self.instance, "--output", "json"]
                .into_iter()
                .map(str::to_owned),
        );
        let exit = self
            .runner
            .run(tree, &argv)
            .with_context(|| format!("`{PROGRAM} {label}` could not be run"))?;
        let line = format!("{PROGRAM} {label} --instance {}", self.instance);
        let document: Option<Value> = serde_json::from_str(&exit.stdout).ok();
        if !exit.success() {
            let coded = document
                .as_ref()
                .and_then(|document| document["error"]["code"].as_str())
                .map(|code| format!("{code}: "))
                .unwrap_or_default();
            let stderr: String = exit.stderr.trim().chars().take(STDERR_BYTES).collect();
            return Err(Failed {
                line,
                code: exit.code,
                stderr: format!("{coded}{stderr}"),
            }
            .into());
        }
        match document {
            Some(document) if document["ok"] == true && !document["result"].is_null() => {
                Ok(document["result"].clone())
            }
            _ => bail!("{line} reported no result: its output is not an ok answer"),
        }
    }

    /// The schema and the revision of `operation`, described once and kept.
    fn described(&self, tree: &Path, operation: &'static str) -> Result<(String, String)> {
        if let Some(held) = self.described.borrow().get(operation) {
            return Ok(held.clone());
        }
        let answer = self.connectors(
            tree,
            &["describe", "--operation", operation],
            &format!("operations describe {operation}"),
        )?;
        let text = |name: &str| {
            answer[name]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    anyhow!("`connectors operations describe {operation}` reported no {name}")
                })
        };
        let held = (text("schema")?, text("revision")?);
        self.described.borrow_mut().insert(operation, held.clone());
        Ok(held)
    }

    /// Invokes `operation` once on `input`: the provider's `body`.
    fn invoke(&self, tree: &Path, operation: &'static str, input: &Value) -> Result<Value> {
        let (schema, revision) = self.described(tree, operation)?;
        let input = input.to_string();
        let answer = self.connectors(
            tree,
            &[
                "invoke",
                "--operation",
                operation,
                "--schema",
                &schema,
                "--revision",
                &revision,
                "--input-json",
                &input,
            ],
            &format!("operations invoke {operation}"),
        )?;
        let outcome = &answer["result"];
        match outcome["status"].as_u64() {
            Some(200..=299) | None => {}
            Some(status) => bail!(
                "`connectors operations invoke {operation}` on the instance {} reported the status {status}",
                self.instance
            ),
        }
        Ok(outcome["body"].clone())
    }

    /// The change request `number`, with the pipeline its head names.
    fn read(&self, tree: &Path, number: u64) -> Result<Request> {
        let body = self.invoke(
            tree,
            GET,
            &json!({"id": self.project, "merge_request_iid": number}),
        )?;
        let field = |name: &str| -> Result<String> {
            body[name]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("`connectors operations invoke {GET}` reported no {name}"))
        };
        let state = match body["state"].as_str() {
            Some("opened" | "locked") => PullRequestState::Open,
            Some("merged") => PullRequestState::Merged,
            Some("closed") => PullRequestState::Closed,
            other => bail!(
                "`connectors operations invoke {GET}` reported the state {other:?}, not opened, locked, merged or closed"
            ),
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

    /// The checks on the head of change request `number`, and the pipeline they were read
    /// on, `None` while no pipeline of that head says anything.
    fn read_checks(&self, tree: &Path, number: u64) -> Result<(Checks, String, Option<u64>)> {
        let request = self.read(tree, number)?;
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
                "`connectors operations invoke {PIPELINE}` reported a pipeline in the status {other:?}"
            ),
        };
        Ok((checks, head, Some(id)))
    }

    /// The project the push and the scan go to.
    fn push_url(&self) -> String {
        format!("https://{}/{}.git", self.origin.0, self.origin.1)
    }

    /// The scan before a push and a request: the run's scan adapter's.
    fn scanner(&self) -> Option<Scanner<'_>> {
        match &self.scan {
            Scan::Gates { policy } => Some(Scanner {
                runner: &*self.runner,
                repository: &self.origin.1,
                policy: policy.as_deref(),
            }),
            Scan::None { .. } => None,
        }
    }
}

impl Forge for ConnectorsForge {
    fn name(&self) -> &str {
        "connectors"
    }

    /// The run's scan adapter over what pushing would publish and the request's title and body,
    /// every commit of the head; under a declared `none`, nothing is scanned.
    fn scan_outgoing(&self, tree: &Path, dir: &Path, outgoing: &Outgoing<'_>) -> Result<()> {
        head_is_a_commit(tree, outgoing.branch, outgoing.head)?;
        let Some(scanner) = self.scanner() else {
            return Ok(());
        };
        let held = held_by(tree, "origin", outgoing.base, || Ok(None))?;
        scanner.outgoing(
            tree,
            dir,
            outgoing,
            held.as_deref(),
            self.scan_limit,
            write_request,
            "pull request",
        )
    }

    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &crate::git::CommitIdentity,
    ) -> Result<IdentityScan> {
        match self.scanner() {
            Some(scanner) => scanner.identity(tree, dir, identity, "the forge connectors"),
            None => Ok(IdentityScan::NotScanned(
                self.scan.not_scanned().unwrap_or_default(),
            )),
        }
    }

    fn push(&self, tree: &Path, branch: &str) -> Result<()> {
        let at = exact_ref(tree, &format!("refs/heads/{branch}"))
            .ok_or_else(|| anyhow!("{branch} names no commit; nothing was pushed"))?;
        self.push_revision(tree, branch, &at)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> Result<()> {
        let argv: Vec<String> = [
            "git".to_owned(),
            "push".to_owned(),
            self.push_url(),
            format!("{revision}:refs/heads/{branch}"),
            "--no-follow-tags".to_owned(),
            "--recurse-submodules=no".to_owned(),
        ]
        .to_vec();
        push_over_https(
            &*self.runner,
            tree,
            &argv,
            &self.push_url(),
            "the forge connectors",
        )
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Result<u64> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::RESPONSE))?;
        let (input, published) = write_request(dir, request)?;
        if let Some(scanner) = self.scanner() {
            succeed(&*self.runner, tree, &scanner.argv(&[&input, &published])).with_context(|| {
                format!(
                    "the pull request for {branch} did not pass scan-text over {} and was not opened; its title and description stayed on this machine, but the branch {branch} was pushed before this scan",
                    published.display(),
                    branch = request.branch
                )
            })?;
        }
        let head = exact_ref(tree, &format!("refs/heads/{}", request.branch)).ok_or_else(|| {
            anyhow!(
                "{} names no commit; no change request was opened",
                request.branch
            )
        })?;
        let body = self.invoke(
            tree,
            CREATE,
            &json!({
                "id": self.project,
                "sha": head,
                "body": {
                    "source_branch": request.branch,
                    "target_branch": request.base,
                    "title": request.title,
                    "description": request.body,
                },
            }),
        )?;
        std::fs::write(&output, serde_json::to_vec_pretty(&body)?)
            .with_context(|| format!("write the response {}", output.display()))?;
        body["iid"].as_u64().ok_or_else(|| {
            anyhow!(
                "`connectors operations invoke {CREATE}` reported no iid: no change request number"
            )
        })
    }

    fn pull_request(&self, tree: &Path, number: u64) -> Result<PullRequestView> {
        Ok(self.read(tree, number)?.view)
    }

    fn checks(&self, tree: &Path, number: u64) -> Result<Checks> {
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

    /// Merges at `head` with the pipeline the checks were read on: the reading kept by
    /// [`Forge::checks`] when it is of `head`, else a reading made now, which must be of `head`
    /// too; a head with none is refused with no merge invoked.
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Result<()> {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let output = fresh(dir.join(Self::MERGE_RESPONSE))?;
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
        let body = self.invoke(
            tree,
            MERGE,
            &json!({
                "id": self.project,
                "merge_request_iid": number,
                "pipeline_id": pipeline,
                "body": {"sha": head},
            }),
        )?;
        std::fs::write(&output, serde_json::to_vec_pretty(&body)?)
            .with_context(|| format!("write the response {}", output.display()))?;
        if let Some(state) = body["state"].as_str()
            && state != "merged"
        {
            bail!(
                "`connectors operations invoke {MERGE}` did not merge change request {number}: its state is {state}"
            );
        }
        Ok(())
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
    }
}
