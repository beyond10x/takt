//! Seatbelt, the macOS platform sandbox, as takt confines a child with it there: a profile takt
//! writes ([`Profile`]), applied by [`SANDBOX_EXEC`] before it executes the child. On macOS it
//! stands where Linux has bubblewrap or namespaces of takt's own:
//!
//! | child | profile |
//! |---|---|
//! | the promotion's build and its binary's `--version` (`crate::promotion::Confinement`) | [`build`]: no network, writes only to the directories the build owns, reads under the user's home only of the ones it needs |
//! | the test command (`crate::forge::without_login`) | [`test_command`]: writes only to the tree, its git directories but their hooks and configuration, the fresh home, which holds its temporary directory, and the cargo caches; no read of a registry login, the user's keychains, the runtime directory or a bus socket, and no connection to a bus or launchd socket |
//!
//! Neither starts a program outside its profile ([`NO_LAUNCH`]), and neither writes the user's
//! temporary and cache directories under `/var/folders`, which the system's tools read for
//! every program of the user.
//!
//! Every path enters the profile as a parameter (`-D P<n>=<path>`, read as `(param "P<n>")`),
//! never inside its text, so no path is quoted and none can end a rule. Seatbelt matches the
//! path a file is reached by after links (`/tmp` is `/private/tmp`), so each path is given as
//! [`real`] makes it. The profile is written on every platform, so its rules are checked where
//! takt's tests run; it is applied only on macOS.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The program that applies a profile and then executes the child under it, macOS's own; a host
/// without it confines nothing there, and what takt would confine with it is refused by name.
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// What a rule does to the operations it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `allow`.
    Allow,
    /// `deny`.
    Deny,
}

impl Action {
    fn word(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

/// How a rule matches a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// The path and everything under it (`subpath`).
    Subpath,
    /// The path itself (`literal`).
    Literal,
    /// A unix socket bound at the path, as a connection's remote end (`remote unix-socket
    /// (path-literal …)`), for `network-outbound`.
    Socket,
}

/// What starts a program outside the profile, denied in every profile takt writes: a job
/// submitted to launchd (`job-creation`, as `launchctl submit` does), an application opened
/// through LaunchServices (`lsopen`), an Apple event sent to another process
/// (`appleevent-send`), and the services that start programs for a caller (`mach-lookup`):
/// LaunchServices, Apple events, service management and the background task manager. Not
/// every service: the compiler, the linker, git and the shell look up others (the directory
/// service, logging, notifications), which start nothing.
pub const NO_LAUNCH: [&str; 4] = [
    "(deny job-creation)",
    "(deny lsopen)",
    "(deny appleevent-send)",
    "(deny mach-lookup (global-name \"com.apple.coreservices.launchservicesd\") (global-name \"com.apple.coreservices.appleevents\") (global-name-regex #\"^com\\.apple\\.lsd\\.\") (global-name \"com.apple.xpc.smd\") (global-name-regex #\"^com\\.apple\\.backgroundtaskmanagement\"))",
];

/// The sockets launchd makes for the user's session, the ssh agent's among them
/// (`/private/tmp/com.apple.launchd.<random>/Listeners`), which a test command never connects to.
const LAUNCHD_SOCKETS: &str = "(deny network-outbound (remote unix-socket (path-regex #\"^/private/tmp/com\\.apple\\.launchd\\.\")))";

/// The devices every confined child writes to: the null and zero devices, its terminal, the
/// descriptors `/dev/fd` names and the tracing helper the dynamic loader opens.
const DEVICES: &str = "(allow file-write* (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/dtracehelper\") (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))";

/// A Seatbelt profile: its rules in order, a later one over an earlier one for what both match,
/// and the paths its rules read as parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    rules: Vec<String>,
    params: Vec<OsString>,
}

impl Default for Profile {
    fn default() -> Self {
        Self::new()
    }
}

