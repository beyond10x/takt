//! `takt`: drive a repository's work through engineering protocols, every step validated.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use clap::{Parser, Subcommand};
use takt::Frontier as _;
use takt::forge::command::CommandForge;
use takt::forge::connectors::ConnectorsForge;
use takt::forge::gitlab::Glab;
use takt::forge::local::Local;
use takt::forge::{BotRoute, Forge, Process, Runner, repository_of};
use takt::frontier::Aep;
use takt::harness::{ClaudeCode, Harness, Kind, SessionHarness};
use takt::scan::Scan;
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
    /// A built-in protocol's declared settings.
    #[command(subcommand)]
    Protocol(ProtocolCommands),
    /// The repository's scenarios: a story at a known seed commit, run from a fresh base.
    #[command(subcommand)]
    Scenario(ScenarioCommands),
    /// Print, per run of a case under `.takt/cases/`, its outcome, steps, tokens,
    /// refusals, `NoProgress`, errors and wall time, read from the case's `events.jsonl` and, for
    /// a scenario run's outcome, from the held-out check beside it; a case without a record is
    /// listed as `no record`. The binary a case's runs were made with is
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
    /// shares, a row per step, the defects and what the sources did not record. With --cases,
    /// instead, the units accepted, tokens and interventions per repository and UTC day.
    #[command(after_help = REPORT_EXIT_CODES)]
    Report(ReportArgs),
    /// Write one offline HTML page of the runs under a directory: every `takt-trace/1` file
    /// under it is a run and a lane on one time axis, each operation a mark at its time naming
    /// its step, action and outcome; a click on a harness step's mark opens that step's
    /// transcript (`<repo-key>/<case>/run-<r>/step-<n>.jsonl` under the directory, as the
    /// case's step line or the layout names it; past 256 KiB, its head and tail). The page
    /// holds its stylesheet, script and data inline and fetches nothing; it is written `0600`,
    /// as a transcript carries what the model read.
    #[command(after_help = VIEW_EXIT_CODES)]
    View(ViewArgs),
    /// Promote a takt revision: compare its saved scorecard with the promoted baseline's under
    /// the scorecard's rule, and with the series start's on its scenarios, the latest
    /// --baseline-only record or --rebaseline under a newer metric definition, so steps each
    /// within the rule cannot add up past it (a --rebaseline that only adds a scenario keeps the
    /// series start, and the scenario it adds is judged against the --rebaseline that added it),
    /// and, when no scenario is worse than any of them, build a fresh clone of
    /// that revision confined through the Substrate adapter a run's `--confinement substrate`
    /// would choose (`systemd` around `linux`, `linux` or `macos`, by the facts it proves: the
    /// build directory the workspace, rustup's default toolchain as its settings file names it,
    /// never one on PATH, and the host's registry cache read-only, the fresh install root the
    /// one writable host root, the environment cleared, the network isolated, `cargo build
    /// --locked --offline`), install the binary with
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
    /// Replay one judgement step of a recorded case from the state it started from: the
    /// revision it started at, the case's evidence, decisions and record up to it, and the
    /// prompt it was given, rebuilt from the case's record and compared byte for byte with the
    /// first line of the step's recorded transcript before anything runs. Each replay runs the
    /// harness (`claude`) once in a scratch worktree of the repository at that revision,
    /// detached, removed afterwards, and is recorded outside the case and the repository's tree
    /// as `replay.json` (`takt-replay/1`) and `transcript.jsonl`. The case directory is only
    /// read.
    #[command(after_help = REPLAY_EXIT_CODES)]
    Replay(ReplayArgs),
}

#[derive(clap::Args)]
struct ReplayArgs {
    /// The case directory, `<repo>/.takt/cases/<id>`; only read.
    #[arg(long, value_name = "DIR")]
    case: PathBuf,
    /// The step to replay, as the case's record numbers it in its run: a judgement step, one
    /// whose line is the harness's.
    #[arg(long, value_name = "N")]
    step: u64,
    /// How many times the step is replayed, each in a scratch worktree of its own.
    #[arg(long, value_name = "K", default_value_t = 1,
          value_parser = clap::value_parser!(u32).range(1..))]
    times: u32,
    /// The run of the case the step is in, from 1 (the run that opened it); needed only when
    /// several runs record a step of that number.
    #[arg(long, value_name = "R")]
    run: Option<u64>,
    /// The repository that holds the case's commits; without it, the tree the case directory
    /// lies in.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,
    /// The transcripts directory the recorded step's transcript is read in; without it
    /// `$XDG_STATE_HOME/takt/transcripts`, where `takt run` keeps them.
    #[arg(long, value_name = "DIR")]
    transcripts: Option<PathBuf>,
    /// Where the replays are recorded, one directory `replay-<k>` each, none there yet;
    /// outside the case directory and the repository's tree. Without it
    /// `$XDG_STATE_HOME/takt/replays/<repo-key>/<case>/run-<r>/step-<n>/<instant>`.
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// The harness's model, when it takes one.
    #[arg(long)]
    model: Option<String>,
    /// The approval gate the recorded run was given (`takt run --gate`); without it, the
    /// case's protocol's default, as `takt run` takes it.
    #[arg(long)]
    gate: Option<String>,
}

/// Exit 3 of `takt replay`: the prompt rebuilt from the record is not the recorded one.
const EXIT_PROMPT_DIFFERS: u8 = 3;

/// What `takt replay --help` says about the exit codes.
const REPLAY_EXIT_CODES: &str = "Exit codes:
  0  replayed, each replay recorded; the prompt was the recorded one byte for byte, or no recorded transcript was found to compare with (said on stdout)
  1  refused, nothing run and nothing written: the case or its record does not read, the record holds no such step or holds it in several runs (give --run), the step is not a judgement step (a validator's, or one that recorded no harness step line), the record names no revision or decision for it, the replays directory is inside the case, the tree it lies in or the repository's tree, or a replay's directory is there already, a link stands on the way to or inside the rebuilt case, among its evidence or decisions, or at the case's intent.md or protocol.yaml (an empty replay directory may be left), or the scratch worktree cannot be made; the reason on stderr
  2  the arguments do not parse
  3  the prompt rebuilt from the record differs from the first line of the recorded step's transcript; nothing run
  5  replayed and recorded, but the harness failed in at least one replay; its error is in that replay's record";

/// `takt replay`: the step's starting state read from the case's record, the rebuilt prompt
/// compared with the recorded one, then each replay run and recorded
/// ([`takt::replay::replay_once`]).
fn replay(args: ReplayArgs, out: &mut dyn Write) -> Result<u8> {
    let case_dir = args
        .case
        .canonicalize()
        .with_context(|| format!("the case directory {}", args.case.display()))?;
    let repo = match &args.repo {
        Some(repo) => repo
            .canonicalize()
            .with_context(|| format!("the repository {}", repo.display()))?,
        None => takt::replay::tree_of(&case_dir)?,
    };
    let start = takt::replay::starting_state(&case_dir, args.step, args.run)?;
    let gate = args
        .gate
        .clone()
        .or_else(|| start.protocol.default_gate(&start.inputs));
    let claude = ClaudeCode {
        model: args.model.clone(),
        ..ClaudeCode::default()
    };
    let recorded = match args
        .transcripts
        .clone()
        .or_else(takt::run::default_transcripts)
    {
        Some(root) => takt::replay::recorded_prompt_line(&start, &root, &repo)?,
        None => None,
    };
    let rebuilt = takt::replay::rebuilt_prompt_line(&claude, &start, gate.as_deref(), &repo)?;
    writeln!(
        out,
        "step {} of run {} of case {} ({} recorded): revision {} from {}; decision {}",
        start.step,
        start.run,
        start.case,
        start.action,
        start.revision,
        start.revision_from,
        start.decision_file
    )?;
    match &recorded {
        Some((path, line)) if *line == rebuilt => {
            writeln!(out, "prompt: as recorded in {}", path.display())?;
        }
        Some((path, line)) => {
            let at = line
                .iter()
                .zip(&rebuilt)
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| line.len().min(rebuilt.len()));
            eprintln!(
                "takt: the prompt rebuilt for step {} differs from the recorded one in {} from byte {at} ({} bytes recorded, {} rebuilt); nothing is replayed",
                start.step,
                path.display(),
                line.len(),
                rebuilt.len()
            );
            return Ok(EXIT_PROMPT_DIFFERS);
        }
        None => writeln!(
            out,
            "prompt: no recorded transcript of the step was found; not compared"
        )?,
    }
    let base = match &args.out {
        // Absolute now: git resolves a relative path inside the repository, not where takt checked it.
        Some(dir) => std::path::absolute(dir)?,
        None => takt::run::default_replays()
            .ok_or_else(|| {
                anyhow!("no replays directory: neither XDG_STATE_HOME nor HOME is set; give --out")
            })?
            .join(takt::harness::repo_key(&repo)?)
            .join(&start.case)
            .join(format!("run-{}", start.run))
            .join(format!("step-{}", start.step))
            .join(takt::events::instant().replace(':', "")),
    };
    takt::replay::check_out_dir(&base, &case_dir, &repo)?;
    let recorded_line = recorded.as_ref().map(|(_, line)| line.as_slice());
    let mut failed = false;
    for k in 1..=args.times {
        let replayed = takt::replay::replay_once(
            &start,
            &repo,
            &claude,
            gate.as_deref(),
            &base.join(format!("replay-{k}")),
            recorded_line,
        )?;
        let changed = if replayed.changed.is_empty() {
            "nothing".to_owned()
        } else {
            replayed.changed.join(", ")
        };
        match &replayed.result {
            Ok(action) => writeln!(
                out,
                "replay {k}: {action}; tokens {}; changed {changed}; prompt {}; recorded in {}",
                replayed
                    .tokens
                    .map_or("not reported".to_owned(), |tokens| tokens.to_string()),
                replayed.prompt,
                replayed.dir.display()
            )?,
            Err(why) => {
                failed = true;
                writeln!(
                    out,
                    "replay {k}: error: {why}; prompt {}; recorded in {}",
                    replayed.prompt,
                    replayed.dir.display()
                )?;
            }
        }
    }
    Ok(if failed { EXIT_ERROR } else { EXIT_DONE })
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
    /// when its revision is the baseline's, its case ids are the stored scorecard's exactly but
    /// for the cases of whole scenarios it adds, and every metric definition is at or above the
    /// stored version with one of them higher (a baseline saved before a definition changed,
    /// re-scored under the new one) or a scenario added (the one way a scenario joins the
    /// corpus a promotion compares). The record
    /// appended to the chain names the scorecard it replaces by its sha256. With --dry-run and
    /// --install-root only.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = ["baseline_only", "revision", "binary_sha256", "rejudge"]
    )]
    rebaseline: Option<PathBuf>,
    /// Record the baseline only: the scorecard of the revision installed today, compared with
    /// nothing; nothing is built or installed. Refused when the store holds a baseline already,
    /// and for a scorecard holding no scenario case, which would judge nothing.
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
    /// (`.takt/cases/<case>/binary.txt`, first line `<path>\t<sha256>`, every further
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
  1  refused, nothing written or installed but the head beside the store where none was found (written from the chain as it stands, said on stderr; not on --dry-run): the scorecard, the store, the install root or the build directory is inside the repository; the scorecard has more than one link, names no full revision, is not named by it, differs from HEAD, is no ancestor of the checkout's origin/main as last fetched (or there is none), a local main moved by hand counting for nothing, was saved under another rule or metric definition, holds a run made with another binary than the revision's (the run named; its `opened` line names another revision, the revision -dirty, or none; with --binary-sha256 the binary.txt statements are checked instead) or a scenario run without its held-out result, or a scenario case whose run tokens are unknown, a harness step of it having recorded no tokens or 0 (the case and the step named; with --baseline-only and --rebaseline too, so a re-read under tokens definition 2 that finds a run the stored scorecard summed as 0 is no rebaseline); the baseline's scorecard, or one the series compares the candidate with, holds such a case (the record, the case and the step named), nothing judged; the checkout has uncommitted or untracked files or index entries marked assume-unchanged or skip-worktree, or commits a rust-toolchain.toml with a `path`; the toolchain is not found; another promotion holds the store's lock; no baseline is recorded; the installed binary does not name the latest record's revision, or names none and hashes to another digest than the record's binary_sha256, or to none it records (with --baseline-only, the revision recorded); --revision or --binary-sha256 without --baseline-only, --binary-sha256 without --revision, a --revision that is no full commit id or not the scorecard's, a --binary-sha256 that is no sha256; with --binary-sha256, an installed binary whose --version names a revision or that hashes to another digest (both named), a run of the scorecard that names no revision beside runs that name one, or a run that states no binary or another (the run named); the revision or its tree was refused (without --rejudge) or recorded already; --rejudge without an earlier refusal of the revision or its tree, with --baseline-only, or on a scorecard sharing a case with one it was refused on (the case named); --rebaseline with a scorecard of another revision than the baseline's, missing a case of the stored scorecard's or holding one more that is not of a scenario it adds whole, one the stored scorecard lacks (the first named; a new scenario joins the corpus only this way), under a metric definition below the stored one (named), with none newer when it adds no scenario, or with no baseline; a scenario the baseline has has fewer cases than the rule's trials on either side, or a scenario only the candidate has fewer on its side; the scorecard and the baseline's hold no scenario case between them (`no scenario case`), or a scenario only the candidate has (`not in the saved scorecard`), however many its cases: the comparison covers every scenario either side has; --baseline-only with a scorecard holding no scenario case, a baseline that would judge nothing; the store holds a file takt does not read, a record that disagrees with its name, instant or scorecard copy, a chain of records with a gap, or one that ends before the head beside the store (promotions.head) or runs past it; the record could not be written; the reason on stderr
  2  the arguments do not parse
  3  refused on the scorecard: a scenario is worse than the baseline's, or than the series start's, the latest --baseline-only record or --rebaseline under a newer metric definition, or, for a scenario a later --rebaseline added, than that --rebaseline's (the record and the scenario named); the refusal is recorded (not with --dry-run), nothing installed
  5  the candidate held but its clone, its confined build or its install failed (no Substrate adapter on the host serving the build among them, each tried named with what it lacked), the built binary names another revision, or the binary names another revision or hashes differently once in place (then the binary before it is back in place and the record removed); nothing recorded, the installed binary unchanged";

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
    /// cases than the rule's trials, none included, gets no verdict. Every scenario either side
    /// has is compared: one the saved scorecard lacks gets no verdict either (`not in the saved
    /// scorecard`), and no scenario case on either side is no comparison (`no scenario case`).
    /// Tokens are printed as n, min, median and max per scenario and side, with the difference
    /// of the medians.
    #[arg(long, value_name = "FILE")]
    against: Option<PathBuf>,
    /// With --against, judge the candidate on the steps it targets, by action (the actions its
    /// finding names), comma-separated: per scenario and action, each case's tokens at the step
    /// on each side and the saved side's cheapest; a step is better when every candidate case is
    /// below the saved cheapest, or a one-sided exact Mann-Whitney U test (a tie counts against
    /// the candidate) rejects equality at p <= 1/20 with 5 cases a side or more. A case of
    /// either side that took no step of the action is named and the step is not better. The
    /// whole run is still judged as --against judges it: better on its steps but worse on the
    /// run is refused, naming both. Both scorecards must be under the step_tokens definition. A
    /// targeted step whose tokens a harness step did not record is unknown, never 0: it refuses
    /// the judgement, naming the case and the step.
    #[arg(
        long,
        value_name = "ACTION",
        value_delimiter = ',',
        num_args = 1..,
        requires = "against"
    )]
    steps: Vec<String>,
    /// Score runs whose layers differ (ADR 0101). Each run's `opened` line records the layers
    /// it ran on, name to revision, each read from what ran (takt's build, the harness
    /// program's version, the model the harness's stream reported); the scorecard prints each
    /// distinct set of layers with its runs' count and, without this flag, refuses to score
    /// runs whose layers differ, naming the layer and the runs. A run that recorded no layers
    /// (a record from before the field) is "not recorded", never a revision, and differs from
    /// one that did.
    #[arg(long)]
    across_layers: bool,
}

/// What `takt scorecard --help` says about the exit codes.
const SCORECARD_EXIT_CODES: &str = "Exit codes:
  0  printed (and saved); with --against, no scenario is worse than the saved one; with --steps too, better on every targeted step as well
  1  refused: the repository, a case's events.jsonl, or the saved scorecard does not read (a binary.txt that does not read leaves its case's runs stating no binary, the case listed with the reason); a scenario case has no held-out.txt beside its record, or one that does not read or states no result (the case named); without --across-layers, runs whose layers differ (the layer and both runs named: a revision each, or that a run recorded none), nothing printed or saved; --save without --revision, or under a --revision that is not the full commit id (40 lower-case hex digits), onto a file that exists, or that cannot be written; --save with a run made with another binary than the --revision's (the run named; its `opened` line names another revision or the revision -dirty, or names none, no `binary` or `unknown revision`, beside runs that name one), nothing written; with --against, a scenario the saved scorecard has with fewer cases than the rule's trials (`too few cases on the saved side (<n> of <trials>)`), or any scenario with fewer on this side, none included (`too few cases (<n> of <trials>)`), or a scenario the saved scorecard lacks, however many its cases here (`not in the saved scorecard`): no verdict given, as a promotion refuses; with --against, no scenario case on either side (`no scenario case`), nothing compared; a saved scorecard with a run whose outcome disagrees with its held-out result; with --steps, a saved scorecard with no step_tokens definition (saved before it) or under another version, or an empty action; with --steps, a scenario case of either side whose tokens at a targeted step are unknown, a harness step of it having recorded no tokens or 0 (the case and the step named), nothing judged; with --against, with or without --steps, a scenario case of either side whose run tokens are unknown, a harness step of it having recorded no tokens or 0, which is never read as 0 (the case and the step named), nothing judged; the reason on stderr
  2  the arguments do not parse
  3  with --against: a scenario is worse: at some outcome rank its cases at that rank or better fall short of the saved share by more than the rule's margin, or its median of tokens exceeds the saved maximum (a scenario with no run on this side is exit 1, too few cases); with --steps, whatever the steps: refused, naming the steps' result and the scenarios worse on the run
  4  with --steps: no scenario is worse on the run, but a targeted step is not better (or no case on either side took it)";

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
#[command(group(clap::ArgGroup::new("input").required(true).multiple(true).args(["case", "trace", "cases"])))]
struct ReportArgs {
    /// A cases directory, `<repo>/.takt/cases` or `<repo>/.engineering/cases`; repeatable.
    /// Prints, instead of a run report, one row per repository and UTC day with the units
    /// accepted (cases whose last stop is `Completed (accepted)`, on that run's day), the
    /// tokens the harness reported on the runs opened that day (`unknown` when a harness step
    /// recorded none, or 0) and the interventions (decisions in `authority.yaml` by a
    /// `human:` approver, granted or denied, on the day given), then a total row. Text only;
    /// not beside --case, --steps or --trace.
    #[arg(long, value_name = "DIR", num_args = 1.., conflicts_with_all = ["case", "steps", "trace"])]
    cases: Vec<PathBuf>,
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
    /// How to print: `text` (markdown), or `json`, one document with each run's outcome, stop
    /// and repository obligations; `json` is refused beside --cases.
    #[arg(long, value_enum, default_value = "text")]
    format: ShowFormat,
}

#[derive(clap::Args)]
struct ViewArgs {
    /// The directory to read: every `takt-trace/1` file and step transcript under it, never
    /// through a link, `.git` and `target` skipped.
    dir: PathBuf,
    /// Write the page there, replacing a file there; `takt-view.html` in the current
    /// directory without it.
    #[arg(long, value_name = "FILE", default_value = "takt-view.html")]
    output: PathBuf,
}

