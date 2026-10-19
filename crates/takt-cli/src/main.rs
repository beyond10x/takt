//! `takt`: drive a repository's work through engineering protocols, every step validated.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use clap::{Parser, Subcommand};
use takt::Frontier as _;
use takt::forge::{BotRoute, Forge, Process, Runner, repository_of};
use takt::frontier::Aep;
use takt::harness::{ClaudeCode, Harness};
use takt::validator::{
    ChangeRead, CiRead, DEFAULT_PROJECT, GateRun, PrMerged, PrOpen, TestsRun, Validator,
};
use takt::{LoadedProtocol, Run, RunOptions, Stop, Unit};

/// The git revision this binary was built from, as `build.rs` read it: `<sha>`, `<sha>-dirty`
/// or `unknown revision`.
const REVISION: &str = env!("TAKT_REVISION");

#[derive(Parser)]
#[command(
    name = "takt",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("TAKT_REVISION"), ")"),
    about = "Composable agentic work: one bounded, validated run at a time"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run on a repository: take the intent, open a case or resume one, loop until a stop.
    #[command(after_help = EXIT_CODES)]
    Run(Box<RunArgs>),
    /// Grant a case a capability: write the grant into the case's authority.yaml and record the
    /// approval on the case's story in the planning store, that record committed on the case's
    /// branch, on which the tree must be, clean. An intent's case and an improvement turn
    /// (`self.improvement/1`) get the grant alone: nothing in the store, no commit, no store
    /// needed. The next run resumes the case.
    Approve(ApproveArgs),
    /// List the built-in protocols.
    Protocols,
    /// The repository's scenarios: a story at a known seed commit, run from a fresh base.
    #[command(subcommand)]
    Scenario(ScenarioCommands),
    /// Print, per run of a case under `.engineering/cases/`, its outcome, steps, tokens,
    /// refusals, `NoProgress`, errors and wall time, read from the case's `events.jsonl` only;
    /// a case without one is listed as `no record`. The binary a case's runs were made with is
    /// read from a `binary.txt` beside the record when the operator wrote one (first line
    /// `<path>\t<sha256>`, every further line a note), saved with each run as the operator's
    /// statement, never a figure. A scenario run's outcome comes from the held-out check the
    /// runner wrote beside the record (`held-out.txt`, first line `PASS` or the failure):
    /// `failed` unless it passed, its own stop printed beside it and a stop claiming completion
    /// over a failed check named as a disagreement; a scenario case without one is refused.
    #[command(after_help = SCORECARD_EXIT_CODES)]
    Scorecard(ScorecardArgs),
    /// Write a case's run record as one `takt-trace/1` file: a header line on the run, then
    /// one line per operation (a step, a wait on the forge, a refusal, a stop, the resume
    /// after one) in one vocabulary, read from the case's `events.jsonl`; token and cost
    /// figures from the harness's raw usage only, summaries never copied.
    #[command(after_help = TRACE_EXIT_CODES)]
    Trace(TraceArgs),
    /// Print where runs' time and tokens went: one table across the runs given (wall time
    /// split by actor, steps, model calls, turns, tokens, cost), then per run the actors'
    /// shares, a row per step, the defects and what the sources did not record.
    #[command(after_help = REPORT_EXIT_CODES)]
    Report(ReportArgs),
    /// Promote a takt revision: compare its saved scorecard with the promoted baseline's under
    /// the scorecard's rule and, when no scenario is worse, build a fresh clone of that
    /// revision confined in bubblewrap (`bwrap`: only the clone, rustup's default toolchain
    /// as its settings file names it, never one on PATH, the host's registry cache read-only
    /// and the system directories visible, the environment cleared, `cargo build --locked
    /// --offline`), install the binary with
    /// `cargo install --locked --path crates/takt-cli`, record the promotion under
    /// `$XDG_STATE_HOME/takt/promotions/` (`~/.local/state/takt/promotions/`), where the latest
    /// record is the next baseline, and then move the binary into place. The revision must be
    /// an ancestor of the checkout's `origin/main`, and every run of its scorecard made with its own
    /// binary, as the run's `opened` line names it. One promotion at a time
    /// under the store's lock; a refused candidate is recorded and not judged again until it
    /// changes, or on a scorecard of new runs with --rejudge; the installed binary must name the
    /// latest record's revision before anything is judged.
    #[command(after_help = PROMOTE_EXIT_CODES)]
    Promote(PromoteArgs),
}

#[derive(clap::Args)]
struct PromoteArgs {
    /// The takt checkout at the candidate revision, clean: `HEAD` must equal the scorecard's
    /// revision and no tracked file may differ from it. Not with --rebaseline.
    #[arg(
        long,
        required_unless_present = "rebaseline",
        conflicts_with = "rebaseline"
    )]
    repo: Option<PathBuf>,
    /// The candidate's saved scorecard, `scorecard-<revision>.json` with a full commit id,
    /// outside the repository. Not with --rebaseline.
    #[arg(
        long,
        value_name = "FILE",
        required_unless_present = "rebaseline",
        conflicts_with = "rebaseline"
    )]
    scorecard: Option<PathBuf>,
    /// Replace the baseline's scorecard with FILE, nothing built or installed: accepted only
    /// when its revision is the baseline's, its case ids are the stored scorecard's exactly, and
    /// every metric definition is at or above the stored version with one of them higher (a
    /// baseline saved before a definition changed, re-scored under the new one). The record
    /// appended to the chain names the scorecard it replaces by its sha256. With --dry-run and
    /// --install-root only.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = ["baseline_only", "revision", "binary_sha256", "rejudge"]
    )]
    rebaseline: Option<PathBuf>,
    /// Record the baseline only: the scorecard of the revision installed today, compared with
    /// nothing; nothing is built or installed. Refused when the store holds a baseline already.
    #[arg(long)]
    baseline_only: bool,
    /// With --baseline-only: the revision the installed binary was built from, a full commit
    /// id equal to the scorecard's, for a binary built before `--version` named its revision;
    /// the record marks it as stated by the operator. Refused without --baseline-only.
    #[arg(long, value_name = "SHA")]
    revision: Option<String>,
    /// With --baseline-only and --revision: the installed binary's sha256, lower-case hex.
    /// `<root>/bin/takt` must name no revision and hash to it, and every run of the scorecard
    /// must state the same digest in a `binary.txt` beside its record
    /// (`.engineering/cases/<case>/binary.txt`, first line `<path>\t<sha256>`, every further
    /// line the operator's note, copied into the record); otherwise refused naming the binary's
    /// digest or the run, nothing written. Refused when the binary's `--version` names a
    /// revision, and without --baseline-only or --revision.
    #[arg(long, value_name = "HEX")]
    binary_sha256: Option<String>,
    /// Print the verdict and the record that would be written; write nothing, install nothing.
    #[arg(long)]
    dry_run: bool,
    /// Judge again a candidate whose revision or tree was refused, on a scorecard of new runs:
    /// one whose digest differs from every scorecard it was refused on. The record names the
    /// latest refusal and its digest; refused again, it is recorded beside the first refusal.
    /// Refused without an earlier refusal and with --baseline-only.
    #[arg(long)]
    rejudge: bool,
    /// Where the binary is installed, `cargo install --root`; without it `CARGO_INSTALL_ROOT`,
    /// else `CARGO_HOME`, else `~/.cargo`.
    #[arg(long, value_name = "DIR")]
    install_root: Option<PathBuf>,
}

/// What `takt promote --help` says about the exit codes.
const PROMOTE_EXIT_CODES: &str = "Exit codes:
  0  promoted and recorded; or the baseline recorded (--baseline-only); or the baseline's scorecard replaced (--rebaseline); or, with --dry-run, the candidate would be promoted or the baseline would be recorded
  1  refused, nothing written or installed but the head beside the store where none was found (written from the chain as it stands, said on stderr; not on --dry-run): the scorecard, the store, the install root or the build directory is inside the repository; the scorecard has more than one link, names no full revision, is not named by it, differs from HEAD, is no ancestor of the checkout's origin/main as last fetched (or there is none), a local main moved by hand counting for nothing, was saved under another rule or metric definition, holds a run made with another binary than the revision's (the run named; its `opened` line names another revision, the revision -dirty, or none; with --binary-sha256 the binary.txt statements are checked instead) or a scenario run without its held-out result; the checkout has uncommitted or untracked files or index entries marked assume-unchanged or skip-worktree, or commits a rust-toolchain.toml with a `path`; bubblewrap (`bwrap`) or a toolchain is not found; another promotion holds the store's lock; no baseline is recorded; the installed binary does not name the latest record's revision, or names none and hashes to another digest than the record's binary_sha256, or to none it records (with --baseline-only, the revision recorded); --revision or --binary-sha256 without --baseline-only, --binary-sha256 without --revision, a --revision that is no full commit id or not the scorecard's, a --binary-sha256 that is no sha256; with --binary-sha256, an installed binary whose --version names a revision or that hashes to another digest (both named), a run of the scorecard that names no revision beside runs that name one, or a run that states no binary or another (the run named); the revision or its tree was refused (without --rejudge) or recorded already; --rejudge without an earlier refusal of the revision or its tree, with --baseline-only, or on a scorecard sharing a case with one it was refused on (the case named); --rebaseline with a scorecard of another revision than the baseline's, of other cases than the stored scorecard's (the first named), under a metric definition below the stored one (named) or none newer, or with no baseline; a scenario the baseline has has fewer cases than the rule's trials on either side, or a scenario only the candidate has fewer on its side; the store holds a file takt does not read, a record that disagrees with its name, instant or scorecard copy, a chain of records with a gap, or one that ends before the head beside the store (promotions.head) or runs past it; the record could not be written; the reason on stderr
  2  the arguments do not parse
  3  refused on the scorecard: a scenario is worse than the baseline's; the refusal is recorded (not with --dry-run), nothing installed
  5  the candidate held but its clone, its confined build or its install failed, the built binary names another revision, or the binary names another revision or hashes differently once in place (then the binary before it is back in place and the record removed); nothing recorded, the installed binary unchanged";

#[derive(clap::Args)]
struct ScorecardArgs {
    /// The repository worktree whose cases are read.
    #[arg(long)]
    repo: PathBuf,
    /// Save the scorecard as `<dir>/scorecard-<revision>.json`, named by --revision, which it
    /// needs; a file there already is not overwritten.
    #[arg(long, value_name = "DIR")]
    save: Option<PathBuf>,
    /// The revision of takt under test, the full commit id (40 lower-case hex digits), that
    /// names the saved scorecard; never the scenario repository's HEAD. Every run's `opened`
    /// line must name it exactly, or no run's line name a revision (no `binary`, or `unknown
    /// revision`).
    #[arg(long, value_name = "SHA", requires = "save")]
    revision: Option<String>,
    /// Compare each scenario with the same scenario in a saved scorecard, over cases (a case's
    /// outcome is the best a run of it completed with, else its last run's), under this
    /// scorecard's rule, printed first (trials a scenario needs, margin of cases): worse when,
    /// for some outcome rank (accepted, then ApprovalRequired or Waiting, then a budget stop,
    /// then NoProgress and every other stop, then an error), its cases at that rank or better
    /// fall short of the saved share of its n by more than the margin, each such rank printed
    /// with both counts; or when its median of tokens exceeds the saved maximum. A side with fewer
    /// cases than the rule's trials, none included, gets no verdict. Tokens are printed as n, min,
    /// median and max per scenario and side, with the difference of the medians.
    #[arg(long, value_name = "FILE")]
    against: Option<PathBuf>,
}

/// What `takt scorecard --help` says about the exit codes.
const SCORECARD_EXIT_CODES: &str = "Exit codes:
  0  printed (and saved); with --against, no scenario is worse than the saved one
  1  refused: the repository, a case's events.jsonl, or the saved scorecard does not read (a binary.txt that does not read leaves its case's runs stating no binary, the case listed with the reason); a scenario case has no held-out.txt beside its record, or one that does not read or states no result (the case named); --save without --revision, or under a --revision that is not the full commit id (40 lower-case hex digits), onto a file that exists, or that cannot be written; --save with a run made with another binary than the --revision's (the run named; its `opened` line names another revision or the revision -dirty, or names none, no `binary` or `unknown revision`, beside runs that name one), nothing written; with --against, a scenario the saved scorecard has with fewer cases than the rule's trials (`too few cases on the saved side (<n> of <trials>)`), or any scenario with fewer on this side, none included (`too few cases (<n> of <trials>)`): no verdict given, as a promotion refuses; a saved scorecard with a run whose outcome disagrees with its held-out result; the reason on stderr
  2  the arguments do not parse
  3  with --against: a scenario is worse: at some outcome rank its cases at that rank or better fall short of the saved share by more than the rule's margin, or its median of tokens exceeds the saved maximum (a scenario with no run on this side is exit 1, too few cases)";

