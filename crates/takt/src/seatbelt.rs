//! Seatbelt, the macOS platform sandbox, as takt confines a child with it there: a profile takt
//! writes ([`Profile`]), applied by [`SANDBOX_EXEC`] before it executes the child. On macOS it
//! stands where Linux has bubblewrap or namespaces of takt's own:
//!
//! | child | profile |
//! |---|---|
//! | the promotion's build and its binary's `--version` (`crate::promotion::Confinement`) | [`build`]: no network, writes only to the directories the build owns, reads under the user's home only of the ones it needs |
//! | the test command (`crate::forge::without_login`) | [`test_command`]: writes only to the tree, the fresh home, the temporary directory and the cargo caches; no read of a registry login, the user's keychains, the runtime directory or a bus socket |
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

/// The profile the promotion's build and its binary's `--version` run under: no network; writes
/// only to the devices and under `writable`; reads under each of `homes` only under `writable`
/// and `readable`, and of the directories above them there (`file-read-metadata`). Every
/// path real ([`real`]).
#[must_use]
pub fn build(writable: &[PathBuf], readable: &[PathBuf], homes: &[PathBuf]) -> Profile {
    let real_all = |paths: &[PathBuf]| -> Vec<PathBuf> { paths.iter().map(|p| real(p)).collect() };
    let writable = real_all(writable);
    let readable = real_all(readable);
    let homes = real_all(homes);
    let seen: Vec<PathBuf> = writable.iter().chain(&readable).cloned().collect();
    let mut profile = Profile::new();
    profile
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
    /// Written to as a whole: the tree, the fresh home and the temporary directories.
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

/// The profile a test command runs under on macOS: writes only to the devices, under
/// `writable` and to the cargo caches; no read nor write of a registry login, the user's
/// keychains, a hidden directory or a hidden file, and no connection to a hidden socket. The
/// network is not denied, as it is not on Linux. Every path real ([`real`]).
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
    let mut profile = Profile::new();
    profile
        .deny("file-write*")
        .devices()
        .paths(Action::Allow, "file-write*", Filter::Subpath, &writable)
        .paths(Action::Allow, "file-write*", Filter::Subpath, &caches)
        .paths(Action::Allow, "file-write*", Filter::Literal, &tops);
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

/// This user's temporary and cache directories as macOS names them
/// (`confstr(_CS_DARWIN_USER_TEMP_DIR)` and `confstr(_CS_DARWIN_USER_CACHE_DIR)`), under
/// `/var/folders`, where the developer tools (`xcrun`, the compiler and the linker) keep their
/// temporary files and caches whatever `TMPDIR` says; each one the system names.
#[cfg(target_os = "macos")]
#[must_use]
pub fn user_dirs() -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    let mut found = Vec::new();
    for name in [
        libc::_CS_DARWIN_USER_TEMP_DIR,
        libc::_CS_DARWIN_USER_CACHE_DIR,
    ] {
        let mut buffer = vec![0_u8; 1024];
        // SAFETY: `confstr` writes at most `buffer.len()` bytes, NUL included, into the buffer.
        let needed = unsafe { libc::confstr(name, buffer.as_mut_ptr().cast(), buffer.len()) };
        if needed == 0 || needed > buffer.len() {
            continue;
        }
        buffer.truncate(needed - 1);
        let dir = PathBuf::from(OsStr::from_bytes(&buffer));
        if dir.is_absolute() {
            found.push(dir);
        }
    }
    found
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