/// What `takt view --help` says about the exit codes.
const VIEW_EXIT_CODES: &str = "Exit codes:
  0  written
  1  refused: the directory is not one or holds no takt-trace/1 trace (the directory named), a file that begins as a trace does not read as one (the file named), or the page cannot be written; nothing written, the reason on stderr
  2  the arguments do not parse";

/// What `takt trace --help` says about the exit codes.
const TRACE_EXIT_CODES: &str = "Exit codes:
  0  written
  1  refused: the case has no events.jsonl, the record or the steps file does not read, or the output cannot be written; the reason on stderr
  2  the arguments do not parse";

/// What `takt report --help` says about the exit codes.
const REPORT_EXIT_CODES: &str = "Exit codes:
  0  printed
  1  refused: a case has no events.jsonl, a record, steps file or trace does not read, or --steps is given more often than --case; with --cases, a cases directory, an events.jsonl or an authority.yaml that does not read (named); the reason on stderr
  2  the arguments do not parse, neither --case, --trace nor --cases is given, --cases is given beside --case, --steps or --trace, or --format json beside --cases";

#[derive(Subcommand)]
enum ProtocolCommands {
    /// Print each case input a built-in protocol declares: its values and description, takt's
    /// built-in default, and the value the run file `--config` names sets for it, with that
    /// file's path and line (`unset` when it sets none). `--format json` prints the same as
    /// data. A protocol takt does not know is refused, naming the ones it knows.
    #[command(after_help = PROTOCOL_SHOW_EXIT_CODES)]
    Show(ProtocolShowArgs),
}

/// What `takt protocol show --help` says about the exit codes.
const PROTOCOL_SHOW_EXIT_CODES: &str = "Exit codes:
  0  printed
  1  refused: the protocol is not one takt knows (the reason names the ones it does), or the run file does not read; the reason on stderr
  2  the arguments do not parse";

/// When the console is in colour (`--color`).
#[derive(Clone, Copy, clap::ValueEnum)]
enum ColorArg {
    /// When standard output is a terminal and `NO_COLOR` is unset or empty.
    Auto,
    /// Always.
    Always,
    /// Never.
    Never,
}

impl From<ColorArg> for takt::console::ColorMode {
    fn from(color: ColorArg) -> Self {
        match color {
            ColorArg::Auto => Self::Auto,
            ColorArg::Always => Self::Always,
            ColorArg::Never => Self::Never,
        }
    }
}

/// The console a run's `looping` flags ask for, its standard output a terminal or not
/// (`terminal`) and `NO_COLOR` holding `no_color` ([`takt::console::color_on`]).
fn console_of(
    looping: &LoopArgs,
    terminal: bool,
    no_color: Option<&std::ffi::OsStr>,
) -> takt::console::Console {
    takt::console::Console {
        verbose: looping.verbose,
        reads: !looping.quiet_steps,
        diff: !looping.quiet_steps,
        color: takt::console::color_on(looping.color.into(), terminal, no_color),
    }
}

/// How `takt protocol show` prints.
#[derive(Clone, Copy, clap::ValueEnum)]
enum ShowFormat {
    /// Lines, one block per input.
    Text,
    /// One JSON document.
    Json,
}

#[derive(clap::Args)]
struct ProtocolShowArgs {
    /// The built-in protocol, `<name>/<major>` (`software.change/1`), as `takt protocols` lists it.
    protocol: String,
    /// A run file (.toml, .yaml or .yml), the one `takt run --config` reads: the values its
    /// `protocols` section sets are listed with the file's path and line.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// How to print.
    #[arg(long, value_enum, default_value = "text")]
    format: ShowFormat,
}

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
    /// nothing of an earlier run is resumed. Without the bot route nothing is fetched or
    /// pushed, and `<n>` is 22 digits: the 14-digit instant to the second (UTC,
    /// `YYYYMMDDHHMMSS`) and a random 8-digit nonce, so runs from different checkouts very
    /// likely take different ones (two runs in one second share one with a chance of 1 in
    /// 10^8), not consecutive ones. The
    /// hard guard is the checkout's own: a number it knows is passed over, and `git checkout -b`
    /// refuses a branch that exists. A refusal before the push removes the branch and
    /// puts the tree back. A scenario whose section gives `red-base = expected` (its seed's
    /// tests fail on purpose) goes on past a red base as `--continue-past-red-base` does,
    /// without the flag, and says so in a line `red base: expected by scenario <id> in
    /// .engineering/scenarios.conf at <revision>: tests failing at the base do not stop the
    /// run`; the case records the declaration, so a resume of it with `takt run --case` goes on
    /// past its red base too. The declaration only turns that on, and
    /// `continue_past_red_base = false` in `--config` does not turn it off. Any other value of
    /// `red-base` is refused before anything is cut.
    #[command(after_help = SCENARIO_EXIT_CODES)]
    Run(Box<ScenarioRunArgs>),
}

/// Where a repository declares its scenarios, in git's configuration syntax, one section a
/// scenario: `[scenario "<id>"]` with `story = <story id>` and `seed = <commit>`, and
/// `red-base = expected` for a seed whose tests fail on purpose (git's syntax admits no `_` in
/// a key).
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
    /// A file of the run's configuration, as `takt run --config` reads it: the same TOML or
    /// YAML file, a `loom.agent/1` document's `takt` member included, the same keys, a flag
    /// given on the command line winning over the file and the file over the flag's default.
    /// Its `story` is refused, since the scenario names the story, and any key `takt run
    /// --config` refuses is refused too, before anything is fetched, cut or opened, naming the
    /// file.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
    #[command(flatten)]
    looping: LoopArgs,
}

impl ScenarioRunArgs {
    /// Fills each loop flag `matches` did not take from the command line with `--config`'s
    /// value ([`LoopArgs::configured`]); a file that names a `story`, or declares `scan: none`,
    /// is refused.
    fn configured(&mut self, matches: &clap::ArgMatches) -> Result<()> {
        self.looping.renamed(matches);
        let Some(path) = &self.config else {
            return self
                .looping
                .configured(takt::run_config::RunConfig::default(), matches);
        };
        let file = takt::run_config::RunConfig::read(path)?;
        if file.story.is_some() {
            return Err(anyhow!(
                "the run configuration file {} is refused for a scenario run: its `story` is the scenario's to name (`story` in its section of {SCENARIOS}); nothing was changed",
                path.display()
            ));
        }
        if file.scan == Some(takt::scan::ScanName::None) {
            return Err(anyhow!(
                "the run configuration file {} is refused for a scenario run: it declares `scan: none`, and a scenario run scans what a session proposes with the scan adapter `gates` only; leave `scan: none` out and set {}; nothing was changed",
                path.display(),
                takt::scan::gates::POLICY_VAR
            ));
        }
        self.looping.configured(file, matches)?;
        Ok(())
    }
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
    /// The story to deliver, by id, instead of the frontier's next ready one: its open case is
    /// resumed, if it has one; else it is taken as the frontier takes a story, which it must be
    /// ready for (`proposed`, nothing blocking it, every dependency done). A story the store
    /// does not list, or one not ready, is refused before any case opens, naming why. Not with
    /// --intent or --case.
    #[arg(long, value_name = "ID", conflicts_with_all = ["intent", "case"])]
    story: Option<String>,
    /// The run configuration file `story` came from, when --story did not name it.
    #[arg(skip)]
    story_from: Option<PathBuf>,
    /// The protocol: a built-in id (`takt protocols`) or a `protocol/1` file. Not given, a story
    /// the planning store gives opens on its tag `protocol:<name>/<major>`, else the run file's
    /// `protocol`, else `story.delivery/3`, an --intent on `software.change/1` (takt's defaults document),
    /// and a resumed case runs on its own. Given for a resumed case, it must be the case's own:
    /// another is refused before the run, nothing changed.
    #[arg(long)]
    protocol: Option<String>,
    /// A case input, `name=value`; repeatable. It wins over a story tag `input:<name>=<value>`,
    /// which wins over the run file's `protocols` section, which wins over takt's built-in
    /// default; each value is recorded with its source in `case.yaml`. One the protocol a case
    /// opens on does not declare is refused before the case is opened. Given for a resumed case, any input is refused
    /// before the case is resumed: the case keeps the inputs it was opened with. A
    /// `loop.improvement/1` case requires `loop_baseline=<full commit id>`, the promoted revision
    /// its baseline is, a commit of the repository: it opens the case's baseline and is no case
    /// input. A `self.improvement/1` turn requires `finding=<story id>`, a finding the planning
    /// store holds, and `candidate=<full commit id>`, the candidate delivered outside the case;
    /// its baseline is the promotion store's latest promoted revision. Neither is a case input.
    /// Its case input `surface` is computed from the candidate's change against the baseline's
    /// `.engineering/judge-allow.yaml`; given, it must be the computed one. Its finding is read on
    /// the mainline, `main` as the forge reports it, or the local `main` under `--forge none`,
    /// which the operator is trusted with. It opens only once that tip is read and found not to
    /// hold the candidate: a candidate it holds was merged before a turn could grant it (open
    /// the turn on a fresh candidate), and a tip that cannot be read (the forge's route or the
    /// network down, no `main`) is refused too, naming the forge and why, before anything is
    /// written (open the turn again once the tip can be read). The turn records the sha256 of the binary that opens
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
    /// root that does not exist opens no turn. A resumed turn keeps the one it was opened with:
    /// given again, by any path whose canonical form is that root, it is accepted; another root,
    /// or any for a turn that recorded none or for a case that is no turn, is refused naming
    /// what the case keeps. Not read by any other case.
    #[arg(long)]
    install_root: Option<PathBuf>,
    /// A file of the run's configuration, TOML (`.toml`) or YAML (`.yaml`, `.yml`):
    /// `story`, `iterations`, `hours`, `tokens` (a whole number above zero; another value exits
    /// 2), `harness`, `model`, `endpoint_file` (a relative path
    /// read beside the file), `endpoint_token_env`, `forge`, `test_cmd`, `state_dir` (a
    /// relative path read beside the file), `continue_past_red_base` (a boolean) and `records`
    /// (`default` or `trial`), each the flag's of that name, and `scan`, which no flag sets: the
    /// scan adapter what a session
    /// proposes passes, `gates` (the default: `b10x-gates scan-text` under the policy
    /// `B10X_GATES_POLICY` names) or `none` (nothing is scanned, each proposal checked only for
    /// the shape of a credential, said in one line before the first step; refused while
    /// `B10X_GATES_POLICY` is set). Under `gates` with no policy, `--harness codex` and
    /// `--harness harness`, whose sessions may read outside their copy, are refused before the
    /// case opens. A YAML `loom.agent/1` document, as Loom mounts it at `/config/agent.yaml`,
    /// gives them in its `takt` member, and nothing without one; its `runtime.state_dir` is the
    /// `state_dir` when `takt` names none. A flag given on the command
    /// line wins over the file, the file over the flag's default: the file's `story` is taken
    /// only when neither --story, --intent nor --case is given, and its `endpoint_token_env`
    /// only when no credential flag is. The file's `endpoint_file` and `endpoint_token_env` are
    /// read only when the harness is `harness`. Any other key, or a `scan` that is neither
    /// `gates` nor `none`, is refused before the case opens, naming the file and the line, never
    /// a value written there. A container takes its run from a mounted one. `takt scenario run
    /// --config` reads the same file, and refuses one that declares `scan: none`. `llm`, the
    /// model routes, an `llm.routes/1` document or `{path: <file>}` naming one beside the file,
    /// which no flag gives; llm validates it, and a refused section opens no case. With `--harness
    /// harness`, `model` names a route or alias of the section, and that route is the endpoint:
    /// no `--endpoint-*` flag, `endpoint_file` or `endpoint_token_env` may sit beside it.
    /// `protocol`, the protocol a story from the planning store opens on, and `protocols`, a mapping
    /// of a protocol to the values of its declared case inputs; an unknown protocol, input or
    /// value is refused naming the file and the line.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// The scan adapter `--config`'s file declares, read by [`RunArgs::configured`]; no flag.
    #[arg(skip)]
    declared_scan: Option<takt::scan::ScanName>,
    /// The protocol and the inputs `--config`'s file sets, read by [`RunArgs::configured`]; no flag.
    #[arg(skip)]
    protocol_settings: takt::run_config::ProtocolSettings,
    /// The forge settings `--config`'s file sets, read by [`RunArgs::configured`]; no flag.
    #[arg(skip)]
    forge_settings: takt::run_config::ForgeSettings,
    #[command(flatten)]
    looping: LoopArgs,
}

/// Whether the flag `id` was not given on the command line `matches` parsed.
fn unset(matches: &clap::ArgMatches, id: &str) -> bool {
    !matches!(
        matches.value_source(id),
        Some(clap::parser::ValueSource::CommandLine)
    )
}

impl RunArgs {
    /// Fills each flag `matches` did not take from the command line with `--config`'s value,
    /// when the file sets it; without `--config`, nothing changes. The file's `story` names
    /// the work only when no flag does (--story, --intent, --case), and is kept with the file
    /// it came from, so a refusal names that file; a run that opens no story (a
    /// `self.improvement/1` turn) leaves it out.
    fn configured(&mut self, matches: &clap::ArgMatches) -> Result<()> {
        self.looping.renamed(matches);
        let Some(path) = &self.config else {
            return self
                .looping
                .configured(takt::run_config::RunConfig::default(), matches);
        };
        let mut file = takt::run_config::RunConfig::read(path)?;
        if let Some(story) = file.story.take()
            && ["story", "intent", "case"]
                .into_iter()
                .all(|id| unset(matches, id))
        {
            self.story = Some(story);
            self.story_from = Some(path.clone());
        }
        self.declared_scan = file.scan;
        self.protocol_settings = file.protocol_settings.clone();
        self.forge_settings = file.forge_settings();
        self.looping.configured(file, matches)?;
        Ok(())
    }
}

impl LoopArgs {
    /// Fills each loop flag `matches` did not take from the command line with `file`'s value,
    /// when it sets it, for `takt run` and `takt scenario run` alike.
    fn configured(
        &mut self,
        mut file: takt::run_config::RunConfig,
        matches: &clap::ArgMatches,
    ) -> Result<()> {
        if let Some(path) = &self.llm_config {
            file.set_llm_file(path)?;
        }
        let unset = |id: &str| unset(matches, id);
        let llm = self.llm_section(&file, &unset)?;
        let looping = self;
        looping.confinement_given =
            matches.value_source("confinement") != Some(clap::parser::ValueSource::DefaultValue);
        looping.llm = llm;
        looping.llm_digest = file.llm_digest;
        if unset("iterations") && file.iterations.is_some() {
            looping.iterations = file.iterations;
        }
        if unset("hours") && file.hours.is_some() {
            looping.hours = file.hours;
        }
        if let Some(tokens) = file.tokens.filter(|_| unset("tokens")) {
            looping.tokens = tokens;
        }
        if unset("model") && unset("endpoint_model") && file.model.is_some() {
            looping.model = file.model;
        }
        if unset("forge") && file.forge.is_some() {
            looping.forge = file.forge;
        }
        if let Some(setting) = &file.harness
            && unset("harness")
        {
            looping.harness = setting.harness().to_owned();
        }
        let base = match &file.harness {
            Some(takt::run_config::HarnessSetting::Session(request)) => Some(request),
            _ => None,
        };
        looping.session = takt::run_config::SessionRequest::with_flags(
            base,
            &looping.harness,
            looping.session_flags(),
        )?;
        // The file's endpoint is b10x-harness's: under another harness, which a flag may have
        // chosen over the file's, it is left out rather than refused as a flag nobody gave.
        if unset("endpoint_file") && file.endpoint_file.is_some() && looping.harness == "harness" {
            looping.endpoint.endpoint_file = file.endpoint_file;
        }
        // The file's variable is a credential as --endpoint-token-env is: a credential the
        // command line names wins over it, by a flag (the variable's, a token file's, a
        // secret's) or by the `[credential]` of an --endpoint-file given there (one that does
        // not read counts as naming one: its own refusal follows); under another harness it is
        // left out as the endpoint file is. It is staged as the flag's is
        // ([`EndpointArgs::take_variable`]), after this.
        let line_credential = !unset("endpoint_file")
            && looping
                .endpoint
                .endpoint_file
                .as_deref()
                .is_some_and(|path| {
                    takt::harness::EndpointFields::read(path).map_or(true, |fields| {
                        fields.token_file.is_some()
                            || fields.secret.is_some()
                            || fields.env.is_some()
                    })
                });
        if file.endpoint_token_env.is_some()
            && looping.harness == "harness"
            && !line_credential
            && [
                "endpoint_token_env",
                "endpoint_token_file",
                "endpoint_secret",
            ]
            .into_iter()
            .all(unset)
        {
            looping.endpoint.endpoint_token_env = file.endpoint_token_env;
        }
        if let Some(test_cmd) = file.test_cmd.filter(|_| unset("test_cmd")) {
            looping.test_cmd = test_cmd;
        }
        if unset("state_dir") && file.state_dir.is_some() {
            looping.state_dir = file.state_dir;
        }
        if let Some(go_on) = file
            .continue_past_red_base
            .filter(|_| unset("continue_past_red_base"))
        {
            looping.continue_past_red_base = go_on;
        }
        looping.setup = file.setup.unwrap_or_default();
        looping.records_of_file = file.records.filter(|_| unset("records"));
        looping.commit = looping.commit_settings(file.commit.as_ref())?;
        Ok(())
    }

    /// The commit settings of the run: the defaults, the configuration's `commit` section over
    /// them and the flags over that, refused naming the member when one cannot be used. None
    /// when neither gives one, so the library's default stands.
    fn commit_settings(
        &self,
        file: Option<&takt::commit_style::Section>,
    ) -> Result<Option<takt::commit_style::Settings>> {
        let mut flags = takt::commit_style::Section::default();
        if let Some(style) = &self.commit_style {
            flags.style = takt::commit_style::Style::named(style);
        }
        flags.trailer.clone_from(&self.commit_trailer);
        if !self.commit_types.is_empty() {
            let mut types = std::collections::BTreeMap::new();
            for pair in &self.commit_types {
                let (action, kind) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--commit-types takes ACTION=TYPE pairs"))?;
                types.insert(action.to_owned(), kind.to_owned());
            }
            flags.types = Some(types);
        }
        if file.is_none() && flags == takt::commit_style::Section::default() {
            return Ok(None);
        }
        let mut settings = takt::commit_style::Settings::default();
        if let Some(section) = file {
            settings.apply(section);
        }
        settings.apply(&flags);
        settings
            .check()
            .map_err(|why| anyhow!("{why}; nothing was started"))?;
        Ok(Some(settings))
    }