#[derive(clap::Args)]
struct TraceArgs {
    /// The case directory, holding `events.jsonl`.
    #[arg(long)]
    case: PathBuf,
    /// A runner shim's `steps.jsonl` beside the record: one `claude --output-format json`
    /// result per harness call, with `started_at` and `recorded_at`; read for the model, cost,
    /// turns, durations and token components a record written before takt recorded them
    /// lacks. The trace's notes say what it supplied.
    #[arg(long, value_name = "FILE")]
    steps: Option<PathBuf>,
    /// The run's name in the trace; the case id without it.
    #[arg(long)]
    run: Option<String>,
    /// Write the trace there instead of standard output; a file there is replaced.
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(clap::Args)]
#[command(group(clap::ArgGroup::new("input").required(true).multiple(true).args(["case", "trace"])))]
struct ReportArgs {
    /// A case directory, holding `events.jsonl`; repeatable.
    #[arg(long, value_name = "DIR")]
    case: Vec<PathBuf>,
    /// A runner shim's `steps.jsonl` for the --case given at the same position; repeatable,
    /// never more often than --case.
    #[arg(long, value_name = "FILE")]
    steps: Vec<PathBuf>,
    /// A `takt-trace/1` file `takt trace` wrote; repeatable.
    #[arg(long, value_name = "FILE")]
    trace: Vec<PathBuf>,
}

/// What `takt trace --help` says about the exit codes.
const TRACE_EXIT_CODES: &str = "Exit codes:
  0  written
  1  refused: the case has no events.jsonl, the record or the steps file does not read, or the output cannot be written; the reason on stderr
  2  the arguments do not parse";

/// What `takt report --help` says about the exit codes.
const REPORT_EXIT_CODES: &str = "Exit codes:
  0  printed
  1  refused: a case has no events.jsonl, a record, steps file or trace does not read, or --steps is given more often than --case; the reason on stderr
  2  the arguments do not parse, or neither --case nor --trace is given";

#[derive(Subcommand)]
enum ScenarioCommands {
    /// Run a scenario from a fresh base: on a clean tree, fetch the remote's `trial/*` branches
    /// through the bot route, cut `trial/<n>` from the scenario's seed commit (`<n>` one past
    /// the highest `trial/<n>` the checkout then knows, local or remote), check it out, refuse
    /// unless the store there shows the scenario's story ready (`proposed`, nothing blocking
    /// it, its dependencies done), push it through the bot route with a lease that only lets
    /// it create the branch, and refuse unless the push reports it created, then open a case
    /// for the story on `story.delivery/1`, whose pull request targets `trial/<n>`. The case is
    /// `scenario-<id>-trial-<n>`, refused before anything is cut when it or its branch exists;
    /// nothing of an earlier run is resumed. A refusal before the push removes the branch and
    /// puts the tree back.
    #[command(after_help = SCENARIO_EXIT_CODES)]
    Run(Box<ScenarioRunArgs>),
}

/// Where a repository declares its scenarios, in git's configuration syntax, one section a
/// scenario: `[scenario "<id>"]` with `story = <story id>` and `seed = <commit>`.
const SCENARIOS: &str = ".engineering/scenarios.conf";

#[derive(clap::Args)]
struct ScenarioRunArgs {
    /// The repository worktree.
    #[arg(long)]
    repo: PathBuf,
    /// The scenario, by the id its section in `.engineering/scenarios.conf` carries.
    scenario: String,
    /// The revision `.engineering/scenarios.conf` is read from, without a checkout.
    #[arg(long, default_value = "main")]
    declared_on: String,
    #[command(flatten)]
    looping: LoopArgs,
}

#[derive(clap::Args)]
struct ApproveArgs {
    /// The repository worktree.
    #[arg(long)]
    repo: PathBuf,
    /// The case, by id.
    case: String,
    /// The capability granted, `repository.merge` for the merge.
    capability: String,
    /// Who grants it: `human:<id>` for a person, `agent:<role>` for an agent role.
    #[arg(long)]
    by: String,
}

#[derive(clap::Args)]
struct RunArgs {
    /// The repository worktree.
    #[arg(long)]
    repo: PathBuf,
    /// What the run is meant to achieve; without it and without --case, the open case of a
    /// story being worked is resumed: one whose take is committed on its case's branch, or, from
    /// a tree not on a case's branch, one the planning store lists active; from a tree on a
    /// case's branch, a case the store lists active is resumed by a run from the base. Else the
    /// next ready story is the unit and the store records it taken. A `self.improvement/1` turn
    /// opened without it takes its finding's title as its intent.
    #[arg(long)]
    intent: Option<String>,
    /// The case to resume, by id, instead of opening one; not with --intent.
    #[arg(long)]
    case: Option<String>,
    /// The protocol: a built-in id (`takt protocols`) or a `protocol/1` file. Not given, a story
    /// the planning store gives opens on `story.delivery/1`, an --intent on `software.change/1`,
    /// and a resumed case runs on its own. Given for a resumed case, it must be the case's own:
    /// another is refused before the run, nothing changed.
    #[arg(long)]
    protocol: Option<String>,
    /// A case input, `name=value`; repeatable. One the protocol a case opens on does not declare
    /// is refused before the case is opened. Given for a resumed case, any input is refused
    /// before the case is resumed: the case keeps the inputs it was opened with. A
    /// `loop.improvement/1` case requires `loop_baseline=<full commit id>`, the promoted revision
    /// its baseline is, a commit of the repository: it opens the case's baseline and is no case
    /// input. A `self.improvement/1` turn requires `finding=<story id>`, a finding the planning
    /// store holds, and `candidate=<full commit id>`, the candidate delivered outside the case;
    /// its baseline is the promotion store's latest promoted revision. Neither is a case input.
    /// Its case input `surface` is computed from the candidate's change against the baseline's
    /// `.engineering/judge-allow.yaml`; given, it must be the computed one. Its finding is read on
    /// the mainline, `main` as the forge reports it, or the local `main` under `--forge none`,
    /// which the operator is trusted with. The turn records the sha256 of the binary that opens
    /// it, the promoted one, and every resume is refused before any step unless it is run by that
    /// binary: once `takt promote` has installed the candidate, finish the turn with the binary it
    /// replaced, which it keeps at `$XDG_STATE_HOME/takt/judges/<sha256>` and names.
    #[arg(long = "input", value_name = "NAME=VALUE")]
    inputs: Vec<String>,
    /// The case id; generated when absent.
    #[arg(long)]
    case_id: Option<String>,
    /// The checkout of the scenario repository a `self.improvement/1` turn's finding names its
    /// scenario in, handed to `scripts/check-finding --scenarios`; not read by any other case.
    #[arg(long)]
    scenarios: Option<PathBuf>,
    /// The install root a `self.improvement/1` turn opened by this run reads the promoted
    /// binary under (`<root>/bin/takt`, where `takt promote --install-root` installs it),
    /// recorded in the case, canonical, so every resume reads the same one. Not given,
    /// `CARGO_INSTALL_ROOT`, else `CARGO_HOME`, else `~/.cargo`, as `takt promote` takes it. A
    /// root that does not exist opens no turn. Refused for a resumed case, which keeps the one it
    /// was opened with; not read by any other case.
    #[arg(long)]
    install_root: Option<PathBuf>,
    #[command(flatten)]
    looping: LoopArgs,
}

/// What a run's loop is given, a scenario run's as a plain run's.
#[derive(clap::Args)]
struct LoopArgs {
    /// Stop after this many steps.
    #[arg(long)]
    iterations: Option<u64>,
    /// Stop after this many hours.
    #[arg(long)]
    hours: Option<f64>,
    /// Stop once the harness reported this many tokens.
    #[arg(long)]
    tokens: Option<u64>,
    /// The test command, split at white space, run without a shell.
    #[arg(long, default_value = "cargo test --quiet")]
    test_cmd: String,
    /// The harness: `claude` or `none` (only validators run).
    #[arg(long, default_value = "claude")]
    harness: String,
    /// The harness's model, when it takes one.
    #[arg(long)]
    model: Option<String>,
    /// The approval gate the harness may request to end the run. Not given, the case's
    /// protocol's first action that requires a capability and that the case's inputs do not
    /// block for good: `repository.merge` for a delivery or a software change,
    /// `candidate.merge` or `candidate.merge_judge_surface` for an improvement turn.
    #[arg(long)]
    gate: Option<String>,
    /// The gates document, relative to the repository; given, it must be a file, and the gate
    /// validator runs it. Not given, `.engineering/gates.yaml` when that is a file.
    #[arg(long)]
    gates: Option<PathBuf>,
    /// The `engineering-assertions-project/1` file that admits the commands and providers the
    /// gates bind, relative to the repository; given, it must be a file. Not given,
    /// `.engineering/assertions-project.yaml` when that is a file.
    #[arg(long)]
    project: Option<PathBuf>,
    /// The forge: `bot` (the repository's bot route: `b10x-gates` pushes the case's branch,
    /// opens its pull request and merges it, `gh` reads the pull request's state and checks) or
    /// `none` (no pull request is opened; `pr.open`, `ci.read` and `repository.merge` are left
    /// to the harness). Not given, `bot` when the repository's `origin` is a repository on
    /// github.com, the host the bot route serves, else `none`.
    #[arg(long)]
    forge: Option<String>,
    /// The least free space, in GiB, a step needs on the file system of the repository's tree
    /// and, when `CARGO_TARGET_DIR` is set, of the one holding that directory, each read before
    /// every step (a harness step, the test command, a gate, any deterministic step); under it
    /// the run stops `DiskLow` naming the path, its free space and this threshold, exit 3,
    /// before the step instead of failing in it. `0` turns the check off. A free space that
    /// cannot be read stops nothing and is said once.
    #[arg(long, value_name = "GIB", default_value_t = 10)]
    min_free_gib: u64,
    /// The author and committer of every commit of the case this run opens, `'<name>
    /// <email>'`, over the one `.engineering/commit-identity.conf` names (`[identity]` `name` and
    /// `email`), read at the commit the case forks from, or for a scenario run at
    /// --declared-on. Neither given, the commits are `takt <takt@localhost>`, never the
    /// operator's git configuration; a run whose forge is the bot route, which pushes them, is
    /// refused before its case opens. Recorded in the case: a resumed case and `takt approve`
    /// commit with the identity it was opened with, and another given for it is refused.
    #[arg(long, value_name = "'NAME <EMAIL>'")]
    commit_identity: Option<takt::git::CommitIdentity>,
}