impl Profile {
    /// A profile that allows everything, for the rules added after it to narrow.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: vec!["(version 1)".to_owned(), "(allow default)".to_owned()],
            params: Vec::new(),
        }
    }

    /// Denies starting a program outside the profile ([`NO_LAUNCH`]).
    pub fn no_launch(&mut self) -> &mut Self {
        self.rules
            .extend(NO_LAUNCH.iter().map(|rule| (*rule).to_owned()));
        self
    }

    /// Denies `operation` (`file-write*`, `network*`) wherever a later rule does not allow it.
    pub fn deny(&mut self, operation: &str) -> &mut Self {
        self.rules.push(format!("(deny {operation})"));
        self
    }

    /// Allows writing to the devices every child writes to ([`DEVICES`]).
    pub fn devices(&mut self) -> &mut Self {
        self.rules.push(DEVICES.to_owned());
        self
    }

    /// One rule that `action`s `operation` on each of `paths` as `filter` matches it; none when
    /// `paths` is empty, as a rule with no filter would match everything.
    pub fn paths<P: AsRef<Path>>(
        &mut self,
        action: Action,
        operation: &str,
        filter: Filter,
        paths: impl IntoIterator<Item = P>,
    ) -> &mut Self {
        let mut rule = format!("({} {operation}", action.word());
        let mut named = false;
        for path in paths {
            let param = format!("P{}", self.params.len());
            self.params.push(path.as_ref().as_os_str().to_os_string());
            rule.push_str(&match filter {
                Filter::Subpath => format!(" (subpath (param \"{param}\"))"),
                Filter::Literal => format!(" (literal (param \"{param}\"))"),
                Filter::Socket => {
                    format!(" (remote unix-socket (path-literal (param \"{param}\")))")
                }
            });
            named = true;
        }
        if named {
            rule.push(')');
            self.rules.push(rule);
        }
        self
    }

    /// The profile's text, one rule a line.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = self.rules.join("\n");
        text.push('\n');
        text
    }

    /// The paths its rules read, `P0` first.
    #[must_use]
    pub fn params(&self) -> &[OsString] {
        &self.params
    }

    /// The arguments [`SANDBOX_EXEC`] is given before the child's program: each parameter as
    /// `-D P<n>=<path>`, then `-p` and the text.
    #[must_use]
    pub fn args(&self) -> Vec<OsString> {
        let mut args = Vec::with_capacity(self.params.len() * 2 + 2);
        for (n, path) in self.params.iter().enumerate() {
            let mut define = OsString::from(format!("P{n}="));
            define.push(path);
            args.push(OsString::from("-D"));
            args.push(define);
        }
        args.push(OsString::from("-p"));
        args.push(OsString::from(self.text()));
        args
    }
}

/// `path` as Seatbelt matches it: its real path, links resolved; for a path that does not exist
/// yet, its nearest existing ancestor's real path with the rest joined on. A relative path that
/// does not resolve is returned as it is.
#[must_use]
pub fn real(path: &Path) -> PathBuf {
    if let Ok(found) = path.canonicalize() {
        return found;
    }
    let mut rest: Vec<&OsStr> = Vec::new();
    let mut at = path;
    while let Some(parent) = at.parent() {
        if let Some(name) = at.file_name() {
            rest.push(name);
        }
        if let Ok(found) = parent.canonicalize() {
            return rest.iter().rev().fold(found, |dir, name| dir.join(name));
        }
        at = parent;
    }
    path.to_path_buf()
}

/// The directories above each of `paths` that lie under one of `homes`, each home included:
/// what a child must be able to look up (`file-read-metadata`) to reach a path it may read
/// under a home whose reads are denied. Real paths, each once.
#[must_use]
pub fn ancestors_under(paths: &[PathBuf], homes: &[PathBuf]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for path in paths {
        for dir in path.ancestors().skip(1) {
            if homes.iter().any(|home| dir.starts_with(home)) && !found.iter().any(|d| d == dir) {
                found.push(dir.to_path_buf());
            }
        }
    }
    found
}

