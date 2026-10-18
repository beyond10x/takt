//! `takt`: drive a repository's work through engineering protocols, every step validated.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use anyhow::{Context as _, Result, anyhow};
use clap::{Parser, Subcommand};
use takt::forge::{BotRoute, Forge, repository_of};
use takt::frontier::Aep;
use takt::harness::{ClaudeCode, Harness};
use takt::validator::{CiRead, DEFAULT_PROJECT, GateRun, PrMerged, PrOpen, TestsRun, Validator};
use takt::{LoadedProtocol, Run, RunOptions, Stop, Unit};

#[derive(Parser)]
#[command(
    name = "takt",
    version,
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
    /// approval on the case's story in the planning store. The next run resumes the case.
    Approve(ApproveArgs),
    /// List the built-in protocols.
    Protocols,
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
    /// Who grants it.
    #[arg(long)]
    by: String,
}

#[derive(clap::Args)]
struct RunArgs {
    /// The repository worktree.
    #[arg(long)]
    repo: PathBuf,
    /// What the run is meant to achieve; without it and without --case, the open case of a
    /// story the planning store lists active is resumed, else the next ready story is the unit
    /// and the store records it taken.
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
    /// is refused before the case is opened.
    #[arg(long = "input", value_name = "NAME=VALUE")]
    inputs: Vec<String>,
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
    /// The case id; generated when absent.
    #[arg(long)]
    case_id: Option<String>,
    /// The approval gate the harness may request to end the run.
    #[arg(long, default_value = "repository.merge")]
    gate: String,
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
}

/// A harness that performs nothing. Asked while an approval is pending, it requests the gate
/// action, changing no file, so the run stops `ApprovalRequired` and `takt approve` is the
/// next step; asked for a judgement step with no approval pending, it ends the run with an
/// error, `stopped: error:`, because nothing performs that step.
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
/// Exit 3: the run stopped on a budget or with nothing more it may do.
const EXIT_STOPPED: u8 = 3;
/// Exit 4: the run stopped for an approval; `takt approve` is the next step.
const EXIT_APPROVAL: u8 = 4;
/// Exit 5: the run ended on an error once its case was open, `stopped: error:`.
const EXIT_ERROR: u8 = 5;