/// A harness that performs nothing. Asked while an approval is pending, it requests the gate
/// action, changing no file, so the run stops `ApprovalRequired` and `takt approve` is the
/// next step; offered an action that records a choice (`change.handover`), it takes it,
/// changing no file, so the work committed by hand is handed over as it stands; asked for any
/// other judgement step, it ends the run with an error, `stopped: error:`, because nothing
/// performs that step.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &takt::StepContext<'_>) -> Result<takt::StepReport> {
        if let Some(gate) = context.approvals.first() {
            return Ok(takt::StepReport {
                action: gate.clone(),
                summary: "no harness: nothing but the approval is left; requesting it".to_owned(),
                tokens: None,
            });
        }
        if let Some(choice) = context.choices.first() {
            return Ok(takt::StepReport {
                action: choice.clone(),
                summary: "no harness: the work committed is taken as it stands".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// Exit 0: the run completed, the frontier had nothing ready, or the command was done.
const EXIT_DONE: u8 = 0;
/// Exit 1: refused before the run, the reason on stderr (`main`).
const EXIT_REFUSED: u8 = 1;
/// Exit 3: the run stopped on a budget, with nothing more it may do, or with too little free
/// disk for a step.
const EXIT_STOPPED: u8 = 3;
/// Exit 4: the run stopped for an approval; `takt approve` is the next step.
const EXIT_APPROVAL: u8 = 4;
/// Exit 5: the run ended on an error once its case was open, `stopped: error:`.
const EXIT_ERROR: u8 = 5;
/// Exit 6: the run waited on the forge and its budget ended first, or it waits on something
/// local and stopped at once, `stopped: Waiting`.
const EXIT_WAITING: u8 = 6;
/// Exit 7: no step can move the case, a claim left decided only by an action the run has no
/// performer for, `stopped: Unperformable`.
const EXIT_UNPERFORMABLE: u8 = 7;

/// What `takt run --help` says about the exit codes; each line opens with its code.
const EXIT_CODES: &str = "Exit codes:
  0  stopped: Completed (an outcome is legitimate), or frontier: nothing ready
  1  refused before the run (an argument, the repository, the protocol or the case); the reason on stderr, no step run
  2  the arguments do not parse
  3  stopped: a budget (Iterations, Hours, Tokens), NothingAdmissible, StepRefused or NoProgress; or DiskLow (<path>: <free> free, under <threshold>): before a step, the file system of the tree or of CARGO_TARGET_DIR had less free than --min-free-gib; the case stays open
  4  stopped: ApprovalRequired; `takt approve` grants it and the next run resumes the case
  5  stopped: error: the run ended on an error once its case was open (a forge that refused, a validator or the harness that failed); the case stays open
  6  stopped: Waiting (<actions>; <the budget spent>, no step or hour budget, or locally: <what it waits for>): the run waited on the forge (checks still running, a pull request's head behind the push, a merge held for a required check) and its budget (--iterations, --hours) ended first, or it had none to wait in; or it waits on something local (a saved scorecard, a promoted binary, a merge into the local main) and stopped at once after the step that observed it, naming the file it reads, with no pause and no observation again; nothing recorded for it, the case stays open and the next run resumes it
  7  stopped: Unperformable (<claim>; nothing performs it without <the forge, the gates document>): no step left moves an open obligation, and the only action that decides <claim> is admissible but has no performer in this run (--forge none, no gates document); no judgement step is taken, the case stays open";

/// What `takt scenario run --help` says about the exit codes: [`EXIT_CODES`] without `frontier:
/// nothing ready`, which only `takt run` prints; a scenario whose story is not ready is exit 1.
const SCENARIO_EXIT_CODES: &str = "Exit codes:
  0  stopped: Completed (an outcome is legitimate)
  1  refused before the run (an argument, the repository, the scenario, its seed, its story not ready there, its trial base or its case); the reason on stderr, no step run
  2  the arguments do not parse
  3  stopped: a budget (Iterations, Hours, Tokens), NothingAdmissible, StepRefused or NoProgress; or DiskLow (<path>: <free> free, under <threshold>): before a step, the file system of the tree or of CARGO_TARGET_DIR had less free than --min-free-gib; the case stays open
  4  stopped: ApprovalRequired; `takt approve` grants it and `takt run --case <case>` resumes the case
  5  stopped: error: the run ended on an error once its case was open (a forge that refused, a validator or the harness that failed); the case stays open
  6  stopped: Waiting (<actions>; <the budget spent>, no step or hour budget, or locally: <what it waits for>): the run waited on the forge (checks still running, a pull request's head behind the push, a merge held for a required check) and its budget (--iterations, --hours) ended first, or it had none to wait in; or it waits on something local (a saved scorecard, a promoted binary, a merge into the local main) and stopped at once after the step that observed it, naming the file it reads, with no pause and no observation again; nothing recorded for it, the case stays open and `takt run --case <case>` resumes the case
  7  stopped: Unperformable (<claim>; nothing performs it without <the forge, the gates document>): no step left moves an open obligation, and the only action that decides <claim> is admissible but has no performer in this run (--forge none, no gates document); no judgement step is taken, the case stays open";

fn main() -> ExitCode {
    let cli = Cli::parse();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match run(cli, &mut out) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("takt: {error:#}");
            ExitCode::from(EXIT_REFUSED)
        }
    }
}

/// Runs the command, printing to `out`; the exit code (an error is exit 1 from `main`, a clap
/// usage error exit 2, the codes `takt run --help` lists as [`EXIT_CODES`]):
///
/// | code | when |
/// |---|---|
/// | 0 | `Completed`; the frontier had nothing ready; `approve` and `protocols` done |
/// | 1 | refused before the run, the reason on stderr, no step run |
/// | 2 | the arguments do not parse (clap) |
/// | 3 | any other stop: a budget, `NothingAdmissible`, `StepRefused`, `NoProgress`, `DiskLow` |
/// | 4 | `ApprovalRequired`: `takt approve` is the next step |
/// | 5 | `stopped: error:`, the run ended on an error once its case was open: a forge that refused, a validator or the harness that failed |
/// | 6 | `Waiting`: the run waited on the forge until its budget ended, or waits on something local and stopped at once; the next run resumes the case |
/// | 7 | `Unperformable`: no step can move the case; a claim left is decided only by an action the run has no performer for |
fn run(cli: Cli, out: &mut dyn Write) -> Result<u8> {
    match cli.command {
        Commands::Protocols => {
            for (name, major) in b10x_canon_engineering_list() {
                writeln!(out, "{name}/{major}")?;
            }
            Ok(EXIT_DONE)
        }
        Commands::Approve(args) => {
            let ApproveArgs {
                repo,
                case,
                capability,
                by,
            } = args;
            let repo = repo
                .canonicalize()
                .with_context(|| format!("the repository {}", repo.display()))?;
            let approval =
                takt::frontier::approve(&repo, &Aep::default(), &case, &capability, &by)?;
            writeln!(
                out,
                "approved: {} for case {} by {}, written to {}",
                approval.capability,
                approval.case,
                approval.approver,
                approval.reference()
            )?;
            match &approval.story {
                Some(story) => {
                    writeln!(out, "recorded: approval on {story} in the planning store")?
                }
                None => writeln!(
                    out,
                    "recorded: nothing in the planning store; the case delivers no story (it was opened on an intent, or is an improvement turn)"
                )?,
            }
            Ok(EXIT_DONE)
        }
        Commands::Scenario(ScenarioCommands::Run(args)) => scenario_run(*args, out),
        Commands::Scorecard(ScorecardArgs {
            repo,
            save,
            revision,
            against,
        }) => {
            let worse = takt::scorecard::report(
                &repo,
                save.as_deref(),
                revision.as_deref(),
                against.as_deref(),
                out,
            )?;
            Ok(if worse > 0 { 3 } else { EXIT_DONE })
        }
        Commands::Trace(TraceArgs {
            case,
            steps,
            run,
            out: file,
        }) => {
            let trace = takt::trace::Trace::from_case(&case, steps.as_deref(), run.as_deref())?;
            match file {
                Some(path) => {
                    trace.save(&path)?;
                    writeln!(
                        out,
                        "written: {} ({} operations, {})",
                        path.display(),
                        trace.ops.len(),
                        takt::trace::FORMAT
                    )?;
                }
                None => trace.write(out)?,
            }
            Ok(EXIT_DONE)
        }
        Commands::Report(ReportArgs { case, steps, trace }) => {
            if steps.len() > case.len() {
                return Err(anyhow!(
                    "--steps is given {} times for {} --case: give at most one per case, in the same order",
                    steps.len(),
                    case.len()
                ));
            }
            let mut traces = Vec::new();
            for (n, dir) in case.iter().enumerate() {
                traces.push(takt::trace::Trace::from_case(
                    dir,
                    steps.get(n).map(PathBuf::as_path),
                    None,
                )?);
            }
            for path in &trace {
                traces.push(takt::trace::Trace::load(path)?);
            }
            takt::report::print(&traces, out)?;
            Ok(EXIT_DONE)
        }
        Commands::Promote(PromoteArgs {
            repo,
            scorecard,
            rebaseline,
            baseline_only,
            revision,
            binary_sha256,
            dry_run,
            rejudge,
            install_root,
        }) => {
            let store = takt::promotion::store()?;
            let install_root = match install_root {
                Some(root) => root,
                None => takt::promotion::install_root()?,
            };
            if let Some(scorecard) = rebaseline {
                let request = takt::promotion::Rebaseline {
                    scorecard: &scorecard,
                    store: &store,
                    install_root: &install_root,
                    dry_run,
                    clock: takt::now,
                };
                takt::promotion::rebaseline(&request, out)?;
                return Ok(EXIT_DONE);
            }
            let (Some(repo), Some(scorecard)) = (repo, scorecard) else {
                return Err(anyhow!(
                    "--repo and --scorecard are needed without --rebaseline"
                ));
            };
            let repo = repo
                .canonicalize()
                .with_context(|| format!("--repo {}", repo.display()))?;
            let build_dir = takt::promotion::build_dir()?;
            let request = takt::promotion::Request {
                repo: &repo,
                scorecard: &scorecard,
                store: &store,
                install_root: &install_root,
                build_dir: &build_dir,
                baseline_only,
                revision: revision.as_deref(),
                binary_sha256: binary_sha256.as_deref(),
                dry_run,
                rejudge,
                clock: takt::now,
            };
            match takt::promotion::promote(&request, out) {
                Ok(takt::promotion::Outcome::Regression(_))
                | Ok(takt::promotion::Outcome::WouldRefuse(_)) => Ok(EXIT_STOPPED),
                Ok(_) => Ok(EXIT_DONE),
                Err(error)
                    if error
                        .downcast_ref::<takt::promotion::BuildFailed>()
                        .is_some() =>
                {
                    eprintln!("takt: {error:#}");
                    Ok(EXIT_ERROR)
                }
                Err(error) => Err(error),
            }
        }
        Commands::Run(args) => {
            let RunArgs {
                repo,
                intent,
                case,
                protocol,
                inputs,
                case_id,
                scenarios,
                install_root,
                looping,
            } = *args;
            looping.check()?;
            let repo = repo
                .canonicalize()
                .with_context(|| format!("the repository {}", repo.display()))?;
            let inputs = inputs
                .iter()
                .map(|pair| {
                    pair.split_once('=')
                        .map(|(k, v)| (k.to_owned(), v.to_owned()))
                        .ok_or_else(|| anyhow!("an input is `name=value`, not `{pair}`"))
                })
                .collect::<Result<Vec<_>>>()?;
            // The protocol given is read now, so a wrong one is refused before anything is
            // opened. Not given, a case opens on the protocol for its unit: a story the store
            // gives on the one that delivers it through a pull request, an intent on a software
            // change; a resumed case runs on its own whatever is given here.
            let given = protocol
                .as_deref()
                .map(LoadedProtocol::resolve)
                .transpose()?;
            let opening = match (&protocol, &intent) {
                (Some(protocol), _) => protocol.clone(),
                (None, Some(_)) => SOFTWARE_CHANGE.to_owned(),
                (None, None) => STORY_DELIVERY.to_owned(),
            };
            let opens_on = match &given {
                Some(given) => given.clone(),
                None => LoadedProtocol::builtin(&opening)?,
            };
            // The case this run resumes, found before anything is resumed: resuming checks the
            // tree out onto the case's branch and records its head, so every refusal of what was
            // given for it comes first. A resumed case keeps the protocol and the inputs it was
            // opened with: a `--protocol` that names another, or any `--input`, is refused.
            // A turn opens a case whether or not an intent is given: with none, its finding's
            // title is the intent, and no case of the planning store is resumed for it.
            let opens_turn = case.is_none() && opens_on.id == takt::case::TURN_PROTOCOL;
            let resuming =
                case_to_resume(&repo, case.as_deref(), intent.is_some() || opens_turn, out)?;
            let given_for_resume = given.as_ref().zip(protocol.as_deref());
            let first_input = inputs.first().map(|(name, _)| name.clone());
            if let Some(resuming) = &resuming {
                refuse_for_resume(
                    &resuming.id,
                    &resuming.protocol,
                    given_for_resume,
                    first_input.as_deref(),
                )?;
                if install_root.is_some() {
                    return Err(anyhow!(
                        "--install-root is given for the case {}, which keeps the install root it was opened with; nothing was resumed",
                        resuming.id
                    ));
                }
                // A turn runs on the embedded protocol alone, and only by the binary that judges
                // it, recorded when it opened: both checked before the case is resumed, so no
                // step is taken otherwise.
                if let Some(why) = takt::case::turn_protocol_refusal(&resuming.protocol)? {
                    return Err(anyhow!(why));
                }
                takt::case::check_id(&resuming.id)?;
                if let Some(why) = takt::case::turn_judge_refusal(
                    &repo.join(takt::case::CASES_DIR).join(&resuming.id),
                    Path::new(RUNNING_BINARY),
                )? {
                    return Err(anyhow!(why));
                }
            }
            // A loop case opens with its baseline: `--input loop_baseline=<full commit id>`, no
            // case input of the protocol, is taken out of the inputs and checked here, so a
            // missing or wrong one is refused before anything is written.
            let mut inputs = inputs;
            let loop_baseline =
                if case.is_none() && resuming.is_none() && opens_on.id == takt::case::LOOP_PROTOCOL
                {
                    let given = inputs
                        .iter()
                        .position(|(name, _)| name == takt::case::LOOP_BASELINE)
                        .map(|at| inputs.remove(at).1);
                    Some(takt::case::loop_baseline(&repo, given.as_deref())?)
                } else {
                    None
                };
            // The forge, read before the turn, whose mainline it reads: the forge's `main`, or the
            // local `main` with none.
            let forge = forge_of(&repo, looping.forge.as_deref())?;
            // The identity a case opened here commits with: the one given, else the one the
            // repository configures at the commit the case forks from, required when the run
            // pushes its commits. A resumed case keeps its own, and is refused here when it has
            // none and the run pushes, or when another is given.
            let pushes = forge.forge.is_some();
            let commit_as = match &looping.commit_identity {
                Some(given) => takt::case::CommitAs::Given(given.clone()),
                None => takt::case::CommitAs::Configured { required: pushes },
            };
            if let Some(resuming) = &resuming {
                refuse_identity_for_resume(
                    &repo,
                    &resuming.id,
                    looping.commit_identity.as_ref(),
                    pushes,
                )?;
            }
            let mainline = forge.forge.clone().map_or(
                takt::improvement::Mainline::Local,
                takt::improvement::Mainline::Forge,
            );
            // An improvement turn opens with its finding and its candidate, `--input
            // finding=<story id>` and `--input candidate=<full commit id>`, no case inputs of the
            // protocol, taken out of the inputs and checked here with the promoted baseline, so
            // a missing or wrong one is refused before anything is written; its `surface`, given
            // or not, is computed, and put back as the case input the computation says.
            let turn = if case.is_none()
                && resuming.is_none()
                && opens_on.id == takt::case::TURN_PROTOCOL
            {
                if let Some(why) = takt::case::turn_protocol_refusal(&opens_on)? {
                    return Err(anyhow!(why));
                }
                let mut taken = |name: &str| {
                    inputs
                        .iter()
                        .position(|(given, _)| given == name)
                        .map(|at| inputs.remove(at).1)
                };
                let finding = taken(takt::case::TURN_FINDING);
                let candidate = taken(takt::case::TURN_CANDIDATE);
                let surface = taken(takt::case::TURN_SURFACE);
                // The binary that judges the turn is this one, which must be the promoted:
                // read as it runs, whatever its path holds by now.
                let binary = PathBuf::from(RUNNING_BINARY);
                let mut opened = takt::case::turn_opening(
                    &repo,
                    finding.as_deref(),
                    candidate.as_deref(),
                    surface.as_deref(),
                    &takt::promotion::store()?,
                    &takt::case::Judge {
                        revision: REVISION,
                        binary: &binary,
                    },
                    &mainline,
                )?;
                inputs.push((takt::case::TURN_SURFACE.to_owned(), opened.surface.clone()));
                // Recorded canonical, the directory itself: no `..` and no link left on the
                // way, which could later lead elsewhere. A root that is not there opens no
                // turn.
                let root = match &install_root {
                    Some(root) => root.clone(),
                    None => takt::promotion::install_root()?,
                };
                opened.install_root = Some(match std::fs::canonicalize(&root) {
                    Ok(root) if root.is_dir() => root,
                    Ok(_) => {
                        return Err(anyhow!(
                            "the install root {} is no directory; a turn opens on the install root `takt promote` installs into; nothing was written",
                            root.display()
                        ));
                    }
                    Err(error) => {
                        return Err(anyhow!(
                            "the install root {} is not there ({error}); a turn opens on the install root `takt promote` installs into, which must exist; nothing was written",
                            root.display()
                        ));
                    }
                });
                Some(opened)
            } else {
                None
            };
            let intent = intent.or_else(|| turn.as_ref().map(|turn| turn.title.clone()));
            // A case keeps only the inputs its protocol declares: one it does not declare would
            // be dropped without a word, so a case to be opened refuses it before anything is.
            if case.is_none()
                && resuming.is_none()
                && let Some((name, _)) = inputs
                    .iter()
                    .find(|(name, _)| !opens_on.inputs.iter().any(|(declared, _)| declared == name))
            {
                let declared: Vec<&str> = opens_on.inputs.iter().map(|(d, _)| d.as_str()).collect();
                return Err(anyhow!(
                    "--input {name}: {} declares no input {name}; it declares {}",
                    opens_on.id,
                    if declared.is_empty() {
                        "none".to_owned()
                    } else {
                        declared.join(", ")
                    }
                ));
            }
            // The software change's inputs select the obligations it owes; left out, they take
            // the least demanding values a change can carry, said on the first line of the run.
            if opens_on.id == SOFTWARE_CHANGE {
                for (name, value) in [
                    ("risk", "standard"),
                    ("change_kind", "feature"),
                    ("affects_security_boundary", "false"),
                ] {
                    if !inputs.iter().any(|(k, _)| k == name) {
                        inputs.push((name.to_owned(), value.to_owned()));
                    }
                }
            }
            let options = looping.options(opening, inputs, case_id);
            let harness = harness_of(&looping)?;
            let harness = harness.as_ref();
            let mut validators = validators_of(&repo, &looping, scenarios)?;
            forge.register(&mut validators);
            // An improvement turn's merge validators read the mainline, with a forge or without.
            validators.push(Box::new(takt::improvement::CandidateMerged::new(
                mainline.clone(),
            )));
            validators.push(Box::new(takt::improvement::CandidateMerged::judge_surface(
                mainline,
            )));
            let forge_line = forge.line;
            let store = Aep::default();
            // The unit: the case to resume, the intent given, or else the open case of an
            // active story or the next ready story of the planning store; a case is opened
            // before the store is told, so a run that cannot open its case takes nothing.
            let mut run = match (case, intent, resuming) {
                (Some(_), Some(_), _) => {
                    return Err(anyhow!(
                        "--case resumes a case and --intent opens one; give one of them"
                    ));
                }
                (Some(case), None, _) => Run::resume(&repo, options, &case, harness, &validators)?,
                (None, Some(intent), _) => Run::open_unit_as(
                    &repo,
                    options,
                    Unit::Intent(intent),
                    &commit_as,
                    harness,
                    &validators,
                )?,
                // The open case of an active story, found and checked above: resumed as found,
                // the store not read again.
                (None, None, Some(resuming)) => {
                    let run = Run::resume(&repo, options, &resuming.id, harness, &validators)?;
                    let title = match &run.unit {
                        Unit::Story(story) => story.title.clone(),
                        Unit::Intent(intent) => intent.clone(),
                    };
                    writeln!(
                        out,
                        "frontier: resumed {}: {title} (case {})",
                        resuming.story.as_deref().unwrap_or_default(),
                        resuming.id
                    )?;
                    run
                }
                // Nothing to resume, as found above: the next ready story is taken, and nothing
                // is resumed whatever the store says by now, so no case runs that was not
                // checked.
                (None, None, None) => {
                    match Run::take_next_as(
                        &repo,
                        options,
                        &store,
                        &commit_as,
                        harness,
                        &validators,
                        out,
                    )? {
                        Some(run) => run,
                        None => {
                            writeln!(out, "frontier: nothing ready")?;
                            return Ok(EXIT_DONE);
                        }
                    }
                }
            };
            // The loop case just opened gets its baseline and the snapshot of its definitions
            // before any step.
            if let Some(baseline) = &loop_baseline {
                run.case.open_loop_baseline(&repo, baseline)?;
            }
            // The turn just opened gets its revisions, its finding and its validators'
            // independence before any step.
            if let Some(turn) = &turn {
                run.case.open_turn(turn)?;
            }
            // A story's case that terminates accepted records its test result and its move
            // in the store; an intent's run has no story and writes nothing there.
            run.record_on(&store);
            finish(&mut run, forge_line.as_deref(), &looping, out)
        }
    }
}

impl LoopArgs {
    /// Refuses, before anything of the run is done, an hour budget that is not a finite
    /// positive number: `--hours inf` would never end a wait, `--hours NaN` never compare, and
    /// zero or less would be spent before the first step.
    fn check(&self) -> Result<()> {
        if let Some(hours) = self.hours
            && !(hours.is_finite() && hours > 0.0)
        {
            return Err(anyhow!(
                "--hours is {hours}: an hour budget is a finite number above 0"
            ));
        }
        Ok(())
    }

    /// The run's options: the protocol a case opens on, its inputs and its id, with this loop's
    /// budgets and gate.
    fn options(
        &self,
        protocol: String,
        inputs: Vec<(String, String)>,
        case_id: Option<String>,
    ) -> RunOptions {
        RunOptions {
            protocol,
            intent: String::new(),
            inputs,
            iterations: self.iterations,
            hours: self.hours,
            tokens: self.tokens,
            case_id,
            gate: self.gate.clone(),
        }
    }
}

/// The harness `--harness` names: `claude` with `--model`, or `none`.
fn harness_of(looping: &LoopArgs) -> Result<Box<dyn Harness>> {
    match looping.harness.as_str() {
        "claude" => Ok(Box::new(ClaudeCode {
            model: looping.model.clone(),
            ..ClaudeCode::default()
        })),
        "none" => Ok(Box::new(NoHarness)),
        other => Err(anyhow!("no harness `{other}`: claude or none")),
    }
}

/// The validators that need no forge: the test command, `change.read`, the gate when a gates
/// document is given or at its default path in `repo`, with the project file likewise, and
/// the loop scorecard's three, and an improvement turn's five but its merge validators,
/// `scenarios` the scenario repository's checkout its scenario checks are given.
fn validators_of(
    repo: &Path,
    looping: &LoopArgs,
    scenarios: Option<PathBuf>,
) -> Result<Vec<Box<dyn Validator>>> {
    let mut validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new(&looping.test_cmd)?),
        Box::new(ChangeRead),
    ];
    let gates = repository_file(repo, "gates", looping.gates.clone(), DEFAULT_GATES)?;
    let project = repository_file(repo, "project", looping.project.clone(), DEFAULT_PROJECT)?;
    if let Some(gates) = gates {
        let mut gate = GateRun::new(gates);
        if let Some(project) = project {
            gate = gate.with_project(project);
        }
        validators.push(Box::new(gate));
    }
    // The loop scorecard's validators: due only in a `loop.improvement/1` case, whose
    // protocol alone has their actions.
    validators.push(Box::new(takt::loop_scorecard::LoopMeasure));
    validators.push(Box::new(takt::loop_scorecard::LoopDefinitionCheck));
    validators.push(Box::new(takt::loop_scorecard::LoopCompare));
    // An improvement turn's validators: due only in a `self.improvement/1` case, whose
    // protocol alone has their actions. Its merge validators read the mainline, the forge's or
    // the local `main`, and are registered beside the forge's.
    let checking = takt::improvement::Checking::from_env().with_scenarios(scenarios);
    validators.push(Box::new(takt::improvement::FindingFile(checking.clone())));
    validators.push(Box::new(takt::improvement::ScenarioReproduce(
        checking.clone(),
    )));
    validators.push(Box::new(takt::improvement::ScenarioVerify(checking)));
    validators.push(Box::new(takt::improvement::ScorecardCompare::from_env()));
    validators.push(Box::new(takt::improvement::BinaryPromote::from_env()));
    Ok(validators)
}

/// The forge a run goes through, the line that says so once its case is open, and, for the bot
/// route, the route a scenario's trial bases are fetched and created through.
struct Forged {
    forge: Option<Rc<dyn Forge>>,
    line: Option<String>,
    trials: Option<TrialRoute>,
}

/// The bot route's two lines for a scenario's trial bases, beside the forge's own: a fetch of
/// the remote's `trial/*` branches, so the next number is chosen knowing them, and a push that
/// may only create `trial/<n>`. The push carries `--force-with-lease=refs/heads/trial/<n>:`
/// (the branch must not exist), so a `trial/<n>` another run created after the fetch is
/// rejected, never fast-forwarded; and it reports in `--porcelain` form, where only a `*` (new
/// ref) for the branch counts: a branch the remote already holds at the same commit is "up to
/// date", exit 0, which the lease does not refuse.
///
/// The fetch names the remote's HTTPS URL, not `remote`: `origin` may be an SSH URL, and takt
/// gives no child the user's SSH agent, so a fetch from it has no credential. Over HTTPS it
/// goes under the bot route's credential (or none, for a public repository). Its child gets
/// `GIT_ALLOW_PROTOCOL=https` ([`Self::FETCH_ENV`]), so an `insteadOf` rule in the
/// repository's own configuration, which the bot route's git still reads, cannot turn it back
/// into SSH: git refuses the transport and never starts `ssh`. Only the fetch gets it; a push or
/// another bot-route line keeps whatever transport it uses. It is held to
/// [`Self::FETCH_TIMEOUT`], and its child can wait on no prompt ([`takt::forge::NO_PROMPT`]).
struct TrialRoute {
    /// The remote the bot route pushes to; its remote-tracking refs take the fetched branches.
    remote: String,
    /// The remote's HTTPS URL, `https://github.com/<owner>/<repo>.git`, the fetch's source.
    url: String,
    /// What the fetch runs as, held to [`Self::FETCH_TIMEOUT`].
    fetcher: Process,
    runner: Box<dyn Runner>,
}

impl TrialRoute {
    /// How long the fetch may take before it is killed and the run refused.
    const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

    /// What the fetch's child is started with beyond every forge child's: git may use HTTPS
    /// alone, whatever URL the repository's configuration rewrites the fetch's to.
    const FETCH_ENV: [(&str, &str); 1] = [("GIT_ALLOW_PROTOCOL", "https")];

    /// The trial route of `bot`: its remote, the HTTPS URL of its repository, the fetch held to
    /// [`Self::FETCH_TIMEOUT`] and the push to the forge's limits.
    fn of(bot: &BotRoute) -> Self {
        Self {
            remote: bot.remote.clone(),
            url: format!("https://{}/{}.git", BotRoute::HOST, bot.repository),
            fetcher: Process {
                limits: takt::forge::Limits {
                    timeout: Self::FETCH_TIMEOUT,
                    ..Process::LIMITS
                },
            },
            runner: Box::new(Process::default()),
        }
    }

    /// The line that fetches the remote's trial branches, from its HTTPS URL, into its
    /// remote-tracking refs.
    fn fetch_argv(&self) -> Vec<String> {
        let refspec = format!("+refs/heads/trial/*:refs/remotes/{}/trial/*", self.remote);
        ["b10x-gates", "bot", "--", "fetch", &self.url, &refspec]
            .map(str::to_owned)
            .to_vec()
    }

    /// The line that creates the remote's `trial` from the local one, reporting each ref on
    /// stdout; the lease refuses it when the remote has the branch.
    fn push_argv(&self, trial: &str) -> Vec<String> {
        let lease = format!("--force-with-lease=refs/heads/{trial}:");
        let refspec = format!("refs/heads/{trial}:refs/heads/{trial}");
        [
            "b10x-gates",
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

    /// `exit`, how `argv` ended; a program that did not exit 0 is an error naming the line and
    /// what it said.
    fn succeed(argv: &[String], exit: takt::forge::Exit) -> Result<takt::forge::Exit> {
        if !exit.success() {
            return Err(anyhow!(
                "{} exited {}: {}",
                argv.join(" "),
                exit.code
                    .map_or("on a signal".to_owned(), |code| code.to_string()),
                one_line(&exit.stderr)
            ));
        }
        Ok(exit)
    }

    /// Fetches the remote's trial branches over HTTPS only ([`Self::FETCH_ENV`]), killed past
    /// [`Self::FETCH_TIMEOUT`].
    fn fetch(&self, tree: &Path) -> Result<()> {
        let argv = self.fetch_argv();
        let exit = self.fetcher.run_with(tree, &argv, &Self::FETCH_ENV)?;
        Self::succeed(&argv, exit).map(|_| ())
    }

    /// Pushes `trial` and answers only when the push exits 0 and reports it created on the
    /// remote. A refusal says what the push reported for the branch and what that means for
    /// the remote's `trial`: as it was (`=` up to date, `!` rejected), moved (` ` or `+`, which
    /// only a route that dropped the lease reports), or not known (no report for it).
    fn create(&self, tree: &Path, trial: &str) -> Result<()> {
        let argv = self.push_argv(trial);
        let exit = self.runner.run(tree, &argv)?;
        let target = format!(":refs/heads/{trial}");
        let reported = exit.stdout.lines().find_map(|line| {
            let mut fields = line.split('\t');
            let flag = fields.next()?;
            let refs = fields.next()?;
            refs.ends_with(&target)
                .then(|| (flag.to_owned(), fields.next().unwrap_or("").to_owned()))
        });
        let exited = exit
            .code
            .map_or("on a signal".to_owned(), |code| format!("{code}"));
        let (flag, summary) = match reported {
            Some((flag, _)) if flag == "*" && exit.success() => return Ok(()),
            Some(found) => found,
            None => {
                return Err(anyhow!(
                    "{trial} was not created on the remote, as far as takt can tell: {} exited {exited} and reported nothing for it ({}); whether the remote's {trial} changed is not known",
                    argv.join(" "),
                    one_line(&format!("{}\n{}", exit.stdout, exit.stderr))
                ));
            }
        };
        let meaning = match flag.as_str() {
            "=" => format!("the remote holds {trial} already, at this commit, and it is as it was"),
            "!" => format!(
                "the push was rejected: the remote holds {trial} already, and it is as it was"
            ),
            " " | "+" => format!(
                "the remote's {trial} was there already and was moved, not created: the route did not hold the push to its lease; another run's base may have moved"
            ),
            _ => format!("the remote's {trial} was not created"),
        };
        Err(anyhow!(
            "{trial} was not created on the remote: {} exited {exited} and reported `{flag} {summary}` for it: {meaning}",
            argv.join(" ")
        ))
    }
}

impl Forged {
    /// Adds the forge's validators, `pr.open`, `ci.read` and `repository.merge`, when there is
    /// a forge; `ci.read` given what the validators already there run from the tree, the test
    /// command among it, for its judge surface.
    fn register(&self, validators: &mut Vec<Box<dyn Validator>>) {
        if let Some(forge) = &self.forge {
            let runs: Vec<String> = validators.iter().flat_map(|v| v.runs()).collect();
            validators.push(Box::new(PrOpen::new(forge.clone())));
            validators.push(Box::new(CiRead::new(forge.clone()).with_runs(runs)));
            validators.push(Box::new(PrMerged::new(forge.clone())));
        }
    }
}

/// The forge: the bot route when asked for, or by default when `origin` is a repository on
/// github.com, the host the bot route serves. `--forge bot` elsewhere is refused in one line
/// before any case is opened; by default another host gets no forge, said in one line once the
/// case is open.
fn forge_of(repo: &Path, forge: Option<&str>) -> Result<Forged> {
    let bot = |repo: &Path| -> Result<Forged> {
        let bot = BotRoute::for_tree(repo)?;
        let line = format!(
            "forge: bot ({}); pr.open, ci.read and repository.merge go through the bot route",
            bot.repository
        );
        let trials = TrialRoute::of(&bot);
        Ok(Forged {
            forge: Some(Rc::new(bot)),
            line: Some(line),
            trials: Some(trials),
        })
    };
    let none = |line: Option<String>| Forged {
        forge: None,
        line,
        trials: None,
    };
    Ok(match forge {
        Some("none") => none(None),
        Some("bot") => bot(repo)?,
        None => match takt::git::remote_url(repo, "origin")
            .ok()
            .as_deref()
            .map(repository_of)
        {
            None => none(None),
            Some(Some(origin)) if BotRoute::serves(&origin.host) => bot(repo)?,
            Some(Some(origin)) => none(Some(format!(
                "forge: none (origin {} is not {})",
                origin.host,
                BotRoute::HOST
            ))),
            Some(None) => none(Some(
                "forge: none (origin names no <owner>/<repo> on a host)".to_owned(),
            )),
        },
        Some(other) => return Err(anyhow!("no forge `{other}`: bot or none")),
    })
}

/// Runs the open case `run` to its stop, `forge_line` said first, the run given this binary's
/// [`REVISION`] to record; the exit code. An error the loop ends on once the case is open, a
/// forge that refused the pull request among them, is a stop like any other: one line on
/// stdout, its own code. Every run the CLI makes stops `Unperformable` when no step can move
/// its case ([`Run::stop_when_unperformable`], off in the library until a caller turns it on),
/// and `DiskLow` before a step when the tree's or `CARGO_TARGET_DIR`'s file system has less
/// free than `--min-free-gib` ([`Run::stop_when_disk_below`]).
fn finish(
    run: &mut Run<'_>,
    forge_line: Option<&str>,
    looping: &LoopArgs,
    out: &mut dyn Write,
) -> Result<u8> {
    if let Some(line) = forge_line {
        writeln!(out, "{line}")?;
    }
    run.binary = Some(REVISION.to_owned());
    run.take_default_gate();
    run.stop_when_unperformable();
    run.stop_when_disk_below(
        looping.min_free_gib.saturating_mul(1 << 30),
        std::env::var_os("CARGO_TARGET_DIR")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from),
    );
    let stop = match run.run(out) {
        Ok(stop) => stop,
        Err(error) => {
            writeln!(
                out,
                "stopped: error: {}; steps {}, tokens {}, case {}",
                one_line(&format!("{error:#}")),
                run.steps,
                run.tokens,
                run.case.dir.display()
            )?;
            return Ok(EXIT_ERROR);
        }
    };
    writeln!(
        out,
        "stopped: {stop}; steps {}, tokens {}, case {}",
        run.steps,
        run.tokens,
        run.case.dir.display()
    )?;
    Ok(match stop {
        Stop::Completed(_) => EXIT_DONE,
        Stop::ApprovalRequired(_) => EXIT_APPROVAL,
        Stop::Waiting { .. } => EXIT_WAITING,
        Stop::Unperformable { .. } => EXIT_UNPERFORMABLE,
        Stop::DiskLow { .. } => EXIT_STOPPED,
        _ => EXIT_STOPPED,
    })
}

/// A scenario as the repository declares it in [`SCENARIOS`]: one story of its store, run from
/// a fresh base cut at the seed commit each time.
struct Scenario {
    id: String,
    story: String,
    seed: String,
}

impl Scenario {
    /// The scenario `id` as [`SCENARIOS`] at `revision` declares it, read without a checkout.
    /// An id that cannot name a case and a branch, a declaration that is missing, lacks `story`
    /// or `seed`, gives one twice, or gives a key takt does not read, is refused.
    fn read(repo: &Path, revision: &str, id: &str) -> Result<Self> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(anyhow!(
                "scenario {id}: a scenario id is letters, digits, `-` and `_`, so it can name a case and a branch"
            ));
        }
        let entries = takt::git::config_at(repo, revision, SCENARIOS)?.ok_or_else(|| {
            anyhow!(
                "scenario {id}: {revision} holds no {SCENARIOS} in {}; the scenarios are declared there",
                repo.display()
            )
        })?;
        let prefix = format!("scenario.{id}.");
        let (mut story, mut seed) = (None, None);
        let mut declared = false;
        for (key, value) in entries {
            let Some(name) = key.strip_prefix(&prefix) else {
                continue;
            };
            declared = true;
            let slot = match name {
                "story" => &mut story,
                "seed" => &mut seed,
                other => {
                    return Err(anyhow!(
                        "scenario {id}: {SCENARIOS} at {revision} gives `{other}`, which takt does not read; a scenario gives `story` and `seed`"
                    ));
                }
            };
            let value = value
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow!("scenario {id}: {SCENARIOS} at {revision} gives `{name}` no value")
                })?;
            if slot.replace(value.trim().to_owned()).is_some() {
                return Err(anyhow!(
                    "scenario {id}: {SCENARIOS} at {revision} gives `{name}` twice"
                ));
            }
        }
        if !declared {
            return Err(anyhow!(
                "scenario {id}: {SCENARIOS} at {revision} declares no scenario {id}"
            ));
        }
        let missing =
            |name: &str| anyhow!("scenario {id}: {SCENARIOS} at {revision} gives no `{name}`");
        Ok(Self {
            id: id.to_owned(),
            story: story.ok_or_else(|| missing("story"))?,
            seed: seed.ok_or_else(|| missing("seed"))?,
        })
    }
}