    /// The session settings the command line's flags give, under their metaharness names.
    fn session_flags(&self) -> takt::run_config::SessionFlags<'_> {
        takt::run_config::SessionFlags {
            effort: self.effort.as_deref(),
            max_turns: self.max_turns,
            max_budget_usd: self.max_budget_usd.as_deref(),
            strict_version: self.strict_version,
            context: &self.context,
        }
    }

    /// The `llm` section `file` holds, when the run may use it, refused before any case opens
    /// otherwise: under `--harness claude` or `codex`, which takt points at no endpoint (a `none`
    /// run takes no endpoint, and the section is left unused); beside another endpoint source, a
    /// flag, `--endpoint-file` or `--endpoint-token-env` alike, or the file's own `endpoint_file`
    /// or `endpoint_token_env`, naming both; and under `--harness harness` with no `model` to name
    /// its route. Its route is resolved later, by [`LoopArgs::take_variable`].
    fn llm_section(
        &self,
        file: &takt::run_config::RunConfig,
        unset: &dyn Fn(&str) -> bool,
    ) -> Result<Option<takt::run_config::LlmRoutes>> {
        let Some(llm) = &file.llm_routes else {
            return Ok(None);
        };
        let harness = if unset("harness") {
            file.harness.as_ref().map_or(
                self.harness.as_str(),
                takt::run_config::HarnessSetting::harness,
            )
        } else {
            self.harness.as_str()
        };
        if harness == "claude" || harness == "codex" {
            return Err(anyhow!(
                "the run configuration file's `llm` section is refused with --harness {harness}: takt points an endpoint at b10x-harness only, so give --harness harness, or leave the `llm` section out; nothing was started"
            ));
        }
        let mut sources: Vec<&str> = self.endpoint.all_given();
        if file.endpoint_file.is_some() {
            sources.push("the run file's `endpoint_file`");
        }
        if file.endpoint_token_env.is_some() {
            sources.push("the run file's `endpoint_token_env`");
        }
        if !sources.is_empty() {
            return Err(anyhow!(
                "the run configuration file's `llm` section and {} both give the run's endpoint: an `llm` section routes `--model` through llm, and an endpoint source gives the endpoint directly; give one of them, not both; nothing was started",
                sources.join(" and ")
            ));
        }
        let model_given = self.model.is_some() || file.model.is_some();
        if harness == "harness" && !model_given {
            return Err(anyhow!(
                "the run configuration file's `llm` section names its route by `model`, and neither --model nor the file's `model` gives one; nothing was started"
            ));
        }
        Ok(Some(llm.clone()))
    }

    /// The records profile given: `--records`, else, for a new case only, the configuration
    /// file's `records`; `None` when neither gives one. A resumed case keeps the profile it was
    /// opened under (story:a-run-keeps-every-decision-only-on-a-trial). The value was checked by
    /// the flag's parser or when the file was read.
    fn records_profile(&self, resumed: bool) -> Option<takt::case::Records> {
        let named = self.records.as_deref().or(if resumed {
            None
        } else {
            self.records_of_file.as_deref()
        });
        named.and_then(takt::case::Records::named)
    }
}

/// The command line parsed, a `takt run --config` or `takt scenario run --config` file applied
/// under it.
fn parsed() -> Result<Cli> {
    use clap::{CommandFactory as _, FromArgMatches as _};
    let matches = Cli::command().get_matches();
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    match &mut cli.command {
        Commands::Run(args) => {
            if let Some(run) = matches.subcommand_matches("run") {
                args.configured(run)?;
            }
        }
        Commands::Scenario(ScenarioCommands::Run(args)) => {
            if let Some(run) = matches
                .subcommand_matches("scenario")
                .and_then(|scenario| scenario.subcommand_matches("run"))
            {
                args.configured(run)?;
            }
        }
        Commands::Report(args)
            if !args.cases.is_empty() && matches!(args.format, ShowFormat::Json) =>
        {
            Cli::command()
                .error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "--format json does not apply to --cases, which prints text only",
                )
                .exit();
        }
        _ => {}
    }
    Ok(cli)
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
    /// Stop once the harness reported this many tokens, counted over this invocation and read
    /// before every step, so a step under way runs to its end; the run stops `Tokens (<N>)`,
    /// exit 3. Not given, the run file's `tokens`, else 3000000: no run goes unbounded.
    #[arg(long, value_name = "N", default_value_t = DEFAULT_TOKENS)]
    tokens: u64,
    /// Stop once the run's active time, its wall time less the time it waits on the forge
    /// between two reads, reaches this many seconds; read before every step, so a step under way
    /// runs to its end. The run stops `ActiveTime` naming the budget, the active time and the
    /// time waited, exit 3. Waits on the forge spend none of it, and it bounds none: a run with
    /// no --iterations or --hours still has nothing to wait in.
    #[arg(long, value_name = "SECONDS")]
    active_seconds: Option<u64>,
    /// The test command, split at white space, run without a shell.
    #[arg(long, default_value = "cargo test --quiet")]
    test_cmd: String,
    /// The harness, each through metaharness: `claude` (Claude Code), `codex` (Codex),
    /// `harness` (b10x-harness), or `none` (only validators run).
    #[arg(long, default_value = "claude")]
    harness: String,
    /// The harness program the run's sessions start, by absolute path, in place of the one found
    /// on `PATH`; the run records its path, sha256 and `--version` on `opened`, the one found on
    /// `PATH` too when this is not given. A flag only. A relative path, one that names no
    /// executable file, or one given with `--harness none` is refused before the case opens.
    #[arg(long, value_name = "PATH")]
    harness_program: Option<PathBuf>,
    /// The harness's model, when it takes one. Under `--harness harness` it names a route of the
    /// `llm` section, when the run has one.
    #[arg(long, conflicts_with = "endpoint_model")]
    model: Option<String>,
    /// The old name of `--model`, kept for one minor release: the same flag, with one line on
    /// standard error naming `--model`.
    #[arg(long, value_name = "MODEL", conflicts_with = "model")]
    endpoint_model: Option<String>,
    /// The `llm.routes/1` document, a `.toml`, `.yaml` or `.yml` file, as llm's `--config` names
    /// it. It replaces the run file's `llm` section, with its routes, its digest and its refusals;
    /// `--model` names one of its routes under `--harness harness`, and the `opened` line records
    /// its digest as `llm_digest`. Given with or without `--config`.
    #[arg(long, value_name = "FILE")]
    llm_config: Option<PathBuf>,
    /// The reasoning effort the session asks of the model, in the vendor's own vocabulary, as
    /// `metaharness run --effort` takes it. Refused until takt's launch carries it.
    #[arg(long, value_name = "LEVEL")]
    effort: Option<String>,
    /// A ceiling on the session's turns, as `metaharness run --max-turns` takes it. Refused under
    /// `--harness codex`, whose launch takes no turn ceiling.
    #[arg(long, value_name = "N")]
    max_turns: Option<u32>,
    /// A ceiling in US dollars on what the session may spend, as `metaharness run
    /// --max-budget-usd` takes it. Refused until takt's launch carries it.
    #[arg(long, value_name = "USD")]
    max_budget_usd: Option<String>,
    /// Refuse before the run when the installed vendor version is outside metaharness's pin, as
    /// `metaharness run --strict-version` takes it. Refused until takt's launch carries it.
    #[arg(long)]
    strict_version: bool,
    /// A file whose text the prompt carries, as `metaharness run --context` takes it, repeatable.
    /// Refused until takt's launch carries it.
    #[arg(long, value_name = "FILE")]
    context: Vec<PathBuf>,
    #[command(flatten)]
    endpoint: EndpointArgs,
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
    /// opens its pull request, reads its state and checks and merges it; refused before any
    /// case opens unless `b10x-gates` is on `PATH` and `B10X_GATES_POLICY` names a file),
    /// `gitlab` (a repository on a GitLab host: git pushes the case's branch over HTTPS under
    /// `glab`'s credential helper, `glab api` opens the merge request, reads its state and head
    /// pipeline and merges it; what it publishes passes the run's scan adapter first, under
    /// `gates` `b10x-gates scan-text`, under a declared `scan: none` no scan; refused before
    /// any case opens unless `glab auth status --hostname <host>` exits 0; the run file's `transport: api` with a `credential` calls the GitLab API instead, `glab` never asked), `command` (the exact
    /// argument lists the run file's `forge_commands` section names for `open`, `read`, `checks` and
    /// `merge`, run without a shell; the push stays takt's scanned push; never a default, and
    /// refused for a scenario run), `connectors` (`connectors operations invoke` on the instance the
    /// run file's `forge_instance` names, its preflight listing change request create, read and
    /// merge and pipeline read before any case opens; the push stays takt's scanned push; never a
    /// default, and refused for a scenario run), `local` (the bare repository on this machine the
    /// tree's `origin` names by its path, outside the tree, the tree cloned with `--no-local`: git
    /// pushes the case's branch to it, a request record under its `takt/requests` opens the pull
    /// request, the run file's `local_checks` argument list runs as its checks, as the test
    /// command is, in a fresh repository at the head; the merge is made in the hub under the
    /// run's commit identity, moves the base only from the tip it was judged at, and moves
    /// nothing when that tip holds the head; nothing is scanned, nothing leaves the machine;
    /// refused before any case opens without `local_checks`, with checks the confinement cannot
    /// run, or without such an `origin`; never a default, refused for a scenario run) or `none` (no pull
    /// request is opened; when `pr.open`,
    /// `ci.read` or `repository.merge` is the only action that decides what is left, nothing
    /// performs it: the run stops `Unperformable`, exit 7, and the case stays open). Not given,
    /// `bot` when the repository's `origin` is a repository on github.com, the host the bot
    /// route serves; `gitlab` when it is on another host and `glab` is logged in to it; else
    /// `none`, said in one line naming what `glab` said. A scenario run creates its trial base
    /// only through the bot route: it refuses `gitlab` before anything is opened, and on another
    /// host its default is `none`, said in one line, `glab` never asked.
    #[arg(long)]
    forge: Option<String>,
    /// The least free space, in GiB, a step needs on the file system of the repository's tree
    /// and, when `CARGO_TARGET_DIR` is set, of the one holding that directory, each read before
    /// every step (a harness step, the test command, a gate, any deterministic step); under it
    /// the run stops `DiskLow` naming the path, its free space and this threshold, exit 8,
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
    /// The adapter every child of the run starts through, each a Substrate adapter; takt has no
    /// confinement mechanism of its own. `substrate`: the test command runs through the first
    /// of `systemd` around `linux`, `linux` (with --confinement-cgroup-root when given) and
    /// `macos` whose facts serve its profile: the tree as the workspace, its `target/`
    /// writable, the toolchain and the host's cargo caches as read-only roots, the tree as
    /// the working directory, the network isolated, and where the adapter serves `limits` the
    /// timeout, 2048 processes, 8 GiB of memory and an hour of CPU time; a test command its
    /// profile does not serve (it serves `cargo` on a Rust toolchain; a Python one among those it
    /// does not), a gate's commands,
    /// which the assertions library starts itself, and a host none of whose adapters serves
    /// the profile or a fact --confinement-needs names are refused by name and never run
    /// unconfined. `none`: the test command runs through Substrate's `local` adapter, declared
    /// insecure with the reason the `case` line and the `opened` event state: on the host as
    /// this user, in a cleared environment without credentials, with a scratch home and
    /// `TMPDIR`; it can read what this user can; the reason says whether the run was started
    /// with --confinement none or got `none` as the default. The `case` line the run prints
    /// and its `opened` event name the adapter and the facts it proved.
    #[arg(long, default_value = "none", value_parser = ["none", "substrate"])]
    confinement: String,
    /// Whether --confinement was given on the command line rather than left to its default;
    /// set by [`LoopArgs::configured`].
    #[arg(skip)]
    confinement_given: bool,
    /// A contract family or fact the run's test command needs beyond those every run needs,
    /// by its Substrate name (`limits`, `network_egress`, `private_tmpdir`, …): an adapter that
    /// does not serve or prove it is not chosen, and with none left the run is refused by
    /// name before anything starts. Repeatable.
    #[arg(long = "confinement-needs", value_name = "NAME")]
    confinement_needs: Vec<String>,
    /// For `--confinement substrate`: a cgroup v2 directory delegated to this user, which the
    /// `linux` adapter places each run under so it serves `limits` without `systemd`.
    #[arg(long, value_name = "DIR")]
    confinement_cgroup_root: Option<PathBuf>,
    /// The state directory the run keeps its host records in, in place of
    /// `$XDG_STATE_HOME/takt` (`~/.local/state/takt` without it): each harness step's
    /// transcript under `<DIR>/transcripts`, where the step's line names it relative to that
    /// directory (`takt replay --transcripts <DIR>/transcripts` reads it there), and what a
    /// discarded step sets aside under `<DIR>/set-aside`. A relative path is read against
    /// the current directory. For a container whose home is not kept, a directory on a volume
    /// that is.
    #[arg(long, value_name = "DIR")]
    state_dir: Option<PathBuf>,
    /// Go on past a red base: the case's first `tests.run` failing before any step changed the
    /// tree does not stop the run (`RedBase`), nor does its check again on a resume, for a base
    /// whose tests fail on purpose.
    #[arg(long)]
    continue_past_red_base: bool,
    /// Print every decision's full frontier, `frontier: <action> (<status>), …`, beside what the
    /// run may do: without it the run says the admissible actions once (`may:`) and then only
    /// when they change (`may now: +<added> -<removed>`).
    #[arg(long)]
    verbose: bool,
    /// When the console is in colour: `auto` when standard output is a terminal and `NO_COLOR`
    /// is unset or empty (<https://no-color.org>), `always` (for output piped through `tee` into
    /// a terminal), or `never`. Colour is never written to a case's records.
    #[arg(long, value_enum, value_name = "WHEN", default_value = "auto")]
    color: ColorArg,
    /// List neither what each harness step looked at (its transcript's tool calls, under its
    /// `said:` line) nor the diff it committed (before its `effect:` line); nothing else
    /// changes.
    #[arg(long)]
    quiet_steps: bool,
    /// The `setup` commands `--config` gave, run when the case opens; set from the file, never from
    /// a flag.
    #[arg(skip)]
    setup: Vec<takt::run_config::SetupCommand>,
    /// The digest of the `llm.routes/1` document `--config` gave, recorded on the run's `opened`
    /// line; set from the file, never from a flag.
    #[arg(skip)]
    llm_digest: Option<String>,
    /// The `llm.routes/1` document `--config` gave, parsed by llm; set from the file, never from a
    /// flag. Given with `--harness harness`, `--model` names one of its routes.
    #[arg(skip)]
    llm: Option<takt::run_config::LlmRoutes>,
    /// The route `--model` names in `llm`, resolved by llm before any case opens
    /// ([`LoopArgs::take_variable`]): the endpoint the session is pointed at and the key it takes.
    #[arg(skip)]
    route: Option<takt::harness::RouteEndpoint>,
    /// The records profile the case keeps its decisions under: `default` keeps the latest
    /// `decisions/NNNN.json` alone and goes on numbering from it, `trial` keeps every one. A
    /// scenario run and an improvement case (`self.improvement/1`, `loop.improvement/1`) keep
    /// every one whatever this says. Not given, a new case runs under `default` (or the file's
    /// `records`), and a resumed case under the profile it was opened under. The `opened` line
    /// names the profile the run keeps.
    #[arg(long, value_name = "default|trial", value_parser = ["default", "trial"])]
    records: Option<String>,
    /// The configuration file's `records`, kept apart from the flag: it counts as given only for
    /// a new case.
    #[arg(skip)]
    records_of_file: Option<String>,
    /// How a step commit's message is written: `semantic` (the default: the harness's reply
    /// carries a `commit` of `<type>: <imperative subject>` and a body of 3 or 4 lines),
    /// `repository` (the harness writes it as the repository's instruction files require) or
    /// `action` (`<action>: <summary>`). When a line of an instruction file at the case's base
    /// matches `(?i)commit (message|style|convention)`, `repository` is the style in effect
    /// whatever this says; the `opened` line records the style in effect and that line's file and
    /// number. Over the configuration's `commit.style`.
    #[arg(long, value_name = "STYLE", value_parser = ["semantic", "repository", "action"])]
    commit_style: Option<String>,
    /// The key of the trailer takt writes after the message, from the case's story: `Refs` by
    /// default, an empty one for none. A case opened on an intent gets none. Over the
    /// configuration's `commit.trailer`.
    #[arg(long, value_name = "KEY")]
    commit_trailer: Option<String>,
    /// The commit type of an action, `ACTION=TYPE`, repeatable or comma-separated: by default
    /// `tests.write=test`, and `implementation.edit=fix` for a story tagged `bug`, else `feat`;
    /// an action not named is `chore`. Over the configuration's `commit.types`.
    #[arg(long, value_name = "ACTION=TYPE", value_delimiter = ',')]
    commit_types: Vec<String>,
    /// The commit settings the configuration file and the flags above give, checked before the
    /// case opens; set by [`LoopArgs::configured`].
    #[arg(skip)]
    commit: Option<takt::commit_style::Settings>,
    /// The session request the run file's `harness` mapping gives, with each flag's setting over
    /// its key ([`takt::run_config::SessionRequest::with_flags`]): metaharness's settings the
    /// session is started with.
    #[arg(skip)]
    session: Option<takt::run_config::SessionRequest>,
}

/// A harness that performs nothing. Offered an action the case holds a grant for, it takes that
/// action, changing no file (story:harness-none-takes-a-granted-action). Asked while an approval
/// is pending, it requests the gate action, changing no file, so the run stops
/// `ApprovalRequired` and `takt approve` is the next step; offered an action that records a
/// choice (`change.handover`), it takes it, changing no file, so the work committed by hand is
/// handed over as it stands; asked for any other judgement step, it ends the run with an error,
/// `stopped: error:`, because nothing performs that step.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &takt::StepContext<'_>) -> Result<takt::StepReport> {
        if let Some(granted) = context.granted.first() {
            return Ok(takt::StepReport {
                action: granted.clone(),
                summary: "no harness: the action the case is granted is taken".to_owned(),
                tokens: None,
            });
        }
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

    /// No session runs, so nothing it read is proposed: a run without a harness is never
    /// refused for want of a scan.
    fn reads_outside_its_copy(&self) -> bool {
        false
    }
}

/// Exit 0: the run completed, the frontier had nothing ready, or the command was done.
const EXIT_DONE: u8 = 0;
/// Exit 1: refused before the run, the reason on stderr (`main`).
const EXIT_REFUSED: u8 = 1;
/// Exit 2: the arguments do not parse, clap's own, and a run file's `tokens` that is no whole
/// number above zero ([`takt::run_config::TokensRefused`]), as `--tokens x` does not parse.
const EXIT_USAGE: u8 = 2;
/// The token budget of a run that names none, by `--tokens` or its run file's `tokens`.
const DEFAULT_TOKENS: u64 = 3_000_000;
/// Exit 3: the run stopped on a budget, with nothing more it may do, or on the tests: red at the
/// base, or failing the same tests again.
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
/// Exit 8: the run stopped before a step with too little free disk for it, `stopped: DiskLow`;
/// the machine spoiled the trial, so a runner discards it rather than resume it.
const EXIT_DISK_LOW: u8 = 8;
/// Exit 9: a judgement step declined the intent with no file changed, or the case is declined
/// at its current case revision, `stopped: Declined (<reason>)`; apart from exit 3's
/// `NoProgress`, so a correct refusal and a stuck run read apart.
const EXIT_DECLINED: u8 = 9;