/// The profile the promotion's build and its binary's `--version` run under: no network; no
/// program started outside it ([`NO_LAUNCH`]); writes only to the devices and under `writable`;
/// reads under each of `homes` only under `writable` and `readable`, and of the directories
/// above them there (`file-read-metadata`). Every path real ([`real`]).
#[must_use]
pub fn build(writable: &[PathBuf], readable: &[PathBuf], homes: &[PathBuf]) -> Profile {
    let real_all = |paths: &[PathBuf]| -> Vec<PathBuf> { paths.iter().map(|p| real(p)).collect() };
    let writable = real_all(writable);
    let readable = real_all(readable);
    let homes = real_all(homes);
    let seen: Vec<PathBuf> = writable.iter().chain(&readable).cloned().collect();
    let mut profile = Profile::new();
    profile
        .no_launch()
        .deny("network*")
        .deny("file-write*")
        .devices()
        .paths(Action::Allow, "file-write*", Filter::Subpath, &writable)
        .paths(Action::Deny, "file-read*", Filter::Subpath, &homes)
        .paths(Action::Allow, "file-read*", Filter::Subpath, &seen)
        .paths(
            Action::Allow,
            "file-read-metadata",
            Filter::Literal,
            ancestors_under(&seen, &homes),
        );
    profile
}

/// What [`test_command`] confines a test command to.
#[derive(Debug, Clone, Default)]
pub struct TestCommand {
    /// Written to as a whole: the tree, its git directories ([`GitDirs::writable`]) and the
    /// fresh home, which holds its temporary directory.
    pub writable: Vec<PathBuf>,
    /// The `CARGO_HOME` the command sees, whose caches it writes (`registry`, `git`, and the
    /// lock and cache files at its top) and whose registry logins it never reads.
    pub cargo_home: Option<PathBuf>,
    /// The user's homes, whose `Library/Keychains` it never reads.
    pub user_homes: Vec<PathBuf>,
    /// Directories it never reads nor writes: the runtime directory.
    pub hidden_dirs: Vec<PathBuf>,
    /// Files it never reads, writes nor connects to: the sockets a bus address names.
    pub hidden_files: Vec<PathBuf>,
    /// Directories it never writes, though they lie under `writable`: the hooks of the git
    /// directory ([`GitDirs::sealed_dirs`]).
    pub sealed_dirs: Vec<PathBuf>,
    /// Files it never writes, though they lie under `writable`: the configuration of the git
    /// directories and what names them ([`GitDirs::sealed_files`]).
    pub sealed_files: Vec<PathBuf>,
}

/// The git directories of a worktree as a test command there writes them ([`git_dirs`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitDirs {
    /// The directories outside the tree it writes as a whole: a linked worktree's own git
    /// directory and the common one.
    pub writable: Vec<PathBuf>,
    /// The directories it never writes: the common git directory's `hooks`, which git runs.
    pub sealed_dirs: Vec<PathBuf>,
    /// The files it never writes: the tree's `.git`, which names its git directory; the common
    /// directory's `config` and `config.worktree`, which name the programs git runs (a filter,
    /// `core.fsmonitor`, a pager); and a linked worktree's own `config.worktree` and
    /// `commondir`, which names the common directory.
    pub sealed_files: Vec<PathBuf>,
}

