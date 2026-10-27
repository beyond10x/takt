//! The seam every child process of a run starts through: the adapter in use ([`current`]) is
//! asked before any program starts, and the test command is run by it ([`Confinement::run_test`]).
//! Both adapters run the test command through Substrate's confinement contract
//! (`substrate_host::confinement`); takt holds no confinement mechanism of its own.
//!
//! | adapter | name | a program | the test command |
//! |---|---|---|---|
//! | [`Unconfined`] | `none` | admitted | run by Substrate's `local` adapter, requested with a reason ([`LOCAL_REASON`]), so the run is declared insecure ([`unconfined`]): on the host, no credential and no login location in its environment, a scratch home and `TMPDIR`, a timeout and a bounded tail |
//! | [`Substrate`] | `substrate` | admitted, but a Python, which takt would otherwise start for a test command at case open, and every command a gate's library starts itself | run by the adapter chosen by facts from [`CANDIDATES`] (`systemd` around `linux`, `linux`, `macos`): the tree its workspace, its `target/` the build's, the toolchain and the cargo caches read-only, no network, process bounds where the adapter serves `limits`; one it cannot serve is refused by name, never run unconfined |
//!
//! [`Command`] is the one type of takt that starts a process: [`std::process::Command`] with
//! the adapter asked first, at every start. No module of takt but this one names
//! [`std::process::Command`]. Confining the harness session, git, the forge and `aep` is not
//! this seam's yet: the adapter admits them, and they run as they did.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, CommandArgs, CommandEnvs, ExitStatus, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Context as _, anyhow, bail};
use serde::{Deserialize, Serialize};
use substrate_host::confinement::{
    self as substrate, AdapterFacts, AdapterRequest, AdmittedProfile, ConfinementAdapter,
    ConfinementError, ConfinementProfile, ContractFamily, DeclaredWritableRoot, FACT_ORDER,
    FAMILY_ORDER, Fact, HostConfig, InnerAdapter, Network, ProcessBounds, ReadOnlyRoot, RunExit,
    RunId, RunObservation, RunOutput, RunState, Signal, WritableHostRoot,
};

use crate::forge::{Finished, Limits, TimedOut};

/// A child process to start: [`std::process::Command`], whose every start ([`Command::spawn`],
/// [`Command::output`], [`Command::status`]) asks the adapter in use ([`current`]) for its
/// program first and starts nothing when it is refused.
#[derive(Debug)]
pub struct Command {
    inner: std::process::Command,
}

impl Command {
    /// A command for `program`, as [`std::process::Command::new`].
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            inner: std::process::Command::new(program),
        }
    }

    /// One more argument.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    /// More arguments.
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    /// One variable set.
    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(&mut self, key: K, value: V) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    /// Variables set.
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.envs(vars);
        self
    }

    /// One variable removed from what the child inherits.
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    /// Nothing inherited.
    pub fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self
    }

    /// The child's working directory.
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.inner.current_dir(dir);
        self
    }

    /// The child's standard input.
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdin(cfg);
        self
    }

    /// The child's standard output.
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdout(cfg);
        self
    }

    /// The child's standard error.
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stderr(cfg);
        self
    }

    /// The child's process group, as [`std::os::unix::process::CommandExt::process_group`].
    #[cfg(unix)]
    pub fn process_group(&mut self, group: i32) -> &mut Self {
        std::os::unix::process::CommandExt::process_group(&mut self.inner, group);
        self
    }

    /// `step` run in the child between fork and exec, as
    /// [`std::os::unix::process::CommandExt::pre_exec`].
    ///
    /// # Safety
    ///
    /// What [`std::os::unix::process::CommandExt::pre_exec`] requires of `step`: it runs in the
    /// forked child, where only async-signal-safe calls may be made.
    #[cfg(unix)]
    pub unsafe fn pre_exec<F>(&mut self, step: F) -> &mut Self
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        // SAFETY: the caller holds `step` to what `pre_exec` requires.
        unsafe {
            std::os::unix::process::CommandExt::pre_exec(&mut self.inner, step);
        }
        self
    }

    /// The program, as given.
    #[must_use]
    pub fn get_program(&self) -> &OsStr {
        self.inner.get_program()
    }

    /// The arguments, as given.
    pub fn get_args(&self) -> CommandArgs<'_> {
        self.inner.get_args()
    }

    /// The variables set or removed, as given.
    pub fn get_envs(&self) -> CommandEnvs<'_> {
        self.inner.get_envs()
    }

    /// The working directory, when one was given.
    #[must_use]
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.inner.get_current_dir()
    }

    /// Starts the child once the adapter in use admitted its program.
    ///
    /// # Errors
    ///
    /// The adapter's refusal ([`refused`]), and nothing started; else what the start returned.
    pub fn spawn(&mut self) -> io::Result<Child> {
        current().admit(self.get_program())?;
        self.inner.spawn()
    }

    /// Runs the child to its end once the adapter in use admitted its program, as
    /// [`std::process::Command::output`].
    ///
    /// # Errors
    ///
    /// The adapter's refusal ([`refused`]), and nothing started; else what the run returned.
    pub fn output(&mut self) -> io::Result<Output> {
        current().admit(self.get_program())?;
        self.inner.output()
    }

    /// Runs the child to its end once the adapter in use admitted its program, as
    /// [`std::process::Command::status`].
    ///
    /// # Errors
    ///
    /// The adapter's refusal ([`refused`]), and nothing started; else what the run returned.
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        current().admit(self.get_program())?;
        self.inner.status()
    }
}

/// The test command one observation runs: its argument line, run without a shell in `tree`,
/// held to `limits`.
#[derive(Debug, Clone, Copy)]
pub struct TestRun<'a> {
    /// The program and its arguments.
    pub argv: &'a [String],
    /// The tree it runs in.
    pub tree: &'a Path,
    /// What it is held to.
    pub limits: Limits,
}

/// The Substrate adapter a run's test command goes through, as the run's `opened` event records
/// it and its first line names it ([`AdapterRecord::describe`]): the adapter, the one it wraps,
/// the contract families it serves and the facts it proved when it opened, all by their
/// contract names, and the reason a declared-insecure run was requested with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterRecord {
    /// The adapter: `local`, `linux`, `macos` or `systemd`.
    pub adapter: String,
    /// The adapter `systemd` wraps: `linux` or `local`; absent for every other adapter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner: Option<String>,
    /// The contract families it serves, in the contract's order.
    pub families: Vec<String>,
    /// The facts it proved, in the contract's order.
    pub facts: Vec<String>,
    /// Why the run is acceptable declared insecure: present exactly when the adapter proved
    /// `declared_insecure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_insecure: Option<String>,
}

impl AdapterRecord {
    /// The record of an opened adapter's proof, `reason` its declared-insecure reason.
    fn of(facts: &AdapterFacts, reason: Option<&str>) -> Self {
        Self {
            adapter: name_of(&facts.adapter),
            inner: facts.inner.map(|inner| name_of(&inner)),
            families: facts.served_families.iter().map(name_of).collect(),
            facts: facts.facts.iter().map(name_of).collect(),
            declared_insecure: facts
                .declared_insecure()
                .then(|| reason.unwrap_or_default().to_owned()),
        }
    }

    /// The adapter as the run's first line names it: `Substrate linux`, `Substrate systemd
    /// around linux`, or `Substrate local, declared insecure: <reason>`.
    #[must_use]
    pub fn describe(&self) -> String {
        let name = match &self.inner {
            Some(inner) => format!("{} around {inner}", self.adapter),
            None => self.adapter.clone(),
        };
        match &self.declared_insecure {
            Some(reason) => format!("Substrate {name}, declared insecure: {reason}"),
            None => format!("Substrate {name}"),
        }
    }

