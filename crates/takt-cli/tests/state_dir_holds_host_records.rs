//! story:the-state-directory-holds-every-host-record: the state directory a run is given holds
//! every record the run keeps on the host, its session transcripts and the files a discarded
//! step moves aside (`<state_dir>/set-aside`), and a `loom.agent/1` document's
//! `runtime.state_dir` is the state directory when neither `--state-dir` nor the run
//! configuration's `takt.state_dir` names one. Without any of the three, both stay under
//! `$XDG_STATE_HOME/takt`. The `claude` is a double; nothing reaches a model.

mod state_dir_holds_host_records {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    fn git(tree: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(tree)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    fn executable(path: &Path, text: &str) {
        use std::os::unix::fs::PermissionsExt as _;
        fs::write(path, text).expect("a script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
    }

    /// A scratch world under this crate's build directory, removed on drop: a repository with one
    /// commit that ignores `cache/`, whose `cache/state.txt` exists before the run; a program
    /// directory holding a `claude` double that rewrites that file in the repository itself, as a
    /// process that writes outside its scratch copy would, so the step is refused and discarded;
    /// a `config` directory and an `XDG_STATE_HOME` of its own.
    struct World(PathBuf);

    impl Drop for World {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    impl World {
        fn new(name: &str) -> Self {
            let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("state-dir-holds-{name}-{}", std::process::id()));
            if outer.exists() {
                fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
            }
            let world = Self(outer);
            for dir in [world.repo(), world.bin(), world.config_dir(), world.xdg()] {
                fs::create_dir_all(dir).expect("a directory");
            }
            fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
            fs::write(world.repo().join(".gitignore"), "cache/\n").expect("a file");
            fs::create_dir_all(world.repo().join("cache")).expect("a directory");
            fs::write(
                world.repo().join("cache/state.txt"),
                "kept before the run\n",
            )
            .expect("a file");
            git(&world.repo(), &["init", "-q", "-b", "main"]);
            git(&world.repo(), &["add", "-A"]);
            git(&world.repo(), &["commit", "-q", "-m", "start"]);
            executable(
                &world.bin().join("claude"),
                &format!(
                    "#!/bin/sh\nmkdir -p '{repo}/cache'\necho rewritten > '{repo}/cache/state.txt'\nprintf '%s\\n' '{{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{{\\\"action\\\": \\\"implementation.edit\\\", \\\"summary\\\": \\\"rewrote a cache\\\"}}\",\"usage\":{{\"input_tokens\":3,\"output_tokens\":4}}}}'\n",
                    repo = world.repo().display()
                ),
            );
            world
        }

        fn repo(&self) -> PathBuf {
            self.0.join("repo")
        }

        fn bin(&self) -> PathBuf {
            self.0.join("bin")
        }

        fn config_dir(&self) -> PathBuf {
            self.0.join("config")
        }

        fn xdg(&self) -> PathBuf {
            self.0.join("xdg-state")
        }

        /// Writes `text` as `config/<file>` and returns its path.
        fn document(&self, file: &str, text: &str) -> PathBuf {
            let path = self.config_dir().join(file);
            fs::write(&path, text).expect("a run configuration file");
            path
        }

        /// `takt run --harness claude` on the repository with an intent, two steps, a passing test
        /// command and `args`, the double first on the program path.
        fn run(&self, args: &[&str]) -> Output {
            Command::new(env!("CARGO_BIN_EXE_takt"))
                .args([
                    "run",
                    "--repo",
                    self.repo().to_str().expect("utf-8"),
                    "--intent",
                    "probe",
                    "--harness",
                    "claude",
                    "--forge",
                    "none",
                    "--iterations",
                    "2",
                    "--test-cmd",
                    "true",
                ])
                .args(args)
                .env(
                    "PATH",
                    format!(
                        "{}:{}",
                        self.bin().display(),
                        std::env::var("PATH").unwrap_or_default()
                    ),
                )
                .env("XDG_STATE_HOME", self.xdg())
                .env_remove("B10X_GATES_POLICY")
                .env_remove("DBUS_SESSION_BUS_ADDRESS")
                .output()
                .expect("takt runs")
        }

        /// The transcripts the case's run record names, relative to the transcripts directory.
        fn named(&self) -> Vec<String> {
            let cases: Vec<PathBuf> = fs::read_dir(self.repo().join(".engineering/cases"))
                .expect("the cases directory")
                .map(|entry| entry.expect("an entry").path())
                .collect();
            assert_eq!(cases.len(), 1, "{cases:?}");
            fs::read_to_string(cases[0].join("events.jsonl"))
                .expect("the run record")
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
                .filter_map(|line| line["transcript"].as_str().map(str::to_owned))
                .collect()
        }
    }

    /// Every regular file under `dir`.
    fn files_under(dir: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(files_under(&path));
            } else {
                found.push(path);
            }
        }
        found
    }

