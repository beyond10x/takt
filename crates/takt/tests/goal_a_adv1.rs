//! Adversarial cases against the cluster-delivery wave (`4ec3ae24..4def3240`): the run file's
//! `tokens` (story:a-run-stops-at-a-default-token-budget), the bot forge's preflight
//! (story:forge-preflight) and its read of the open pull requests for a head
//! (story:pr-open-not-idempotent-on-resume). Each case states what it expects of the code and
//! why; nothing leaves the machine.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use takt::forge::{BotRoute, Exit, Forge, Runner};

/// The tests below that change `PATH` take this first: the environment is the process's.
static ENV: Mutex<()> = Mutex::new(());

/// A scratch directory of this case's own under the crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("goal-a-adv1-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
        }
        fs::create_dir_all(&dir).expect("scratch");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `<scratch>/bin/b10x-gates`, an executable that does nothing, put first on `PATH` so the
/// preflight finds a `b10x-gates`; the guard holds the environment while the test runs.
fn gates_on_path(scratch: &Scratch) -> std::sync::MutexGuard<'static, ()> {
    let guard = ENV
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let bin = scratch.0.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let program = bin.join("b10x-gates");
    fs::write(&program, "#!/bin/sh\nexit 0\n").expect("a program");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![bin];
    dirs.extend(std::env::split_paths(&path));
    let joined = std::env::join_paths(dirs).expect("a PATH");
    // SAFETY: every test of this binary that reads or writes the environment holds `ENV`.
    unsafe {
        std::env::set_var("PATH", joined);
    }
    guard
}

/// A runner that answers nothing: the preflight runs no program.
struct Nothing;

impl Runner for Nothing {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        anyhow::bail!("no program runs here: {argv:?}")
    }
}

// --- U1: the run file's `tokens` -------------------------------------------------------------

/// story:a-run-stops-at-a-default-token-budget, Acceptance 5, and the run file's own contract
/// (`--config` in `takt run --help`: "`tokens` (a whole number above zero; another value exits
/// 2)"): a `tokens` key written with no value, YAML's null, is another value. It is refused as
/// `tokens: 0` is, never read as "not given", which runs the default of 3,000,000 the file's
/// author did not write.
#[test]
fn a_run_file_tokens_with_no_value_is_refused_as_tokens_0_is() {
    let scratch = Scratch::new("tokens-null");
    for (name, text) in [
        ("empty.yaml", "iterations: 3\ntokens:\n"),
        ("tilde.yaml", "iterations: 3\ntokens: ~\n"),
        ("null.yaml", "iterations: 3\ntokens: null\n"),
    ] {
        let path = scratch.0.join(name);
        fs::write(&path, text).expect("a run file");
        match takt::run_config::RunConfig::read(&path) {
            Ok(config) => panic!(
                "{name}: `tokens` with no value is read, tokens {:?}, so the run takes the default budget instead of exiting 2",
                config.tokens
            ),
            Err(error) => assert!(
                error
                    .downcast_ref::<takt::run_config::TokensRefused>()
                    .is_some(),
                "{name}: refused, but not as a `tokens` that is no whole number above zero (exit 1, not 2): {error:#}"
            ),
        }
    }
}

// --- U2: the bot forge's preflight -----------------------------------------------------------

/// story:forge-preflight's outcome: "a missing one is refused with exit 1 before any step ...
/// not found at `pr.open` after the budget is spent". `b10x-gates` loads its policy only from a
/// regular file of the user's own, readable by that user alone (`protected file ownership or
/// permissions invalid`): a policy file left `0644`, as a `git pull` in the policy's checkout
/// leaves it, is one every scan before a push refuses. The preflight passes it, so the run
/// spends its steps and fails at `pr.open`.
#[test]
fn the_preflight_refuses_a_policy_b10x_gates_refuses_to_load_mode_0644() {
    let scratch = Scratch::new("policy-0644");
    let _env = gates_on_path(&scratch);
    let policy = scratch.0.join("policy.json");
    fs::write(&policy, "{}").expect("a policy");
    fs::set_permissions(&policy, fs::Permissions::from_mode(0o644)).expect("mode");
    let route = BotRoute::new("owner/repo", Some(policy.clone()), Box::new(Nothing));
    assert!(
        route.preflight().is_err(),
        "the preflight passes {} at mode 0644, which b10x-gates refuses to load",
        policy.display()
    );
}

