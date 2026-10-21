//! The seam every child process of a run starts through: the adapter in use ([`current`]) is
//! asked before any program starts, and the test command is run by it ([`Confinement::run_test`]).
//!
//! | adapter | name | a program | the test command |
//! |---|---|---|---|
//! | [`Unconfined`] | `none` | admitted | started here ([`unconfined`]): no credential, a scratch home, no login of the user's, a timeout and a bounded tail ([`crate::forge::run_child`]) |
//! | [`Substrate`] | `substrate` | admitted, but a Python, which takt would otherwise start for a test command at case open, and every command a gate's library starts itself | run by the `SubstrateRunner` of loom-sdk: a fixed `PATH` of read-only toolchain roots, only `target/` writable, no network; one it cannot serve is refused by its toolchain, never run unconfined |
//!
//! [`Command`] is the one type of takt that starts a process: [`std::process::Command`] with
//! the adapter asked first, at every start. No module of takt but this one names
//! [`std::process::Command`]. Confining the harness session, git, the forge and `aep` is not
//! this seam's yet: the adapter admits them, and they run as they did.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Child, CommandArgs, CommandEnvs, ExitStatus, Output, Stdio};
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

use anyhow::{Context as _, anyhow};
use loom_sdk::intake::slice::confinement::{ConfinementRefusal, prepare_delegated_cgroup};
use loom_sdk::intake::slice::executor::TestCommand;
use loom_sdk::{ConfinementError, SubstrateRunner, TestRunner as _};

use crate::forge::{Finished, Limits, TimedOut};

/// Set in the environment of a takt started again in a delegated cgroup v2 scope of its own
/// ([`delegate`]): [`Substrate::open`] prepares that scope and opens nothing anywhere else.
pub const DELEGATED: &str = "TAKT_CONFINEMENT_DELEGATED";

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
        current().admit(self.inner.get_program())?;
        self.inner.spawn()
    }

    /// Runs the child to its end once the adapter in use admitted its program, as
    /// [`std::process::Command::output`].
    ///
    /// # Errors
    ///
    /// The adapter's refusal ([`refused`]), and nothing started; else what the run returned.
    pub fn output(&mut self) -> io::Result<Output> {
        current().admit(self.inner.get_program())?;
        self.inner.output()
    }

    /// Runs the child to its end once the adapter in use admitted its program, as
    /// [`std::process::Command::status`].
    ///
    /// # Errors
    ///
    /// The adapter's refusal ([`refused`]), and nothing started; else what the run returned.
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        current().admit(self.inner.get_program())?;
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
}

/// The adapter `none`: every program is admitted, and the test command is started as
/// [`unconfined`] starts it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unconfined;

impl Confinement for Unconfined {
    fn name(&self) -> &str {
        "none"
    }

    fn admit(&self, _program: &OsStr) -> io::Result<()> {
        Ok(())
    }
}