/// What `takt run --help` says about the exit codes; each line opens with its code.
const EXIT_CODES: &str = "Exit codes:
  0  stopped: Completed (an outcome is legitimate), or frontier: nothing ready
  1  refused before the run (an argument, the repository, the protocol or the case); the reason on stderr, no step run; or stopped: HarnessUnqualified (the harness binary does not qualify under the run's hermetic mode, found before any model call: its version, the failing row, the qualified versions and --harness-program), the case stays open and the next run resumes it; or stopped: SetupFailed (<name> exited <code>): a command of the run file's `setup` list did not end 0 when the case opened, before the red-base check and any model call; its output is kept under the state directory
  2  the arguments do not parse, or the run file's `tokens` is no whole number above zero (the file and the line named), before any case opens
  3  stopped: a budget (Iterations, Hours, Tokens, ActiveTime), NothingAdmissible, StepRefused, NoProgress, RedBase (the case's first tests.run failed before any step changed its base, before any model call: the failed tests and the rerun hint), RepeatedTestFailure (2 tests.run results in a row, each at a new revision, failed the same tests: the tests) or RepeatedGateFailure (3 gate.run results in a row, each at a new revision, failed the same gates: the gates); the case stays open
  4  stopped: ApprovalRequired; `takt approve` grants it and the next run resumes the case
  5  stopped: error: the run ended on an error once its case was open (a forge that refused, a validator or the harness that failed; a judgement step whose session ends without the line naming its action is tried 3 times before the run ends on it, each try a step that --iterations counts, charged the tokens its session reported; a budget a try spent stops the run with exit 3); the case stays open
  6  stopped: Waiting (<actions>; <the budget spent>, no step or hour budget, or locally: <what it waits for>): the run waited on the forge (checks still running, a pull request's head behind the push, a merge held for a required check) and its budget (--iterations, --hours) ended first, or it had none to wait in; or it waits on something local (a saved scorecard, a promoted binary, a merge into the local main) and stopped at once after the step that observed it, naming the file it reads, with no pause and no observation again; nothing recorded for it, the case stays open and the next run resumes it
  7  stopped: Unperformable (<claim>; nothing performs it without <the forge, the gates document>): no step left moves an open obligation, and the only action that decides <claim> is admissible but has no performer in this run (--forge none, no gates document); no judgement step is taken, the case stays open; at pr.opened the delivery first prints `candidate: <full commit id>`, its code commits (not takt's take, record or approval commits) replayed on the base branch's tip under the configured identity and named by refs/takt/candidates/<case>, or `candidate: none: <why>` (no identity configured, or a code commit that needs what was left out)
  8  stopped: DiskLow (<path>: <free> free, under <threshold>): before a step, the file system of the tree or of CARGO_TARGET_DIR had less free than --min-free-gib; the case stays open, but a case with a DiskLow run is a DiskLow trial, which leaves its scenario without a scorecard verdict, so a trial runner discards it rather than resume it
  9  stopped: Declined (<reason>): a judgement step named `decline` instead of an offered action, with no file changed, its reason the reply's summary; recorded as a `decline` line in events.jsonl with the case revision, never as evidence; the case stays open, a run on it at that case revision stops Declined again before any step, an argument-less run passes it over, and a commit that moves its revision makes it step again";

/// What `takt scenario run --help` says about the exit codes: [`EXIT_CODES`] without `frontier:
/// nothing ready`, which only `takt run` prints; a scenario whose story is not ready is exit 1.
const SCENARIO_EXIT_CODES: &str = "Exit codes:
  0  stopped: Completed (an outcome is legitimate)
  1  refused before the run (an argument, the repository, the scenario, its seed, its story not ready there, its trial base or its case); the reason on stderr, no step run; or stopped: HarnessUnqualified (the harness binary does not qualify under the run's hermetic mode, found before any model call: its version, the failing row, the qualified versions and --harness-program), the case stays open and the next run resumes it
  2  the arguments do not parse, or the run file's `tokens` is no whole number above zero (the file and the line named), before any case opens
  3  stopped: a budget (Iterations, Hours, Tokens, ActiveTime), NothingAdmissible, StepRefused, NoProgress, RedBase (the case's first tests.run failed before any step changed its base, before any model call: the failed tests and the rerun hint), RepeatedTestFailure (2 tests.run results in a row, each at a new revision, failed the same tests: the tests) or RepeatedGateFailure (3 gate.run results in a row, each at a new revision, failed the same gates: the gates); the case stays open
  4  stopped: ApprovalRequired; `takt approve` grants it and `takt run --case <case>` resumes the case
  5  stopped: error: the run ended on an error once its case was open (a forge that refused, a validator or the harness that failed; a judgement step whose session ends without the line naming its action is tried 3 times before the run ends on it, each try a step that --iterations counts, charged the tokens its session reported; a budget a try spent stops the run with exit 3); the case stays open
  6  stopped: Waiting (<actions>; <the budget spent>, no step or hour budget, or locally: <what it waits for>): the run waited on the forge (checks still running, a pull request's head behind the push, a merge held for a required check) and its budget (--iterations, --hours) ended first, or it had none to wait in; or it waits on something local (a saved scorecard, a promoted binary, a merge into the local main) and stopped at once after the step that observed it, naming the file it reads, with no pause and no observation again; nothing recorded for it, the case stays open and `takt run --case <case>` resumes the case
  7  stopped: Unperformable (<claim>; nothing performs it without <the forge, the gates document>): no step left moves an open obligation, and the only action that decides <claim> is admissible but has no performer in this run (--forge none, no gates document); no judgement step is taken, the case stays open; at pr.opened the delivery first prints `candidate: <full commit id>`, its code commits (not takt's take, record or approval commits) replayed on the base branch's tip under the configured identity and named by refs/takt/candidates/<case>, or `candidate: none: <why>` (no identity configured, or a code commit that needs what was left out)
  8  stopped: DiskLow (<path>: <free> free, under <threshold>): before a step, the file system of the tree or of CARGO_TARGET_DIR had less free than --min-free-gib; the case stays open, but a case with a DiskLow run is a DiskLow trial, which leaves its scenario without a scorecard verdict, so a trial runner discards it rather than resume it
  9  stopped: Declined (<reason>): a judgement step named `decline` instead of an offered action, with no file changed, its reason the reply's summary; recorded as a `decline` line in events.jsonl with the case revision, never as evidence; the case stays open, `takt run --case <case>` at that case revision stops Declined again before any step, and a commit that moves its revision makes it step again";

fn main() -> ExitCode {
    let mut cli = match parsed() {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("takt: {error:#}");
            if error
                .downcast_ref::<takt::run_config::TokensRefused>()
                .is_some()
            {
                return ExitCode::from(EXIT_USAGE);
            }
            return ExitCode::from(EXIT_REFUSED);
        }
    };
    // The endpoint's credential variable leaves takt's environment here, on the one thread
    // takt has, before it starts any process.
    let taken = match &mut cli.command {
        Commands::Run(args) => args.looping.take_variable(),
        Commands::Scenario(ScenarioCommands::Run(args)) => args.looping.take_variable(),
        _ => Ok(()),
    };
    if let Err(error) = taken {
        eprintln!("takt: {error:#}");
        return ExitCode::from(EXIT_REFUSED);
    }
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
/// | 3 | any other stop: a budget, `NothingAdmissible`, `StepRefused`, `NoProgress`, `RedBase`, `RepeatedTestFailure`, `RepeatedGateFailure` |
/// | 4 | `ApprovalRequired`: `takt approve` is the next step |
/// | 5 | `stopped: error:`, the run ended on an error once its case was open: a forge that refused, a validator or the harness that failed; a judgement step whose session names no action only after `NO_ACTION_TRIES` tries, each a step |
/// | 6 | `Waiting`: the run waited on the forge until its budget ended, or waits on something local and stopped at once; the next run resumes the case |
/// | 7 | `Unperformable`: no step can move the case; a claim left is decided only by an action the run has no performer for |
/// | 8 | `DiskLow`: before a step, a file system it builds on had less free than `--min-free-gib`; a runner discards the trial rather than resume it |
fn run(cli: Cli, out: &mut dyn Write) -> Result<u8> {
    match cli.command {
        Commands::Protocol(ProtocolCommands::Show(args)) => {
            let ProtocolShowArgs {
                protocol,
                config,
                format,
            } = args;
            let file = match &config {
                Some(path) => takt::run_config::RunConfig::read(path)?.protocol_settings,
                None => takt::run_config::ProtocolSettings::default(),
            };
            let listing = takt::protocol::list_settings(&protocol, &file)?;
            match format {
                ShowFormat::Text => write!(out, "{}", listing.text())?,
                ShowFormat::Json => writeln!(out, "{}", listing.json_text())?,
            }
            Ok(EXIT_DONE)
        }
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
        Commands::Replay(args) => replay(args, out),
        Commands::Scorecard(ScorecardArgs {
            repo,
            save,
            revision,
            against,
            steps,
            across_layers,
        }) => {
            let reported = takt::scorecard::report(
                &repo,
                save.as_deref(),
                revision.as_deref(),
                against.as_deref(),
                &steps,
                across_layers,
                out,
            )?;
            Ok(if reported.worse > 0 {
                3
            } else if reported.steps == Some(takt::scorecard::StepVerdict::NotBetter) {
                4
            } else {
                EXIT_DONE
            })
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
        Commands::Report(ReportArgs {
            cases,
            case,
            steps,
            trace,
            format,
        }) => {
            if !cases.is_empty() {
                takt::report::print_case_sums(&takt::report::case_sums(&cases)?, out)?;
                return Ok(EXIT_DONE);
            }
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
            match format {
                ShowFormat::Text => takt::report::print(&traces, out)?,
                ShowFormat::Json => takt::report::print_json(&traces, out)?,
            }
            Ok(EXIT_DONE)
        }
        Commands::View(ViewArgs { dir, output }) => {
            let rendered = takt::view::write(&dir, &output)?;
            writeln!(
                out,
                "written: {} ({} runs, {} operations, {} of {} harness steps with a transcript)",
                output.display(),
                rendered.runs,
                rendered.ops,
                rendered.transcripts,
                rendered.transcripts + rendered.missing
            )?;
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
                story,
                story_from,
                protocol,
                inputs,
                case_id,
                scenarios,
                install_root,
                config,
                declared_scan,
                protocol_settings,
                forge_settings,
                looping,
            } = *args;
            looping.check()?;
            // The run's scan adapter, `gates` unless the configuration file declares `none`:
            // `none` with a gates policy set too is refused before anything is read or opened.
            let scan = Scan::of_run(declared_scan.zip(config.as_deref()))?;
            let repo = repo
                .canonicalize()
                .with_context(|| format!("the repository {}", repo.display()))?;
            looping.confine(&repo)?;
            looping.check_set_aside(&repo)?;
            looping.check_state_dir(&repo)?;
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
            // `--protocol`, else the run file's `protocol` for a story of the store, else the
            // default for the unit. A story's own `protocol:` tag is read once the story is known.
            let (opening, _, opens_on) =
                takt::protocol::choose_protocol(&takt::protocol::SettingsRequest {
                    file: &protocol_settings,
                    flag_protocol: protocol.as_deref(),
                    flag_inputs: &inputs,
                    tags: &[],
                    default_protocol: takt::protocol::default_protocol(intent.is_none()),
                    file_protocol_applies: intent.is_none() && case.is_none(),
                })?;
            // The case this run resumes, found before anything is resumed: resuming checks the
            // tree out onto the case's branch and records its head, so every refusal of what was
            // given for it comes first. A resumed case keeps the protocol and the inputs it was
            // opened with: a `--protocol` that names another, or any `--input`, is refused.
            // A turn opens a case whether or not an intent is given: with none, its finding's
            // title is the intent, and no case of the planning store is resumed for it.
            let opens_turn = case.is_none() && opens_on.id == takt::case::TURN_PROTOCOL;
            // A turn opens no story: the configuration file's `story` is left out for it, where
            // --story is refused.
            let story = story.filter(|_| !(opens_turn && story_from.is_some()));
            // What named the story, for its refusals.
            let named_by = |story: &str| match &story_from {
                Some(file) => format!("story {story} from {}", file.display()),
                None => format!("--story {story}"),
            };
            if let Some(story) = &story
                && opens_turn
            {
                return Err(anyhow!(
                    "--story {story}: a {} turn opens on its finding (--input {}=<story id>), never on a named story; nothing was changed",
                    takt::case::TURN_PROTOCOL,
                    takt::case::TURN_FINDING
                ));
            }
            let resuming = case_to_resume(
                &repo,
                case.as_deref(),
                story.as_deref(),
                intent.is_some() || opens_turn,
                out,
            )?;
            let given_for_resume = given.as_ref().zip(protocol.as_deref());
            let first_input = inputs.first().map(|(name, _)| name.clone());
            if let Some(resuming) = &resuming {
                refuse_for_resume(
                    &resuming.id,
                    &resuming.protocol,
                    given_for_resume,
                    first_input.as_deref(),
                )?;
                if let Some(given) = &install_root {
                    resume_install_root(&repo, &resuming.id, &resuming.protocol.id, given)?;
                }
                // A turn runs on the embedded protocol alone, and only by the binary that judges
                // it, recorded when it opened: both checked before the case is resumed, so no
                // step is taken otherwise.
                if let Some(why) = takt::case::turn_protocol_refusal(&resuming.protocol)? {
                    return Err(anyhow!(why));
                }
                takt::case::check_id(&resuming.id)?;
                if let Some(why) = takt::case::turn_judge_refusal(
                    &takt::case::CaseDir::locate(&repo, &resuming.id),
                    &running_binary()?,
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
            // The identity a local forge's merge commits carry: the one the case commits with, as
            // the scan at open reads it.
            let local_identity = if looping.forge.as_deref() == Some("local") {
                identity_at_open(
                    &repo,
                    looping.commit_identity.as_ref(),
                    case.as_deref()
                        .or(resuming.as_ref().map(|resuming| resuming.id.as_str())),
                )?
                .map(|(identity, _)| identity)
            } else {
                None
            };
            // The forge, read before the turn, whose mainline it reads: the forge's `main`, or the
            // local `main` with none.
            let forge = forge_with(
                &repo,
                looping.forge.as_deref(),
                &scan,
                &forge_settings,
                &resuming
                    .as_ref()
                    .map_or(&opens_on, |resuming| &resuming.protocol)
                    .actions,
                looping.state_dir.as_deref(),
                local_identity,
                looping.min_free_gib.saturating_mul(1 << 30),
            )?;
            // A resumed turn reads the mainline through the forge it recorded when it opened,
            // from the repository it recorded, as `takt approve` does: a run through another,
            // or on another repository, is refused before anything is written.
            if let Some(resuming) = &resuming {
                let run_forge = forge.forge.as_ref().map_or("none", |forge| forge.name());
                if let Some(why) = takt::case::turn_forge_refusal(
                    &takt::case::CaseDir::locate(&repo, &resuming.id),
                    &resuming.id,
                    run_forge,
                    forge.repository.as_deref(),
                )? {
                    return Err(anyhow!(why));
                }
            }
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
            let mainline = forge.mainline();
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
                // read as it runs on Linux, whatever its path holds by now; elsewhere, read at
                // its path (`running_binary`).
                let binary = running_binary()?;
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
            // A fresh case on an intent opens on settings resolved here: `--protocol`, the run
            // file, the default, and each declared input from `--input`, the run file or takt's
            // default, each with its source. One it does not declare is refused before anything is
            // opened. A fresh case on a story resolves them per story below, once its tags are
            // known; a resumed case keeps its own, and its run file is checked all the same.
            let fresh = case.is_none() && resuming.is_none();
            let story_unit = fresh && intent.is_none();
            if !fresh {
                takt::protocol::check_run_file(&protocol_settings, None)?;
            }
            let mut applied = if fresh && !story_unit {
                Some(settings_for(
                    &protocol_settings,
                    protocol.as_deref(),
                    &inputs,
                    &[],
                    false,
                )?)
            } else {
                None
            };
            let options = match &applied {
                Some(settings) => {
                    looping.options(settings.spec.clone(), settings.pairs(), case_id.clone())
                }
                None => looping.options(opening, inputs.clone(), case_id.clone()),
            };
            let harness = harness_of(&looping)?;
            let harness = harness.as_ref();
            // A session that may read outside its copy proposes what a scan must pass: under
            // `gates` with no policy nothing would scan it, so the run is refused before its
            // case opens or resumes, naming the policy and `scan: none` in `--config`.
            if harness.reads_outside_its_copy()
                && let Some(why) = scan.refused_at_open(&looping.harness, true)
            {
                return Err(anyhow!(why));
            }
            let mut validators = validators_of(&repo, &looping, scenarios)?;
            forge.register(&mut validators);
            // An improvement turn's merge validators read the mainline, with a forge or without.
            validators.push(Box::new(takt::improvement::CandidateMerged::new(
                mainline.clone(),
            )));
            validators.push(Box::new(takt::improvement::CandidateMerged::judge_surface(
                mainline,
            )));
            // Through the bot route the identity the run's commits carry is scanned, as the push
            // would scan it, after every refusal of what was given and before the case opens or
            // resumes: before step 1.
            if let Some(forge) = &forge.forge
                && let Some((identity, source)) = identity_at_open(
                    &repo,
                    looping.commit_identity.as_ref(),
                    resuming.as_ref().map(|resuming| resuming.id.as_str()),
                )?
            {
                scan_identity_at_open(
                    forge.as_ref(),
                    &scan,
                    &repo,
                    &identity,
                    &source,
                    looping.state_dir.as_deref(),
                    out,
                )?;
            }
            let forge_line = forge.line;
            let store = Aep::default();
            // The unit: the case to resume, the intent given, or else the open case of an
            // active story or the next ready story of the planning store; a case is opened
            // before the store is told, so a run that cannot open its case takes nothing.
            let resumed = case.is_some() || resuming.is_some();
            // Where a discarded step sets its files aside: the state directory's, given to the
            // run at open, so that opening reads neither `XDG_STATE_HOME` nor `HOME`.
            let set_aside = looping.state_dir.as_deref().map(state_set_aside);
            let mut run = match (case, intent, resuming) {
                (Some(_), Some(_), _) => {
                    return Err(anyhow!(
                        "--case resumes a case and --intent opens one; give one of them"
                    ));
                }
                (Some(case), None, _) => Run::resume_in(
                    &repo,
                    options,
                    &case,
                    set_aside.as_deref(),
                    harness,
                    &validators,
                )?,
                (None, Some(intent), _) => Run::open_unit_in(
                    &repo,
                    options,
                    Unit::Intent(intent),
                    &commit_as,
                    set_aside.as_deref(),
                    harness,
                    &validators,
                )?,
                // The open case of an active story, found and checked above: resumed as found,
                // the store not read again.
                (None, None, Some(resuming)) => {
                    let run = Run::resume_in(
                        &repo,
                        options,
                        &resuming.id,
                        set_aside.as_deref(),
                        harness,
                        &validators,
                    )?;
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
                // checked. A story `--story` names is taken the same way, the frontier narrowed
                // to it; one it does not offer is refused, no case opened.
                (None, None, None) => {
                    let named = story.as_deref().map(|story| Named {
                        store: &store,
                        story,
                    });
                    let frontier: &dyn takt::Frontier = match &named {
                        Some(named) => named,
                        None => &store,
                    };
                    // Where the tree is before the take, which reads the store on the base and
                    // so moves a tree on a case's branch there: a refused `--story` puts it back.
                    let left = match &story {
                        Some(_) => Some(match takt::git::current_branch(&repo)? {
                            Some(branch) => branch,
                            None => takt::git::head(&repo)?,
                        }),
                        None => None,
                    };
                    match take_next_with_settings(
                        &repo,
                        frontier,
                        &store,
                        &|tags| {
                            settings_for(
                                &protocol_settings,
                                protocol.as_deref(),
                                &inputs,
                                tags,
                                true,
                            )
                        },
                        &|settings| {
                            looping.options(
                                settings.spec.clone(),
                                settings.pairs(),
                                case_id.clone(),
                            )
                        },
                        options,
                        &commit_as,
                        set_aside.as_deref(),
                        harness,
                        &validators,
                        out,
                    )? {
                        Some((run, settings)) => {
                            applied = Some(settings);
                            run
                        }
                        None => {
                            if let (Some(story), Some(left)) = (&story, &left) {
                                let refusal =
                                    match story_not_taken(&repo, &store, story, &named_by(story)) {
                                        Ok(refusal) | Err(refusal) => refusal,
                                    };
                                return Err(
                                    match takt::git::run(&repo, &["checkout", "-q", left]) {
                                        Ok(_) => refusal,
                                        Err(undo) => refusal.context(format!(
                                            "and the tree could not be put back on {left}: {undo:#}"
                                        )),
                                    },
                                );
                            }
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
            // The case just opened records where its protocol and each input's value came from.
            if let Some(settings) = &applied {
                run.case
                    .record_settings(settings.protocol_source.as_str(), &settings.sources())?;
                writeln!(out, "{}", settings.line())?;
            }
            // A story's case that terminates accepted records its test result and its move
            // in the store; an intent's run has no story and writes nothing there.
            run.record_on(&store);
            // A new case runs under `default` unless the flag or the file says otherwise; a resumed
            // one keeps the profile it was opened under unless the flag says otherwise.
            let records = match looping.records_profile(resumed) {
                Some(records) => Some(records),
                None if resumed => None,
                None => Some(takt::case::Records::default()),
            };
            finish(
                &mut run,
                forge_line.as_deref(),
                scan,
                &looping,
                records,
                out,
            )
        }
    }
}

impl LoopArgs {
    /// Takes the flags given under the names they had for one minor release: each prints one line
    /// on standard error naming the new name, and `--endpoint-model`, the old name of `--model`, is
    /// the model it names. Called before anything else reads the loop's flags.
    fn renamed(&mut self, matches: &clap::ArgMatches) {
        const RENAMED: [(&str, &str, &str); 6] = [
            (
                "endpoint_base_url",
                "--endpoint-base-url",
                "--model-endpoint",
            ),
            ("endpoint_wire", "--endpoint-wire", "--model-wire"),
            ("endpoint_model", "--endpoint-model", "--model"),
            (
                "endpoint_token_file",
                "--endpoint-token-file",
                "--subscription-token-file",
            ),
            (
                "endpoint_token_pointer",
                "--endpoint-token-pointer",
                "--subscription-token-pointer",
            ),
            ("endpoint_secret", "--endpoint-secret", "--api-key-secret"),
        ];
        for (id, old, new) in RENAMED {
            if !unset(matches, id) {
                eprintln!("takt: {old} is renamed {new}; the old name works for one minor release");
            }
        }
        if let Some(model) = self.endpoint_model.take() {
            self.model = Some(model);
        }
    }

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
        if self.active_seconds == Some(0) {
            return Err(anyhow!(
                "--active-seconds is 0: an active-time budget is a number of seconds above 0"
            ));
        }
        Ok(())
    }

    /// Refuses a state directory named by --state-dir or the run configuration's `state_dir`
    /// that cannot hold the run's transcripts, before anything of the run is opened, naming it
    /// and why: its `transcripts` must lie outside `repo`, every part of its way a directory
    /// (or not there yet, then made `0700`, as the run makes it) and writable. Without one,
    /// nothing is checked: the default location keeps no transcript when it cannot be used,
    /// said on standard error ([`finish`]).
    fn check_state_dir(&self, repo: &Path) -> Result<()> {
        let Some(dir) = &self.state_dir else {
            return Ok(());
        };
        let refused = |why: anyhow::Error| {
            anyhow!(
                "the state directory {} (--state-dir, or the run configuration's `state_dir`) cannot be used: {why:#}; nothing was opened",
                takt::git::shown(dir.as_os_str())
            )
        };
        let root =
            takt::harness::transcripts_root(&state_transcripts(dir), repo).map_err(refused)?;
        // What this check makes is taken back when it refuses, so a refused state directory is
        // left as it was found.
        let mut made = Vec::new();
        let checked = takt::harness::make_transcripts_root(&root, &mut made)
            .map_err(refused)
            .and_then(|()| {
                let probe = root.join(format!(".takt-write-check-{}", std::process::id()));
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&probe)
                    .map_err(|error| {
                        refused(anyhow!("{} is not writable: {error}", root.display()))
                    })?;
                let _ = std::fs::remove_file(&probe);
                Ok(())
            });
        if checked.is_err() {
            for made in made.iter().rev() {
                let _ = std::fs::remove_dir(made);
            }
        }
        checked
    }

    /// Refuses a state directory named by --state-dir or the run configuration's `state_dir`
    /// whose set-aside directory, `<state_dir>/set-aside`, cannot be used by the run (not
    /// absolute, inside `repo`, a file on its way), before anything of the run is opened,
    /// naming it and why; [`LoopArgs::check_state_dir`] checks the transcripts' directory.
    fn check_set_aside(&self, repo: &Path) -> Result<()> {
        let Some(dir) = &self.state_dir else {
            return Ok(());
        };
        takt::git::set_aside_root(&state_set_aside(dir), repo).map_err(|why| {
            anyhow!(
                "the state directory {} (--state-dir, or the run configuration's `state_dir`) cannot be used: {why:#}; nothing was opened",
                takt::git::shown(dir.as_os_str())
            )
        })?;
        Ok(())
    }

    /// Installs the adapter --confinement names for a run on `repo`, before anything of the
    /// run is done: `substrate` opens the first Substrate adapter whose facts serve the test
    /// command's profile ([`takt::confinement::CANDIDATES`]); `none` checks that Substrate's
    /// `local` adapter admits it, declared insecure with a reason that says whether the flag
    /// was given or `none` is the default. A refusal names what was missing, and
    /// nothing is started.
    fn confine(&self, repo: &Path) -> Result<()> {
        let options = takt::confinement::Options {
            needs: self.confinement_needs.clone(),
            cgroup_root: self.confinement_cgroup_root.clone(),
        };
        let refused = |error: anyhow::Error| {
            anyhow!(
                "confinement {} refused the run on {}: {error:#}; nothing was started",
                self.confinement,
                repo.display()
            )
        };
        if self.confinement == "substrate" {
            let substrate = takt::confinement::Substrate::open(repo, &options).map_err(refused)?;
            takt::confinement::install(std::sync::Arc::new(substrate));
            return Ok(());
        }
        if options.cgroup_root.is_some() {
            return Err(refused(anyhow!(
                "--confinement-cgroup-root is for --confinement substrate"
            )));
        }
        let none = takt::confinement::check_none(repo, &options, self.confinement_given)
            .map_err(refused)?;
        takt::confinement::install(std::sync::Arc::new(none));
        Ok(())
    }

    /// Under `--harness harness`, takes the variable the endpoint's credential names out of
    /// takt's environment ([`EndpointArgs::take_variable`]). With an `llm` section, resolves the
    /// route `--model` names, before any case opens, and takes out of takt's environment the
    /// variables holding the key llm resolved for it ([`takt::harness::take_environment_holding`])
    /// and every variable the document's `env:` references name, in any endpoint.
    /// Resolved once: a second call keeps the first route.
    fn take_variable(&mut self) -> Result<()> {
        if self.harness != "harness" {
            return Ok(());
        }
        if let Some(llm) = &self.llm {
            if self.route.is_none() {
                let model = self.model.as_deref().unwrap_or_default();
                let home = std::env::var_os("HOME").map(PathBuf::from);
                let route =
                    takt::harness::RouteEndpoint::resolve(llm.routes(), model, home.as_deref())?;
                if let Some(secret) = &route.secret {
                    // SAFETY: takt has started no thread yet (its callers run before any is
                    // started), so nothing reads the environment while it changes.
                    unsafe { takt::harness::take_environment_holding(secret) };
                }
                for variable in llm.variables() {
                    // SAFETY: as above, no thread has started.
                    unsafe { takt::harness::take_environment_variable(variable) };
                }
                self.route = Some(route);
            }
            return Ok(());
        }
        self.endpoint.take_variable();
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
            tokens: Some(self.tokens),
            case_id,
            gate: self.gate.clone(),
        }
    }
}

/// What a case opens on, resolved from the command line, the run file and the tags of the story it
/// opens on (none for an intent): [`takt::protocol::resolve_settings`].
fn settings_for(
    file: &takt::run_config::ProtocolSettings,
    flag_protocol: Option<&str>,
    flag_inputs: &[(String, String)],
    tags: &[String],
    story: bool,
) -> Result<takt::protocol::Settings> {
    takt::protocol::resolve_settings(&takt::protocol::SettingsRequest {
        file,
        flag_protocol,
        flag_inputs,
        tags,
        default_protocol: takt::protocol::default_protocol(story),
        file_protocol_applies: story,
    })
}

/// A frontier that lists the candidates of `inner` into `seen` and offers none, so that
/// [`Run::take_next_as_in`] puts the tree on the base, reads them as it reads them, and opens
/// nothing.
struct Listing<'a> {
    inner: &'a dyn takt::Frontier,
    seen: std::cell::RefCell<Vec<String>>,
}

impl takt::Frontier for Listing<'_> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn candidates(&self, repo: &Path) -> Result<Vec<String>> {
        *self.seen.borrow_mut() = self.inner.candidates(repo)?;
        Ok(Vec::new())
    }

    fn active(&self, repo: &Path) -> Result<Vec<String>> {
        self.inner.active(repo)
    }

    fn show(&self, repo: &Path, id: &str) -> Result<takt::Story> {
        self.inner.show(repo, id)
    }

    fn take(&self, repo: &Path, id: &str) -> Result<takt::frontier::Take> {
        self.inner.take(repo, id)
    }

    fn record_approval(
        &self,
        repo: &Path,
        story: &str,
        approval: &takt::case::Approval,
    ) -> Result<()> {
        self.inner.record_approval(repo, story, approval)
    }
}

/// A frontier narrowed to the one candidate `story` of `inner`, which is then taken exactly as
/// `inner` would take it.
struct Only<'a> {
    inner: &'a dyn takt::Frontier,
    story: &'a str,
}

impl takt::Frontier for Only<'_> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn candidates(&self, repo: &Path) -> Result<Vec<String>> {
        let mut candidates = self.inner.candidates(repo)?;
        candidates.retain(|id| id == self.story);
        Ok(candidates)
    }

    fn active(&self, repo: &Path) -> Result<Vec<String>> {
        self.inner.active(repo)
    }

    fn show(&self, repo: &Path, id: &str) -> Result<takt::Story> {
        self.inner.show(repo, id)
    }

    fn take(&self, repo: &Path, id: &str) -> Result<takt::frontier::Take> {
        self.inner.take(repo, id)
    }

    fn record_approval(
        &self,
        repo: &Path,
        story: &str,
        approval: &takt::case::Approval,
    ) -> Result<()> {
        self.inner.record_approval(repo, story, approval)
    }

    fn record_test_result(
        &self,
        repo: &Path,
        story: &str,
        reference: &str,
        at: &str,
    ) -> Result<takt::frontier::Written> {
        self.inner.record_test_result(repo, story, reference, at)
    }

    fn implemented(&self, repo: &Path, story: &str) -> Result<takt::frontier::Written> {
        self.inner.implemented(repo, story)
    }
}

/// [`Run::take_next_as_in`] with each story's settings resolved from its own tags: the candidates
/// are listed as the run reads them (the tree put on the base), then each is offered alone, with
/// the options `options_for` makes of the settings `resolve` makes of its tags, until one is
/// taken. A story whose tags or whose settings are refused is the error, before its case is
/// opened. The settings of the story taken come back with its run.
#[allow(clippy::too_many_arguments)]
fn take_next_with_settings<'a>(
    repo: &Path,
    frontier: &dyn takt::Frontier,
    store: &Aep,
    resolve: &dyn Fn(&[String]) -> Result<takt::protocol::Settings>,
    options_for: &dyn Fn(&takt::protocol::Settings) -> RunOptions,
    listing: RunOptions,
    commit_as: &takt::case::CommitAs,
    set_aside: Option<&Path>,
    harness: &'a dyn Harness,
    validators: &'a [Box<dyn Validator>],
    out: &mut dyn Write,
) -> Result<Option<(Run<'a>, takt::protocol::Settings)>> {
    let listed = Listing {
        inner: frontier,
        seen: std::cell::RefCell::new(Vec::new()),
    };
    if Run::take_next_as_in(
        repo, listing, &listed, commit_as, set_aside, harness, validators, out,
    )?
    .is_some()
    {
        return Err(anyhow!("listing the ready stories opened a case"));
    }
    for id in listed.seen.into_inner() {
        let tags = store.tags(repo, &id)?;
        let settings = resolve(&tags).with_context(|| format!("the story {id}"))?;
        let only = Only {
            inner: frontier,
            story: &id,
        };
        if let Some(run) = Run::take_next_as_in(
            repo,
            options_for(&settings),
            &only,
            commit_as,
            set_aside,
            harness,
            validators,
            out,
        )? {
            return Ok(Some((run, settings)));
        }
    }
    Ok(None)
}

/// The harness `--harness` names, each through the metaharness session executor, proposing from
/// a scratch copy what takt's edit effect applies, with `--model`: `claude`, Claude Code on the
/// user's claude.ai login; `codex`, Codex on the user's ChatGPT login (`~/.codex/auth.json`);
/// `harness`, b10x-harness pointed at the endpoint the `--endpoint-*` flags and
/// `--endpoint-file` give ([`EndpointArgs`]), by default the Anthropic Messages API on the
/// user's claude.ai subscription token (`~/.claude/.credentials.json`), the endpoint
/// b10x-harness's own `claude` provider names; or `none`. An `--endpoint-*` flag given with
/// another harness is refused.
fn harness_of(looping: &LoopArgs) -> Result<Box<dyn Harness>> {
    let session = |kind| -> Result<SessionHarness> {
        let mut session = SessionHarness::of(kind);
        session.model.clone_from(&looping.model);
        if let Some(program) = &looping.harness_program {
            session.program = program.to_string_lossy().into_owned();
        }
        if let Some(max_turns) = looping
            .session
            .as_ref()
            .and_then(|request| request.max_turns())
        {
            session.max_turns = max_turns;
        }
        session.check_program()?;
        Ok(session)
    };
    if let Some(program) = &looping.harness_program {
        if looping.harness == "none" {
            return Err(anyhow!(
                "--harness-program {} names the program a session starts, and --harness none starts none; nothing was started",
                takt::git::shown(program.as_os_str())
            ));
        }
        if !program.is_absolute() {
            return Err(anyhow!(
                "--harness-program {} is not an absolute path; nothing was started",
                takt::git::shown(program.as_os_str())
            ));
        }
    }
    if looping.harness != "harness"
        && let Some(flag) = looping.endpoint.given()
    {
        return Err(anyhow!(
            "{flag} points --harness harness at its endpoint; --harness {} takes none",
            looping.harness
        ));
    }
    match looping.harness.as_str() {
        "claude" => Ok(Box::new(session(Kind::Claude)?)),
        "codex" => Ok(Box::new(session(Kind::Codex)?)),
        "harness" => {
            use takt::harness::{Credential, StagedToken};
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let mut session = session(Kind::B10x)?;
            let config = match &looping.route {
                Some(route) => {
                    session.model = Some(route.config.model.clone());
                    route.config.clone()
                }
                None if looping.llm.is_some() => {
                    return Err(anyhow!(
                        "the run's route was not resolved before its case opens; nothing was started"
                    ));
                }
                None => looping.endpoint.config(home.as_deref())?,
            };
            session.point_at(&config);
            // A key is staged now, before any case opens, and held by the harness, so the file
            // goes when the run ends, on success or on an error: a variable's value, or a route's
            // key as llm resolved it.
            match &config.credential {
                Credential::Environment { name } => {
                    session.hold_staged(StagedToken::stage(
                        name,
                        looping.endpoint.variable.as_ref(),
                        &StagedToken::candidates(),
                    )?);
                }
                Credential::Route { endpoint, .. } => {
                    let key = looping
                        .route
                        .as_ref()
                        .and_then(|route| route.secret.as_ref());
                    session.hold_staged(StagedToken::stage(
                        endpoint,
                        key,
                        &StagedToken::candidates(),
                    )?);
                }
                Credential::SubscriptionFile { .. } | Credential::Secret { .. } => {}
            }
            session.home = home;
            Ok(Box::new(session))
        }
        "none" => Ok(Box::new(NoHarness)),
        other => Err(anyhow!(
            "no harness `{other}`: claude, codex, harness or none"
        )),
    }
}

/// Where `--harness harness` points b10x-harness, read only under it: each field a flag's,
/// else `--endpoint-file`'s, else the default. A credential is named, never given: a token
/// file's path or a secret's name.
#[derive(clap::Args)]
struct EndpointArgs {
    /// `--harness harness`: the endpoint's base URL, its origin and API prefix. Not given,
    /// `--endpoint-file`'s `base_url`, else `https://api.anthropic.com/v1`. `https://`, or
    /// `http://` to a loopback host; no userinfo, query or fragment. Another base URL takes a
    /// credential by flag or file: the default token is sent to the default one alone.
    #[arg(long, value_name = "URL", conflicts_with = "endpoint_base_url")]
    model_endpoint: Option<String>,
    /// The old name of `--model-endpoint`, kept for one minor release, with one line on standard
    /// error naming the new name.
    #[arg(long, value_name = "URL")]
    endpoint_base_url: Option<String>,
    /// `--harness harness`: the model API the endpoint speaks, `anthropic-messages` or
    /// `openai-responses`. Not given, `--endpoint-file`'s `wire`, else `anthropic-messages`.
    #[arg(long, value_name = "WIRE", conflicts_with = "endpoint_wire")]
    model_wire: Option<String>,
    /// The old name of `--model-wire`, kept for one minor release, with one line on standard
    /// error naming the new name.
    #[arg(long, value_name = "WIRE")]
    endpoint_wire: Option<String>,
    /// `--harness harness`: the file holding a subscription token, which b10x-harness reads;
    /// not with --api-key-secret. Not given, `--endpoint-file`'s credential, else
    /// `~/.claude/.credentials.json` at `/claudeAiOauth/accessToken`. A relative path is read
    /// against the current directory. A file that is missing, or whose pointer resolves to no
    /// token, is refused before the case opens, exit 1.
    #[arg(long, value_name = "PATH", conflicts_with = "endpoint_token_file")]
    subscription_token_file: Option<PathBuf>,
    /// The old name of `--subscription-token-file`, kept for one minor release, with one line on
    /// standard error naming the new name.
    #[arg(long, value_name = "PATH")]
    endpoint_token_file: Option<PathBuf>,
    /// `--harness harness`: the JSON pointer to the token inside --subscription-token-file, given
    /// only with it; not given, the whole file is the token.
    #[arg(
        long,
        value_name = "JSON POINTER",
        conflicts_with = "endpoint_token_pointer"
    )]
    subscription_token_pointer: Option<String>,
    /// The old name of `--subscription-token-pointer`, kept for one minor release, with one line
    /// on standard error naming the new name.
    #[arg(long, value_name = "JSON POINTER")]
    endpoint_token_pointer: Option<String>,
    /// `--harness harness`: the API key as a secret's name in the owner's secrets store,
    /// `<namespace>/<name>`, handed to b10x-harness by name, which the harness resolves; takt
    /// reads no secret and opens no store. Not with --subscription-token-file.
    #[arg(
        long,
        value_name = "NAMESPACE/NAME",
        conflicts_with = "endpoint_secret"
    )]
    api_key_secret: Option<String>,
    /// The old name of `--api-key-secret`, kept for one minor release, with one line on standard
    /// error naming the new name.
    #[arg(long, value_name = "NAMESPACE/NAME")]
    endpoint_secret: Option<String>,
    /// `--harness harness`: the name of the environment variable holding the API key, on the
    /// openai-responses wire only (--endpoint-wire openai-responses) until the session
    /// executor can pass an API-key file. takt takes it out of its own environment before it
    /// starts anything, so no process it starts sees it, and on Linux with glibc overwrites
    /// its value in takt's initial environment block, which /proc/<pid>/environ reads. It
    /// stages the value as a token file of mode 0600 in a new directory of mode 0700 under the
    /// first of `$XDG_RUNTIME_DIR`, `/dev/shm` and the temporary directory that b10x-harness's
    /// token-file reader accepts, removed when the run ends, by SIGTERM, SIGINT or SIGHUP too
    /// (SIGKILL leaves it). Not with --endpoint-token-file or
    /// --endpoint-secret. A name that is no portable variable name, another wire, a variable
    /// unset or empty, or no directory accepted, is refused before the case opens, exit 1.
    #[arg(long, value_name = "NAME")]
    endpoint_token_env: Option<String>,
    /// `--harness harness`: a TOML file of the same fields, each under a flag: `base_url`,
    /// `wire`, `model`, and a `[credential]` table of `token_file` with an optional
    /// `token_pointer` (a relative path read beside the file), of `secret`, or of `env`.
    #[arg(long, value_name = "PATH")]
    endpoint_file: Option<PathBuf>,
    /// The value of the variable the credential names, taken out of takt's environment
    /// ([`Self::take_variable`]).
    #[arg(skip)]
    variable: Option<takt::harness::VariableValue>,
}

impl EndpointArgs {
    /// The first `--endpoint-*` flag given, by its name.
    fn given(&self) -> Option<&'static str> {
        self.all_given().into_iter().next()
    }

    /// Every endpoint flag given, under the name it was given by, in the order the flags are
    /// listed. `--endpoint-model` is `--model` and is not among them.
    fn all_given(&self) -> Vec<&'static str> {
        [
            ("--model-endpoint", self.model_endpoint.is_some()),
            ("--endpoint-base-url", self.endpoint_base_url.is_some()),
            ("--model-wire", self.model_wire.is_some()),
            ("--endpoint-wire", self.endpoint_wire.is_some()),
            (
                "--subscription-token-file",
                self.subscription_token_file.is_some(),
            ),
            ("--endpoint-token-file", self.endpoint_token_file.is_some()),
            (
                "--subscription-token-pointer",
                self.subscription_token_pointer.is_some(),
            ),
            (
                "--endpoint-token-pointer",
                self.endpoint_token_pointer.is_some(),
            ),
            ("--api-key-secret", self.api_key_secret.is_some()),
            ("--endpoint-secret", self.endpoint_secret.is_some()),
            ("--endpoint-token-env", self.endpoint_token_env.is_some()),
            ("--endpoint-file", self.endpoint_file.is_some()),
        ]
        .into_iter()
        .filter_map(|(flag, given)| given.then_some(flag))
        .collect()
    }

    /// The fields the flags give: each new name's value, else its old name's. The model is not
    /// among them: `--model` names it, and the file's `model` is the endpoint's.
    fn fields(&self) -> takt::harness::EndpointFields {
        takt::harness::EndpointFields {
            base_url: self
                .model_endpoint
                .clone()
                .or_else(|| self.endpoint_base_url.clone()),
            wire: self
                .model_wire
                .clone()
                .or_else(|| self.endpoint_wire.clone()),
            model: None,
            token_file: self
                .subscription_token_file
                .clone()
                .or_else(|| self.endpoint_token_file.clone()),
            token_pointer: self
                .subscription_token_pointer
                .clone()
                .or_else(|| self.endpoint_token_pointer.clone()),
            secret: self
                .api_key_secret
                .clone()
                .or_else(|| self.endpoint_secret.clone()),
            env: self.endpoint_token_env.clone(),
        }
    }

    /// Takes the variable the credential names, the flags' or else the endpoint file's, out of
    /// takt's environment and keeps its value, so no process takt starts inherits it. A name
    /// that is no portable variable name, or a file that does not read, takes nothing; the
    /// endpoint's own reading refuses it before any case opens. Called only while takt runs one
    /// thread.
    fn take_variable(&mut self) {
        use takt::harness::{
            EndpointFields, VariableValue, portable_variable_name, take_environment_variable,
        };
        let file = self
            .endpoint_file
            .as_deref()
            .and_then(|path| EndpointFields::read(path).ok());
        let flags = self.fields();
        let Some(name) = EndpointFields::variable(&flags, file.as_ref())
            .filter(|name| portable_variable_name(name))
            .map(str::to_owned)
        else {
            return;
        };
        // SAFETY: takt has started no thread yet (its callers run before any is started), so
        // nothing reads the environment while it changes; the name is non-empty and holds no
        // `=` or NUL.
        let value: Option<VariableValue> = unsafe { take_environment_variable(&name) };
        if self.variable.is_none() {
            self.variable = value;
        }
    }

    /// The endpoint the session is pointed at: the flags over the file over the default under
    /// `home`, a token file made absolute and checked to hold a token where its pointer points,
    /// a secret's name checked by no read, a variable's name checked to be one, before any case
    /// opens.
    fn config(&self, home: Option<&Path>) -> Result<takt::harness::EndpointConfig> {
        use takt::harness::{EndpointConfig, EndpointFields};
        let flags = self.fields();
        let file = self
            .endpoint_file
            .as_deref()
            .map(EndpointFields::read)
            .transpose()?;
        let config = EndpointConfig::of(&flags, file.as_ref(), home)?;
        config.check_token()?;
        Ok(config)
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
/// route, the route a scenario's trial bases are fetched and created through and the repository
/// it reads, which an improvement turn records and reads its mainline from.
struct Forged {
    forge: Option<Rc<dyn Forge>>,
    line: Option<String>,
    trials: Option<TrialRoute>,
    repository: Option<String>,
}

impl Forged {
    /// The mainline an improvement turn reads through this forge: the repository's `main` for
    /// the bot route, the forge's for another forge, the local `main` with none.
    fn mainline(&self) -> takt::improvement::Mainline {
        match (&self.forge, &self.repository) {
            (Some(forge), Some(repository)) => takt::improvement::Mainline::Repository {
                forge: forge.clone(),
                repository: repository.clone(),
            },
            (Some(forge), None) => takt::improvement::Mainline::Forge(forge.clone()),
            (None, _) => takt::improvement::Mainline::Local,
        }
    }
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
    /// The bot route the two lines are built by ([`BotRoute::trial_fetch_argv`],
    /// [`BotRoute::trial_push_argv`]): its remote, whose remote-tracking refs take the fetched
    /// branches and which the push goes to, and its HTTPS URL, the fetch's source.
    route: BotRoute,
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
        let mut route = BotRoute::new(
            bot.repository.clone(),
            bot.policy.clone(),
            Box::new(Process::default()),
        );
        route.remote.clone_from(&bot.remote);
        Self {
            route,
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
        self.route.trial_fetch_argv()
    }

    /// The line that creates the remote's `trial` from the local one, reporting each ref on
    /// stdout; the lease refuses it when the remote has the branch.
    fn push_argv(&self, trial: &str) -> Vec<String> {
        self.route.trial_push_argv(trial)
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

/// [`forge_of`] under what the run file says of the forge (`settings`): `forge: command` runs the
/// commands of its `forge_commands` section, an entry the protocol's `actions` need that is
/// missing refused here, before any case opens; `state_dir` (else a directory of this run under the
/// state home) holds the command forge's standard error files; the body file is the case's own,
/// in its artifacts. `forge: local` runs the run file's `local_checks` as its checks, refused
/// here without them, or when `origin` is no bare repository on this machine outside the tree,
/// or when `identity`, the run's commit identity its merge commits carry, is none; its checks'
/// clones go under `state_dir` as the command forge's files do, and run only with `min_free`
/// bytes free (the run's `--min-free-gib`). Settings that do not fit the forge are refused,
/// naming their keys.
#[allow(clippy::too_many_arguments)]
fn forge_with(
    repo: &Path,
    forge: Option<&str>,
    scan: &Scan,
    settings: &takt::run_config::ForgeSettings,
    actions: &[String],
    state_dir: Option<&Path>,
    identity: Option<takt::git::CommitIdentity>,
    min_free: u64,
) -> Result<Forged> {
    settings.check_for(forge)?;
    match forge.unwrap_or_default() {
        "local" => return local_forge(repo, settings, state_dir, identity, min_free),
        "command" => return command_forge(repo, scan, settings, actions, state_dir),
        _ => {}
    }
    if let Some(connectors) = ConnectorsForge::select(
        repo,
        forge,
        settings,
        scan.clone(),
        Box::new(Process::default()),
    )? {
        let line = format!(
            "forge: connectors ({}); pr.open, ci.read and repository.merge run connectors operations invoke",
            connectors.identity()
        );
        return Ok(Forged {
            forge: Some(Rc::new(connectors)),
            line: Some(line),
            trials: None,
            repository: None,
        });
    }
    if forge == Some("gitlab")
        && settings.transport == Some(takt::forge::gitlab::Transport::Api)
        && let Some(source) = &settings.credential
    {
        let glab = Glab::for_tree_api(repo, scan, source)?;
        let line = format!(
            "forge: gitlab ({} {}); pr.open, ci.read and repository.merge go through the GitLab API under {source}",
            glab.host, glab.project
        );
        return Ok(Forged {
            forge: Some(Rc::new(glab)),
            line: Some(line),
            trials: None,
            repository: None,
        });
    }
    forge_of(repo, forge, scan)
}

/// `forge: command` of [`forge_with`]: the commands of `settings`' `forge_commands`, checked
/// against the protocol's `actions`, their standard error under `state_dir`.
fn command_forge(
    repo: &Path,
    scan: &Scan,
    settings: &takt::run_config::ForgeSettings,
    actions: &[String],
    state_dir: Option<&Path>,
) -> Result<Forged> {
    let commands = settings.commands.clone().unwrap_or_default();
    let state = match state_dir {
        Some(dir) => state_under(dir, "forge-command"),
        None => takt::promotion::store()?
            .with_file_name("forge-command")
            .join(run_directory()),
    };
    let command = CommandForge::for_tree(
        repo,
        commands,
        actions,
        state,
        scan.clone(),
        Box::new(Process::default()),
    )?;
    Ok(Forged {
        forge: Some(Rc::new(command)),
        line: Some(
            "forge: command; pr.open, ci.read and repository.merge run the commands of the run file's forge_commands"
                .to_owned(),
        ),
        trials: None,
        repository: None,
    })
}

/// `forge: local` of [`forge_with`]: the hub `origin` names ([`Local::hub_of`]), the checks
/// `settings`' `local_checks` run as the test command is, held to [`Local::CHECKS_LIMITS`], their
/// repositories under `state_dir`, not run under `min_free` bytes free, the merge commits under
/// `identity`; refused, before any case opens, without an identity, or with checks whose
/// program the run's confinement cannot run as a test command (under `substrate`, anything but
/// `cargo`).
fn local_forge(
    repo: &Path,
    settings: &takt::run_config::ForgeSettings,
    state_dir: Option<&Path>,
    identity: Option<takt::git::CommitIdentity>,
    min_free: u64,
) -> Result<Forged> {
    let checks = settings.local_checks.clone().unwrap_or_default();
    if let Some(program) = checks.first() {
        let confinement = takt::confinement::current();
        confinement.admit_test(program).map_err(|error| {
            anyhow!(
                "--forge local runs `local_checks` as the test command is, and confinement {} cannot run its program `{program}`: {error:#}; nothing was opened",
                confinement.name()
            )
        })?;
    }
    let hub = Local::hub_of(repo)?;
    let identity = identity.ok_or_else(|| {
        anyhow!(
            "--forge local makes each merge commit in the hub {} under the run's commit identity, and there is none: give --commit-identity '<name> <email>', or `identity.name` and `identity.email` in {}; nothing was opened",
            hub.display(),
            takt::git::COMMIT_IDENTITY_FILE
        )
    })?;
    let state = match state_dir {
        Some(dir) => state_under(dir, "forge-local"),
        None => takt::promotion::store()?
            .with_file_name("forge-local")
            .join(run_directory()),
    };
    let local = Local::for_tree(repo, checks, state, identity)?.with_min_free(min_free);
    let line = format!(
        "forge: local ({}); pr.open, ci.read and repository.merge go through the hub on this machine, the checks are the run file's local_checks, run as the test command is",
        local.hub().display()
    );
    Ok(Forged {
        forge: Some(Rc::new(local)),
        line: Some(line),
        trials: None,
        repository: None,
    })
}

/// The forge: the bot route when asked for, or by default when `origin` is a repository on
/// github.com, the host the bot route serves; `glab` when asked for, or by default when `origin`
/// is on another host and `glab` is logged in to it. `--forge bot` elsewhere, `--forge gitlab` on
/// github.com or without a login are refused in one line before any case is opened; by default
/// another host without a login gets no forge, said in one line once the case is open. `glab`
/// scans through the run's scan adapter `scan`; the bot route scans as it always does.
fn forge_of(repo: &Path, forge: Option<&str>, scan: &Scan) -> Result<Forged> {
    forge_through(repo, forge, &|tree| Glab::for_tree(tree, scan))
}

/// [`forge_of`], the `glab` adapter of a tree, and its login check, made by `glab_of`.
fn forge_through(
    repo: &Path,
    forge: Option<&str>,
    glab_of: &dyn Fn(&Path) -> Result<Glab>,
) -> Result<Forged> {
    let bot = |repo: &Path| -> Result<Forged> {
        let bot = BotRoute::for_tree(repo)?;
        bot.preflight()?;
        let line = format!(
            "forge: bot ({}); pr.open, ci.read and repository.merge go through the bot route",
            bot.repository
        );
        let trials = TrialRoute::of(&bot);
        let repository = bot.repository.clone();
        Ok(Forged {
            forge: Some(Rc::new(bot)),
            line: Some(line),
            trials: Some(trials),
            repository: Some(repository),
        })
    };
    let gitlab = |glab: Glab| -> Forged {
        let line = format!(
            "forge: gitlab ({} {}); pr.open, ci.read and repository.merge go through glab",
            glab.host, glab.project
        );
        Forged {
            forge: Some(Rc::new(glab)),
            line: Some(line),
            trials: None,
            repository: None,
        }
    };
    let none = |line: Option<String>| Forged {
        forge: None,
        line,
        trials: None,
        repository: None,
    };
    Ok(match forge {
        Some("none") => none(None),
        Some("bot") => bot(repo)?,
        Some("gitlab") => gitlab(glab_of(repo)?),
        None => match takt::git::remote_url(repo, "origin")
            .ok()
            .as_deref()
            .map(repository_of)
        {
            None => none(None),
            Some(Some(origin)) if BotRoute::serves(&origin.host) => bot(repo)?,
            Some(Some(origin)) => match glab_of(repo) {
                Ok(glab) => gitlab(glab),
                Err(error) => none(Some(format!(
                    "forge: none (origin {} is not {}, and glab is not logged in to it: {})",
                    origin.host,
                    BotRoute::HOST,
                    one_line(&format!("{error:#}"))
                ))),
            },
            Some(None) => none(Some(
                "forge: none (origin names no <owner>/<repo> on a host)".to_owned(),
            )),
        },
        Some(other) => {
            return Err(anyhow!(
                "no forge `{other}`: bot, gitlab, command, connectors, local or none"
            ));
        }
    })
}

/// The forge of a scenario run: [`forge_of`], but never `glab`. A scenario's trial base
/// `trial/<n>` is created on the remote only through the bot route ([`TrialRoute`]), so
/// `--forge gitlab`, `command`, `connectors` and `local` are refused before anything is fetched,
/// cut or opened, and by default an `origin` on a host other than github.com gets no forge, said
/// in one line, `glab` never asked.
fn scenario_forge_of(repo: &Path, forge: Option<&str>, scan: &Scan) -> Result<Forged> {
    match forge {
        Some("gitlab") => Err(anyhow!(
            "--forge gitlab is not for a scenario run: its trial base trial/<n> is created on the remote only through the bot route, and glab opens a merge request against a base it cannot create; give --forge bot or --forge none; nothing was changed"
        )),
        Some("command") => Err(anyhow!(
            "--forge command is not for a scenario run: its trial base trial/<n> is created on the remote only through the bot route, which the configured commands cannot do; give --forge bot or --forge none; nothing was changed"
        )),
        Some("connectors") => Err(anyhow!(
            "--forge connectors is not for a scenario run: its trial base trial/<n> is created on the remote only through the bot route, which `connectors operations invoke` cannot do; give --forge bot or --forge none; nothing was changed"
        )),
        Some("local") => Err(anyhow!(
            "--forge local is not for a scenario run: its trial base trial/<n> is created on the remote only through the bot route, and a hub on this machine is no such remote; give --forge bot or --forge none; nothing was changed"
        )),
        None => match takt::git::remote_url(repo, "origin")
            .ok()
            .as_deref()
            .map(repository_of)
        {
            Some(Some(origin)) if !BotRoute::serves(&origin.host) => Ok(Forged {
                forge: None,
                line: Some(format!(
                    "forge: none (origin {} is not {}, and a scenario run creates its trial base only through the bot route)",
                    origin.host,
                    BotRoute::HOST
                )),
                trials: None,
                repository: None,
            }),
            _ => forge_of(repo, None, scan),
        },
        Some(_) => forge_of(repo, forge, scan),
    }
}

/// The transcripts directory under the state directory `dir`, `<dir>/transcripts`, a relative
/// `dir` read against the current directory.
fn state_transcripts(dir: &Path) -> PathBuf {
    state_under(dir, "transcripts")
}

/// The set-aside directory under the state directory `dir`, `<dir>/set-aside`, where what a
/// discarded step moved aside goes ([`state_transcripts`] for `dir`).
fn state_set_aside(dir: &Path) -> PathBuf {
    state_under(dir, "set-aside")
}

/// A directory name no other run on the machine has: the process and the instant it started.
fn run_directory() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

/// `name` under the state directory `dir`, a relative `dir` read against the current directory.
fn state_under(dir: &Path, name: &str) -> PathBuf {
    std::path::absolute(dir)
        .unwrap_or_else(|_| dir.to_path_buf())
        .join(name)
}

/// Runs the open case `run` to its stop, `forge_line` said first, the run given this binary's
/// [`REVISION`] to record; the exit code. An error the loop ends on once the case is open, a
/// forge that refused the pull request among them, is a stop like any other: one line on
/// stdout, its own code. Every run the CLI makes stops `Unperformable` when no step can move
/// its case ([`Run::stop_when_unperformable`], off in the library until a caller turns it on),
/// and `DiskLow` before a step when the tree's or `CARGO_TARGET_DIR`'s file system has less
/// free than `--min-free-gib` ([`Run::stop_when_disk_below`]). The run goes under `scan`
/// ([`Run::scan_with`]); a declared `none` is said in one line after the forge's, before the
/// run's first line. `records` is the profile the case keeps its decisions under when one is
/// given ([`Run::keep_records`]); none keeps the run's own.
fn finish(
    run: &mut Run<'_>,
    forge_line: Option<&str>,
    scan: Scan,
    looping: &LoopArgs,
    records: Option<takt::case::Records>,
    out: &mut dyn Write,
) -> Result<u8> {
    if let Some(line) = forge_line {
        writeln!(out, "{line}")?;
    }
    if let Some(line) = scan.declared_line(&looping.harness) {
        writeln!(out, "{line}")?;
    }
    run.scan_with(scan);
    run.binary = Some(REVISION.to_owned());
    run.llm_digest = looping.llm_digest.clone();
    // The transcripts directory is made now, before any step; one that cannot be (a read-only
    // home, a file on the way) keeps no transcript, said on standard error, and the run goes on
    // as a step whose transcript could not be kept does. --state-dir, or the configuration's
    // `state_dir`, holds them in place of the state home.
    // An explicit one was checked before the run opened ([`LoopArgs::check_state_dir`]).
    let transcripts = match &looping.state_dir {
        Some(dir) => Some(state_transcripts(dir)),
        None => takt::run::default_transcripts(),
    };
    if let Some(transcripts) = transcripts
        && let Err(error) = run.transcripts_in(transcripts)
    {
        eprintln!("no step transcript is kept: {error:#}");
    }
    run.take_default_gate();
    if let Some(seconds) = looping.active_seconds {
        run.cap_active_time(seconds);
    }
    run.stop_when_unperformable();
    run.console(console_of(
        looping,
        std::io::IsTerminal::is_terminal(&std::io::stdout()),
        std::env::var_os("NO_COLOR").as_deref(),
    ));
    if looping.continue_past_red_base {
        run.continue_past_red_base();
    }
    run.declare_setup(looping.setup.clone());
    if let Some(settings) = &looping.commit {
        run.commit_settings(settings.clone());
    }
    if let Some(records) = records {
        run.keep_records(records);
    }
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
                "stopped: error: {}; steps {}, tokens {}{}, case {}",
                one_line(&format!("{error:#}")),
                run.steps,
                run.tokens,
                stop_figures(run),
                run.case.dir.display()
            )?;
            return Ok(EXIT_ERROR);
        }
    };
    writeln!(
        out,
        "stopped: {stop}; steps {}, tokens {}{}, case {}",
        run.steps,
        run.tokens,
        stop_figures(run),
        run.case.dir.display()
    )?;
    Ok(match stop {
        Stop::Completed(_) => EXIT_DONE,
        Stop::ApprovalRequired(_) => EXIT_APPROVAL,
        Stop::Waiting { .. } => EXIT_WAITING,
        Stop::Unperformable { .. } => EXIT_UNPERFORMABLE,
        Stop::DiskLow { .. } => EXIT_DISK_LOW,
        Stop::HarnessUnqualified { .. } => EXIT_REFUSED,
        Stop::SetupFailed { .. } => EXIT_REFUSED,
        Stop::Declined(_) => EXIT_DECLINED,
        _ => EXIT_STOPPED,
    })
}