/// The same for a policy named through a symbolic link: `b10x-gates` reads the policy with
/// `symlink_metadata` and refuses a link (`protected file must be a regular file`); the
/// preflight opens through it and passes.
#[test]
fn the_preflight_refuses_a_policy_named_through_a_symbolic_link() {
    let scratch = Scratch::new("policy-link");
    let _env = gates_on_path(&scratch);
    let target = scratch.0.join("policy.json");
    fs::write(&target, "{}").expect("a policy");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("mode");
    let link = scratch.0.join("policy-link.json");
    std::os::unix::fs::symlink(&target, &link).expect("a link");
    let route = BotRoute::new("owner/repo", Some(link.clone()), Box::new(Nothing));
    assert!(
        route.preflight().is_err(),
        "the preflight passes {}, a link, which b10x-gates refuses to load",
        link.display()
    );
}

/// The preflight checks the file `B10X_GATES_POLICY` names from takt's working directory, but
/// every `b10x-gates` line runs in the tree (`--policy <as given>`, the child's directory the
/// tree): a relative policy that is there from where takt was started and not from the tree
/// passes the preflight, and every read and scan of the run is then given a policy that names
/// no file.
#[test]
fn the_preflight_checks_the_policy_b10x_gates_is_given_from_the_tree() {
    let scratch = Scratch::new("policy-relative");
    let _env = gates_on_path(&scratch);
    let tree = scratch.0.join("repo");
    fs::create_dir_all(&tree).expect("a tree");
    let policy = scratch.0.join("policy.json");
    fs::write(&policy, "{}").expect("a policy");
    fs::set_permissions(&policy, fs::Permissions::from_mode(0o600)).expect("mode");
    // The policy relative to this test's working directory, the crate's directory.
    let here = std::env::current_dir().expect("a working directory");
    let up = here.components().count();
    let mut relative = PathBuf::new();
    for _ in 1..up {
        relative.push("..");
    }
    relative.push(policy.strip_prefix("/").expect("absolute"));
    assert!(
        relative.is_relative() && relative.is_file(),
        "{}",
        relative.display()
    );

    struct Given(PathBuf);
    impl Runner for Given {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.run_env(tree, argv, &[])
        }
        fn run_env(
            &self,
            tree: &Path,
            argv: &[String],
            _: &[(&str, &str)],
        ) -> anyhow::Result<Exit> {
            let at = argv.iter().position(|a| a == "--policy").expect("--policy");
            let from_tree = tree.join(&argv[at + 1]);
            // b10x-gates runs in `tree`, so it opens the policy from there.
            assert!(
                from_tree.is_file(),
                "b10x-gates is given --policy {} in {}, which names no file there (the preflight checked {})",
                argv[at + 1],
                tree.display(),
                self.0.display()
            );
            let out = argv.iter().position(|a| a == "--output").expect("--output");
            fs::write(
                &argv[out + 1],
                r#"{"state":"open","merged":false,"head":{"ref":"b","sha":"c"},"base":{"ref":"main"}}"#,
            )?;
            Ok(Exit {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    let route = BotRoute::new(
        "owner/repo",
        Some(relative.clone()),
        Box::new(Given(relative.clone())),
    );
    route.preflight().expect(
        "the preflight passes the relative policy, which is there from takt's working directory",
    );
    route
        .pull_request(&tree, 7)
        .expect("the read is given a policy that is there");
}

// --- U3: the open pull requests for a head ---------------------------------------------------

/// Answers every list of pull requests with `listed`.
struct Lists(&'static str);

impl Runner for Lists {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let out = argv.iter().position(|a| a == "--output").expect("--output");
        fs::write(&argv[out + 1], self.0)?;
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// An open pull request of the case's branch name whose head repository GitHub no longer
/// names (`"repo": null`, a fork deleted) is not this repository's: `find_open_pull_request`
/// says it takes an entry "of this repository when it names the head's repository", and one
/// that names none is taken, so `pr.open` binds the case to a pull request whose head it can
/// never push to.
#[test]
fn an_open_pull_request_whose_head_repository_is_gone_is_not_taken() {
    let scratch = Scratch::new("head-repo-null");
    let route = BotRoute::new(
        "owner/repo",
        None,
        Box::new(Lists(
            r#"[{"number":9,"state":"open","head":{"ref":"takt/c1","sha":"0123456789abcdef0123456789abcdef01234567","repo":null},"base":{"ref":"main","repo":{"full_name":"owner/repo"}}}]"#,
        )),
    );
    assert_eq!(
        route
            .find_open_pull_request(&scratch.0, "takt/c1", "main")
            .expect("reads"),
        None,
        "a pull request with no head repository is taken as the case's"
    );
}