/// What `takt run --help` says about the exit codes; each line opens with its code.
const EXIT_CODES: &str = "Exit codes:
  0  stopped: Completed (an outcome is legitimate), or frontier: nothing ready
  1  refused before the run (an argument, the repository, the protocol or the case); the reason on stderr, no step run
  2  the arguments do not parse
  3  stopped: a budget (Iterations, Hours, Tokens), NothingAdmissible, StepRefused or NoProgress
  4  stopped: ApprovalRequired; `takt approve` grants it and the next run resumes the case
  5  stopped: error: the run ended on an error once its case was open (a forge that refused, a validator or the harness that failed); the case stays open";

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
/// | 3 | any other stop: a budget, `NothingAdmissible`, `StepRefused`, `NoProgress` |
/// | 4 | `ApprovalRequired`: `takt approve` is the next step |
/// | 5 | `stopped: error:`, the run ended on an error once its case was open: a forge that refused, a validator or the harness that failed |
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
                    "recorded: nothing; the case was opened on an intent, not a story"
                )?,
            }
            Ok(EXIT_DONE)
        }
        Commands::Run(args) => {
            let RunArgs {
                repo,
                intent,
                case,
                protocol,
                inputs,
                iterations,
                hours,
                tokens,
                test_cmd,
                harness,
                model,
                case_id,
                gate,
                gates,
                project,
                forge,
            } = *args;
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
            // A case keeps only the inputs its protocol declares: one it does not declare would
            // be dropped without a word, so a case to be opened refuses it before anything is.
            // A resumed case reads its own inputs, never these.
            if case.is_none()
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
            let mut inputs = inputs;
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
            let options = RunOptions {
                protocol: opening,
                intent: String::new(),
                inputs,
                iterations,
                hours,
                tokens,
                case_id,
                gate: Some(gate),
            };
            let claude = ClaudeCode {
                model,
                ..ClaudeCode::default()
            };
            let harness: &dyn Harness = match harness.as_str() {
                "claude" => &claude,
                "none" => &NoHarness,
                other => return Err(anyhow!("no harness `{other}`: claude or none")),
            };
            let mut validators: Vec<Box<dyn Validator>> = vec![Box::new(TestsRun::new(&test_cmd)?)];
            let gates = repository_file(&repo, "gates", gates, DEFAULT_GATES)?;
            let project = repository_file(&repo, "project", project, DEFAULT_PROJECT)?;
            if let Some(gates) = gates {
                let mut gate = GateRun::new(gates);
                if let Some(project) = project {
                    gate = gate.with_project(project);
                }
                validators.push(Box::new(gate));
            }
            // The forge: the bot route when asked for, or by default when `origin` is a
            // repository on github.com, the host the bot route serves. `--forge bot` elsewhere
            // is refused in one line before any case is opened; by default another host gets
            // no forge, said in one line once the case is open.
            let bot = |repo: &Path| -> Result<(Option<BotRoute>, Option<String>)> {
                let bot = BotRoute::for_tree(repo)?;
                let line = format!(
                    "forge: bot ({}); pr.open, ci.read and repository.merge go through the bot route",
                    bot.repository
                );
                Ok((Some(bot), Some(line)))
            };
            let (forge, forge_line) = match forge.as_deref() {
                Some("none") => (None, None),
                Some("bot") => bot(&repo)?,
                None => match takt::git::remote_url(&repo, "origin")
                    .ok()
                    .as_deref()
                    .map(repository_of)
                {
                    None => (None, None),
                    Some(Some(origin)) if BotRoute::serves(&origin.host) => bot(&repo)?,
                    Some(Some(origin)) => (
                        None,
                        Some(format!(
                            "forge: none (origin {} is not {})",
                            origin.host,
                            BotRoute::HOST
                        )),
                    ),
                    Some(None) => (
                        None,
                        Some("forge: none (origin names no <owner>/<repo> on a host)".to_owned()),
                    ),
                },
                Some(other) => return Err(anyhow!("no forge `{other}`: bot or none")),
            };
            if let Some(forge) = forge {
                let forge: Rc<dyn Forge> = Rc::new(forge);
                validators.push(Box::new(PrOpen::new(forge.clone())));
                validators.push(Box::new(CiRead::new(forge.clone())));
                validators.push(Box::new(PrMerged::new(forge)));
            }
            // The unit: the case to resume, the intent given, or else the open case of an
            // active story or the next ready story of the planning store; a case is opened
            // before the store is told, so a run that cannot open its case takes nothing.
            let mut run = match (case, intent) {
                (Some(_), Some(_)) => {
                    return Err(anyhow!(
                        "--case resumes a case and --intent opens one; give one of them"
                    ));
                }
                (Some(case), None) => Run::resume(&repo, options, &case, harness, &validators)?,
                (None, Some(intent)) => {
                    Run::open_unit(&repo, options, Unit::Intent(intent), harness, &validators)?
                }
                (None, None) => match Run::open_next(
                    &repo,
                    options,
                    &Aep::default(),
                    harness,
                    &validators,
                    out,
                )? {
                    Some(run) => run,
                    None => {
                        writeln!(out, "frontier: nothing ready")?;
                        return Ok(EXIT_DONE);
                    }
                },
            };
            // A resumed case keeps the protocol it was opened on: a `--protocol` that names
            // another is refused here, after the case was read and before anything ran. A case
            // opened by this run is on the protocol given, by construction.
            if let (Some(given), Some(spec)) = (&given, &protocol) {
                refuse_another_protocol(&run.case, given, spec)?;
            }
            if let Some(line) = &forge_line {
                writeln!(out, "{line}")?;
            }
            // An error the loop ends on once the case is open, a forge that refused the pull
            // request among them, is a stop like any other: one line on stdout, its own code.
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
                _ => EXIT_STOPPED,
            })
        }
    }
}