/// The trial base a scenario run cuts next: one past the highest `trial/<n>` the checkout
/// knows, a local branch or a remote's (as last fetched), `0` when there is none. Names under
/// `trial/` that are not all digits are not trial bases. A number too large for `u64`, or a
/// highest number nothing follows, is refused: never skipped, never wrapped to a number in use.
fn next_trial(repo: &Path) -> Result<u64> {
    let known = takt::git::refs(repo, &["refs/heads/trial", "refs/remotes"])?;
    let mut highest: Option<u64> = None;
    for name in &known {
        let Some(n) = name.strip_prefix("refs/heads/trial/").or_else(|| {
            name.strip_prefix("refs/remotes/")
                .and_then(|rest| rest.split_once('/'))
                .and_then(|(_, branch)| branch.strip_prefix("trial/"))
        }) else {
            continue;
        };
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let n: u64 = n.parse().map_err(|_| {
            anyhow!("{name} numbers a trial past the largest a scenario run counts to; nothing was changed")
        })?;
        highest = Some(highest.map_or(n, |h| h.max(n)));
    }
    match highest {
        None => Ok(0),
        Some(n) => n.checked_add(1).ok_or_else(|| {
            anyhow!("trial/{n} is the highest trial branch known, and no number follows it; nothing was changed")
        }),
    }
}