/// The git directories of the worktree at `tree`. In a linked worktree, whose `.git` is a file
/// naming its git directory (`gitdir: <path>`, a relative one from `tree`), that directory and
/// the common one its `commondir` names (a relative one from it; the git directory itself
/// without one) are written, each when it is a directory; in a main worktree its `.git`
/// directory lies in the tree, which is written already. Either way the common directory's
/// hooks and configuration and what names the directories are sealed ([`GitDirs`]). Empty for a
/// tree with no `.git`, or whose `.git` file names no directory.
#[must_use]
pub fn git_dirs(tree: &Path) -> GitDirs {
    let dot_git = tree.join(".git");
    let sealed = |common: &Path, mut sealed_files: Vec<PathBuf>, writable: Vec<PathBuf>| {
        for file in ["config", "config.worktree"].map(|name| common.join(name)) {
            if !sealed_files.contains(&file) {
                sealed_files.push(file);
            }
        }
        GitDirs {
            writable,
            sealed_dirs: vec![common.join("hooks")],
            sealed_files,
        }
    };
    if dot_git.is_dir() {
        return sealed(&dot_git, vec![dot_git.clone()], Vec::new());
    }
    let Ok(text) = std::fs::read_to_string(&dot_git) else {
        return GitDirs::default();
    };
    let Some(named) = text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("gitdir: "))
    else {
        return GitDirs::default();
    };
    let git_dir = tree.join(named.trim_end());
    if !git_dir.is_dir() {
        return GitDirs::default();
    }
    let own = vec![
        dot_git,
        git_dir.join("config.worktree"),
        git_dir.join("commondir"),
    ];
    let common = std::fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|common| git_dir.join(common.trim_end()))
        .filter(|common| common.is_dir());
    match common {
        Some(common) => sealed(&common, own, vec![git_dir.clone(), common.clone()]),
        None => sealed(&git_dir, own, vec![git_dir.clone()]),
    }
}

/// The files at the top of a `CARGO_HOME` cargo writes while it builds: the package cache's
/// locks and the global cache's database.
const CARGO_TOP: [&str; 6] = [
    ".package-cache",
    ".package-cache-mutate",
    ".global-cache",
    ".global-cache-journal",
    ".global-cache-wal",
    ".global-cache-shm",
];

/// The files under a `CARGO_HOME` that hold a registry login.
pub const CARGO_LOGINS: [&str; 2] = ["credentials.toml", "credentials"];

/// The profile a test command runs under on macOS: no program started outside it
/// ([`NO_LAUNCH`]); writes only to the devices, under `writable` but the sealed directories and
/// files, and to the cargo caches; no read nor write of a registry login, the user's keychains,
/// a hidden directory or a hidden file, and no connection to a hidden socket nor to a launchd
/// socket under `/private/tmp`. The network is not denied otherwise, as it is not on Linux.
/// Every path real ([`real`]).
#[must_use]
pub fn test_command(confined: &TestCommand) -> Profile {
    let writable: Vec<PathBuf> = confined.writable.iter().map(|p| real(p)).collect();
    let cargo_home = confined.cargo_home.as_deref().map(real);
    let mut caches: Vec<PathBuf> = Vec::new();
    let mut tops: Vec<PathBuf> = Vec::new();
    let mut logins: Vec<PathBuf> = Vec::new();
    if let Some(cargo_home) = &cargo_home {
        caches.extend(["registry", "git"].iter().map(|sub| cargo_home.join(sub)));
        tops.extend(CARGO_TOP.iter().map(|name| cargo_home.join(name)));
        logins.extend(CARGO_LOGINS.iter().map(|name| cargo_home.join(name)));
    }
    let keychains: Vec<PathBuf> = confined
        .user_homes
        .iter()
        .map(|home| real(home).join("Library/Keychains"))
        .collect();
    let hidden_dirs: Vec<PathBuf> = confined.hidden_dirs.iter().map(|p| real(p)).collect();
    let hidden_files: Vec<PathBuf> = confined.hidden_files.iter().map(|p| real(p)).collect();
    let sealed_dirs: Vec<PathBuf> = confined.sealed_dirs.iter().map(|p| real(p)).collect();
    let sealed_files: Vec<PathBuf> = confined.sealed_files.iter().map(|p| real(p)).collect();
    let mut profile = Profile::new();
    profile
        .no_launch()
        .deny("file-write*")
        .devices()
        .paths(Action::Allow, "file-write*", Filter::Subpath, &writable)
        .paths(Action::Allow, "file-write*", Filter::Subpath, &caches)
        .paths(Action::Allow, "file-write*", Filter::Literal, &tops)
        .paths(Action::Deny, "file-write*", Filter::Subpath, &sealed_dirs)
        .paths(Action::Deny, "file-write*", Filter::Literal, &sealed_files);
    for operation in ["file-read*", "file-write*"] {
        profile
            .paths(Action::Deny, operation, Filter::Literal, &logins)
            .paths(Action::Deny, operation, Filter::Subpath, &keychains)
            .paths(Action::Deny, operation, Filter::Subpath, &hidden_dirs)
            .paths(Action::Deny, operation, Filter::Literal, &hidden_files);
    }
    profile.paths(
        Action::Deny,
        "network-outbound",
        Filter::Socket,
        &hidden_files,
    );
    profile.rules.push(LAUNCHD_SOCKETS.to_owned());
    profile
}