/// What the `stopped:` line names between its tokens and its case: the run's wall time, its cost
/// when a step reported one, and its commits with the paths they committed
/// ([`takt::console::stop_figures`]).
fn stop_figures(run: &Run<'_>) -> String {
    let (commits, paths) = run.commits();
    let wall = u64::try_from(run.elapsed().as_millis()).unwrap_or(u64::MAX);
    takt::console::stop_figures(wall, run.cost_usd(), commits, paths)
}

/// A scenario as the repository declares it in [`SCENARIOS`]: one story of its store, run from
/// a fresh base cut at the seed commit each time.
struct Scenario {
    id: String,
    story: String,
    seed: String,
    /// The section gives `red-base = expected`: the seed's tests fail on purpose, and the run
    /// goes on past its red base.
    red_base_expected: bool,
}

impl Scenario {
    /// The scenario `id` as [`SCENARIOS`] at `revision` declares it, read without a checkout.
    /// An id that cannot name a case and a branch, a file git does not read as its
    /// configuration syntax, a declaration that is missing, lacks `story` or `seed`, gives a
    /// key twice, gives a key takt does not read, or gives `red-base` another value than
    /// `expected`, is refused.
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
        let entries = takt::git::config_at(repo, revision, SCENARIOS)
            .map_err(|error| {
                let refused = format!(
                    "scenario {id}: {SCENARIOS} at {revision} does not read as git's configuration syntax"
                );
                // The spelling is named only to a file that gives it: git reads keys without
                // regard to case, so neither does the hint.
                let underscored = takt::git::blob(repo, &format!("{revision}:{SCENARIOS}"))
                    .is_ok_and(|text| {
                        String::from_utf8_lossy(&text)
                            .to_ascii_lowercase()
                            .contains("red_base")
                    });
                error.context(if underscored {
                    format!(
                        "{refused}, whose keys are letters, digits and `-` (`red-base`, not `red_base`)"
                    )
                } else {
                    refused
                })
            })?
            .ok_or_else(|| {
            anyhow!(
                "scenario {id}: {revision} holds no {SCENARIOS} in {}; the scenarios are declared there",
                repo.display()
            )
        })?;
        let prefix = format!("scenario.{id}.");
        let (mut story, mut seed, mut red_base) = (None, None, None);
        let mut declared = false;
        for (key, value) in entries {
            let Some(name) = key.strip_prefix(&prefix) else {
                continue;
            };
            declared = true;
            let slot = match name {
                "story" => &mut story,
                "seed" => &mut seed,
                "red-base" => &mut red_base,
                other => {
                    return Err(anyhow!(
                        "scenario {id}: {SCENARIOS} at {revision} gives `{other}`, which takt does not read; a scenario gives `story` and `seed`, and may give `red-base`"
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
        let red_base_expected = match red_base.as_deref() {
            None => false,
            Some("expected") => true,
            Some(other) => {
                return Err(anyhow!(
                    "scenario {id}: {SCENARIOS} at {revision} gives `red-base = {other}`; `red-base` takes one value, `expected`, for a seed whose tests fail on purpose; nothing was changed"
                ));
            }
        };
        Ok(Self {
            id: id.to_owned(),
            story: story.ok_or_else(|| missing("story"))?,
            seed: seed.ok_or_else(|| missing("seed"))?,
            red_base_expected,
        })
    }
}

/// The trial branches the checkout knows: every local `trial/…` and every remote's (as last
/// fetched), by full ref name.
fn known_trials(repo: &Path) -> Result<Vec<String>> {
    takt::git::refs(repo, &["refs/heads/trial", "refs/remotes"])
}

/// The `<n>` of the trial base `name` names, a full ref name `refs/heads/trial/<n>` or
/// `refs/remotes/<remote>/trial/<n>`; none for another ref, or a name under `trial/` that is
/// not all digits, which is no trial base.
fn trial_number(name: &str) -> Option<&str> {
    let n = name.strip_prefix("refs/heads/trial/").or_else(|| {
        name.strip_prefix("refs/remotes/")
            .and_then(|rest| rest.split_once('/'))
            .and_then(|(_, branch)| branch.strip_prefix("trial/"))
    })?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(n)
}

/// The trial base a scenario run cuts next: one past the highest `trial/<n>` the checkout
/// knows, a local branch or a remote's (as last fetched), `0` when there is none. Names under
/// `trial/` that are not all digits are not trial bases. A number too large for `u128`, or a
/// highest number nothing follows, is refused: never skipped, never wrapped to a number in use.
/// Only a run through the bot route numbers its trial so ([`unique_trial`] otherwise).
fn next_trial(repo: &Path) -> Result<u128> {
    let known = known_trials(repo)?;
    let mut highest: Option<u128> = None;
    for name in &known {
        let Some(n) = trial_number(name) else {
            continue;
        };
        let n: u128 = n.parse().map_err(|_| {
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
fn trial_free(repo: &Path, scenario: &str, n: u128) -> Result<String> {
    let case_id = format!("scenario-{scenario}-trial-{n}");
    takt::case::check_id(&case_id)?;
    if let Some(what) = case_taken(repo, &case_id)? {
        return Err(anyhow!(
            "scenario {scenario}: {what} exists already, though no trial/{n} branch is known: the case of trial/{n} would collide with it; nothing was pushed"
        ));
    }
    Ok(case_id)
}

/// What of the case `case_id` is there already, its directory or its branch `takt/<case>`;
/// none when neither is. `case_id` is one [`takt::case::check_id`] accepted.
fn case_taken(repo: &Path, case_id: &str) -> Result<Option<String>> {
    if takt::case::CaseDir::locate(repo, case_id).exists() {
        return Ok(Some(format!("the case {case_id}")));
    }
    let branch = takt::case::Branch::name_of(case_id);
    if !takt::git::refs(repo, &[&format!("refs/heads/{branch}")])?.is_empty() {
        return Ok(Some(format!("the case's branch {branch}")));
    }
    Ok(None)
}

/// How many nonces a trial number without the bot route can take within one second: eight
/// digits.
const TRIAL_NONCES: u64 = 100_000_000;

/// The trial number and case id of a scenario run without the bot route, which has no remote
/// to reserve a number on: the instant `at` (`YYYY-MM-DDTHH:MM:SSZ`, as [`takt::now`] gives it)
/// to the second, its 14 digits, then an 8-digit nonce, 22 digits in all, past `u64`, so it
/// is counted in `u128`. Runs from different checkouts very likely take different numbers (two
/// runs in one second share one with a chance of 1 in 10^8) and their cases go into one
/// scorecard; not consecutive ones. The
/// nonce is `nonce` (random, [`trial_nonce`]) when that number is free in this checkout, else
/// the next free one after it, wrapping: a number whose trial branch is known, local or a
/// remote's, or whose case or case branch is there, is passed over. That, and `git checkout
/// -b` refusing a branch that exists, is the hard guard; it holds within a checkout only. A
/// case id [`takt::case::check_id`] refuses is refused, and so is a second whose every number
/// is taken, before any branch is cut.
fn unique_trial(repo: &Path, scenario: &str, at: &str, nonce: u64) -> Result<(u128, String)> {
    let digits: String = at.chars().filter(char::is_ascii_digit).collect();
    let second: u128 = match digits.len() {
        14 => digits.parse().ok(),
        _ => None,
    }
    .ok_or_else(|| anyhow!("the instant {at} gives no second to number a trial by"))?;
    let known = known_trials(repo)?;
    for k in 0..TRIAL_NONCES {
        let n = second * u128::from(TRIAL_NONCES)
            + u128::from((nonce % TRIAL_NONCES + k) % TRIAL_NONCES);
        let case_id = format!("scenario-{scenario}-trial-{n}");
        takt::case::check_id(&case_id)?;
        let digits = n.to_string();
        if known
            .iter()
            .any(|name| trial_number(name) == Some(digits.as_str()))
        {
            continue;
        }
        if case_taken(repo, &case_id)?.is_none() {
            return Ok((n, case_id));
        }
    }
    Err(anyhow!(
        "scenario {scenario}: every trial number of {at} is taken in {}; nothing was changed",
        repo.display()
    ))
}

/// The nonce of a trial number without the bot route ([`unique_trial`]): eight random bytes
/// from the operating system (`/dev/urandom`), modulo [`TRIAL_NONCES`]. Nothing a run shares
/// with another goes into it: runs each in a PID namespace of its own have the same process id,
/// and containers started alike the same host name. Where `/dev/urandom` does not read, the
/// standard library's hasher, keyed from the operating system's randomness per process, over
/// the process id and the clock's nanoseconds.
fn trial_nonce() -> u64 {
    use std::hash::{BuildHasher as _, Hasher as _};
    use std::io::Read as _;
    let mut bytes = [0_u8; 8];
    let random = std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map(|()| u64::from_le_bytes(bytes))
        .unwrap_or_else(|_| {
            let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
            hasher.write_u32(std::process::id());
            hasher.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |since| since.as_nanos()),
            );
            hasher.finish()
        });
    random % TRIAL_NONCES
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
        config: _,
        mut looping,
    } = args;
    looping.check()?;
    let repo = repo
        .canonicalize()
        .with_context(|| format!("the repository {}", repo.display()))?;
    looping.confine(&repo)?;
    looping.check_set_aside(&repo)?;
    looping.check_state_dir(&repo)?;
    let scenario = Scenario::read(&repo, &declared_on, &scenario)?;
    let harness = harness_of(&looping)?;
    let harness = harness.as_ref();
    // A scenario run cannot declare `scan: none` (its --config refuses it), so its scan adapter is `gates`: a session
    // that may read outside its copy is refused without a policy before anything is cut.
    let scan = Scan::from_env();
    if harness.reads_outside_its_copy()
        && let Some(why) = scan.refused_at_open(&looping.harness, false)
    {
        return Err(anyhow!("scenario {}: {why}", scenario.id));
    }
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
    let forge = scenario_forge_of(&repo, looping.forge.as_deref(), &scan)?;
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
    let identity_source = if looping.commit_identity.is_some() {
        "--commit-identity".to_owned()
    } else {
        format!("{} at {declared_on}", takt::git::COMMIT_IDENTITY_FILE)
    };
    let scanned_identity = identity.clone();
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
    // Through the bot route the number is one past the highest known and the push's lease
    // reserves it on the remote; without it, no remote reserves anything, so the number is the
    // instant and a nonce, unique across checkouts.
    let (n, case_id) = if forge.trials.is_some() {
        let n = next_trial(&repo)?;
        (n, trial_free(&repo, &scenario.id, n)?)
    } else {
        unique_trial(&repo, &scenario.id, &takt::now(), trial_nonce())?
    };
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
        // Through the bot route the identity is scanned, as the push would scan it, before the
        // trial base is pushed or any step is taken.
        if let (Some(forge), Some(identity)) = (&forge.forge, &scanned_identity) {
            scan_identity_at_open(
                forge.as_ref(),
                &scan,
                &repo,
                identity,
                &identity_source,
                looping.state_dir.as_deref(),
                out,
            )
            .with_context(|| format!("scenario {}", scenario.id))?;
        }
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
    // The declaration only turns going on past a red base on: no flag or `--config` value
    // turns an expected red base back off. The line says what the run was told, not what the
    // base's tests do: a green seed may declare it too.
    if scenario.red_base_expected {
        looping.continue_past_red_base = true;
        writeln!(
            out,
            "red base: expected by scenario {} in {SCENARIOS} at {declared_on}: tests failing at the base do not stop the run",
            scenario.id
        )?;
    }
    let options = looping.options(SCENARIO_PROTOCOL.to_owned(), Vec::new(), Some(case_id));
    let store = Aep::default();
    let set_aside = looping.state_dir.as_deref().map(state_set_aside);
    let mut run = take_story(
        &repo,
        options,
        story,
        &commit_as,
        set_aside.as_deref(),
        harness,
        &validators,
        out,
    )?;
    // The case keeps the declaration, read at `--declared-on` and never from its tree, so a
    // resume with `takt run --case` goes on past its red base without the flag.
    if scenario.red_base_expected {
        run.case.record_red_base_expected()?;
    }
    // As `takt run`: a case that terminates accepted records its test result and its move in
    // the store, on the case's branch. A scenario run keeps every decision whatever `--records`
    // says, a resume of it too: the trial's record is what the scenario's scorecard reads, and
    // the library's rule for a scenario case says so (`Records::for_case`).
    run.record_on(&store);
    finish(&mut run, forge.line.as_deref(), scan, &looping, None, out)
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

/// The store's frontier narrowed to the one story `takt run --story` names: its candidates and
/// its active stories are the store's own, kept only when they are that story, and every other
/// answer and write is the store's, so [`Run::take_next_as`] takes it exactly as it takes the
/// frontier's next, or takes nothing when the frontier would not offer it.
struct Named<'a> {
    store: &'a Aep,
    story: &'a str,
}

impl takt::Frontier for Named<'_> {
    fn name(&self) -> &str {
        self.store.name()
    }

    fn candidates(&self, repo: &Path) -> Result<Vec<String>> {
        let mut candidates = self.store.candidates(repo)?;
        candidates.retain(|id| id == self.story);
        Ok(candidates)
    }

    fn active(&self, repo: &Path) -> Result<Vec<String>> {
        let mut active = self.store.active(repo)?;
        active.retain(|id| id == self.story);
        Ok(active)
    }

    fn show(&self, repo: &Path, id: &str) -> Result<takt::Story> {
        self.store.show(repo, id)
    }

    fn take(&self, repo: &Path, id: &str) -> Result<takt::frontier::Take> {
        self.store.take(repo, id)
    }

    fn record_approval(
        &self,
        repo: &Path,
        story: &str,
        approval: &takt::case::Approval,
    ) -> Result<()> {
        self.store.record_approval(repo, story, approval)
    }

    fn record_test_result(
        &self,
        repo: &Path,
        story: &str,
        reference: &str,
        at: &str,
    ) -> Result<takt::frontier::Written> {
        self.store.record_test_result(repo, story, reference, at)
    }

    fn implemented(&self, repo: &Path, story: &str) -> Result<takt::frontier::Written> {
        self.store.implemented(repo, story)
    }
}

/// The refusal of `takt run --story <story>` when the frontier narrowed to it took nothing and
/// opened no case: why, read from the store as the take read it (on the base), by the frontier's
/// own rule ([`takt::frontier::is_ready`], [`not_ready`]); `named` says what named the story
/// (`--story <story>`, or `story <story> from <file>`).
fn story_not_taken(repo: &Path, store: &Aep, story: &str, named: &str) -> Result<anyhow::Error> {
    let all = store.list(repo)?;
    let why = match all.iter().find(|listed| listed.id == story) {
        None => format!("the store lists no {story}"),
        Some(listed) if listed.kind != "story" => {
            format!("{story} is a {}, not a story", listed.kind)
        }
        Some(listed) if listed.status == takt::frontier::TAKEN => format!(
            "{story} is {}, and no open case of it is under the tree to resume",
            takt::frontier::TAKEN
        ),
        Some(listed) if listed.status != takt::frontier::READY => format!(
            "{story} is {}, not {}",
            listed.status,
            takt::frontier::READY
        ),
        Some(listed) if !takt::frontier::is_ready(listed, &all) => format!(
            "{story} is {} but not ready ({})",
            takt::frontier::READY,
            not_ready(listed, &all)
        ),
        Some(_) => format!("{story} is ready but was not taken, as the line above says"),
    };
    Ok(anyhow!(
        "{named}: {why}; the frontier would not take it, so no case was opened"
    ))
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
#[allow(clippy::too_many_arguments)]
fn take_story<'a>(
    repo: &Path,
    options: RunOptions,
    story: takt::Story,
    commit_as: &takt::case::CommitAs,
    set_aside: Option<&Path>,
    harness: &'a dyn Harness,
    validators: &'a [Box<dyn Validator>],
    out: &mut dyn Write,
) -> Result<Run<'a>> {
    let aep = Aep::default();
    let id = story.id.clone();
    let id = id.as_str();
    let title = story.title.clone();
    let mut run = Run::open_unit_in(
        repo,
        options,
        Unit::Story(story),
        commit_as,
        set_aside,
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

/// The running binary, what a turn's judge is read and checked from. On Linux it is
/// `/proc/self/exe`, the file the kernel holds as this process runs, whatever file its path names
/// by now. Elsewhere no such handle exists: it is the path the platform reports for this binary
/// (`std::env::current_exe`), and the file it names is read when the judge is checked, so a file
/// put at that path after the binary started is the one read and checked, not the one running.
/// A path the platform cannot report is an error, never a fallback. `cfg!`, not `#[cfg]`, so that both branches
/// are type-checked on every platform.
fn running_binary() -> Result<PathBuf> {
    if cfg!(target_os = "linux") {
        Ok(PathBuf::from("/proc/self/exe"))
    } else {
        std::env::current_exe().context(
            "resolve this binary's path (std::env::current_exe), what a turn's judge is read and checked from",
        )
    }
}

/// The protocol a scenario run opens its trial case on: a trial's case sets no input, so the one
/// that declares none.
const SCENARIO_PROTOCOL: &str = "story.delivery/1";

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
/// decision that does not read said on `out`; for a run given `--story`, that story's open case
/// alone, the first not finished in case order ([`takt::case::resumable`]), whatever other case
/// is open. `None` when the run opens a case, or when `--case` names none (`Run::resume`
/// refuses that). Nothing is checked out and nothing written.
fn case_to_resume(
    repo: &Path,
    case: Option<&str>,
    story: Option<&str>,
    intent: bool,
    out: &mut dyn Write,
) -> Result<Option<Resuming>> {
    let (id, story) = match (case, intent) {
        (Some(id), false) => (id.to_owned(), None),
        (None, false) => match story {
            Some(story) => match takt::case::resumable(repo, &[story.to_owned()], out)? {
                Some((id, story)) => (id, Some(story)),
                None => return Ok(None),
            },
            None => match Run::next_to_resume(repo, &Aep::default(), out)? {
                Some((id, story)) => (id, Some(story)),
                None => return Ok(None),
            },
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
    let dir = takt::case::CaseDir::locate(repo, id);
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

/// Refuses `--install-root <given>` for the case `id`, on `protocol`, about to be resumed, unless
/// it names the root the case keeps: a `self.improvement/1` turn's recorded install root
/// ([`takt::case::install_root_of`]), which `given` names when its canonical path is that root.
/// A case that is no turn, or a turn that recorded none, refuses any. `id` is one
/// [`takt::case::check_id`] accepted, as [`case_protocol`] read it.
fn resume_install_root(repo: &Path, id: &str, protocol: &str, given: &Path) -> Result<()> {
    let keeps = |what: String| {
        anyhow!(
            "--install-root is given for the case {id}, which keeps the install root it was opened with{what}; nothing was resumed"
        )
    };
    let recorded = if protocol == takt::case::TURN_PROTOCOL {
        takt::case::install_root_of(&takt::case::CaseDir::locate(repo, id))?
    } else {
        None
    };
    let Some(recorded) = recorded else {
        return Err(keeps(String::new()));
    };
    match std::fs::canonicalize(given) {
        Ok(canonical) if canonical == recorded => Ok(()),
        Ok(canonical) => Err(keeps(format!(
            ", {}; --install-root {} names {}, another root",
            recorded.display(),
            given.display(),
            canonical.display()
        ))),
        Err(error) => Err(keeps(format!(
            ", {}; --install-root {} is not there ({error})",
            recorded.display(),
            given.display()
        ))),
    }
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
    let recorded = takt::case::commit_identity_of(&takt::case::CaseDir::locate(repo, id))?;
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
        // No command closes or drops a case: the way past one is to finish it without pushing,
        // or to remove its branch and its directory, after which no run finds it and its story
        // is taken again, the take having been committed on that branch only.
        let dir = takt::case::CaseDir::locate(repo, id);
        let base = takt::case::Branch::read(&dir)
            .ok()
            .and_then(|branch| branch.base)
            .unwrap_or_else(|| "its base branch".to_owned());
        let branch = takt::case::Branch::name_of(id);
        return Err(anyhow!(
            "no commit identity: the case {id} was opened with none, so its commits are {}, and this run's commits leave the machine (the bot route); a case pushes only under the identity it was opened with. Finish it without pushing (`takt run --case {id} --forge none`), or drop it: `git -C {} checkout {base}`, `git -C {} branch -D {branch}` and remove {}; the next run then takes its story again under {} (`identity.name`, `identity.email`) at the base, or under --commit-identity '<name> <email>'; nothing was resumed",
            takt::git::CommitIdentity::takt(),
            repo.display(),
            repo.display(),
            dir.display(),
            takt::git::COMMIT_IDENTITY_FILE
        ));
    }
    Ok(())
}

/// The identity a `takt run` in `repo` commits with, and where it comes from: `given`
/// (`--commit-identity`), else the one the resumed case `resuming` was opened with, else the one
/// the repository configures at the commit a new case forks from (the base of the case whose
/// branch the tree is on, or `HEAD`). `None` when there is none, or the commit is not found:
/// opening the case then refuses or commits as takt's own, as it did before this scan.
fn identity_at_open(
    repo: &Path,
    given: Option<&takt::git::CommitIdentity>,
    resuming: Option<&str>,
) -> Result<Option<(takt::git::CommitIdentity, String)>> {
    if let Some(given) = given {
        return Ok(Some((given.clone(), "--commit-identity".to_owned())));
    }
    if let Some(id) = resuming {
        let recorded = takt::case::commit_identity_of(&takt::case::CaseDir::locate(repo, id))?;
        return Ok(recorded.map(|identity| (identity, format!("the case {id}"))));
    }
    let from = takt::git::current_branch(repo)?
        .as_deref()
        .and_then(|branch| branch.strip_prefix(takt::case::BRANCH_PREFIX))
        .filter(|other| takt::case::check_id(other).is_ok())
        .and_then(|other| {
            takt::case::Branch::read(&takt::case::CaseDir::locate(repo, other))
                .ok()
                .and_then(|branch| branch.base)
        });
    let Ok(base) = takt::git::run(
        repo,
        &["rev-parse", "--verify", from.as_deref().unwrap_or("HEAD")],
    ) else {
        return Ok(None);
    };
    let at = &base[..12.min(base.len())];
    let source = format!("{} at {at}", takt::git::COMMIT_IDENTITY_FILE);
    Ok(takt::git::configured_identity(repo, &base)?.map(|identity| (identity, source)))
}

/// Scans `identity`, which a run's commits carry off the machine, taken from `source`, with the
/// scan `forge` runs before a push ([`Forge::scan_identity`]), when the run opens: before step
/// 1, before its case opens or its trial base is cut. The text scanned is kept under the state
/// directory while the scan runs, never in `repo`. A refusal ends the run, naming the scan's;
/// a scan that cannot run (no policy), or none under the run's declared `scan: none` (`scan`),
/// says "not scanned" on `out`, and the run opens.
fn scan_identity_at_open(
    forge: &dyn Forge,
    scan: &Scan,
    repo: &Path,
    identity: &takt::git::CommitIdentity,
    source: &str,
    state_dir: Option<&Path>,
    out: &mut dyn Write,
) -> Result<()> {
    let root = match state_dir {
        Some(dir) => state_under(dir, "identity-scans"),
        None => takt::promotion::store()?.with_file_name("identity-scans"),
    };
    let dir = root.join(std::process::id().to_string());
    let scanned = forge.scan_identity(repo, &dir, identity);
    let _ = std::fs::remove_dir_all(&dir);
    let scanned = scanned.with_context(|| {
        format!(
            "{source} gives the commit identity {identity}, which this run's commits would carry off the machine, and it did not pass the bot route's scan; give an identity the scan passes (--commit-identity '<name> <email>', or `identity.name` and `identity.email` in {}); no step was taken, nothing was written or pushed",
            takt::git::COMMIT_IDENTITY_FILE
        )
    })?;
    if let takt::forge::IdentityScan::NotScanned(why) = scanned {
        // Under a declared `none` a forge that scans through the run's adapter scans nothing
        // before the push either, nor does the local forge, whose pushes stay on the machine;
        // the bot route scans whatever is declared.
        let after = if forge.name() == "local" || scan.not_scanned().as_ref() == Some(&why) {
            "nothing is scanned before the push either"
        } else {
            "the scan before the push still applies"
        };
        writeln!(
            out,
            "commit identity: {identity} from {source} not scanned ({why}); {after}"
        )?;
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

#[cfg(test)]
#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

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

    /// Whether the run printed `action` admissible: listed on its `may:` line, or added on a
    /// `may now:` line.
    fn may(out: &str, action: &str) -> bool {
        out.lines().any(|line| {
            line.strip_prefix("  may: ")
                .is_some_and(|actions| actions.split_whitespace().any(|a| a == action))
                || line.strip_prefix("  may now: ").is_some_and(|changes| {
                    changes
                        .split_whitespace()
                        .any(|change| change.strip_prefix('+') == Some(action))
                })
        })
    }

    /// Runs `takt <args>` in this process; the exit code and what it printed.
    fn takt(args: &[&str]) -> (u8, String) {
        // These tests run takt in one process, side by side, under one home. The first run makes
        // the transcripts directory; on a home without `.local` another run's step would then see
        // `.local` appear outside its tree and be refused. Make it once, before any run.
        static TRANSCRIPTS: std::sync::Once = std::sync::Once::new();
        TRANSCRIPTS.call_once(|| {
            let state = std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
                });
            if let Some(state) = state {
                let _ = takt::harness::make_transcripts_root(
                    &state.join("takt/transcripts"),
                    &mut Vec::new(),
                );
            }
        });
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
        let mut cases: Vec<PathBuf> = fs::read_dir(repo.join(".takt/cases"))
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
            "--input",
            "gate_on=each_revision",
        ]);
        assert!(out.contains("frontier: took story:ready-one"), "{out}");
        // After the tests, the gate is left with no gates document to run it: the run stops
        // there ahead of its step budget, exit 7.
        assert!(
            out.contains(
                "stopped: Unperformable (gate.green; nothing performs it without the gates document)"
            ),
            "{out}"
        );
        assert_eq!(code, 7, "{out}");
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
        assert!(!repo.join(".takt/cases").exists());
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
        // The loop says Canon's action frontier as `may:`; the store's lines are these.
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
            "--verbose",
        ]);
        assert_eq!(code, 7, "{out}");
        assert!(out.contains("frontier: gate.run (admissible)"), "{out}");
        assert!(may(&out, "gate.run"), "{out}");
        assert!(
            out.contains(
                "stopped: Unperformable (gate.green; nothing performs it without the gates document); steps 0, tokens 0"
            ),
            "{out}"
        );
    }

    /// The delivery protocol, the engineering protocols' built-in, which declares `pr.open` and
    /// `ci.read`.
    const DELIVERY_PROTOCOL: &str = "story.delivery/1";

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

    #[test]
    fn forge_bot_without_an_origin_is_refused_before_the_run() {
        let repo = gated_repository("forge-bot-no-origin", None);
        let error = takt_error(&delivery_args(
            repo.to_str().expect("utf-8"),
            "bot",
            &["--forge", "bot"],
        ));
        assert!(error.contains("origin"), "{error}");
        assert!(!repo.join(".takt/cases").exists());
    }

    /// The bot route pushes and reads under a GitHub App: asked for on another
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
        assert!(!repo.join(".takt/cases").exists());
    }

    /// Answers glab's login check as a glab with no login does, exit 1, and keeps each line
    /// it was asked: the host's own glab is never run.
    struct NotLoggedIn(std::rc::Rc<std::cell::RefCell<Vec<Vec<String>>>>);

    impl takt::forge::Runner for NotLoggedIn {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<takt::forge::Exit> {
            self.run_env(tree, argv, &[])
        }

        fn run_env(
            &self,
            _tree: &Path,
            argv: &[String],
            _env: &[(&str, &str)],
        ) -> anyhow::Result<takt::forge::Exit> {
            self.0.borrow_mut().push(argv.to_vec());
            Ok(takt::forge::Exit {
                code: Some(1),
                stdout: String::new(),
                stderr: "gitlab.example.com has not been authenticated with glab".to_owned(),
            })
        }
    }

    /// Without `--forge`, an origin on another host that glab is not logged in to gets no
    /// forge, said in one line naming what glab said. glab's login check runs through a double
    /// (`forge_through`); the run that prints the line is `forge_cli`'s, under a glab double on
    /// its `PATH`.
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
        let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let glab_of = |tree: &Path| {
            takt::forge::gitlab::Glab::for_tree_through(
                tree,
                &takt::scan::Scan::from_env(),
                Box::new(NotLoggedIn(asked.clone())),
            )
        };
        let forged = super::forge_through(&repo, None, &glab_of).expect("no forge is no error");
        assert!(forged.forge.is_none() && forged.trials.is_none());
        let line = forged.line.expect("the line that says so");
        assert!(
            line.starts_with(
                "forge: none (origin gitlab.example.com is not github.com, and glab is not logged in to it: "
            ) && line.contains("has not been authenticated with glab"),
            "{line}"
        );
        assert_eq!(
            *asked.borrow(),
            [["glab", "auth", "status", "--hostname", "gitlab.example.com"].map(str::to_owned)]
        );
    }