    fn text(output: &Output) -> String {
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    /// Asserts the run kept each transcript its record names under `state/transcripts`, and that
    /// the file a discarded step rewrote was moved under `state/set-aside`.
    fn holds_the_host_records(world: &World, state: &Path, said: &str) {
        let transcripts = state.join("transcripts");
        assert!(
            !files_under(&transcripts).is_empty(),
            "a transcript under {}\n{said}",
            transcripts.display()
        );
        let named = world.named();
        assert!(!named.is_empty(), "the record names a transcript\n{said}");
        for name in &named {
            assert!(
                transcripts.join(name).is_file(),
                "{name} lies under {}\n{said}",
                transcripts.display()
            );
        }
        moved_under(&state.join("set-aside"), said);
    }

    /// Asserts the file the discarded step rewrote is under `set_aside`, at its own relative path.
    fn moved_under(set_aside: &Path, said: &str) {
        assert!(
            files_under(set_aside)
                .iter()
                .any(|path| path.ends_with("cache/state.txt")),
            "the rewritten cache/state.txt is under {}\n{said}",
            set_aside.display()
        );
    }

    /// Asserts nothing at all was written under `XDG_STATE_HOME`.
    fn nothing_under_the_state_home(world: &World, said: &str) {
        assert_eq!(
            files_under(&world.xdg()),
            Vec::<PathBuf>::new(),
            "nothing kept under XDG_STATE_HOME\n{said}"
        );
    }

    /// Acceptance 1: a discarded step's file goes under `--state-dir`'s `set-aside`, and nothing
    /// is written under `$XDG_STATE_HOME/takt`.
    #[test]
    fn set_aside_goes_under_the_state_dir() {
        let world = World::new("set-aside");
        let state = world.0.join("volume");
        let output = world.run(&["--state-dir", state.to_str().expect("utf-8")]);
        let said = text(&output);
        holds_the_host_records(&world, &state, &said);
        nothing_under_the_state_home(&world, &said);
    }

    /// Acceptance 2: a `loom.agent/1` document's `runtime.state_dir`, with no `takt.state_dir`,
    /// is the state directory: the transcripts are under its `transcripts`, and nothing is written
    /// under `$XDG_STATE_HOME`. Its other `runtime` members are Loom's, and are not refused.
    #[test]
    fn loom_runtime_state_dir_is_the_default() {
        let world = World::new("runtime");
        let state = world.0.join("runtime-state");
        let config = world.document(
            "agent.yaml",
            &format!(
                "format: loom.agent/1\nagent: {{id: a, name: A, brief: b}}\nruntime:\n  state_dir: \"{}\"\n  connectors: []\n",
                state.display()
            ),
        );
        let output = world.run(&["--config", config.to_str().expect("utf-8")]);
        let said = text(&output);
        holds_the_host_records(&world, &state, &said);
        nothing_under_the_state_home(&world, &said);
    }

    /// Acceptance 3: with `takt.state_dir` beside `runtime.state_dir`, the transcripts are under
    /// `takt.state_dir`; `--state-dir` wins over both.
    #[test]
    fn takt_state_dir_wins_over_runtime() {
        let by_file = World::new("takt-wins");
        let runtime = by_file.0.join("runtime-state");
        let named = by_file.0.join("takt-state");
        let config = by_file.document(
            "agent.yaml",
            &format!(
                "format: loom.agent/1\nruntime:\n  state_dir: \"{}\"\ntakt:\n  state_dir: \"{}\"\n",
                runtime.display(),
                named.display()
            ),
        );
        let output = by_file.run(&["--config", config.to_str().expect("utf-8")]);
        let said = text(&output);
        holds_the_host_records(&by_file, &named, &said);
        assert!(
            !runtime.exists(),
            "runtime.state_dir is not used beside takt.state_dir\n{said}"
        );
        nothing_under_the_state_home(&by_file, &said);

        let by_flag = World::new("flag-wins");
        let runtime = by_flag.0.join("runtime-state");
        let named = by_flag.0.join("takt-state");
        let flag = by_flag.0.join("flag-state");
        let config = by_flag.document(
            "agent.yaml",
            &format!(
                "format: loom.agent/1\nruntime:\n  state_dir: \"{}\"\ntakt:\n  state_dir: \"{}\"\n",
                runtime.display(),
                named.display()
            ),
        );
        let output = by_flag.run(&[
            "--config",
            config.to_str().expect("utf-8"),
            "--state-dir",
            flag.to_str().expect("utf-8"),
        ]);
        let said = text(&output);
        holds_the_host_records(&by_flag, &flag, &said);
        assert!(
            !runtime.exists() && !named.exists(),
            "neither the document's state directory is used beside --state-dir\n{said}"
        );
        nothing_under_the_state_home(&by_flag, &said);
    }

    /// Acceptance 4: without `--state-dir`, `takt.state_dir` or `runtime.state_dir`, the
    /// transcripts are under `$XDG_STATE_HOME/takt/transcripts` and the discarded step's file is
    /// under `$XDG_STATE_HOME/takt/set-aside`, where they are today.
    #[test]
    fn no_state_dir_is_todays_location() {
        let world = World::new("default");
        let output = world.run(&[]);
        let said = text(&output);
        let transcripts = world.xdg().join("takt/transcripts");
        let named = world.named();
        assert!(!named.is_empty(), "the record names a transcript\n{said}");
        for name in &named {
            assert!(
                transcripts.join(name).is_file(),
                "{name} lies under {}\n{said}",
                transcripts.display()
            );
        }
        moved_under(&world.xdg().join("takt/set-aside"), &said);
    }
}