/// Runs the test command as takt starts it under `none`: without a shell, with no credential,
/// in a fresh home of its own with no login of the user's in reach
/// ([`crate::forge::without_login`]), held to its limits ([`crate::forge::run_child`]). The
/// program is still asked of the adapter in use, as every [`Command`] is.
///
/// # Errors
///
/// An empty line, a refusal, the host's refusal of the namespaces
/// ([`crate::forge::refused_namespaces`]), or what [`crate::forge::run_child`] returns.
pub fn unconfined(test: &TestRun<'_>) -> anyhow::Result<Finished> {
    let (program, args) = test
        .argv
        .split_first()
        .ok_or_else(|| anyhow!("an empty test command"))?;
    let mut command = Command::new(program);
    command.args(args).current_dir(test.tree);
    crate::forge::without_credentials(&mut command, |_| false);
    let home = crate::validator::FreshHome::new()?;
    home.apply(&mut command);
    crate::forge::without_login(&mut command, home.dir());
    let finished = crate::forge::run_child(&mut command, test.limits)
        .map_err(crate::forge::refused_namespaces);
    drop(home);
    finished
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

/// The adapter `substrate`: the test command runs under the `SubstrateRunner` of loom-sdk, in a
/// delegated cgroup v2 scope this process was started in ([`DELEGATED`], [`delegate`]).
///
/// - The test command is `cargo` and its arguments, run with a fixed `PATH` of read-only
///   toolchain roots, `target/` the one writable directory and no network. Any other program is
///   refused by its toolchain, never run unconfined: the profile serves only a Rust toolchain.
/// - A Python is refused wherever takt would start one: at case open takt asks the Python a
///   test command runs what it imports, which would run it unconfined.
/// - A program a gate's library starts itself ([`Confinement::admit_delegated`]) is refused: the
///   library starts it outside this seam, so it would run unconfined.
/// - Every other program (git, the forge's, the harness's) is admitted and runs as under `none`.
#[derive(Debug, Clone)]
pub struct Substrate {
    runner: SubstrateRunner,
}

/// The runner of this process's delegated scope, prepared once: the scope's leaf made and this
/// process moved into it, its controllers enabled.
fn delegated_runner() -> Result<SubstrateRunner, ConfinementError> {
    static PREPARED: OnceLock<Result<SubstrateRunner, ConfinementError>> = OnceLock::new();
    if std::env::var_os(DELEGATED).is_none() {
        return Err(ConfinementError {
            reason: ConfinementRefusal::CgroupUndelegated,
            detail: format!(
                "takt opens Substrate only in a delegated cgroup v2 scope it was started in ({DELEGATED}=1, as `takt run --confinement substrate` starts itself again under `systemd-run --user --scope -p Delegate=yes`); it never searches for another, which could be another program's"
            ),
        });
    }
    PREPARED
        .get_or_init(|| prepare_delegated_cgroup().map(|root| SubstrateRunner::new(Some(root))))
        .clone()
}

impl Substrate {
    /// The adapter, its backend probed on the workspace `tree`: bubblewrap, the delegated scope
    /// and the driver Substrate opens there. No test is run.
    ///
    /// # Errors
    ///
    /// The refusal loom-sdk names: [`ConfinementRefusal::CgroupUndelegated`] outside a
    /// delegated scope ([`undelegated`]), a missing backend, a workspace it does not serve.
    pub fn open(tree: &Path) -> Result<Self, ConfinementError> {
        let runner = delegated_runner()?;
        runner.preflight(tree)?;
        Ok(Self { runner })
    }
}

/// Why Substrate does not serve the test command `program`, naming its toolchain.
fn unserved(program: &str) -> String {
    let name = Path::new(program).file_name().map_or_else(
        || program.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let toolchain = if crate::run::python_like(&name) {
        "Python".to_owned()
    } else {
        format!("that of `{program}`")
    };
    format!(
        "the test command's toolchain, {toolchain}, is not one the Substrate profile of loom-sdk serves: it serves a Rust toolchain run as `cargo` alone; the test command is not run unconfined"
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
                "Python is a toolchain the Substrate profile of loom-sdk does not serve, and takt starts none outside it",
            ));
        }
        Ok(())
    }

    fn admit_delegated(&self, program: &OsStr) -> io::Result<()> {
        Err(refused(
            self.name(),
            program,
            "the engineering assertions library starts a gate's commands and providers itself, outside the Substrate profile of loom-sdk, so they would run unconfined",
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
        let command = TestCommand::new(program, args).with_timeout(test.limits.timeout);
        let ran = self
            .runner
            .run(&command, test.tree)
            .map_err(|error| anyhow!("confinement substrate: the SubstrateRunner of loom-sdk refused the test command: {error}"))?;
        let mut stdout = ran.output.into_bytes();
        let stdout_len = u64::try_from(stdout.len()).unwrap_or(u64::MAX);
        let stdout_cut = stdout.len() > test.limits.output;
        if stdout_cut {
            stdout.drain(..stdout.len() - test.limits.output);
        }
        let finished = Finished {
            code: ran.exit_code,
            stdout,
            stderr: Vec::new(),
            stdout_cut,
            stdout_head: Vec::new(),
            stdout_len,
        };
        if ran.timed_out {
            return Err(anyhow::Error::new(TimedOut {
                program: program.clone(),
                timeout: test.limits.timeout,
                output: finished,
            }));
        }
        Ok(finished)
    }
}

/// Whether `error` is Substrate's refusal for want of a delegated cgroup scope, which
/// [`delegate`] answers.
#[must_use]
pub fn undelegated(error: &ConfinementError) -> bool {
    matches!(error.reason, ConfinementRefusal::CgroupUndelegated)
}

/// Starts this program again, with the arguments it was given, in a delegated cgroup v2 scope
/// of its own (`systemd-run --user --scope --quiet -p Delegate=yes`, [`DELEGATED`] set), waits
/// for it and returns its exit code; 128 and the signal's number when a signal ended it.
///
/// # Errors
///
/// When `systemd-run` cannot be started.
pub fn delegate() -> anyhow::Result<i32> {
    let program = std::env::current_exe().context("find this program to start it again")?;
    let status = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
        .arg(&program)
        .args(std::env::args_os().skip(1))
        .env(DELEGATED, "1")
        .status()
        .context(
            "start takt again in a delegated cgroup scope (systemd-run --user --scope -p Delegate=yes)",
        )?;
    #[cfg(unix)]
    if let Some(signal) = std::os::unix::process::ExitStatusExt::signal(&status) {
        return Ok(128 + signal);
    }
    Ok(status.code().unwrap_or(1))
}