    /// Whether the two name the same adapter around the same inner one.
    fn same_adapter(&self, other: &Self) -> bool {
        self.adapter == other.adapter && self.inner == other.inner
    }
}

/// A Confinement adapter: what every child process of a run starts through.
pub trait Confinement: Send + Sync {
    /// The adapter's name, as the run's first line and its `opened` event state it.
    fn name(&self) -> &str;

    /// Whether `program` may start, as a [`Command`] names it; a refusal is an error naming the
    /// adapter and the program ([`refused`]).
    ///
    /// # Errors
    ///
    /// The refusal.
    fn admit(&self, program: &OsStr) -> io::Result<()>;

    /// Whether `program` may be started by a library takt hands it to, which starts it itself
    /// (a gate's command or provider): asked before the library runs; [`Confinement::admit`]
    /// unless the adapter says otherwise.
    ///
    /// # Errors
    ///
    /// The refusal.
    fn admit_delegated(&self, program: &OsStr) -> io::Result<()> {
        self.admit(program)
    }

    /// Runs the test command and returns how it ended; [`unconfined`] unless the adapter says
    /// otherwise.
    ///
    /// # Errors
    ///
    /// A refusal, a failure to start or wait, or a [`TimedOut`] past its timeout.
    fn run_test(&self, test: &TestRun<'_>) -> anyhow::Result<Finished> {
        unconfined(test)
    }

    /// The Substrate adapter the test command goes through, as the run's `opened` event records
    /// it; none unless the adapter says otherwise.
    fn substrate(&self) -> Option<AdapterRecord> {
        None
    }
}

/// The reason a `none` run's test command is requested of Substrate's `local` adapter with,
/// which declares it insecure: on the `opened` event and the run's first line.
pub const LOCAL_REASON: &str = "the run was started with --confinement none: its test command runs on the host as this user, unconfined";

/// The contract families every test command needs: it is started, watched, its output read and
/// waited for.
const RUN_FAMILIES: [ContractFamily; 4] = [
    ContractFamily::ProcessRun,
    ContractFamily::ProcessWatch,
    ContractFamily::OutputRead,
    ContractFamily::Wait,
];

/// How often a run is observed while it runs.
const POLL: Duration = Duration::from_millis(10);

/// How long a confined run killed at its timeout has to end and its output to close: the
/// `systemd` adapter's own drain of a killed tree.
const KILLED: Duration = Duration::from_secs(10);

/// The adapter `none`: every program is admitted, and the test command is run by Substrate's
/// `local` adapter, declared insecure ([`unconfined`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct Unconfined;

impl Confinement for Unconfined {
    fn name(&self) -> &str {
        "none"
    }

    fn admit(&self, _program: &OsStr) -> io::Result<()> {
        Ok(())
    }

    fn substrate(&self) -> Option<AdapterRecord> {
        local()
            .ok()
            .map(|adapter| AdapterRecord::of(adapter.facts(), Some(LOCAL_REASON)))
    }
}

/// Substrate's `local` adapter, opened once under the host's policy
/// (`SUBSTRATE_CONFINEMENT_LOCAL`).
fn local() -> anyhow::Result<Arc<dyn ConfinementAdapter>> {
    static LOCAL: OnceLock<Result<Arc<dyn ConfinementAdapter>, String>> = OnceLock::new();
    LOCAL
        .get_or_init(|| {
            substrate::open(AdapterRequest::Local(
                substrate::local::LocalRequest::default(),
            ))
            .map_err(|error| describe(&error))
        })
        .clone()
        .map_err(|error| anyhow!("Substrate's local adapter did not open: {error}"))
}

/// The profile a `none` run asks of `local`: the families every test command needs and those
/// `needs` names, the facts it names, `tree` the workspace, and [`LOCAL_REASON`].
fn local_profile(tree: &Path, needs: &Needs) -> ConfinementProfile {
    let mut families = RUN_FAMILIES.to_vec();
    families.extend(needs.families.iter().cloned());
    ConfinementProfile {
        families,
        facts: needs.facts.clone(),
        local_reason: Some(LOCAL_REASON.to_owned()),
        workspace: Some(tree.to_path_buf()),
        ..ConfinementProfile::default()
    }
}

/// Checks before a `none` run opens that Substrate's `local` adapter admits its test command's
/// profile on `tree` with what `options` needs: a family it does not serve, a fact it did not
/// prove, or the host's policy forbidding it is refused by name, and nothing is started.
///
/// # Errors
///
/// The refusal, or a name in `options.needs` that is no family and no fact.
pub fn check_none(tree: &Path, options: &Options) -> anyhow::Result<AdapterRecord> {
    let needs = Needs::parse(&options.needs)?;
    let adapter = local()?;
    adapter
        .admit(&local_profile(tree, &needs))
        .map_err(|error| anyhow!("{}", describe(&error)))?;
    Ok(AdapterRecord::of(adapter.facts(), Some(LOCAL_REASON)))
}

/// Runs the test command as takt starts it under `none`: through Substrate's `local` adapter,
/// requested with [`LOCAL_REASON`] and so declared insecure, without a shell, on the host, in
/// `tree`. Its environment is takt's own without a credential ([`crate::forge::is_credential`]),
/// a login location ([`crate::forge::LOGIN_LOCATIONS`]) or a variable outside Substrate's
/// non-secret shape, with a scratch home and `TMPDIR` of its own
/// ([`crate::validator::FreshHome`]); past its timeout it is killed with what it started, and of
/// each output stream the last `limits.output` bytes of what Substrate kept are returned. The
/// program is still asked of the adapter in use, as every [`Command`] is.
///
/// What the run can read is what this user can: the user's home, its files and the sockets under
/// the runtime directory included. Only its environment names none of them.
///
/// # Errors
///
/// An empty line, a refusal, a failure to start or follow it, what it started still holding its
/// output open once killed, or a [`TimedOut`] past its timeout.
pub fn unconfined(test: &TestRun<'_>) -> anyhow::Result<Finished> {
    let (program, _) = test
        .argv
        .split_first()
        .ok_or_else(|| anyhow!("an empty test command"))?;
    current().admit(OsStr::new(program))?;
    let adapter = local()?;
    let admitted = adapter
        .admit(&local_profile(test.tree, &Needs::default()))
        .map_err(|error| anyhow!("confinement none refused {program}: {}", describe(&error)))?;
    let home = crate::validator::FreshHome::new()?;
    let tmp = home.dir().join("tmp");
    std::fs::create_dir(&tmp).with_context(|| format!("create {}", tmp.display()))?;
    let mark = crate::forge::next_mark();
    let command = substrate::Command {
        argv: test.argv.to_vec(),
        env: host_environment(&home, &tmp, &mark)?,
    };
    let deadline = Instant::now() + test.limits.timeout;
    let (run, led) = crate::forge::lead(&mark, || {
        let run = start(&*adapter, &admitted, &command)?;
        let pid = run
            .as_str()
            .strip_prefix("local-")
            .and_then(|rest| rest.split('-').next())
            .and_then(|pid| pid.parse().ok())
            .ok_or_else(|| anyhow!("Substrate's local run {} names no process", run.as_str()))?;
        Ok((run, pid))
    })?;
    let outcome = follow_on_host(
        &*adapter,
        &run,
        led.pid(),
        &mark,
        deadline,
        program,
        test.limits,
    );
    let swept = crate::forge::sweep_led(led.pid(), &mark);
    drop(led);
    drop(home);
    let finished = outcome?;
    swept?;
    Ok(finished)
}