    #[test]
    fn an_unknown_forge_is_refused_before_the_run() {
        let repo = gated_repository("forge-unknown", None);
        let error = takt_error(&delivery_args(
            repo.to_str().expect("utf-8"),
            "unknown",
            &["--forge", "hub"],
        ));
        assert_eq!(
            error,
            "no forge `hub`: bot, gitlab, command, connectors, local or none"
        );
        assert!(!repo.join(".takt/cases").exists());
    }

    /// story:a-command-forge-runs-the-configured-commands: `--forge command` with no
    /// `forge_commands` in the run file is refused before the run opens, naming the section.
    #[test]
    fn the_command_forge_needs_its_section_before_the_run() {
        let repo = gated_repository("forge-command-none", None);
        let error = takt_error(&delivery_args(
            repo.to_str().expect("utf-8"),
            "command-none",
            &["--forge", "command"],
        ));
        assert!(error.contains("forge_commands"), "{error}");
        assert!(!repo.join(".takt/cases").exists());
    }

    /// An entry the protocol's actions need and the section lacks is refused naming it, by the
    /// command forge's constructor the run goes through before any case opens.
    #[test]
    fn the_command_forge_refuses_a_missing_entry_by_name() {
        let repo = gated_repository("forge-command-missing", None);
        let tool = vec!["tool".to_owned()];
        let settings = takt::run_config::ForgeSettings {
            commands: Some(takt::forge::command::ForgeCommands {
                open: Some(tool.clone()),
                read: Some(tool.clone()),
                checks: Some(tool),
                ..Default::default()
            }),
            ..Default::default()
        };
        let actions = takt::protocol::LoadedProtocol::builtin("story.delivery/1")
            .expect("a built-in")
            .actions;
        let state = repo.join("forge-state");
        let error = super::forge_with(
            &repo,
            Some("command"),
            &takt::scan::Scan::from_env(),
            &settings,
            &actions,
            Some(&state),
            None,
            0,
        )
        .err()
        .expect("refused");
        assert!(format!("{error:#}").contains("`merge`"), "{error:#}");
        assert!(!state.exists());
        let error = super::scenario_forge_of(&repo, Some("command"), &takt::scan::Scan::from_env())
            .err()
            .expect("refused");
        assert!(
            format!("{error:#}").contains("--forge command"),
            "{error:#}"
        );
    }