/// Refuses `n` as the run's trial number when anything the run would create for it after the
/// push is there already: the case `scenario-<id>-trial-<n>`, as a directory or as its branch
/// `takt/<case>`. The trial branch itself is not, as `n` is past every one known. A case id
/// [`takt::case::check_id`] refuses (a scenario id long enough to take it past 250 bytes) is
/// refused here, before any branch is cut or pushed.
fn trial_free(repo: &Path, scenario: &str, n: u64) -> Result<String> {
    let case_id = format!("scenario-{scenario}-trial-{n}");
    takt::case::check_id(&case_id)?;
    let refused = |what: String| {
        anyhow!(
            "scenario {scenario}: {what} exists already, though no trial/{n} branch is known: the case of trial/{n} would collide with it; nothing was pushed"
        )
    };
    if repo.join(takt::case::CASES_DIR).join(&case_id).exists() {
        return Err(refused(format!("the case {case_id}")));
    }
    let branch = takt::case::Branch::name_of(&case_id);
    if !takt::git::refs(repo, &[&format!("refs/heads/{branch}")])?.is_empty() {
        return Err(refused(format!("the case's branch {branch}")));
    }
    Ok(case_id)
}

/// `takt scenario run`: the scenario read, the tree found clean, `trial/<n>` cut from the seed
/// and checked out, the story found `proposed` there, the branch pushed, then a case opened for
/// the story and run like any other. Until the push succeeds a refusal removes the branch and
/// puts the tree back on the branch, or the commit, it was on.
fn scenario_run(args: ScenarioRunArgs, out: &mut dyn Write) -> Result<u8> {
    let ScenarioRunArgs {
        repo,
        scenario,
        declared_on,
        looping,
    } = args;
    looping.check()?;
    let repo = repo
        .canonicalize()
        .with_context(|| format!("the repository {}", repo.display()))?;
    let scenario = Scenario::read(&repo, &declared_on, &scenario)?;
    let harness = harness_of(&looping)?;
    let harness = harness.as_ref();
    // A tree with uncommitted work is refused, never discarded: a checkout would carry it onto
    // the trial branch, and a store file a stopped run left changed would be read there.
    let uncommitted = takt::git::uncommitted(&repo)?;
    if !uncommitted.is_empty() {
        return Err(anyhow!(
            "scenario {}: the tree {} has uncommitted work: {}; a scenario run starts only from a clean tree, and checking out its trial branch would carry the work along: commit it, or move it out of the tree, first; nothing was changed",
            scenario.id,
            repo.display(),
            uncommitted.join(", ")
        ));
    }
    let seed = takt::git::run(
        &repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{}^{{commit}}", scenario.seed),
        ],
    )
    .map_err(|_| {
        anyhow!(
            "scenario {}: its seed {} is no commit in {}; fetch it first; nothing was changed",
            scenario.id,
            scenario.seed,
            repo.display()
        )
    })?;
    let forge = forge_of(&repo, looping.forge.as_deref())?;
    // The identity the trial's commits carry: the one given, else the one `--declared-on`
    // configures (the pinned seed predates any such file); none through the bot route, which
    // pushes them, is refused before anything is fetched, cut or pushed.
    let identity = match &looping.commit_identity {
        Some(given) => Some(given.clone()),
        None => takt::git::configured_identity(&repo, &declared_on)?,
    };
    if identity.is_none() && forge.forge.is_some() {
        return Err(takt::git::no_identity(&declared_on))
            .with_context(|| format!("scenario {}", scenario.id));
    }
    let commit_as = identity.map_or(takt::case::CommitAs::Takt, takt::case::CommitAs::Given);
    // The fetch is read-only and comes first, so the number is chosen and checked once, on
    // everything the checkout knows after it, and before anything is cut or pushed.
    if let Some(trials) = &forge.trials {
        trials.fetch(&repo).with_context(|| {
            format!(
                "scenario {}: fetch the remote's trial branches; nothing was changed",
                scenario.id
            )
        })?;
    }
    let n = next_trial(&repo)?;
    let case_id = trial_free(&repo, &scenario.id, n)?;
    let trial = format!("trial/{n}");
    let left = takt::git::current_branch(&repo)?;
    let at = takt::git::head(&repo)?;
    takt::git::create_branch(&repo, &trial, Some(&seed))?;
    let back = |error: anyhow::Error| -> anyhow::Error {
        let target = left.as_deref().unwrap_or(&at);
        let restored = takt::git::run(&repo, &["checkout", "-q", target])
            .and_then(|_| takt::git::run(&repo, &["branch", "-q", "-D", &trial]));
        match restored {
            Ok(_) => error,
            Err(undo) => error.context(format!(
                "and {trial} could not be removed, the tree put back on {target}: {undo:#}"
            )),
        }
    };
    let prepared = (|| -> Result<(Vec<Box<dyn Validator>>, takt::Story)> {
        // The story must be ready at the seed by the frontier's own rule, as `takt run` would
        // take it: `proposed`, nothing blocking it, every dependency done.
        let all = Aep::default().list(&repo)?;
        let Some(listed) = all.iter().find(|listed| listed.id == scenario.story) else {
            return Err(anyhow!(
                "scenario {}: the store at its seed {} lists no {}; nothing was pushed",
                scenario.id,
                short(&seed),
                scenario.story
            ));
        };
        if listed.status != takt::frontier::READY {
            return Err(anyhow!(
                "scenario {}: the store at its seed {} gives {} {}, not {}: no known starting state; nothing was pushed",
                scenario.id,
                short(&seed),
                scenario.story,
                listed.status,
                takt::frontier::READY
            ));
        }
        if !takt::frontier::is_ready(listed, &all) {
            return Err(anyhow!(
                "scenario {}: the store at its seed {} gives {} {} but not ready ({}): takt run would not take it, so it is no known starting state; nothing was pushed",
                scenario.id,
                short(&seed),
                scenario.story,
                listed.status,
                not_ready(listed, &all)
            ));
        }
        // Everything read before the push: the story's text and the gates and the project file,
        // the seed's, read on the trial branch.
        let story = Aep::default().show(&repo, &scenario.story)?;
        let mut validators = validators_of(&repo, &looping, None)?;
        forge.register(&mut validators);
        if let Some(trials) = &forge.trials {
            trials.create(&repo, &trial).with_context(|| {
                format!(
                    "scenario {}: create {trial} through the bot route",
                    scenario.id
                )
            })?;
        }
        Ok((validators, story))
    })();
    let (validators, story) = prepared.map_err(back)?;
    writeln!(
        out,
        "scenario {}: {} {} on {trial} at {}, {}",
        scenario.id,
        scenario.story,
        takt::frontier::READY,
        short(&seed),
        match &forge.forge {
            Some(forge) => format!("pushed through {}", forge.name()),
            None => "not pushed: no forge".to_owned(),
        }
    )?;
    let options = looping.options(STORY_DELIVERY.to_owned(), Vec::new(), Some(case_id));
    let store = Aep::default();
    let mut run = take_story(&repo, options, story, &commit_as, harness, &validators, out)?;
    // As `takt run`: a case that terminates accepted records its test result and its move in
    // the store, on the case's branch.
    run.record_on(&store);
    finish(&mut run, forge.line.as_deref(), &looping, out)
}