/// Follows a `local` run led by `leader` to its end: past `deadline` it is killed with what it
/// started; once its leader ended, what still holds its output is killed.
fn follow_on_host(
    adapter: &dyn ConfinementAdapter,
    run: &RunId,
    leader: u32,
    mark: &str,
    deadline: Instant,
    program: &str,
    limits: Limits,
) -> anyhow::Result<Finished> {
    let followed = block_on(follow(adapter, run, Some(leader), deadline))?.map_err(failed)?;
    match followed {
        Followed::Ended(seen) => {
            known(&seen, program)?;
            let output = block_on(adapter.output(run))?.map_err(failed)?;
            Ok(finished(seen.exit, output, limits.output))
        }
        Followed::LeaderGone => {
            let _ = block_on(adapter.signal(run, Signal::Kill))?;
            crate::forge::sweep_led(leader, mark)?;
            let Some(seen) =
                block_on(settle(adapter, run, crate::forge::GRACE))?.map_err(failed)?
            else {
                bail!(
                    "{program} ended, and what it started still held its output open once it was killed"
                );
            };
            known(&seen, program)?;
            let output = block_on(adapter.output(run))?.map_err(failed)?;
            Ok(finished(seen.exit, output, limits.output))
        }
        Followed::TimedOut => {
            let _ = block_on(adapter.signal(run, Signal::Kill))?;
            let until = Instant::now() + crate::forge::GRACE;
            while !crate::forge::exited(leader) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            crate::forge::sweep_led(leader, mark)?;
            let _ = block_on(settle(adapter, run, crate::forge::GRACE))?;
            let output = block_on(adapter.output(run))?.map_err(failed)?;
            Err(anyhow::Error::new(TimedOut {
                program: file_name(program),
                timeout: limits.timeout,
                output: finished(None, output, limits.output),
            }))
        }
    }
}

/// What a `none` run's test command runs with: a scratch home's variables
/// ([`crate::validator::FreshHome::vars`]), `TMPDIR` at `tmp`, [`crate::forge::MARK`] at `mark`,
/// then what takt inherited that is no credential, no login location and of Substrate's
/// non-secret shape, the locale, `PATH`, cargo's and rustup's homes and the Rust toolchain's own
/// first, up to Substrate's limit of variables.
fn host_environment(
    home: &crate::validator::FreshHome,
    tmp: &Path,
    mark: &str,
) -> anyhow::Result<BTreeMap<String, String>> {
    const FIRST: [&str; 15] = [
        "PATH",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "TZ",
        "USER",
        "LOGNAME",
        "SHELL",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "CARGO_TARGET_DIR",
        "RUSTFLAGS",
        "RUST_BACKTRACE",
    ];
    let inherited: BTreeMap<String, String> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, value)| inheritable(name, value))
        .collect();
    let mut env = BTreeMap::new();
    for (name, value) in home.vars(|name| inherited.contains_key(name)) {
        if let Ok(value) = value.into_string()
            && in_shape(name, &value)
        {
            env.insert(name.to_owned(), value);
        }
    }
    let tmp = tmp
        .to_str()
        .ok_or_else(|| anyhow!("{} is not UTF-8", tmp.display()))?;
    env.insert("TMPDIR".to_owned(), tmp.to_owned());
    env.insert(crate::forge::MARK.to_owned(), mark.to_owned());
    let names = FIRST
        .iter()
        .map(|name| (*name).to_owned())
        .chain(inherited.keys().cloned())
        .collect::<Vec<_>>();
    for name in names {
        if env.len() >= MAX_VARIABLES {
            break;
        }
        if let Some(value) = inherited.get(&name) {
            env.entry(name).or_insert_with(|| value.clone());
        }
    }
    Ok(env)
}

/// Whether the variable `name`, set to `value` in takt's environment, passes to a `none` run's
/// test command: not when it is a credential ([`crate::forge::is_credential`]), a login
/// location ([`crate::forge::LOGIN_LOCATIONS`]), the bus a D-Bus activation started takt on,
/// takt's own [`crate::forge::MARK`], or outside Substrate's non-secret shape.
#[must_use]
pub fn inheritable(name: &str, value: &str) -> bool {
    in_shape(name, value)
        && !crate::forge::is_credential(name)
        && !crate::forge::LOGIN_LOCATIONS.contains(&name)
        && name != "DBUS_STARTER_ADDRESS"
        && name != crate::forge::MARK
}

/// Most variables Substrate admits in one run's environment.
const MAX_VARIABLES: usize = 64;

/// Whether a variable is of Substrate's closed, non-secret shape: an upper-case ASCII name of at
/// most 128 bytes that names no credential, a value of at most 4096 bytes without NUL.
fn in_shape(name: &str, value: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_uppercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'_'))
        })
        && value.len() <= 4096
        && !value.contains('\0')
        && ![
            "authorization",
            "bearer",
            "credential",
            "password",
            "proxy",
            "secret",
            "token",
        ]
        .iter()
        .any(|fragment| lower.contains(fragment))
}

/// The error an adapter refuses `program` with: `confinement <adapter> refused <program>:
/// <reason>`.
#[must_use]
pub fn refused(
    adapter: &str,
    program: impl AsRef<OsStr>,
    reason: impl std::fmt::Display,
) -> io::Error {
    io::Error::other(format!(
        "confinement {adapter} refused {}: {reason}",
        program.as_ref().to_string_lossy()
    ))
}

/// The adapter of this process, [`Unconfined`] until another is installed.
fn slot() -> &'static RwLock<Arc<dyn Confinement>> {
    static SLOT: OnceLock<RwLock<Arc<dyn Confinement>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(Arc::new(Unconfined)))
}

/// Makes `adapter` the one every child process of this process starts through, and returns
/// the one it replaces.
pub fn install(adapter: Arc<dyn Confinement>) -> Arc<dyn Confinement> {
    let mut slot = slot().write().unwrap_or_else(PoisonError::into_inner);
    std::mem::replace(&mut *slot, adapter)
}

/// The adapter in use.
#[must_use]
pub fn current() -> Arc<dyn Confinement> {
    Arc::clone(&slot().read().unwrap_or_else(PoisonError::into_inner))
}

/// What the operator requires of the adapter beyond what a test command needs, and the
/// delegated cgroup v2 root a plain `linux` adapter bounds its runs in.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Contract families and facts by their contract names (`limits`, `whole_tree_kill`, ...):
    /// an adapter that does not serve or prove one is not chosen, and with none left the run is
    /// refused naming it.
    pub needs: Vec<String>,
    /// The delegated cgroup v2 root `linux` is opened with, which makes it serve `limits`.
    pub cgroup_root: Option<PathBuf>,
}

/// [`Options::needs`], read as the contract's names.
#[derive(Debug, Clone, Default)]
struct Needs {
    families: Vec<ContractFamily>,
    facts: Vec<Fact>,
}