/// `program` as [`SANDBOX_EXEC`] is to execute it: a name without a `/` found in the first
/// directory of `path` (the child's `PATH`, a relative entry read from `dir`) that holds an
/// executable file of that name, else as it is, for the start to fail on.
#[must_use]
pub fn resolve(program: &OsStr, path: Option<&OsStr>, dir: Option<&Path>) -> OsString {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::PermissionsExt as _;
    if program.as_bytes().contains(&b'/') {
        return program.to_os_string();
    }
    let Some(path) = path else {
        return program.to_os_string();
    };
    for entry in std::env::split_paths(path) {
        let entry = match dir {
            Some(dir) if entry.is_relative() => dir.join(entry),
            _ => entry,
        };
        let candidate = entry.join(program);
        if std::fs::metadata(&candidate)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        {
            return candidate.into_os_string();
        }
    }
    program.to_os_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_is_a_parameter_and_none_is_in_the_text() {
        let mut profile = Profile::new();
        profile.deny("file-write*").paths(
            Action::Allow,
            "file-write*",
            Filter::Subpath,
            ["/a \"b\")", "/c"],
        );
        let text = profile.text();
        assert_eq!(
            text,
            "(version 1)\n(allow default)\n(deny file-write*)\n(allow file-write* (subpath (param \"P0\")) (subpath (param \"P1\")))\n"
        );
        let args = profile.args();
        assert_eq!(
            args,
            ["-D", "P0=/a \"b\")", "-D", "P1=/c", "-p", text.as_str()]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[test]
    fn a_rule_with_no_path_is_left_out() {
        let mut profile = Profile::new();
        profile.paths(
            Action::Deny,
            "file-read*",
            Filter::Literal,
            Vec::<PathBuf>::new(),
        );
        assert_eq!(profile.text(), "(version 1)\n(allow default)\n");
        assert!(profile.params().is_empty());
    }

    #[test]
    fn a_missing_path_is_real_below_its_nearest_existing_ancestor() {
        let dir = std::env::temp_dir()
            .canonicalize()
            .expect("a temporary directory");
        let missing = dir.join("takt-seatbelt-missing/x/y");
        assert_eq!(real(&missing), missing);
        assert_eq!(real(&dir), dir);
    }

    #[test]
    fn the_build_denies_the_network_and_reads_under_a_home_but_what_it_needs() {
        let home = PathBuf::from("/nonexistent-home");
        let profile = build(
            &[home.join("build/source")],
            &[home.join(".rustup/toolchains/stable")],
            std::slice::from_ref(&home),
        );
        let text = profile.text();
        let at = |rule: &str| {
            text.find(rule)
                .unwrap_or_else(|| panic!("{rule} in\n{text}"))
        };
        assert!(at("(deny network*)") < at("(deny file-write*)"));
        assert!(at("(deny file-write*)") < at("(allow file-write* (subpath (param \"P0\")))"));
        assert!(
            at("(deny file-read* (subpath (param \"P1\")))")
                < at("(allow file-read* (subpath (param \"P2\")) (subpath (param \"P3\")))"),
            "the reads allowed come after the home's denial:\n{text}"
        );
        let params: Vec<&OsStr> = profile.params().iter().map(OsString::as_os_str).collect();
        assert_eq!(params[1], home.as_os_str());
        for dir in [
            home.clone(),
            home.join("build"),
            home.join(".rustup"),
            home.join(".rustup/toolchains"),
        ] {
            assert!(params.contains(&dir.as_os_str()), "{dir:?} in {params:?}");
        }
        assert!(
            !params.contains(&Path::new("/").as_os_str()),
            "nothing above the home: {params:?}"
        );
    }

    #[test]
    fn the_test_command_never_reads_a_login_and_writes_only_what_it_owns() {
        let profile = test_command(&TestCommand {
            writable: vec![PathBuf::from("/nonexistent-tree")],
            cargo_home: Some(PathBuf::from("/nonexistent-cargo")),
            user_homes: vec![PathBuf::from("/nonexistent-user")],
            hidden_dirs: vec![PathBuf::from("/nonexistent-runtime")],
            hidden_files: vec![PathBuf::from("/nonexistent-bus")],
            ..TestCommand::default()
        });
        let text = profile.text();
        let params: Vec<String> = profile
            .params()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let rule_of = |path: &str| -> Vec<&str> {
            params
                .iter()
                .enumerate()
                .filter(|(_, p)| *p == path)
                .filter_map(|(n, _)| {
                    let param = format!("(param \"P{n}\")");
                    text.lines().find(|line| line.contains(&param))
                })
                .collect()
        };
        assert!(!text.contains("network*"), "{text}");
        for (path, rules) in [
            ("/nonexistent-tree", &["(allow file-write* (subpath "][..]),
            (
                "/nonexistent-cargo/registry",
                &["(allow file-write* (subpath "],
            ),
            (
                "/nonexistent-cargo/.package-cache",
                &["(allow file-write* (literal "],
            ),
            (
                "/nonexistent-cargo/credentials.toml",
                &["(deny file-read* (literal ", "(deny file-write* (literal "],
            ),
            (
                "/nonexistent-cargo/credentials",
                &["(deny file-read* (literal ", "(deny file-write* (literal "],
            ),
            (
                "/nonexistent-user/Library/Keychains",
                &["(deny file-read* (subpath ", "(deny file-write* (subpath "],
            ),
            (
                "/nonexistent-runtime",
                &["(deny file-read* (subpath ", "(deny file-write* (subpath "],
            ),
            (
                "/nonexistent-bus",
                &[
                    "(deny file-read* (literal ",
                    "(deny file-write* (literal ",
                    "(deny network-outbound (remote unix-socket (path-literal ",
                ],
            ),
        ] {
            let found = rule_of(path);
            assert_eq!(found.len(), rules.len(), "{path}: {found:?}\n{text}");
            for (line, rule) in found.iter().zip(rules) {
                assert!(line.starts_with(rule), "{path}: {line} is not {rule}");
            }
        }
        let last_allow = text.rfind("(allow ").expect("an allow");
        let first_login = text.find("(deny file-read* ").expect("a denial");
        assert!(
            last_allow < first_login,
            "no allow after the denials:\n{text}"
        );
    }

    /// story:wave-35-review-fixes, H1: every profile denies starting a program outside it, a
    /// launchd job, an application, an Apple event and the services that start them, before
    /// any rule that allows something.
    #[test]
    fn every_profile_denies_starting_a_program_outside_it() {
        let build = build(
            &[PathBuf::from("/nonexistent-build")],
            &[],
            &[PathBuf::from("/nonexistent-home")],
        );
        let test = test_command(&TestCommand {
            writable: vec![PathBuf::from("/nonexistent-tree")],
            ..TestCommand::default()
        });
        for (name, profile) in [("build", build), ("test command", test)] {
            let text = profile.text();
            let lines: Vec<&str> = text.lines().collect();
            assert_eq!(
                lines[..2],
                ["(version 1)", "(allow default)"],
                "{name}:\n{text}"
            );
            for rule in [
                "(deny job-creation)",
                "(deny lsopen)",
                "(deny appleevent-send)",
            ] {
                assert!(lines.contains(&rule), "{name} lacks {rule}:\n{text}");
            }
            let lookup = lines
                .iter()
                .find(|line| line.starts_with("(deny mach-lookup "))
                .unwrap_or_else(|| panic!("{name} denies no mach-lookup:\n{text}"));
            for service in [
                "(global-name \"com.apple.coreservices.launchservicesd\")",
                "(global-name \"com.apple.coreservices.appleevents\")",
                "(global-name \"com.apple.xpc.smd\")",
            ] {
                assert!(lookup.contains(service), "{name}: {service} in {lookup}");
            }
            assert!(
                !lines.contains(&"(deny mach-lookup)"),
                "{name}: not every service, as the build's tools look some up:\n{text}"
            );
            let first_allow = lines
                .iter()
                .skip(2)
                .position(|line| line.starts_with("(allow "));
            let last_launch = lines
                .iter()
                .skip(2)
                .rposition(|line| {
                    line.starts_with("(deny job-creation")
                        || line.starts_with("(deny lsopen")
                        || line.starts_with("(deny appleevent-send")
                        || line.starts_with("(deny mach-lookup")
                })
                .expect("the launch denials");
            assert!(
                first_allow.is_none_or(|allow| last_launch < allow),
                "{name}: no rule allows a launch back:\n{text}"
            );
        }
    }

    /// story:wave-35-review-fixes, M2: the test command connects to no socket launchd made for
    /// the user's session, the ssh agent's among them.
    #[test]
    fn the_test_command_connects_to_no_launchd_socket() {
        let text = test_command(&TestCommand::default()).text();
        assert!(
            text.lines().any(|line| line
                == "(deny network-outbound (remote unix-socket (path-regex #\"^/private/tmp/com\\.apple\\.launchd\\.\")))"),
            "{text}"
        );
    }

    /// story:wave-35-review-fixes, M5: what the test command never writes of the git
    /// directories it may write is denied after every rule that allows a write.
    #[test]
    fn the_test_command_never_writes_the_sealed_git_paths() {
        let profile = test_command(&TestCommand {
            writable: vec![
                PathBuf::from("/nonexistent-tree"),
                PathBuf::from("/nonexistent-common"),
            ],
            sealed_dirs: vec![PathBuf::from("/nonexistent-common/hooks")],
            sealed_files: vec![
                PathBuf::from("/nonexistent-common/config"),
                PathBuf::from("/nonexistent-tree/.git"),
            ],
            ..TestCommand::default()
        });
        let text = profile.text();
        let params: Vec<String> = profile
            .params()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let lines: Vec<&str> = text.lines().collect();
        let last_allow = lines
            .iter()
            .rposition(|line| line.starts_with("(allow file-write*"))
            .expect("an allowed write");
        for (path, filter) in [
            ("/nonexistent-common/hooks", "subpath"),
            ("/nonexistent-common/config", "literal"),
            ("/nonexistent-tree/.git", "literal"),
        ] {
            let n = params
                .iter()
                .position(|p| p == path)
                .unwrap_or_else(|| panic!("{path} in {params:?}"));
            let wanted = format!("({filter} (param \"P{n}\"))");
            let at = text
                .lines()
                .position(|line| line.starts_with("(deny file-write* ") && line.contains(&wanted))
                .unwrap_or_else(|| panic!("{path} denied as {filter}:\n{text}"));
            assert!(last_allow < at, "{path} denied after the allows:\n{text}");
        }
    }

    #[test]
    fn a_program_name_is_found_on_the_childs_path() {
        assert_eq!(
            resolve(
                OsStr::new("sh"),
                Some(OsStr::new("/nonexistent:/bin")),
                None
            ),
            OsString::from("/bin/sh")
        );
        assert_eq!(
            resolve(OsStr::new("./x"), Some(OsStr::new("/bin")), None),
            OsString::from("./x")
        );
        assert_eq!(
            resolve(
                OsStr::new("no-such-program-for-takt"),
                Some(OsStr::new("/bin")),
                None
            ),
            OsString::from("no-such-program-for-takt")
        );
    }
}