/// Why `story`, `proposed`, is not ready among `all`, as [`takt::frontier::is_ready`] reads it:
/// what blocks it and each dependency not done, with its status.
fn not_ready(story: &takt::frontier::Listed, all: &[takt::frontier::Listed]) -> String {
    let mut why: Vec<String> = story
        .blocked_by
        .iter()
        .map(|blocker| format!("blocked by {}", blocker.blocker))
        .collect();
    for relation in story
        .relations
        .iter()
        .filter(|r| r.relation == "depends_on")
    {
        let status = all
            .iter()
            .find(|listed| listed.id == relation.target)
            .map(|listed| listed.status.as_str());
        if !status.is_some_and(|status| takt::frontier::DONE.contains(&status)) {
            why.push(format!(
                "depends on {}, {}",
                relation.target,
                status.unwrap_or("not listed")
            ));
        }
    }
    if why.is_empty() {
        "the frontier's ready rule does not hold".to_owned()
    } else {
        why.join("; ")
    }
}

/// The first eight characters of a commit id.
fn short(revision: &str) -> &str {
    &revision[..8.min(revision.len())]
}

/// Opens a case for `story`, read from the store before the call, and takes it, as
/// [`Run::take_next`] does for the next ready one: the case first, then the store told, the take committed on the case's branch as
/// `takt: take <story>`, exactly the paths the store changed, and the case's implementation at
/// that commit. A refusal or a failure removes the case. Unlike `take_next` it is not skipped
/// for a case an earlier scenario run opened for the same story on another trial base: each
/// run of a scenario is a case of its own. The case is opened with, and the take committed by,
/// the identity `commit_as` says.
fn take_story<'a>(
    repo: &Path,
    options: RunOptions,
    story: takt::Story,
    commit_as: &takt::case::CommitAs,
    harness: &'a dyn Harness,
    validators: &'a [Box<dyn Validator>],
    out: &mut dyn Write,
) -> Result<Run<'a>> {
    let aep = Aep::default();
    let id = story.id.clone();
    let id = id.as_str();
    let title = story.title.clone();
    let mut run = Run::open_unit_as(
        repo,
        options,
        Unit::Story(story),
        commit_as,
        harness,
        validators,
    )?;
    let committer = run.committer().clone();
    let taken = (|| -> Result<String> {
        match aep.take(repo, id)? {
            takt::frontier::Take::Taken => {}
            takt::frontier::Take::Refused(refusal) => {
                return Err(anyhow!("the store would not take {id}: {refusal}"));
            }
        }
        let changed = takt::git::uncommitted(repo)?;
        takt::git::commit_paths(repo, &committer, &changed, &takt::run::take_subject(id))
    })();
    let head = match taken {
        Ok(head) => head,
        Err(error) => {
            let dir = run.case.dir.clone();
            return Err(match run.case.discard() {
                Ok(()) => error,
                Err(removal) => error.context(format!(
                    "and the case {} could not be removed: {removal:#}",
                    dir.display()
                )),
            });
        }
    };
    if run.case.revision("implementation").is_some() {
        run.case.set_revision("implementation", &head)?;
    }
    writeln!(out, "frontier: took {id}: {title}")?;
    Ok(run)
}

/// The running binary as the kernel holds it, whatever file its path names by now: what a turn's
/// judge is read and checked from.
const RUNNING_BINARY: &str = "/proc/self/exe";

/// The protocol a store story opens on when `--protocol` is not given.
const STORY_DELIVERY: &str = "story.delivery/1";
/// The protocol an `--intent` opens on when `--protocol` is not given.
const SOFTWARE_CHANGE: &str = "software.change/1";

/// The case a run resumes, read without resuming it.
struct Resuming {
    /// The case id.
    id: String,
    /// The protocol from the case's own `protocol.yaml`.
    protocol: LoadedProtocol,
    /// The active story whose open case it is, when the store gave it.
    story: Option<String>,
}

/// The case a run will resume, read without resuming it: the one `--case` names, when its
/// directory holds a case; for a run with neither `--case` nor `--intent`, the open case of a
/// story being worked, as [`Run::next_to_resume`] names it, the one case such a run resumes, a
/// decision that does not read said on `out`. `None` when the run opens a case, or when
/// `--case` names none (`Run::resume` refuses that). Nothing is checked out and nothing
/// written.
fn case_to_resume(
    repo: &Path,
    case: Option<&str>,
    intent: bool,
    out: &mut dyn Write,
) -> Result<Option<Resuming>> {
    let (id, story) = match (case, intent) {
        (Some(id), false) => (id.to_owned(), None),
        (None, false) => match Run::next_to_resume(repo, &Aep::default(), out)? {
            Some((id, story)) => (id, Some(story)),
            None => return Ok(None),
        },
        (_, true) => return Ok(None),
    };
    Ok(case_protocol(repo, &id)?.map(|protocol| Resuming {
        id,
        protocol,
        story,
    }))
}

/// The protocol of the case `id` under `repo`, from its own `protocol.yaml`; `None` when its
/// directory holds no case. An `id` that is not one name under the cases directory, or not a
/// case id at all, is refused by [`takt::case::check_id`] before it is joined to a path, so
/// nothing outside the cases directory is read.
fn case_protocol(repo: &Path, id: &str) -> Result<Option<LoadedProtocol>> {
    takt::case::check_id(id)?;
    let dir = repo.join(takt::case::CASES_DIR).join(id);
    if !dir.join("case.yaml").is_file() {
        return Ok(None);
    }
    LoadedProtocol::from_file(&dir.join("protocol.yaml")).map(Some)
}