impl Needs {
    /// Each name a contract family or a fact; one that is neither is an error naming the valid
    /// ones.
    fn parse(names: &[String]) -> anyhow::Result<Self> {
        let mut needs = Self::default();
        for name in names {
            let value = serde_json::Value::String(name.clone());
            if let Ok(family) = serde_json::from_value::<ContractFamily>(value.clone()) {
                needs.families.push(family);
            } else if let Ok(fact) = serde_json::from_value::<Fact>(value) {
                needs.facts.push(fact);
            } else {
                bail!(
                    "{name:?} is no contract family and no fact of Substrate's confinement contract: the families are {}; the facts are {}",
                    FAMILY_ORDER
                        .iter()
                        .map(name_of)
                        .collect::<Vec<_>>()
                        .join(", "),
                    FACT_ORDER
                        .iter()
                        .map(name_of)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        Ok(needs)
    }
}

/// An adapter `substrate` may run the test command through, in the order they are tried: the
/// first that opens on this host and admits the run's profile is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate {
    /// `systemd` around `linux`: the user manager's delegated scope as `linux`'s cgroup root,
    /// so `limits` and `whole_tree_kill` are served.
    SystemdLinux,
    /// `linux`: bubblewrap through Substrate's host driver, in the delegated cgroup root
    /// [`Options::cgroup_root`] names when one is given.
    Linux,
    /// `macos`: a Seatbelt profile Substrate renders.
    Macos,
}

/// The adapters `substrate` tries, in order.
pub const CANDIDATES: [Candidate; 3] =
    [Candidate::SystemdLinux, Candidate::Linux, Candidate::Macos];

impl Candidate {
    /// Its name in a refusal: `systemd around linux`, `linux` or `macos`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SystemdLinux => "systemd around linux",
            Self::Linux => "linux",
            Self::Macos => "macos",
        }
    }

    /// The adapter it wraps, as a profile names it.
    fn inner(self) -> Option<InnerAdapter> {
        match self {
            Self::SystemdLinux => Some(InnerAdapter::Linux),
            Self::Linux | Self::Macos => None,
        }
    }

    /// Whether a declared root is visible at its host path alone (`macos`), not at a mount.
    fn paths_are_host_paths(self) -> bool {
        self == Self::Macos
    }
}

/// A directory of this process's own under the system's temporary directory, readable by its
/// owner alone, removed with what is in it when dropped.
#[derive(Debug)]
struct Private(PathBuf);

impl Private {
    fn new(what: &str) -> anyhow::Result<Self> {
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "takt-{what}-{}-{}",
            std::process::id(),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&dir)
            .with_context(|| format!("create {}", dir.display()))?;
        Ok(Self(dir))
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An adapter chosen for a workspace root: what it proved, and the private directory its host
/// driver keeps its capsules in, removed after it.
struct Opened {
    adapter: Arc<dyn ConfinementAdapter>,
    candidate: Candidate,
    record: AdapterRecord,
    root: PathBuf,
    _state: Private,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("candidate", &self.candidate)
            .field("record", &self.record)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// The profile every run asks of an adapter `candidate`: the families a run needs and a
/// filesystem view, those `needs` names, no network, `workspace`; on `macos` exec subpaths too,
/// which a Rust toolchain needs.
fn base_profile(candidate: Candidate, workspace: &Path, needs: &Needs) -> ConfinementProfile {
    let mut families = RUN_FAMILIES.to_vec();
    families.push(ContractFamily::FilesystemView);
    families.extend(needs.families.iter().cloned());
    let mut required = needs.facts.clone();
    if candidate == Candidate::Macos {
        required.push(Fact::ExecSubpaths);
    }
    ConfinementProfile {
        families,
        facts: required,
        inner: candidate.inner(),
        workspace: Some(workspace.to_path_buf()),
        network: Some(Network::Isolated),
        ..ConfinementProfile::default()
    }
}

/// The CPUs this process may run on, the ceiling a run's CPU bandwidth is held to.
fn cores() -> u32 {
    std::thread::available_parallelism()
        .map_or(1, |cores| u32::try_from(cores.get()).unwrap_or(u32::MAX))
}

/// Opens `candidate` with `root` as its host driver's workspace root, its state under `state`
/// and `writable` declared; its refusal, described.
fn open_candidate(
    candidate: Candidate,
    root: &Path,
    state: &Path,
    writable: &[DeclaredWritableRoot],
    options: &Options,
) -> Result<Arc<dyn ConfinementAdapter>, String> {
    let config = || {
        let mut config = HostConfig::minimum(root);
        config.capsule_root = state.join("capsules");
        config.aperture_root = state.join("apertures");
        config.exec_cpu_cores = cores();
        config.writable_roots = writable.to_vec();
        config
    };
    let request = match candidate {
        Candidate::SystemdLinux => AdapterRequest::Systemd(
            substrate::systemd::SystemdRequest::new(substrate::systemd::InnerRequest::Linux(
                Box::new(substrate::linux::LinuxRequest::new(config())),
            )),
        ),
        Candidate::Linux => AdapterRequest::Linux(Box::new(
            substrate::linux::LinuxRequest::new(config())
                .with_cgroup_root(options.cgroup_root.clone()),
        )),
        Candidate::Macos => AdapterRequest::Macos(substrate::macos::MacosRequest::default()),
    };
    match block_on(async move { substrate::open(request) }) {
        Ok(opened) => opened.map_err(|error| describe(&error)),
        Err(error) => Err(format!("{error:#}")),
    }
}

/// The first of [`CANDIDATES`] that opens with `root` its workspace root and `writable`
/// declared, admits the profile `profile` makes of its proof, and on Linux renders a run in
/// `workspace`; with none, an error naming each candidate's refusal.
fn select(
    root: &Path,
    workspace: &Path,
    writable: &[DeclaredWritableRoot],
    options: &Options,
    needs: &Needs,
) -> anyhow::Result<Opened> {
    let state = Private::new("substrate")?;
    let mut refusals = Vec::new();
    for candidate in CANDIDATES {
        let adapter = match open_candidate(candidate, root, &state.0, writable, options) {
            Ok(adapter) => adapter,
            Err(error) => {
                refusals.push(format!("{}: {error}", candidate.label()));
                continue;
            }
        };
        let probe = base_profile(candidate, workspace, needs);
        let admitted = match adapter.admit(&probe) {
            Ok(admitted) => admitted,
            Err(error) => {
                refusals.push(format!("{}: {}", candidate.label(), describe(&error)));
                continue;
            }
        };
        if !candidate.paths_are_host_paths() {
            let probe = substrate::Command {
                argv: vec!["/usr/bin/true".to_owned()],
                env: BTreeMap::new(),
            };
            if let Err(error) = adapter.render(&admitted, &probe) {
                refusals.push(format!("{}: {}", candidate.label(), describe(&error)));
                continue;
            }
        }
        return Ok(Opened {
            record: AdapterRecord::of(adapter.facts(), None),
            adapter,
            candidate,
            root: root.to_path_buf(),
            _state: state,
        });
    }
    bail!(
        "no Substrate adapter on this host serves a run in {}: {}",
        workspace.display(),
        refusals.join("; ")
    )
}

/// What a confined run ended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// Its exit code; none when a signal ended it or it was killed at its timeout.
    pub code: Option<i32>,
    /// The signal that ended it, when one did.
    pub signal: Option<i32>,
    /// Its standard output, the first bytes the adapter kept.
    pub stdout: Vec<u8>,
    /// Its standard error, the first bytes the adapter kept.
    pub stderr: Vec<u8>,
    /// Whether standard output was longer than the adapter kept.
    pub stdout_truncated: bool,
    /// Whether it was killed at its timeout.
    pub timed_out: bool,
}

impl Ran {
    /// Whether it exited 0 before its timeout.
    #[must_use]
    pub fn success(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }

    /// How it ended, as a sentence's end: `exited 3`, `was ended by signal 9`, `timed out`.
    #[must_use]
    pub fn ending(&self) -> String {
        match (self.timed_out, self.code, self.signal) {
            (true, _, _) => "timed out and was killed".to_owned(),
            (false, Some(code), _) => format!("exited {code}"),
            (false, None, Some(signal)) => format!("was ended by signal {signal}"),
            (false, None, None) => "ended without an exit code".to_owned(),
        }
    }
}

impl Opened {
    /// Runs `command` under `profile` to its end, killed past `timeout`. Sweeps are held off
    /// meanwhile: the adapter's process is this process's child in a group of its own.
    ///
    /// One thread starts the run and follows it to its end: a `linux` or `systemd` adapter's
    /// leader dies with the thread that started it (its parent-death signal), so a thread that
    /// ended once the run started would kill it.
    fn run(
        &self,
        profile: &ConfinementProfile,
        command: &substrate::Command,
        timeout: Duration,
    ) -> anyhow::Result<Ran> {
        let adapter = &*self.adapter;
        let admitted = adapter
            .admit(profile)
            .map_err(|error| anyhow!("{}", describe(&error)))?;
        let _held = crate::forge::hold_sweeps();
        let program = command.argv.first().map_or("", String::as_str);
        block_on(async {
            let run = started(adapter, &admitted, command).await?;
            let deadline = Instant::now() + timeout;
            let followed = follow(adapter, &run, None, deadline)
                .await
                .map_err(failed)?;
            let (seen, timed_out) = match followed {
                Followed::Ended(_) | Followed::LeaderGone => {
                    let Some(seen) = settle(adapter, &run, KILLED).await.map_err(failed)? else {
                        bail!("{program} ended, and its output was still open {KILLED:?} later");
                    };
                    (seen, false)
                }
                Followed::TimedOut => {
                    let _ = adapter.signal(&run, Signal::Kill).await;
                    let seen = settle(adapter, &run, KILLED).await.map_err(failed)?;
                    let Some(seen) = seen else {
                        bail!(
                            "{program} was killed at its timeout and had not ended {KILLED:?} later"
                        );
                    };
                    (seen, true)
                }
            };
            known(&seen, program)?;
            let output = adapter.output(&run).await.map_err(failed)?;
            Ok(Ran {
                code: seen.exit.and_then(|exit| exit.code),
                signal: seen.exit.and_then(|exit| exit.signal),
                stdout: output.stdout,
                stderr: output.stderr,
                stdout_truncated: output.stdout_truncated,
                timed_out,
            })
        })?
    }

    /// The profile of a run in `workspace`: [`base_profile`], process bounds for `timeout` where
    /// the adapter serves `limits`.
    fn profile(&self, workspace: &Path, needs: &Needs, timeout: Duration) -> ConfinementProfile {
        let facts = self.adapter.facts();
        let mut profile = base_profile(self.candidate, workspace, needs);
        if facts.serves(&ContractFamily::Limits) {
            profile.process_bounds = Some(bounds(timeout));
        }
        profile
    }
}

/// The bounds a confined run is held to where the adapter serves `limits`: `timeout` and five
/// seconds more (takt kills it at `timeout` itself), at most 4096 processes and threads, 32 GiB
/// of memory, and the CPU time of every core for the whole timeout, each within Substrate's caps.
fn bounds(timeout: Duration) -> ProcessBounds {
    const DAY_MS: u64 = 86_400_000;
    let timeout_ms = u64::try_from(timeout.as_millis())
        .unwrap_or(u64::MAX)
        .saturating_add(5_000)
        .clamp(1_000, DAY_MS);
    ProcessBounds {
        timeout_ms,
        processes: 4096,
        memory_bytes: 32 << 30,
        cpu_millis: timeout_ms.saturating_mul(u64::from(cores())).min(DAY_MS),
    }
}

/// The adapter `substrate`: the test command runs through the Substrate adapter chosen by facts
/// from [`CANDIDATES`] when the run opened ([`Substrate::open`]), on the tree's parent as its
/// workspace root.
///
/// - The test command is `cargo` and its arguments, run as the toolchain's own `cargo`
///   ([`toolchain`]), with the toolchain and the host's cargo caches read-only, the tree the
///   workspace, `target/` the build's directory, a scratch `CARGO_HOME` under it, offline and
///   with no network. Any other program is refused by its toolchain, never run unconfined: the
///   profile serves only a Rust toolchain.
/// - A Python is refused wherever takt would start one: at case open takt asks the Python a
///   test command runs what it imports, which would run it unconfined.
/// - A program a gate's library starts itself ([`Confinement::admit_delegated`]) is refused: the
///   library starts it outside this seam, so it would run unconfined.
/// - Every other program (git, the forge's, the harness's) is admitted and runs as under `none`.
#[derive(Debug)]
pub struct Substrate {
    options: Options,
    needs: Needs,
    record: AdapterRecord,
    opened: Mutex<Option<Opened>>,
}

impl Substrate {
    /// The adapter for test commands in `tree`: the first of [`CANDIDATES`] that opens on the
    /// tree's parent and admits a test command's profile with what `options` needs. No test is
    /// run.
    ///
    /// # Errors
    ///
    /// A name in `options.needs` that is no family and no fact; no candidate left, naming each
    /// one's refusal.
    pub fn open(tree: &Path, options: &Options) -> anyhow::Result<Self> {
        let needs = Needs::parse(&options.needs)?;
        let tree = tree
            .canonicalize()
            .with_context(|| format!("the tree {}", tree.display()))?;
        let opened = open_for_tree(&tree, options, &needs)?;
        Ok(Self {
            options: options.clone(),
            needs,
            record: opened.record.clone(),
            opened: Mutex::new(Some(opened)),
        })
    }

    /// The adapter chosen, as the run's `opened` event records it.
    #[must_use]
    pub fn record(&self) -> &AdapterRecord {
        &self.record
    }
}

/// [`select`] for test commands in `tree`, its parent the workspace root.
fn open_for_tree(tree: &Path, options: &Options, needs: &Needs) -> anyhow::Result<Opened> {
    let root = tree
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", tree.display()))?;
    select(root, tree, &[], options, needs)
}

/// Why Substrate does not serve the test command `program`, naming its toolchain.
fn unserved(program: &str) -> String {
    let name = file_name(program);
    let toolchain = if crate::run::python_like(&name) {
        "Python".to_owned()
    } else {
        format!("that of `{program}`")
    };
    format!(
        "the test command's toolchain, {toolchain}, is not one takt's Substrate profile serves: it serves a Rust toolchain run as `cargo` alone; the test command is not run unconfined"
    )
}

impl Confinement for Substrate {
    fn name(&self) -> &str {
        "substrate"
    }

    fn admit(&self, program: &OsStr) -> io::Result<()> {
        let name = Path::new(program)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if crate::run::python_like(&name) {
            return Err(refused(
                self.name(),
                program,
                "Python is a toolchain takt's Substrate profile does not serve, and takt starts none outside it",
            ));
        }
        Ok(())
    }

    fn admit_delegated(&self, program: &OsStr) -> io::Result<()> {
        Err(refused(
            self.name(),
            program,
            "the engineering assertions library starts a gate's commands and providers itself, outside Substrate, so they would run unconfined",
        ))
    }

