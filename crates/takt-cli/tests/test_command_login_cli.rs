//! story:test-command-reaches-no-forge-login through the binary: the test command `takt run
//! --harness none` runs, and every child of it, gets a scratch `HOME`, no `XDG_RUNTIME_DIR`, no
//! `DBUS_SESSION_BUS_ADDRESS` and no session bus socket, so `gh`, a git credential helper or a
//! keyring client in it finds no login of the operator's, while the same `gh` in the operator's
//! own environment still finds one.
//!
//! The operator here is a scratch one: a home holding a `gh` login and a crates.io token, a
//! runtime directory holding a listening `bus` socket, and a second socket the bus address names
//! outside it. `gh` is a stand-in that looks for a login where `gh` does (`GH_CONFIG_DIR`, then
//! `XDG_CONFIG_HOME/gh`, then `~/.config/gh`; the keyring through the session bus the address
//! names, else `$XDG_RUNTIME_DIR/bus`, else `/run/user/<uid>/bus`), so no network is used.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

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

/// An executable file at `path` holding `text`.
fn script(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// The stand-in `gh auth status`: a login when it finds one where `gh` looks, else none.
const GH: &str = r#"#!/bin/sh
dir=${GH_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/gh}
if [ -f "$dir/hosts.yml" ]; then
  echo "github.com: Logged in to github.com account operator ($dir/hosts.yml)"
  exit 0
fi
bus=${DBUS_SESSION_BUS_ADDRESS:-unix:path=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/bus}
bus=${bus#unix:path=}
if [ -S "$bus" ]; then
  echo "github.com: Logged in to github.com account operator (keyring through $bus)"
  exit 0
fi
echo "You are not logged into any GitHub hosts. To log in, run: gh auth login" >&2
exit 1
"#;

/// The operator's environment: `PATH` with the stand-in `gh` first, `HOME`, `XDG_CONFIG_HOME`,
/// `GH_CONFIG_DIR`, `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` naming the scratch
/// operator's, and `TMPDIR` inside the scratch directory.
fn operator_env(command: &mut Command, outer: &Path) {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![outer.join("bin")];
    dirs.extend(std::env::split_paths(&path));
    command
        .env_clear()
        .env("PATH", std::env::join_paths(dirs).expect("a PATH"))
        .env("HOME", outer.join("home"))
        .env("XDG_CONFIG_HOME", outer.join("home/.config"))
        .env("GH_CONFIG_DIR", outer.join("home/.config/gh"))
        .env("XDG_RUNTIME_DIR", outer.join("r"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}", outer.join("b/bus").display()),
        )
        .env("TMPDIR", outer.join("tmp"));
}

/// The test command run by `takt run --harness none` finds no login, no bus address and no bus
/// socket, cannot unmount what hides them, and its scratch home is gone after the run; the
/// stand-in `gh` in the operator's environment finds the login. On macOS the sandbox denies the
/// reads of the runtime directory, the named socket and the registry login.
#[test]
fn the_test_command_reaches_no_login_of_the_operator() {
    let outer =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("login-{}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let _scratch = Scratch(outer.clone());
    for dir in ["r", "b", "tmp", "home/.config/gh", "home/.cargo"] {
        fs::create_dir_all(outer.join(dir)).expect("a directory");
    }
    fs::write(
        outer.join("home/.config/gh/hosts.yml"),
        "github.com:\n    user: operator\n",
    )
    .expect("a gh login");
    fs::write(
        outer.join("home/.cargo/credentials.toml"),
        "[registry]\ntoken = \"operator-registry-token\"\n",
    )
    .expect("a registry token");
    let _runtime_bus = UnixListener::bind(outer.join("r/bus")).expect("the runtime bus socket");
    let _named_bus = UnixListener::bind(outer.join("b/bus")).expect("the named bus socket");
    script(&outer.join("bin/gh"), GH);

    // In the temporary directory takt runs with, which the test command may write on macOS too.
    let seen = outer.join("tmp/seen");
    let runtime = outer.join("r");
    let named = outer.join("b/bus");
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    script(
        &repo.join("probe.sh"),
        &format!(
            "#!/bin/sh\nexec > '{seen}' 2>&1\n\
             gh auth status; echo \"-- bus\"; echo $DBUS_SESSION_BUS_ADDRESS; echo \"-- socket\"; ls /run/user/$(id -u)/bus\n\
             echo \"-- home $HOME\"\necho \"-- runtime $XDG_RUNTIME_DIR\"\n\
             [ -S '{runtime}/bus' ] && echo 'runtime bus reachable'\n\
             [ -S '{named}' ] && echo 'named bus reachable'\n\
             cat \"${{CARGO_HOME:-$HOME/.cargo}}/credentials.toml\"\n\
             umount '{runtime}'; umount -l '{runtime}'; umount '{named}'; umount -l '{named}'\n\
             [ -S '{runtime}/bus' ] && echo 'runtime bus reachable after umount'\n\
             [ -S '{named}' ] && echo 'named bus reachable after umount'\n\
             echo '-- end'\nexit 0\n",
            seen = seen.display(),
            runtime = runtime.display(),
            named = named.display(),
        ),
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);

    let mut operator = Command::new(outer.join("bin/gh"));
    operator_env(&mut operator, &outer);
    let output = operator.output().expect("gh runs");
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && printed.contains("Logged in"),
        "the operator's own gh finds the login: {printed}"
    );

    let mut takt = Command::new(env!("CARGO_BIN_EXE_takt"));
    takt.args([
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--intent",
        "probe",
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "1",
        "--test-cmd",
        "sh probe.sh",
    ]);
    operator_env(&mut takt, &outer);
    let output = takt.output().expect("takt runs");
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let seen = fs::read_to_string(&seen)
        .unwrap_or_else(|error| panic!("the probe ran ({error}): stdout {out} stderr {err}"));
    assert_eq!(
        output.status.code(),
        Some(3),
        "one iteration then the budget stop: stdout {out} stderr {err}\nprobe: {seen}"
    );
    let lines: Vec<&str> = seen.lines().collect();
    let after = |marker: &str| {
        let at = lines
            .iter()
            .position(|line| *line == marker)
            .unwrap_or_else(|| panic!("{marker:?} in the probe's output:\n{seen}"));
        lines.get(at + 1).copied().unwrap_or_default()
    };

    assert!(
        seen.contains("You are not logged into any GitHub hosts") && !seen.contains("Logged in"),
        "gh finds no login:\n{seen}"
    );
    assert_eq!(after("-- bus"), "", "an empty bus address:\n{seen}");
    assert!(
        after("-- socket").contains("No such file or directory"),
        "no session bus socket under /run/user:\n{seen}"
    );
    assert!(
        lines.contains(&"-- runtime "),
        "no XDG_RUNTIME_DIR:\n{seen}"
    );
    for reachable in [
        "runtime bus reachable",
        "named bus reachable",
        "runtime bus reachable after umount",
        "named bus reachable after umount",
    ] {
        assert!(!lines.contains(&reachable), "{reachable}:\n{seen}");
    }
    assert!(
        !seen.contains("operator-registry-token"),
        "no registry token:\n{seen}"
    );
    assert!(
        lines.contains(&"-- end"),
        "the probe ran to its end:\n{seen}"
    );
    let home = lines
        .iter()
        .find_map(|line| line.strip_prefix("-- home "))
        .unwrap_or_else(|| panic!("a home line:\n{seen}"));
    assert!(
        !home.is_empty() && Path::new(home) != outer.join("home"),
        "a scratch home, not the operator's: {home}\n{seen}"
    );
    assert!(
        !Path::new(home).exists(),
        "the scratch home {home} is removed after the run"
    );
}

/// What `id` prints for `flag`, trimmed.
fn id(flag: &str) -> String {
    let output = Command::new("id").arg(flag).output().expect("id runs");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// One `takt run --harness none` whose test command reports what it sees of the registry login
/// (`${CARGO_HOME:-$HOME/.cargo}/credentials.toml`), with `HOME` as `home` gives it (unset
/// when none), run under bubblewrap with `passwd` as `/etc/passwd`, so the home this user's
/// passwd entry names is the scratch one and the operator's own is never looked at.
fn registry_probe(
    outer: &Path,
    passwd: &Path,
    case: &str,
    home: Option<&std::ffi::OsStr>,
) -> String {
    let seen = outer.join(format!("seen-{case}"));
    let mut takt = Command::new("bwrap");
    takt.args(["--dev-bind", "/", "/", "--ro-bind"])
        .arg(passwd)
        .arg("/etc/passwd")
        .arg("--")
        .arg(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(outer.join("repo"))
        .args([
            "--intent",
            "probe",
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
        ])
        .arg(format!("sh probe.sh {}", seen.display()))
        .args(["--case-id", case])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("XDG_STATE_HOME", outer.join("state"))
        .env("TMPDIR", outer.join("tmp"));
    if let Some(home) = home {
        takt.env("HOME", home);
    }
    let output = takt.output().expect("bwrap runs (CI installs bubblewrap)");
    let printed = format!(
        "exit {:?}\nstdout {}\nstderr {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let seen = fs::read_to_string(&seen)
        .unwrap_or_else(|error| panic!("the probe ran ({error}): {printed}"));
    format!("{seen}\n{printed}")
}

/// Whatever `HOME` takt starts with (unset, relative or set), the test command finds no registry
/// login as a file it can read: the registry login under whichever `CARGO_HOME` the command
/// sees is covered. With no absolute `HOME` the command's `CARGO_HOME` is the passwd home's
/// `.cargo`, here a scratch one holding a stand-in token.
#[test]
fn a_test_command_finds_no_registry_login_whatever_home_takt_has() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("login-cli-registry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    for dir in ["tmp", "state", "home/.cargo"] {
        fs::create_dir_all(outer.join(dir)).expect("a directory");
    }
    fs::write(
        outer.join("home/.cargo/credentials.toml"),
        "token = \"operator-registry-token\"\n",
    )
    .expect("a stand-in token");
    let passwd = outer.join("passwd");
    fs::write(
        &passwd,
        format!(
            "root:x:0:0::/root:/bin/sh\n{}:x:{}:{}::{}:/bin/sh\n",
            id("-un"),
            id("-u"),
            id("-g"),
            outer.join("home").display()
        ),
    )
    .expect("a passwd file");
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    // The probe only reports the kind of the file; it never reads it.
    script(
        &repo.join("probe.sh"),
        "#!/bin/sh\nexec > \"$1\" 2>&1\n\
         echo \"-- home $HOME\"\necho \"-- cargo $CARGO_HOME\"\n\
         f=\"${CARGO_HOME:-$HOME/.cargo}/credentials.toml\"\n\
         if [ -e \"$f\" ]; then echo \"-- kind $(stat -L -c %F \"$f\")\"; else echo '-- kind absent'; fi\n\
         echo '-- end'\nexit 0\n",
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let set = outer.join("home");
    // Every state is run before anything is asserted, so a failure names each state's outcome.
    let mut failed = Vec::new();
    for (case, home) in [
        ("unset", None),
        ("relative", Some(std::ffi::OsStr::new("relative-home"))),
        ("set", Some(set.as_os_str())),
    ] {
        let seen = registry_probe(&outer, &passwd, case, home);
        let kind = seen
            .lines()
            .find_map(|line| line.strip_prefix("-- kind "))
            .unwrap_or("none reported")
            .to_owned();
        let cargo = seen
            .lines()
            .find_map(|line| line.strip_prefix("-- cargo "))
            .unwrap_or("none reported")
            .to_owned();
        eprintln!("HOME {case}: {kind}; CARGO_HOME {cargo}");
        if !seen.contains("-- end") || seen.contains("-- kind regular file") {
            failed.push(format!(
                "HOME {case}: the registry login is a regular file the test command can read, or the probe did not end:\n{seen}"
            ));
        }
        // With no absolute HOME, the command's toolchain home is the passwd home's `.cargo`, the
        // stand-in here, never one resolved inside the tree, and its login is covered.
        if case != "set" {
            if !seen.contains(&format!("-- cargo {}", outer.join("home/.cargo").display())) {
                failed.push(format!(
                    "HOME {case}: the command's CARGO_HOME is not the passwd home's:\n{seen}"
                ));
            }
            if kind != "character special file" {
                failed.push(format!(
                    "HOME {case}: the stand-in registry login is not covered ({kind}):\n{seen}"
                ));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