/// Refuses what was given for the case `id`, which runs on `own` and is about to be resumed:
/// `given`, a protocol named on the command line as its spec, when it is not `own` (another id,
/// or another document under the same id); `input`, the first `--input`, whatever it is, because
/// a resumed case keeps the inputs it was opened with.
fn refuse_for_resume(
    id: &str,
    own: &LoadedProtocol,
    given: Option<(&LoadedProtocol, &str)>,
    input: Option<&str>,
) -> Result<()> {
    if let Some((given, spec)) = given {
        if own.id != given.id {
            return Err(anyhow!(
                "the case {id} runs on {}, not {} (--protocol {spec}): a case keeps the protocol it was opened on; resume it without --protocol; nothing was changed",
                own.id,
                given.id
            ));
        }
        if own.ir_json != given.ir_json {
            return Err(anyhow!(
                "the case {id} runs on its own copy of {}, and --protocol {spec} is another document of {}: a case keeps the protocol it was opened on; resume it without --protocol; nothing was changed",
                own.id,
                given.id
            ));
        }
    }
    if let Some(name) = input {
        return Err(anyhow!(
            "--input {name}: the case {id} is resumed, and a resumed case keeps the inputs it was opened with; run without --input; nothing was changed"
        ));
    }
    Ok(())
}

/// Refuses, before the case `id` is resumed, a `--commit-identity` (`given`) other than the
/// identity the case was opened with, and a run whose commits leave the machine (`pushes`, the
/// bot route) when the case was opened with none: its commits carry `takt <takt@localhost>`.
fn refuse_identity_for_resume(
    repo: &Path,
    id: &str,
    given: Option<&takt::git::CommitIdentity>,
    pushes: bool,
) -> Result<()> {
    let recorded = takt::case::commit_identity_of(&repo.join(takt::case::CASES_DIR).join(id))?;
    if let Some(given) = given
        && recorded.as_ref() != Some(given)
    {
        return Err(anyhow!(
            "--commit-identity {given}: the case {id} was opened with {}, and a resumed case keeps the identity it was opened with; resume it without --commit-identity; nothing was resumed",
            recorded.as_ref().map_or_else(
                || format!(
                    "none, so its commits are {}",
                    takt::git::CommitIdentity::takt()
                ),
                ToString::to_string
            )
        ));
    }
    if recorded.is_none() && pushes {
        return Err(anyhow!(
            "no commit identity: the case {id} was opened with none, so its commits are {}, and this run's commits leave the machine (the bot route); a case pushes only under the identity it opened with: open a new one with {} giving `identity.name` and `identity.email` at its base, or with --commit-identity '<name> <email>'; nothing was resumed",
            takt::git::CommitIdentity::takt(),
            takt::git::COMMIT_IDENTITY_FILE
        ));
    }
    Ok(())
}

/// `text` on one line: its lines, trimmed, the empty ones left out, joined with ` | `.
fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The gates document a run takes when `--gates` is not given and this is a file.
const DEFAULT_GATES: &str = ".engineering/gates.yaml";

/// The file `--<flag>` names, relative to `repo`: given, it must be a file under the repository,
/// or the run is refused in one line before any case is opened; not given, `default` when that
/// is a file, else none.
fn repository_file(
    repo: &Path,
    flag: &str,
    given: Option<PathBuf>,
    default: &str,
) -> Result<Option<PathBuf>> {
    match given {
        Some(path) => {
            let relative = under_repository(repo, flag, path)?;
            if !repo.join(&relative).is_file() {
                return Err(anyhow!("--{flag} {}: not a file", relative.display()));
            }
            Ok(Some(relative))
        }
        None => Ok(repo.join(default).is_file().then(|| PathBuf::from(default))),
    }
}

/// `path`, given to `--<flag>` to name a file of the repository, relative to `repo`: an absolute
/// path under the repository is made relative; a path not under it, absolute elsewhere, climbing
/// out through `..`, or resolving outside through a symlink, is refused.
fn under_repository(repo: &Path, flag: &str, path: PathBuf) -> Result<PathBuf> {
    let refused = |shown: &Path| {
        anyhow!(
            "--{flag} {}: not under the repository {}",
            shown.display(),
            repo.display()
        )
    };
    let relative = if path.is_absolute() {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
        resolved
            .strip_prefix(repo)
            .map(Path::to_path_buf)
            .map_err(|_| refused(&path))?
    } else {
        path
    };
    if relative
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(refused(&relative));
    }
    // A relative path may still point outside through a symlink: where it exists, what it
    // resolves to must lie under the repository too.
    if let Ok(resolved) = repo.join(&relative).canonicalize()
        && !resolved.starts_with(repo)
    {
        return Err(refused(&relative));
    }
    Ok(relative)
}

fn b10x_canon_engineering_list() -> Vec<(String, u32)> {
    LoadedProtocol::builtins()
}

#[cfg(test)]
mod tests {
    //! `takt run --repo R --harness none --iterations 1` with no `--intent` over a copy of a
    //! fixture planning store: the next ready story is the unit, its case is opened and the store
    //! lists it `active`; with nothing ready, no case is opened and the run says so and exits 0;
    //! with `--intent` the frontier is not asked.

    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use clap::Parser as _;

    use super::{Cli, run};

    /// The fixture stores of the library crate, one directory each.
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