    fn run_test(&self, test: &TestRun<'_>) -> anyhow::Result<Finished> {
        let (program, args) = test
            .argv
            .split_first()
            .ok_or_else(|| anyhow!("an empty test command"))?;
        if program != "cargo" {
            return Err(refused(self.name(), program, unserved(program)).into());
        }
        let tree = test
            .tree
            .canonicalize()
            .with_context(|| format!("the tree {}", test.tree.display()))?;
        let mut slot = self.opened.lock().unwrap_or_else(PoisonError::into_inner);
        if slot
            .as_ref()
            .is_none_or(|opened| Some(opened.root.as_path()) != tree.parent())
        {
            // One `systemd` around `linux` is open in a process at a time: the one before goes
            // first.
            *slot = None;
            let opened = open_for_tree(&tree, &self.options, &self.needs)?;
            if !opened.record.same_adapter(&self.record) {
                bail!(
                    "confinement substrate: the adapter chosen for {} is {}, not {}, which the run opened with",
                    tree.display(),
                    opened.record.describe(),
                    self.record.describe()
                );
            }
            *slot = Some(opened);
        }
        let opened = slot
            .as_ref()
            .ok_or_else(|| anyhow!("confinement substrate: no adapter is open"))?;
        let toolchain = toolchain(Some(&tree))?;
        let plan = TestPlan::new(
            opened,
            &tree,
            &toolchain,
            args,
            &self.needs,
            test.limits.timeout,
        )?;
        let ran = opened
            .run(&plan.profile, &plan.command, test.limits.timeout)
            .map_err(|error| {
                anyhow!(
                    "confinement substrate ({}) did not run the test command: {error:#}",
                    self.record.describe()
                )
            })?;
        drop(plan);
        let output = finished(
            Some(RunExit {
                code: ran.code,
                signal: ran.signal,
            }),
            RunOutput {
                stdout: ran.stdout,
                stderr: ran.stderr,
                stdout_truncated: ran.stdout_truncated,
                stderr_truncated: false,
            },
            test.limits.output,
        );
        if ran.timed_out {
            return Err(anyhow::Error::new(TimedOut {
                program: file_name(program),
                timeout: test.limits.timeout,
                output,
            }));
        }
        Ok(output)
    }

    fn substrate(&self) -> Option<AdapterRecord> {
        Some(self.record.clone())
    }
}

/// A directory under a tree's `target/` for one confined test command: its scratch
/// `CARGO_HOME`, and on macOS its home and `TMPDIR`; removed when dropped.
#[derive(Debug)]
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The host's cargo cache directories a confined build reads: the registry's cache and index and
/// git dependencies' databases, which hold every crate an offline build takes; each that exists,
/// canonical. Git checkouts are made again from the databases, offline.
fn cargo_caches() -> Vec<(&'static str, PathBuf)> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| user_home().map(|home| home.join(".cargo")));
    let Some(home) = home else {
        return Vec::new();
    };
    ["registry/cache", "registry/index", "git/db"]
        .into_iter()
        .filter_map(|sub| Some((sub, home.join(sub).canonicalize().ok()?)))
        .filter(|(_, dir)| dir.is_dir())
        .collect()
}

/// This user's home: `HOME` when it is an absolute path, else the home its passwd entry names.
fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .or_else(crate::validator::passwd_home)
}

/// A path as a string Substrate takes, which must be UTF-8.
fn text(path: &Path) -> anyhow::Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("{} is not UTF-8", path.display()))
}

/// Links `link` to `target`, making its directory first.
fn link(target: &Path, link: &Path) -> anyhow::Result<()> {
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("link {} to {}", link.display(), target.display()))
}

/// One confined test command: the profile asked of the adapter, the command, and the scratch
/// directory it uses.
struct TestPlan {
    profile: ConfinementProfile,
    command: substrate::Command,
    _scratch: Scratch,
}

/// Where a Linux adapter shows the toolchain and the cargo caches.
const INSIDE_RUST: &str = "/toolchain/rust";
const INSIDE_CARGO: &str = "/toolchain/cargo";
/// Where a Linux adapter shows the workspace.
const INSIDE_WORKSPACE: &str = "/workspace";

impl TestPlan {
    /// `cargo <args>` in `tree` on `opened`, the toolchain at `toolchain`.
    fn new(
        opened: &Opened,
        tree: &Path,
        toolchain: &Path,
        args: &[String],
        needs: &Needs,
        timeout: Duration,
    ) -> anyhow::Result<Self> {
        static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let target = tree.join("target");
        std::fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;
        let name = format!(
            "takt-run-{}-{}",
            std::process::id(),
            RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let scratch = Scratch(target.join(&name));
        let cargo_home = scratch.0.join("cargo-home");
        std::fs::create_dir_all(&cargo_home)
            .with_context(|| format!("create {}", cargo_home.display()))?;
        let mut profile = opened.profile(tree, needs, timeout);
        let mut env = BTreeMap::from([
            ("CARGO_NET_OFFLINE".to_owned(), "true".to_owned()),
            ("RUSTUP_AUTO_INSTALL".to_owned(), "0".to_owned()),
        ]);
        let caches = cargo_caches();
        let program = if opened.candidate.paths_are_host_paths() {
            let home = scratch.0.join("home");
            std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
            let mut readable = vec![toolchain.to_path_buf()];
            for (sub, dir) in &caches {
                link(dir, &cargo_home.join(sub))?;
                readable.push(dir.clone());
            }
            for dir in &readable {
                profile.read_only_roots.push(ReadOnlyRoot {
                    host_path: text(dir)?,
                    mount: text(dir)?,
                });
            }
            let facts = opened.adapter.facts();
            profile.exec_subpaths =
                vec!["target".to_owned(), text(toolchain)?, "/usr/bin".to_owned()];
            profile
                .exec_subpaths
                .extend(substrate_host::seatbelt::developer_root().and_then(|dir| text(&dir).ok()));
            let git = tree.join(".git");
            if facts.proves(&Fact::SealedPaths)
                && std::fs::symlink_metadata(&git).is_ok_and(|meta| !meta.file_type().is_symlink())
            {
                profile.sealed_paths = vec![".git".to_owned()];
            }
            if facts.proves(&Fact::PrivateTmpdir) {
                profile.private_tmpdir = true;
            } else {
                let tmp = scratch.0.join("tmp");
                std::fs::create_dir_all(&tmp)
                    .with_context(|| format!("create {}", tmp.display()))?;
                env.insert("TMPDIR".to_owned(), text(&tmp)?);
            }
            let bin = toolchain.join("bin");
            env.insert("PATH".to_owned(), format!("{}:/usr/bin:/bin", text(&bin)?));
            env.insert("HOME".to_owned(), text(&home)?);
            env.insert("CARGO_HOME".to_owned(), text(&cargo_home)?);
            env.insert("CARGO_TARGET_DIR".to_owned(), text(&target)?);
            env.insert("RUSTC".to_owned(), text(&bin.join("rustc"))?);
            text(&bin.join("cargo"))?
        } else {
            profile.read_only_roots.push(ReadOnlyRoot {
                host_path: text(toolchain)?,
                mount: INSIDE_RUST.to_owned(),
            });
            for (sub, dir) in &caches {
                let inside = format!("{INSIDE_CARGO}/{sub}");
                link(Path::new(&inside), &cargo_home.join(sub))?;
                profile.read_only_roots.push(ReadOnlyRoot {
                    host_path: text(dir)?,
                    mount: inside,
                });
            }
            let workspace = INSIDE_WORKSPACE;
            env.insert(
                "PATH".to_owned(),
                format!("{INSIDE_RUST}/bin:/usr/bin:/bin"),
            );
            env.insert("HOME".to_owned(), "/tmp".to_owned());
            env.insert(
                "CARGO_HOME".to_owned(),
                format!("{workspace}/target/{name}/cargo-home"),
            );
            env.insert("CARGO_TARGET_DIR".to_owned(), format!("{workspace}/target"));
            env.insert("RUSTC".to_owned(), format!("{INSIDE_RUST}/bin/rustc"));
            format!("{INSIDE_RUST}/bin/cargo")
        };
        let mut argv = vec![program];
        argv.extend(args.iter().cloned());
        Ok(Self {
            profile,
            command: substrate::Command { argv, env },
            _scratch: scratch,
        })
    }
}

/// A handle on the Substrate adapter a promotion builds the candidate and runs its binary
/// through: chosen by facts from [`CANDIDATES`] as a run's is, its workspace a directory under
/// the build directory and, where the adapter serves writable roots, the fresh install root
/// declared writable.
#[derive(Debug)]
pub struct Confined {
    opened: Opened,
    needs: Needs,
}

/// One run through [`Confined::run`]: a program and its arguments in the workspace or `workdir`
/// beneath it, with exactly `env`, the read-only roots and the declared writable roots it names,
/// held to `timeout`.
#[derive(Debug, Clone, Default)]
pub struct RunSpec {
    /// A relative directory beneath the workspace to start in.
    pub workdir: Option<String>,
    /// The program, as the run sees it ([`Confined::sees`]), and its arguments.
    pub argv: Vec<String>,
    /// The whole environment.
    pub env: BTreeMap<String, String>,
    /// Host directories visible read-only, each at the mount a Linux adapter shows it at; on
    /// macOS at its host path.
    pub read_only: Vec<(PathBuf, String)>,
    /// Declared writable roots, by name, each at its mount.
    pub writable: Vec<(String, String)>,
    /// On macOS, the directories programs may run from beneath besides the program itself:
    /// relative ones beneath the workspace; the developer tools are added.
    pub exec: Vec<String>,
    /// How long it may run.
    pub timeout: Duration,
}

impl Confined {
    /// The adapter for runs in `workspace`, a directory directly beneath the workspace root its
    /// parent is, with `writable` (a name and a host directory) declared where the adapter
    /// serves writable roots.
    ///
    /// # Errors
    ///
    /// A name in `options.needs` that is no family and no fact; no candidate left, naming each
    /// one's refusal.
    pub fn open(
        workspace: &Path,
        writable: Option<(&str, &Path)>,
        options: &Options,
    ) -> anyhow::Result<Self> {
        let needs = Needs::parse(&options.needs)?;
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("the workspace {}", workspace.display()))?;
        let root = workspace
            .parent()
            .ok_or_else(|| anyhow!("{} has no parent directory", workspace.display()))?;
        let declared = match writable {
            Some((name, path)) => vec![DeclaredWritableRoot {
                name: name.to_owned(),
                path: path
                    .canonicalize()
                    .with_context(|| format!("the writable root {}", path.display()))?,
            }],
            None => Vec::new(),
        };
        let opened = select(root, &workspace, &declared, options, &needs)?;
        Ok(Self { opened, needs })
    }