/// The protocol a store story opens on when `--protocol` is not given.
const STORY_DELIVERY: &str = "story.delivery/1";
/// The protocol an `--intent` opens on when `--protocol` is not given.
const SOFTWARE_CHANGE: &str = "software.change/1";

/// Refuses `given`, named on the command line as `spec`, for `case` when it is not the protocol
/// the case runs on: another id, or another document under the same id.
fn refuse_another_protocol(case: &takt::CaseDir, given: &LoadedProtocol, spec: &str) -> Result<()> {
    let own = &case.protocol;
    if own.id != given.id {
        return Err(anyhow!(
            "the case {} runs on {}, not {} (--protocol {spec}): a case keeps the protocol it was opened on; resume it without --protocol; nothing was changed",
            case.id,
            own.id,
            given.id
        ));
    }
    if own.ir_json != given.ir_json {
        return Err(anyhow!(
            "the case {} runs on its own copy of {}, and --protocol {spec} is another document of {}: a case keeps the protocol it was opened on; resume it without --protocol; nothing was changed",
            case.id,
            own.id,
            given.id
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

    /// With no harness, the judgement step left ends the run: a `stopped: error:` line naming
    /// it, exit 5, as every error the loop ends on once the case is open.
    #[test]
    fn without_a_gates_document_gate_run_is_a_judgement_step() {
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
        assert_eq!(code, 5, "{out}");
        assert!(
            out.contains("stopped: error: no harness: a judgement step among gate.run is left"),
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

    /// Without `--forge` and without an origin, neither forge validator is registered: after
    /// `tests.run`, `ci.read` is among the judgement steps left to the harness.
    #[test]
    fn without_an_origin_the_forge_is_none_and_ci_read_is_a_judgement_step() {
        let repo = gated_repository("forge-default-none", None);
        let (code, out) = takt(&delivery_args(
            repo.to_str().expect("utf-8"),
            "default-none",
            &[],
        ));
        assert_eq!(code, 5, "a judgement step is left: {out}");
        assert!(!out.contains("forge:"), "{out}");
        assert!(out.contains("step 1: tests.run {}"), "{out}");
        assert!(
            out.contains(
                "stopped: error: no harness: a judgement step among ci.read, gate.run, implementation.edit"
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
        assert_eq!(code, 5, "a judgement step is left: {out}");
        assert!(!out.contains("forge:"), "{out}");
        assert!(
            out.contains(
                "stopped: error: no harness: a judgement step among ci.read, gate.run, implementation.edit"
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
            &["--iterations", "1"],
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

    /// `takt run` with no intent takes the ready story and, stopped at once by a step budget
    /// of zero, leaves its case open; `takt approve` writes the grant into that case and
    /// records the approval on the story; the next `takt run` with no intent resumes the case
    /// and opens no second one, the story still active.
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
            "--iterations",
            "0",
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 3, "{out}");
        assert!(out.contains("frontier: took story:ready-one"), "{out}");
        let case = the_case(&repo);
        let id = case
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        assert!(approvals_recorded(&repo, "story:ready-one").is_empty());
        let (code, out) = takt(&[
            "approve",
            "--repo",
            path,
            &id,
            "repository.merge",
            "--by",
            "operator",
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains(&format!(
                "approved: repository.merge for case {id} by operator, written to {id}/authority.yaml"
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
            recorded[0].contains("takt approve repository.merge --by operator"),
            "{recorded:?}"
        );
        assert!(
            recorded[0].contains(&format!("({id}/authority.yaml)")),
            "{recorded:?}"
        );
        let (code, out) = takt(&[
            "run",
            "--repo",
            path,
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ]);
        assert_eq!(code, 3, "{out}");
        assert!(
            out.contains(&format!(
                "frontier: resumed story:ready-one: Make the check pass (case {id})"
            )),
            "{out}"
        );
        assert!(out.contains("step 1: tests.run {}"), "{out}");
        assert_eq!(the_case(&repo), case, "no second case was opened");
        assert_eq!(status(&repo, "story:ready-one"), "active");
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
}
