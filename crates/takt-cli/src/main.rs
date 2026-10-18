//! `takt`: drive a repository's work through engineering protocols, every step validated.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context as _, Result, anyhow};
use clap::{Parser, Subcommand};
use takt::harness::{ClaudeCode, Harness};
use takt::validator::{TestsRun, Validator};
use takt::{LoadedProtocol, Run, RunOptions, Stop};

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
    /// Run on a repository: take the intent, open a case, loop until a stop.
    Run(Box<RunArgs>),
    /// List the built-in protocols.
    Protocols,
}

#[derive(clap::Args)]
struct RunArgs {
    /// The repository worktree.
    #[arg(long)]
    repo: PathBuf,
    /// What the run is meant to achieve.
    #[arg(long)]
    intent: String,
    /// The protocol: a built-in id or a `protocol/1` file.
    #[arg(long, default_value = "software.change/1")]
    protocol: String,
    /// A case input, `name=value`; repeatable.
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
}

/// A harness that performs nothing: the loop stops when only judgement steps are left.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &takt::StepContext<'_>) -> Result<takt::StepReport> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left (approvals: {})",
            context.offered.join(", "),
            context.approvals.join(", ")
        ))
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("takt: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Protocols => {
            for (name, major) in b10x_canon_engineering_list() {
                println!("{name}/{major}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Run(args) => {
            let RunArgs {
                repo,
                intent,
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
            // The software change's inputs select the obligations it owes; left out, they take
            // the least demanding values a change can carry, said on the first line of the run.
            let mut inputs = inputs;
            if protocol == "software.change/1" || protocol == "software-change/1" {
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
                protocol,
                intent,
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
            let validators: Vec<Box<dyn Validator>> = vec![Box::new(TestsRun::new(&test_cmd)?)];
            let mut run = Run::open(&repo, options, harness, &validators)?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            let stop = run.run(&mut out)?;
            println!(
                "stopped: {stop}; steps {}, tokens {}, case {}",
                run.steps,
                run.tokens,
                run.case.dir.display()
            );
            Ok(match stop {
                Stop::Completed(_) | Stop::ApprovalRequired(_) => ExitCode::SUCCESS,
                _ => ExitCode::from(3),
            })
        }
    }
}

fn b10x_canon_engineering_list() -> Vec<(String, u32)> {
    LoadedProtocol::builtins()
}