    /// story:local-forge-merges-into-a-target-branch: a scenario run refuses `--forge local`
    /// before anything is fetched, cut or opened: its trial base is created only through the bot
    /// route.
    #[test]
    fn the_local_forge_is_refused_for_a_scenario_run() {
        let repo = gated_repository("forge-local-scenario", None);
        let error = super::scenario_forge_of(&repo, Some("local"), &takt::scan::Scan::from_env())
            .err()
            .expect("refused");
        assert!(format!("{error:#}").contains("--forge local"), "{error:#}");
    }

    /// story:trial-numbers-are-unique-across-checkouts: without the bot route a trial number is
    /// the instant's fourteen digits, then the eight-digit nonce, 22 digits; a number whose
    /// trial branch is known (local, or a remote's), whose case is there, or whose case branch
    /// is, is passed over for the next nonce, past 99999999 to 00000000.
    #[test]
    fn a_trial_number_without_the_bot_route_is_the_instant_and_a_free_nonce() {
        let repo = repository("one-ready", "unique-trial");
        let at = "2026-10-19T14:25:30Z";
        let taken = |nonce: u64| {
            super::unique_trial(&repo, "1", at, nonce)
                .expect("a trial number")
                .0
        };
        assert_eq!(
            super::unique_trial(&repo, "1", at, 42).expect("a trial number"),
            (
                2_026_101_914_253_000_000_042,
                "scenario-1-trial-2026101914253000000042".to_owned()
            )
        );
        git(&repo, &["branch", "trial/2026101914253000000042"]);
        git(
            &repo,
            &[
                "update-ref",
                "refs/remotes/origin/trial/2026101914253000000043",
                "HEAD",
            ],
        );
        fs::create_dir_all(repo.join(".takt/cases/scenario-1-trial-2026101914253000000044"))
            .expect("a case directory");
        git(
            &repo,
            &["branch", "takt/scenario-1-trial-2026101914253000000045"],
        );
        assert_eq!(taken(42), 2_026_101_914_253_000_000_046);
        assert_eq!(taken(99_999_999), 2_026_101_914_253_099_999_999);
        git(&repo, &["branch", "trial/2026101914253099999999"]);
        assert_eq!(taken(99_999_999), 2_026_101_914_253_000_000_000);
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
    /// merge is no longer approval-required but admitted, the granted action `--harness none`
    /// takes (story:harness-none-takes-a-granted-action). A grant is given only to a case at the
    /// gate.
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
                &["status", "--porcelain", "--", ".", ":!.takt/cases"]
            ),
            ""
        );
        let (code, out) = takt(&resume);
        assert_eq!(
            code, 3,
            "story:harness-none-takes-a-granted-action: the granted merge is taken, and nothing moves after it: {out}"
        );
        assert!(
            out.contains(&format!(
                "frontier: resumed story:ready-one: Make the check pass (case {id})"
            )),
            "{out}"
        );
        assert!(!out.contains("refused"), "{out}");
        assert!(!out.contains("ApprovalRequired"), "{out}");
        assert!(may(&out, "repository.merge"), "{out}");
        assert!(
            out.contains("step 1: repository.merge (none)"),
            "story:harness-none-takes-a-granted-action: the granted merge is the harness's step: {out}"
        );
        assert!(
            out.contains("stopped: NoProgress"),
            "story:harness-none-takes-a-granted-action: no forge performs the merge, so nothing moves: {out}"
        );
        assert!(
            !out.contains("no harness: a judgement step"),
            "story:harness-none-takes-a-granted-action: the granted merge is not left: {out}"
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
        assert!(!repo.join(".takt/cases").exists());
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
        assert!(!repo.join(".takt/cases").exists());
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
        assert!(!repo.join(".takt/cases").exists());
    }

    /// The console `takt run <args>` asks for, its standard output a terminal or not
    /// (`terminal`) and `NO_COLOR` holding `no_color`.
    fn console(args: &[&str], terminal: bool, no_color: Option<&str>) -> takt::console::Console {
        let cli = Cli::try_parse_from(
            ["takt", "run", "--repo", "/nowhere"]
                .into_iter()
                .chain(args.iter().copied()),
        )
        .expect("the flags parse");
        let super::Commands::Run(run) = cli.command else {
            panic!("not a run");
        };
        super::console_of(&run.looping, terminal, no_color.map(std::ffi::OsStr::new))
    }

    /// story:console-shows-reads-and-diffs, acceptance 3: `--color never` and `NO_COLOR=1`
    /// under the default `auto` are no colour, a terminal under `auto` is, `--color always` is
    /// even piped; `--quiet-steps` turns both listings off and leaves the rest.
    #[test]
    fn color_and_quiet_steps_set_the_console() {
        let auto = console(&[], true, None);
        assert!(auto.color && auto.reads && auto.diff && !auto.verbose);
        assert!(!console(&["--color", "never"], true, None).color);
        assert!(!console(&[], true, Some("1")).color);
        assert!(!console(&["--color", "auto"], false, None).color);
        assert!(console(&["--color", "always"], false, Some("1")).color);
        let quiet = console(&["--quiet-steps", "--verbose"], false, None);
        assert!(!quiet.reads && !quiet.diff && quiet.verbose && !quiet.color);
        assert!(
            Cli::try_parse_from(["takt", "run", "--repo", "/nowhere", "--color", "sometimes"])
                .is_err()
        );
    }
}