    /// The adapter chosen.
    #[must_use]
    pub fn record(&self) -> &AdapterRecord {
        &self.opened.record
    }

    /// Whether it serves declared writable roots outside the workspace; `macos` does not.
    #[must_use]
    pub fn serves_writable_roots(&self) -> bool {
        !self.opened.candidate.paths_are_host_paths()
    }

    /// Where a run sees `host`, which a Linux adapter shows at `inside`: there, or on macOS at
    /// its own path.
    ///
    /// # Errors
    ///
    /// A host path that is not UTF-8.
    pub fn sees(&self, host: &Path, inside: &str) -> anyhow::Result<String> {
        if self.opened.candidate.paths_are_host_paths() {
            text(host)
        } else {
            Ok(inside.to_owned())
        }
    }

    /// Runs `spec` in `workspace` to its end.
    ///
    /// # Errors
    ///
    /// A refusal by the adapter, naming it; a failure to start or follow the run.
    pub fn run(&self, workspace: &Path, spec: &RunSpec) -> anyhow::Result<Ran> {
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("the workspace {}", workspace.display()))?;
        let mut profile = self.opened.profile(&workspace, &self.needs, spec.timeout);
        profile.workdir.clone_from(&spec.workdir);
        let host_paths = self.opened.candidate.paths_are_host_paths();
        for (host, mount) in &spec.read_only {
            let host = host
                .canonicalize()
                .with_context(|| format!("the read-only root {}", host.display()))?;
            let host = text(&host)?;
            profile.read_only_roots.push(ReadOnlyRoot {
                mount: if host_paths {
                    host.clone()
                } else {
                    mount.clone()
                },
                host_path: host,
            });
        }
        if !spec.writable.is_empty() && host_paths {
            bail!(
                "confinement {}: writable roots outside the workspace are not served by it",
                self.opened.record.describe()
            );
        }
        profile.writable_roots = spec
            .writable
            .iter()
            .map(|(name, mount)| WritableHostRoot {
                host: name.clone(),
                mount: mount.clone(),
            })
            .collect();
        if host_paths && !spec.exec.is_empty() {
            profile.exec_subpaths.clone_from(&spec.exec);
            profile
                .exec_subpaths
                .extend(substrate_host::seatbelt::developer_root().and_then(|dir| text(&dir).ok()));
            if self.opened.adapter.facts().proves(&Fact::PrivateTmpdir) {
                profile.private_tmpdir = true;
            }
        }
        let command = substrate::Command {
            argv: spec.argv.clone(),
            env: spec.env.clone(),
        };
        self.opened.run(&profile, &command, spec.timeout)
    }
}

/// The toolchain a confined `cargo` runs from, resolved without running anything or searching
/// `PATH`: under rustup's home (`RUSTUP_HOME`, else `~/.rustup`), the channel `tree`'s
/// `rust-toolchain.toml` or `rust-toolchain` names when it names one, else the
/// `default_toolchain` rustup's `settings.toml` names; that toolchain's directory under
/// `toolchains/` (by its name, or the one directory named by it and a target triple), canonical.
/// Its `bin/cargo` and the sibling `bin/rustc` must be executable files and `lib/rustlib` must
/// exist; anything missing is an error naming it. A confined run installs no toolchain.
///
/// # Errors
///
/// What is missing, named.
pub fn toolchain(tree: Option<&Path>) -> anyhow::Result<PathBuf> {
    let rustup_home = match std::env::var_os("RUSTUP_HOME").filter(|dir| !dir.is_empty()) {
        Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
        _ => user_home()
            .ok_or_else(|| anyhow!("neither RUSTUP_HOME nor a home directory is known"))?
            .join(".rustup"),
    };
    let (name, from) = match tree.map(pinned_channel).transpose()?.flatten() {
        Some(pinned) => pinned,
        None => {
            let settings = rustup_home.join("settings.toml");
            let text = std::fs::read_to_string(&settings).with_context(|| {
                format!(
                    "read {}, rustup's settings, which name the toolchain a confined build runs; none is searched on PATH",
                    settings.display()
                )
            })?;
            let name = text
                .lines()
                .filter_map(|line| line.trim().strip_prefix("default_toolchain"))
                .filter_map(|rest| rest.trim_start().strip_prefix('='))
                .map(|value| value.trim().trim_matches('"').to_owned())
                .find(|value| !value.is_empty());
            let Some(name) = name else {
                bail!(
                    "{} names no default_toolchain; a confined build runs rustup's default toolchain and finds none on PATH",
                    settings.display()
                );
            };
            (name, settings)
        }
    };
    if name.contains('/') || name.starts_with('.') {
        bail!("{}: {name:?} is no toolchain name", from.display());
    }
    let toolchains = rustup_home.join("toolchains");
    let exact = toolchains.join(&name);
    let toolchain = if exact.is_dir() {
        exact
    } else {
        let prefix = format!("{name}-");
        let mut found: Vec<PathBuf> = std::fs::read_dir(&toolchains)
            .with_context(|| format!("read {}", toolchains.display()))?
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        match found.len() {
            1 => found.remove(0),
            0 => bail!(
                "the toolchain {name} ({}) is not installed under {}; a confined build installs none",
                from.display(),
                toolchains.display()
            ),
            n => bail!(
                "the toolchain {name} ({}) matches {n} directories under {}; name it in full",
                from.display(),
                toolchains.display()
            ),
        }
    };
    for binary in ["bin/cargo", "bin/rustc"] {
        if !is_executable(&toolchain.join(binary)) {
            bail!(
                "the toolchain {} ({}) has no executable {binary}; a confined build calls the toolchain's own cargo and its sibling rustc, never a rustup proxy or one on PATH",
                toolchain.display(),
                from.display()
            );
        }
    }
    if !toolchain.join("lib/rustlib").is_dir() {
        bail!(
            "the toolchain {} ({}) has no lib/rustlib",
            toolchain.display(),
            from.display()
        );
    }
    toolchain
        .canonicalize()
        .with_context(|| format!("resolve {}", toolchain.display()))
}

