//! Adversary cases on story:the-state-directory-holds-every-host-record: a run given a state
//! directory holds every host record there, so it needs nothing of `$XDG_STATE_HOME` or `HOME`,
//! makes nothing there, and a resumed case loses and overwrites nothing set aside before. The
//! `claude` is a double that rewrites an ignored file in the repository, so each step is refused
//! and discarded and that file is set aside; nothing reaches a model.

mod state_dir_holds_host_records_adv1 {
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

    /// Where the run's environment points `XDG_STATE_HOME` and `HOME`.
    enum Homes {
        /// Both at empty directories of the world's own.
        Scratch,
        /// `XDG_STATE_HOME` at a regular file, `HOME` at an empty directory.
        StateHomeIsAFile,
        /// `XDG_STATE_HOME` unset, `HOME` at the repository itself.
        HomeIsTheRepository,
        /// Neither set.
        Unset,
    }

    /// A scratch world under this crate's build directory, removed on drop: a repository with one
    /// commit that ignores `cache/`, whose `cache/state.txt` exists before the run; a `claude`
    /// double that rewrites that file in the repository itself; a state home and a home of its
    /// own.
    struct World(PathBuf);

    impl Drop for World {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    impl World {
        fn new(name: &str) -> Self {
            let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("state-dir-adv1-{name}-{}", std::process::id()));
            if outer.exists() {
                fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
            }
            let world = Self(outer);
            for dir in [world.repo(), world.bin(), world.xdg(), world.home()] {
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

        fn xdg(&self) -> PathBuf {
            self.0.join("xdg-state")
        }

        fn home(&self) -> PathBuf {
            self.0.join("home")
        }

        /// `takt run --harness claude` on the repository, `iterations` steps, a passing test
        /// command and `args`, the double first on the program path, the homes as `homes` says.
        fn run(&self, homes: &Homes, iterations: &str, args: &[&str]) -> Output {
            let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
            command
                .args([
                    "run",
                    "--repo",
                    self.repo().to_str().expect("utf-8"),
                    "--harness",
                    "claude",
                    "--forge",
                    "none",
                    "--iterations",
                    iterations,
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
                .env_remove("B10X_GATES_POLICY")
                .env_remove("DBUS_SESSION_BUS_ADDRESS")
                .env_remove("XDG_STATE_HOME")
                .env_remove("HOME");
            match homes {
                Homes::Scratch => {
                    command
                        .env("XDG_STATE_HOME", self.xdg())
                        .env("HOME", self.home());
                }
                Homes::StateHomeIsAFile => {
                    let file = self.0.join("state-home-file");
                    fs::write(&file, "not a directory\n").expect("a file");
                    command
                        .env("XDG_STATE_HOME", &file)
                        .env("HOME", self.home());
                }
                Homes::HomeIsTheRepository => {
                    command.env("HOME", self.repo());
                }
                Homes::Unset => {}
            }
            command.output().expect("takt runs")
        }

        /// Puts the operator's `cache/state.txt` back in the repository, as it was before the
        /// first run, so the next harness step rewrites it again and it is set aside again (a
        /// file the step makes new, with nothing there before, is removed, not set aside).
        fn cache_again(&self) {
            fs::write(self.repo().join("cache/state.txt"), "kept before the run\n")
                .expect("a file");
        }

        /// The ids of the cases under the repository.
        fn cases(&self) -> Vec<String> {
            let Ok(entries) = fs::read_dir(self.repo().join(takt::case::CASES_DIR)) else {
                return Vec::new();
            };
            entries
                .map(|entry| {
                    entry
                        .expect("an entry")
                        .file_name()
                        .to_str()
                        .expect("utf-8")
                        .to_owned()
                })
                .collect()
        }

        /// The transcripts the case's run record names, relative to the transcripts directory.
        fn named(&self) -> Vec<String> {
            let cases = self.cases();
            assert_eq!(cases.len(), 1, "{cases:?}");
            fs::read_to_string(
                self.repo()
                    .join(takt::case::CASES_DIR)
                    .join(&cases[0])
                    .join("events.jsonl"),
            )
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
            .filter_map(|line| line["transcript"].as_str().map(str::to_owned))
            .collect()
        }
    }

    /// Every entry under `dir`, directories included.
    fn entries_under(dir: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            found.push(path.clone());
            if path.is_dir() && !path.is_symlink() {
                found.extend(entries_under(&path));
            }
        }
        found
    }

    /// The set-aside copies of `cache/state.txt` under `dir`.
    fn set_aside_copies(dir: &Path) -> Vec<PathBuf> {
        entries_under(dir)
            .into_iter()
            .filter(|path| path.is_file() && path.ends_with("cache/state.txt"))
            .collect()
    }

    fn text(output: &Output) -> String {
        format!(
            "status {:?}\n{}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    /// Asserts the run kept each transcript its record names under `state/transcripts`, and that
    /// the file a discarded step rewrote was moved under `state/set-aside`.
    fn holds_the_host_records(world: &World, state: &Path, said: &str) {
        let named = world.named();
        assert!(!named.is_empty(), "the record names a transcript\n{said}");
        for name in &named {
            assert!(
                state.join("transcripts").join(name).is_file(),
                "{name} lies under {}\n{said}",
                state.join("transcripts").display()
            );
        }
        assert!(
            !set_aside_copies(&state.join("set-aside")).is_empty(),
            "the rewritten cache/state.txt is under {}\n{said}",
            state.join("set-aside").display()
        );
    }

    /// Story, Outcome: the state directory holds every record the run keeps on the host, so a
    /// run given one starts and keeps them there when `XDG_STATE_HOME` cannot be used (a file in
    /// its place). The default set-aside root under it is not this run's.
    #[test]
    fn a_state_dir_run_starts_when_the_state_home_is_a_file() {
        let world = World::new("state-home-file");
        let state = world.0.join("volume");
        let output = world.run(
            &Homes::StateHomeIsAFile,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        assert!(
            !said.contains("the run does not start"),
            "a run given a state directory does not stop on the state home\n{said}"
        );
        holds_the_host_records(&world, &state, &said);
    }

    /// The same for a `HOME` that holds the repository, with no `XDG_STATE_HOME`, as a container
    /// whose home is its workspace has: `~/.local/state/takt/set-aside` would lie inside the tree,
    /// but this run sets nothing aside there.
    #[test]
    fn a_state_dir_run_starts_when_home_is_the_repository() {
        let world = World::new("home-is-repo");
        let state = world.0.join("volume");
        let output = world.run(
            &Homes::HomeIsTheRepository,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        assert!(
            !said.contains("the run does not start"),
            "a run given a state directory does not stop on the state home\n{said}"
        );
        holds_the_host_records(&world, &state, &said);
    }

    /// With neither `HOME` nor `XDG_STATE_HOME` set, a run given a state directory takes its
    /// harness steps (no `no set-aside directory` refusal) and keeps both records there.
    #[test]
    fn a_state_dir_run_needs_neither_home_nor_state_home() {
        let world = World::new("homes-unset");
        let state = world.0.join("volume");
        let output = world.run(
            &Homes::Unset,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        assert!(!said.contains("no set-aside directory"), "{said}");
        holds_the_host_records(&world, &state, &said);
    }

    /// Acceptance 1, read for directories as well as files: a run given a state directory makes
    /// nothing at all under `XDG_STATE_HOME` or under `HOME/.local/state`, not an empty `takt`.
    #[test]
    fn a_state_dir_run_makes_no_entry_under_the_state_home_or_home() {
        let world = World::new("no-entry");
        let state = world.0.join("volume");
        let output = world.run(
            &Homes::Scratch,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        holds_the_host_records(&world, &state, &said);
        assert_eq!(
            entries_under(&world.xdg()),
            Vec::<PathBuf>::new(),
            "no entry under XDG_STATE_HOME\n{said}"
        );
        assert_eq!(
            entries_under(&world.home().join(".local/state")),
            Vec::<PathBuf>::new(),
            "no entry under HOME/.local/state\n{said}"
        );
    }

    /// A case opened with one state directory and resumed (`--case`) with another: the second
    /// run's set-aside goes under the second directory, and what the first set aside stays under
    /// the first; nothing goes under the state home.
    #[test]
    fn a_resumed_case_sets_aside_under_its_new_state_dir_and_keeps_the_first() {
        let world = World::new("resume-other");
        let first = world.0.join("first");
        let second = world.0.join("second");
        // Two steps: the first runs the tests, the second is the harness step refused and set
        // aside.
        let output = world.run(
            &Homes::Scratch,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                first.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        let cases = world.cases();
        assert_eq!(cases.len(), 1, "{said}");
        assert_eq!(
            set_aside_copies(&first.join("set-aside")).len(),
            1,
            "{said}"
        );
        world.cache_again();
        let output = world.run(
            &Homes::Scratch,
            "1",
            &[
                "--case",
                &cases[0],
                "--state-dir",
                second.to_str().expect("utf-8"),
            ],
        );
        let said = format!("{said}\n---\n{}", text(&output));
        assert_eq!(
            set_aside_copies(&first.join("set-aside")).len(),
            1,
            "the first run's set-aside is kept\n{said}"
        );
        assert_eq!(
            set_aside_copies(&second.join("set-aside")).len(),
            1,
            "the resumed run sets aside under its own state directory\n{said}"
        );
        assert_eq!(entries_under(&world.xdg()), Vec::<PathBuf>::new(), "{said}");
    }

    /// A case resumed twice on the same state directory, each resume's one harness step numbered
    /// `step-1` as the other's is: the second overwrites nothing the first set aside, and no
    /// transcript is overwritten.
    #[test]
    fn a_resumed_case_on_the_same_state_dir_overwrites_nothing_set_aside() {
        let world = World::new("resume-same");
        let state = world.0.join("volume");
        // One step, the tests run: the case is open and nothing is set aside yet.
        let output = world.run(
            &Homes::Scratch,
            "1",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        let cases = world.cases();
        assert_eq!(cases.len(), 1, "{said}");
        let output = world.run(
            &Homes::Scratch,
            "1",
            &[
                "--case",
                &cases[0],
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = format!("{said}\n---\n{}", text(&output));
        assert_eq!(
            set_aside_copies(&state.join("set-aside")).len(),
            1,
            "the first resume set its step aside\n{said}"
        );
        let first_transcripts: Vec<PathBuf> = entries_under(&state.join("transcripts"))
            .into_iter()
            .filter(|path| path.is_file())
            .collect();
        let first_bytes: Vec<Vec<u8>> = first_transcripts
            .iter()
            .map(|path| fs::read(path).expect("a transcript"))
            .collect();
        world.cache_again();
        let output = world.run(
            &Homes::Scratch,
            "1",
            &[
                "--case",
                &cases[0],
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = format!("{said}\n---\n{}", text(&output));
        for (path, bytes) in first_transcripts.iter().zip(&first_bytes) {
            assert_eq!(
                fs::read(path).ok().as_ref(),
                Some(bytes),
                "{} is kept as it was\n{said}",
                path.display()
            );
        }
        assert_eq!(
            set_aside_copies(&state.join("set-aside")).len(),
            2,
            "two set-aside copies, one per run\n{said}"
        );
        let transcripts: Vec<PathBuf> = entries_under(&state.join("transcripts"))
            .into_iter()
            .filter(|path| path.is_file())
            .collect();
        for kept in &first_transcripts {
            assert!(
                transcripts.contains(kept),
                "{} is kept\n{said}",
                kept.display()
            );
        }
        assert!(
            transcripts.len() > first_transcripts.len(),
            "the resumed run kept a transcript of its own\n{said}"
        );
    }

    /// A state directory that is a link into the repository is refused before anything opens:
    /// no case, and nothing set aside into the tree.
    #[test]
    fn a_state_dir_linked_into_the_repository_is_refused_before_anything_opens() {
        let world = World::new("linked");
        let link = world.0.join("link");
        std::os::unix::fs::symlink(world.repo().join("cache/volume"), &link).expect("a link");
        let output = world.run(
            &Homes::Scratch,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                link.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        assert!(!output.status.success(), "{said}");
        assert!(said.contains("nothing was opened"), "{said}");
        assert_eq!(world.cases(), Vec::<String>::new(), "{said}");
    }

    /// A state directory whose `set-aside` is a file is refused before anything opens.
    #[test]
    fn a_state_dir_whose_set_aside_is_a_file_is_refused_before_anything_opens() {
        let world = World::new("set-aside-file");
        let state = world.0.join("volume");
        fs::create_dir_all(&state).expect("a directory");
        fs::write(state.join("set-aside"), "a file\n").expect("a file");
        let output = world.run(
            &Homes::Scratch,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        assert!(!output.status.success(), "{said}");
        assert!(said.contains("nothing was opened"), "{said}");
        assert_eq!(world.cases(), Vec::<String>::new(), "{said}");
    }

    /// A state directory whose set-aside is refused leaves nothing behind: the transcripts
    /// directory the state directory's check would make is not made, since the set-aside check
    /// runs first and refuses a set-aside that is a link into the tree.
    #[test]
    fn a_refused_state_dir_makes_no_transcripts_directory() {
        let world = World::new("refused-no-transcripts");
        let state = world.0.join("volume");
        fs::create_dir_all(&state).expect("a directory");
        std::os::unix::fs::symlink(world.repo().join("cache"), state.join("set-aside"))
            .expect("a link");
        let output = world.run(
            &Homes::Scratch,
            "2",
            &[
                "--intent",
                "probe",
                "--state-dir",
                state.to_str().expect("utf-8"),
            ],
        );
        let said = text(&output);
        assert!(!output.status.success(), "{said}");
        assert!(said.contains("nothing was opened"), "{said}");
        assert!(
            !state.join("transcripts").exists(),
            "no transcripts directory is made under a refused state directory\n{said}"
        );
        assert_eq!(world.cases(), Vec::<String>::new(), "{said}");
    }
}