    fn git(tree: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(args)
            .current_dir(tree)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    /// A scratch repository under this crate's build directory, removed on drop.
    struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_dir_all(&self.0) {
                let message = format!("remove the scratch tree {}: {error}", self.0.display());
                if std::thread::panicking() {
                    eprintln!("{message}");
                } else {
                    panic!("{message}");
                }
            }
        }
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("a directory");
        for entry in fs::read_dir(from).expect("a readable fixture") {
            let entry = entry.expect("an entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("a file type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), &target).expect("a copied file");
            }
        }
    }

    /// A committed copy of the fixture store `fixture`, with a check file beside it, in a
    /// scratch tree of its own: `name` keeps the cases of this process apart.
    fn repository(fixture: &str, name: &str) -> Scratch {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp")
            .join(format!("takt-cli-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
        }
        copy_dir(&Path::new(FIXTURES).join(fixture), &dir);
        fs::write(dir.join("check.txt"), "broken\n").expect("a file");
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "start"]);
        Scratch(dir)
    }

    /// Runs `takt <args>` in this process; the exit code and what it printed.
    fn takt(args: &[&str]) -> (u8, String) {
        let cli = Cli::parse_from(std::iter::once("takt").chain(args.iter().copied()));
        let mut out = Vec::new();
        let code = run(cli, &mut out).expect("takt runs");
        (code, String::from_utf8(out).expect("utf-8"))
    }

    fn status(repo: &Path, id: &str) -> String {
        takt::frontier::Aep::default()
            .list(repo)
            .expect("the store lists")
            .into_iter()
            .find(|a| a.id == id)
            .map(|a| a.status)
            .unwrap_or_else(|| panic!("{id} is listed"))
    }

    /// The one case directory under `repo`.
    fn the_case(repo: &Path) -> PathBuf {
        let mut cases: Vec<PathBuf> = fs::read_dir(repo.join(".engineering/cases"))
            .expect("a cases directory")
            .map(|e| e.expect("an entry").path())
            .collect();
        assert_eq!(cases.len(), 1, "{cases:?}");
        cases.remove(0)
    }

    #[test]
    fn without_an_intent_the_next_ready_story_is_the_unit() {
        let repo = repository("one-ready", "frontier");
        let (code, out) = takt(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ]);
        assert!(out.contains("frontier: took story:ready-one"), "{out}");
        assert!(out.contains("stopped: Iterations (1)"), "{out}");
        assert_eq!(code, 3, "the step budget stop exits 3, as before: {out}");
        let case = the_case(&repo);
        let id = case.file_name().expect("a name").to_string_lossy();
        assert!(id.starts_with("story-ready-one-"), "{id}");
        let intent = fs::read_to_string(case.join("intent.md")).expect("intent.md");
        assert!(
            intent.starts_with("Make the check pass\n\n# Story: ready-one\n"),
            "{intent}"
        );
        assert_eq!(status(&repo, "story:ready-one"), "active");
        assert_eq!(status(&repo, "story:blocked-one"), "proposed");
        assert_eq!(status(&repo, "story:waits-one"), "proposed");
    }

    #[test]
    fn with_nothing_ready_no_case_is_opened_and_the_run_exits_zero() {
        let repo = repository("none-ready", "nothing");
        let (code, out) = takt(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("frontier: nothing ready"), "{out}");
        assert!(!out.contains("case "), "{out}");
        assert!(!repo.join(".engineering/cases").exists());
        assert_eq!(status(&repo, "story:blocked-one"), "proposed");
        assert_eq!(status(&repo, "story:waits-one"), "proposed");
    }

    #[test]
    fn with_an_intent_the_frontier_is_not_asked() {
        let repo = repository("one-ready", "intent");
        let (code, out) = takt(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "make the check pass",
            "--case-id",
            "given",
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 3, "{out}");
        // The loop prints Canon's action frontier each evaluation; the store's lines are these.
        assert!(!out.contains("frontier: took"), "{out}");
        assert!(!out.contains("frontier: nothing ready"), "{out}");
        let case = the_case(&repo);
        assert_eq!(case.file_name().expect("a name"), "given");
        let intent = fs::read_to_string(case.join("intent.md")).expect("intent.md");
        assert_eq!(intent, "make the check pass");
        assert_eq!(status(&repo, "story:ready-one"), "proposed");
    }

    /// The fixture protocol of the library crate that declares `gate_result` and `gate.green`.
    const GATE_PROTOCOL: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../takt/tests/fixtures/gate-only/1.yaml"
    );

    /// A committed scratch repository with a manifest and, at `gates` when given, a gates
    /// document whose one assertion is true.
    fn gated_repository(name: &str, gates: Option<&str>) -> Scratch {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp")
            .join(format!("takt-cli-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
        }
        fs::create_dir_all(&dir).expect("a directory");
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
        if let Some(gates) = gates {
            let path = dir.join(gates);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(
                path,
                "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                 text: The repository has its manifest.\n    assertions:\n      \
                 - 'fs.file.exists(\"Cargo.toml\")'\n",
            )
            .expect("a file");
        }
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "start"]);
        Scratch(dir)
    }

    /// Runs `takt <args>` in this process on a run that fails; the error it answers.
    fn takt_error(args: &[&str]) -> String {
        let cli = Cli::parse_from(std::iter::once("takt").chain(args.iter().copied()));
        let mut out = Vec::new();
        let error = run(cli, &mut out).expect_err("takt refuses");
        format!("{error:#}")
    }

    #[test]
    fn with_the_default_gates_document_the_gate_validator_runs_the_gate() {
        let repo = gated_repository("gates-default", Some(".engineering/gates.yaml"));
        let (code, out) = takt(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "pass the gate",
            "--case-id",
            "gated",
            "--harness",
            "none",
            "--protocol",
            GATE_PROTOCOL,
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("step 1: gate.run {}"), "{out}");
        assert!(out.contains("evidence: gate_result pass"), "{out}");
        assert!(out.contains("stopped: Completed (accepted)"), "{out}");
    }

    #[test]
    fn the_gates_flag_names_the_document() {
        let repo = gated_repository("gates-flag", Some("gates/repository.yaml"));
        let (code, out) = takt(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "pass the gate",
            "--case-id",
            "gated",
            "--harness",
            "none",
            "--protocol",
            GATE_PROTOCOL,
            "--test-cmd",
            "true",
            "--gates",
            "gates/repository.yaml",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("step 1: gate.run {}"), "{out}");
        assert!(out.contains("evidence: gate_result pass"), "{out}");
        assert!(out.contains("stopped: Completed (accepted)"), "{out}");
    }

    /// Without a gates document no validator owns `gate.run`, and it only reads: a harness that
    /// keeps no state, `--harness none` among them, is not offered it
    /// (story:no-inspect-for-stateless-harness). `gate.green` is decided only by it, so the run
    /// stops `Unperformable` naming the claim and the gates document, exit 7, with no step taken.
    #[test]
    fn without_a_gates_document_gate_run_is_left_to_no_one() {
        let repo = gated_repository("gates-none", None);
        let (code, out) = takt(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "pass the gate",
            "--case-id",
            "ungated",
            "--harness",
            "none",
            "--protocol",
            GATE_PROTOCOL,
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 7, "{out}");
        assert!(out.contains("frontier: gate.run (admissible)"), "{out}");
        assert!(
            out.contains(
                "stopped: Unperformable (gate.green; nothing performs it without the gates document); steps 0, tokens 0"
            ),
            "{out}"
        );
    }

    /// The delivery protocol of this repository, which declares `pr.open` and `ci.read`.
    const DELIVERY_PROTOCOL: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../protocols/story-delivery/1.yaml"
    );

    /// `takt run` on the delivery protocol with no harness and a passing test command, `extra`
    /// last.
    fn delivery_args<'a>(repo: &'a str, case: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec![
            "run",
            "--repo",
            repo,
            "--intent",
            "deliver the story",
            "--case-id",
            case,
            "--harness",
            "none",
            "--protocol",
            DELIVERY_PROTOCOL,
            "--test-cmd",
            "true",
        ];
        args.extend_from_slice(extra);
        args
    }

    /// Without `--forge` and without an origin, neither forge validator is registered: no
    /// `forge:` line, no `pr.open` or `ci.read` step. The repository has no gates document
    /// either: after `tests.run` nothing offered moves an open obligation and `gate.run`, which
    /// alone decides `gate.green`, has no performer, so the run stops `Unperformable`, exit 7,
    /// before any judgement step.
    #[test]
    fn without_an_origin_the_forge_is_none_and_no_forge_step_is_taken() {
        let repo = gated_repository("forge-default-none", None);
        let (code, out) = takt(&delivery_args(
            repo.to_str().expect("utf-8"),
            "default-none",
            &[],
        ));
        assert_eq!(code, 7, "{out}");
        assert!(!out.contains("forge:"), "{out}");
        assert!(!out.contains("pr.open {}"), "{out}");
        assert!(!out.contains("ci.read {}"), "{out}");
        assert!(!out.contains("(none)"), "a judgement step: {out}");
        assert!(out.contains("step 1: tests.run {}"), "{out}");
        assert!(
            out.contains(
                "stopped: Unperformable (gate.green; nothing performs it without the gates document); steps 1, tokens 0"
            ),
            "{out}"
        );
    }

    #[test]
    fn with_forge_none_the_forge_validators_are_not_registered_even_with_an_origin() {
        let repo = gated_repository("forge-none", None);
        git(
            &repo,
            &["remote", "add", "origin", "git@github.com:owner/repo.git"],
        );
        let (code, out) = takt(&delivery_args(
            repo.to_str().expect("utf-8"),
            "none",
            &["--forge", "none"],
        ));
        assert_eq!(code, 7, "{out}");
        assert!(!out.contains("forge:"), "{out}");
        assert!(!out.contains("pr.open {}"), "{out}");
        assert!(!out.contains("ci.read {}"), "{out}");
        assert!(!out.contains("(none)"), "a judgement step: {out}");
        assert!(
            out.contains(
                "stopped: Unperformable (gate.green; nothing performs it without the gates document); steps 1, tokens 0"
            ),
            "{out}"
        );
    }

    /// With an origin and no `--forge`, the bot route is the forge, named with the repository
    /// its origin names; the step budget stops the run before anything would be pushed.
    #[test]
    fn with_an_origin_the_default_forge_is_the_bot_route() {
        let repo = gated_repository("forge-default-bot", None);
        git(
            &repo,
            &["remote", "add", "origin", "git@github.com:owner/repo.git"],
        );
        let (code, out) = takt(&delivery_args(
            repo.to_str().expect("utf-8"),
            "default-bot",
            &[
                "--iterations",
                "1",
                "--commit-identity",
                "Test Bot <bot@example.invalid>",
            ],
        ));
        assert_eq!(code, 3, "{out}");
        assert!(out.contains("forge: bot (owner/repo)"), "{out}");
        assert!(out.contains("step 1: tests.run {}"), "{out}");
    }

    #[test]
    fn forge_bot_without_an_origin_is_refused_before_the_run() {
        let repo = gated_repository("forge-bot-no-origin", None);
        let error = takt_error(&delivery_args(
            repo.to_str().expect("utf-8"),
            "bot",
            &["--forge", "bot"],
        ));
        assert!(error.contains("origin"), "{error}");
        assert!(!repo.join(".engineering/cases").exists());
    }

    /// The bot route pushes under a GitHub App and reads through `gh`: asked for on another
    /// host it is refused before any case is opened.
    #[test]
    fn forge_bot_on_a_non_github_origin_is_refused_before_the_run() {
        let repo = gated_repository("forge-bot-elsewhere", None);
        git(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                "git@gitlab.example.com:owner/repo.git",
            ],
        );
        let error = takt_error(&delivery_args(
            repo.to_str().expect("utf-8"),
            "elsewhere",
            &["--forge", "bot"],
        ));
        assert!(error.contains("not github.com"), "{error}");
        assert!(!repo.join(".engineering/cases").exists());
    }

    /// Without `--forge`, an origin on another host gets no forge, and the run says so.
    #[test]
    fn with_a_non_github_origin_the_default_forge_is_none_and_the_run_says_so() {
        let repo = gated_repository("forge-default-elsewhere", None);
        git(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                "https://gitlab.example.com/owner/repo.git",
            ],
        );
        let (code, out) = takt(&delivery_args(
            repo.to_str().expect("utf-8"),
            "default-elsewhere",
            &["--iterations", "1"],
        ));
        assert_eq!(code, 3, "{out}");
        assert!(
            out.contains("forge: none (origin gitlab.example.com is not github.com)"),
            "{out}"
        );
        assert!(!out.contains("forge: bot"), "{out}");
    }

    #[test]
    fn an_unknown_forge_is_refused_before_the_run() {
        let repo = gated_repository("forge-unknown", None);
        let error = takt_error(&delivery_args(
            repo.to_str().expect("utf-8"),
            "unknown",
            &["--forge", "hub"],
        ));
        assert_eq!(error, "no forge `hub`: bot or none");
        assert!(!repo.join(".engineering/cases").exists());
    }

    /// The lines of `aep plan artifact history <id>` in `repo` that record an approval.
    fn approvals_recorded(repo: &Path, id: &str) -> Vec<String> {
        let output = Command::new("aep")
            .args(["plan", "artifact", "history", id])
            .current_dir(repo)
            .output()
            .expect("aep runs");
        assert!(output.status.success(), "aep plan artifact history {id}");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.contains("approval recorded"))
            .map(str::to_owned)
            .collect()
    }

    /// `takt run` with no intent takes the ready story, on `software.change/1`, whose merge
    /// needs nothing but the tests, and, stopped at once by a step budget of zero, leaves its
    /// case open; the next `takt run` with no intent resumes the case and opens no second one,
    /// the story still active, and its tests bring the case to the gate, where it stops
    /// `ApprovalRequired`. `takt approve` writes the grant into that case, records the approval
    /// on the story and commits that record on the case's branch; the run after it resumes the
    /// case at the revision the grant was given at, on a clean tree, and reads the grant: the
    /// merge is no longer approval-required but admitted, a judgement step that no harness
    /// takes here. A grant is given only to a case at the gate.
    #[test]
    fn approve_writes_the_grant_and_the_next_run_resumes_the_open_case() {
        let repo = repository("one-ready", "approve");
        let path = repo.to_str().expect("utf-8");
        let (code, out) = takt(&[
            "run",
            "--repo",
            path,
            "--harness",
            "none",
            "--protocol",
            "software.change/1",
            "--iterations",
            "0",
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 3, "{out}");
        assert!(out.contains("frontier: took story:ready-one"), "{out}");
        assert!(out.contains(" on software.change/1 in "), "{out}");
        let case = the_case(&repo);
        let id = case
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        let resume = [
            "run",
            "--repo",
            path,
            "--harness",
            "none",
            "--test-cmd",
            "true",
        ];
        let (code, out) = takt(&resume);
        assert_eq!(code, 4, "{out}");
        assert!(
            out.contains(&format!(
                "frontier: resumed story:ready-one: Make the check pass (case {id})"
            )),
            "{out}"
        );
        assert!(out.contains("step 1: tests.run {}"), "{out}");
        assert!(
            out.contains("stopped: ApprovalRequired (repository.merge)"),
            "{out}"
        );
        assert_eq!(the_case(&repo), case, "no second case was opened");
        assert_eq!(status(&repo, "story:ready-one"), "active");
        assert!(approvals_recorded(&repo, "story:ready-one").is_empty());
        let gated = git_out(&repo, &["rev-parse", "HEAD"]);
        let (code, out) = takt(&[
            "approve",
            "--repo",
            path,
            &id,
            "repository.merge",
            "--by",
            "human:operator",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains(&format!(
                "approved: repository.merge for case {id} by human:operator, written to {id}/authority.yaml"
            )),
            "{out}"
        );
        assert!(
            out.contains("recorded: approval on story:ready-one in the planning store"),
            "{out}"
        );
        assert!(case.join("authority.yaml").is_file());
        let recorded = approvals_recorded(&repo, "story:ready-one");
        assert_eq!(recorded.len(), 1, "{recorded:?}");
        assert!(
            recorded[0].contains("takt approve repository.merge --by human:operator"),
            "{recorded:?}"
        );
        assert!(
            recorded[0].contains(&format!("({id}/authority.yaml)")),
            "{recorded:?}"
        );
        // The record is committed on the case's branch, alone, and nothing is left uncommitted.
        assert_eq!(
            git_out(&repo, &["log", "-1", "--format=%s"]),
            "takt: approve story:ready-one repository.merge"
        );
        assert_eq!(git_out(&repo, &["rev-parse", "HEAD^"]), gated);
        let committed = git_out(&repo, &["show", "--name-only", "--format=", "HEAD"]);
        assert!(
            committed
                .lines()
                .all(|path| path.starts_with(".engineering/evidence/story/ready-one/")),
            "{committed}"
        );
        assert_eq!(
            git_out(
                &repo,
                &["status", "--porcelain", "--", ".", ":!.engineering/cases"]
            ),
            ""
        );
        let (code, out) = takt(&resume);
        assert_eq!(code, 5, "{out}");
        assert!(
            out.contains(&format!(
                "frontier: resumed story:ready-one: Make the check pass (case {id})"
            )),
            "{out}"
        );
        assert!(!out.contains("refused"), "{out}");
        assert!(!out.contains("ApprovalRequired"), "{out}");
        assert!(out.contains("repository.merge (admissible)"), "{out}");
        assert!(
            out.contains(
                "no harness: a judgement step among implementation.edit, repository.merge, tests.write is left"
            ),
            "{out}"
        );
        assert_eq!(the_case(&repo), case, "no second case was opened");
    }

    /// `git <args>` in `tree`; its standard output, trimmed.
    fn git_out(tree: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(tree)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// `takt run --case <id>` resumes the case given, whatever the store says: a case stopped
    /// by a step budget of zero before its gate validator ran completes through it on resume.
    #[test]
    fn run_with_case_resumes_that_case_and_completes_through_its_validators() {
        let repo = gated_repository("resume", Some(".engineering/gates.yaml"));
        let path = repo.to_str().expect("utf-8");
        let (code, out) = takt(&[
            "run",
            "--repo",
            path,
            "--intent",
            "pass the gate",
            "--case-id",
            "gated",
            "--harness",
            "none",
            "--protocol",
            GATE_PROTOCOL,
            "--test-cmd",
            "true",
            "--iterations",
            "0",
        ]);
        assert_eq!(code, 3, "{out}");
        assert!(out.contains("stopped: Iterations (0)"), "{out}");
        let (code, out) = takt(&[
            "run",
            "--repo",
            path,
            "--case",
            "gated",
            "--harness",
            "none",
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("case gated on "), "{out}");
        assert!(out.contains("step 1: gate.run {}"), "{out}");
        assert!(out.contains("stopped: Completed (accepted)"), "{out}");
        assert_eq!(the_case(&repo).file_name().expect("a name"), "gated");
    }

    #[test]
    fn run_with_case_and_intent_is_refused_before_anything_is_opened() {
        let repo = gated_repository("case-and-intent", None);
        let error = takt_error(&[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--case",
            "one",
            "--intent",
            "another",
            "--harness",
            "none",
        ]);
        assert!(error.contains("--case"), "{error}");
        assert!(error.contains("--intent"), "{error}");
        assert!(!repo.join(".engineering/cases").exists());
    }

    #[test]
    fn approve_on_a_case_that_does_not_exist_is_refused() {
        let repo = gated_repository("approve-absent", None);
        let error = takt_error(&[
            "approve",
            "--repo",
            repo.to_str().expect("utf-8"),
            "absent",
            "repository.merge",
            "--by",
            "operator",
        ]);
        assert!(error.contains("absent"), "{error}");
        assert!(!repo.join(".engineering/cases").exists());
    }

    /// An hour budget that is not a finite number above 0 is refused before the run, for
    /// `takt run` and `takt scenario run`, and no case is opened.
    #[test]
    fn an_hour_budget_that_is_not_finite_and_positive_is_refused_before_the_run() {
        let repo = repository("one-ready", "hours-refused");
        let path = repo.to_str().expect("utf-8");
        for value in ["inf", "NaN", "0", "-1", "1e400"] {
            let hours = format!("--hours={value}");
            for args in [
                vec!["run", "--repo", path, "--harness", "none", &hours],
                vec![
                    "scenario",
                    "run",
                    "--repo",
                    path,
                    "one",
                    "--harness",
                    "none",
                    &hours,
                ],
            ] {
                let error = takt_error(&args);
                assert!(error.contains("--hours"), "{args:?}: {error}");
            }
        }
        assert!(!repo.join(".engineering/cases").exists());
    }
}