/// The channel `tree`'s `rust-toolchain.toml` (or `rust-toolchain`) names, and that file: none
/// when it has neither. A file that names its toolchain otherwise (a `path`) is an error.
fn pinned_channel(tree: &Path) -> anyhow::Result<Option<(String, PathBuf)>> {
    for file in ["rust-toolchain.toml", "rust-toolchain"] {
        let path = tree.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(table) = text.parse::<toml::Table>() {
            let channel = table
                .get("toolchain")
                .and_then(|toolchain| toolchain.get("channel"))
                .and_then(toml::Value::as_str);
            let Some(channel) = channel else {
                bail!(
                    "{} names no toolchain channel; a confined build runs an installed toolchain by its channel",
                    path.display()
                );
            };
            return Ok(Some((channel.to_owned(), path)));
        }
        let channel = text.trim();
        if channel.is_empty() || channel.contains(char::is_whitespace) {
            bail!("{} names no toolchain channel", path.display());
        }
        return Ok(Some((channel.to_owned(), path)));
    }
    Ok(None)
}

/// Whether `path` is a file someone may execute.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// A run followed until it ended, its leader ended with it still running, or its deadline.
enum Followed {
    Ended(RunObservation),
    LeaderGone,
    TimedOut,
}

/// Observes `run` until it is no longer running, `leader` (when one is given) has ended while
/// it still is, or `deadline` passed.
async fn follow(
    adapter: &dyn ConfinementAdapter,
    run: &RunId,
    leader: Option<u32>,
    deadline: Instant,
) -> Result<Followed, ConfinementError> {
    loop {
        let seen = adapter.observe(run).await?;
        if seen.state != RunState::Running {
            return Ok(Followed::Ended(seen));
        }
        if leader.is_some_and(crate::forge::exited) {
            return Ok(Followed::LeaderGone);
        }
        if Instant::now() >= deadline {
            return Ok(Followed::TimedOut);
        }
        tokio::time::sleep(POLL).await;
    }
}

/// `run` once it has ended and its output closed, if that is within `within`.
async fn settle(
    adapter: &dyn ConfinementAdapter,
    run: &RunId,
    within: Duration,
) -> Result<Option<RunObservation>, ConfinementError> {
    match tokio::time::timeout(within, adapter.wait(run)).await {
        Ok(seen) => seen.map(Some),
        Err(_) => Ok(None),
    }
}

/// Starts `command` under `admitted`, again for a moment while its program is busy being written
/// (another thread's child still holds the write handle it inherited until it executes).
fn start(
    adapter: &dyn ConfinementAdapter,
    admitted: &AdmittedProfile,
    command: &substrate::Command,
) -> anyhow::Result<RunId> {
    block_on(started(adapter, admitted, command))?
}

/// [`start`] on the thread that polls it, which a run of an adapter whose leader dies with the
/// thread that started it must outlive.
async fn started(
    adapter: &dyn ConfinementAdapter,
    admitted: &AdmittedProfile,
    command: &substrate::Command,
) -> anyhow::Result<RunId> {
    let mut busy = 0;
    loop {
        match adapter.start(admitted, command).await {
            Ok(run) => return Ok(run),
            Err(error) if error.to_string().contains("os error 26") && busy < 50 => {
                busy += 1;
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            Err(error) => return Err(anyhow!("{}", describe(&error))),
        }
    }
}

/// An error unless the adapter saw how `seen` ended.
fn known(seen: &RunObservation, program: &str) -> anyhow::Result<()> {
    if seen.state == RunState::Unknown {
        bail!("Substrate lost sight of {program}: how it ended is unknown");
    }
    Ok(())
}

/// A run's end as a [`Finished`]: the exit code, and of each stream what the adapter kept, its
/// last `keep` bytes; standard output cut when the adapter truncated it or it was longer.
fn finished(exit: Option<RunExit>, output: RunOutput, keep: usize) -> Finished {
    let mut stdout = output.stdout;
    let mut stderr = output.stderr;
    let stdout_len = u64::try_from(stdout.len()).unwrap_or(u64::MAX);
    let stdout_cut = output.stdout_truncated || stdout.len() > keep;
    if stdout.len() > keep {
        stdout.drain(..stdout.len() - keep);
    }
    if stderr.len() > keep {
        stderr.drain(..stderr.len() - keep);
    }
    Finished {
        code: exit.and_then(|exit| exit.code),
        stdout,
        stderr,
        stdout_cut,
        stdout_head: Vec::new(),
        stdout_len,
    }
}

/// `program`'s file name.
fn file_name(program: &str) -> String {
    Path::new(program).file_name().map_or_else(
        || program.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// A contract name as Substrate's model spells it (`process_run`, `whole_tree_kill`, `linux`).
fn name_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// A refusal of Substrate's contract, its code first and its names as the contract spells them.
#[must_use]
pub fn describe(error: &ConfinementError) -> String {
    let code = error.code();
    match error {
        ConfinementError::AdapterUnserved { adapter, detail } => {
            format!("{code}: {} is not served here: {detail}", name_of(adapter))
        }
        ConfinementError::FamilyUnserved { adapter, family } => {
            format!(
                "{code}: {} does not serve {}",
                name_of(adapter),
                name_of(family)
            )
        }
        ConfinementError::FactUnserved { adapter, fact } => {
            format!(
                "{code}: {} did not prove {}",
                name_of(adapter),
                name_of(fact)
            )
        }
        ConfinementError::LocalRefused { adapter } => format!(
            "{code}: the host's policy ({}) forbids {}",
            substrate::local::LOCAL_POLICY_ENV,
            name_of(adapter)
        ),
        ConfinementError::LocalReasonMissing { adapter } => format!(
            "{code}: {} runs declared insecure and needs a reason",
            name_of(adapter)
        ),
        ConfinementError::ImageUnpinned { adapter } => {
            format!("{code}: {} runs an image by digest only", name_of(adapter))
        }
        ConfinementError::ReservedFieldUnserved { .. } | ConfinementError::Driver(_) => {
            error.to_string()
        }
    }
}

/// A failure of Substrate met while a run was followed.
fn failed(error: ConfinementError) -> anyhow::Error {
    anyhow!("{}", describe(&error))
}

/// The runtime Substrate's adapters run on, started once: two workers, every driver enabled.
fn runtime() -> anyhow::Result<&'static tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("takt-substrate")
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow!("start the runtime Substrate's adapters run on: {error}"))
}

/// Runs `future` to its end on [`runtime`], from a thread of its own, so that a caller already
/// inside a runtime does not block it.
fn block_on<F>(future: F) -> anyhow::Result<F::Output>
where
    F: Future + Send,
    F::Output: Send,
{
    let runtime = runtime()?;
    std::thread::scope(|scope| scope.spawn(|| runtime.block_on(future)).join())
        .map_err(|_| anyhow!("the thread following a Substrate run panicked"))
}
