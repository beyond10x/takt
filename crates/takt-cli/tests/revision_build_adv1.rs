//! story:wave11-review-follow-ups, adversary pass 1 on the build script (items 3 and 4): every
//! filter driver turned off while `status` reads the revision, and a root `.cargo/` counted as
//! compiled in. The scratch repositories are laid out as `revision_build.rs` lays them out: a
//! root manifest, the probe crate under `crates/`, its build script the one under attack. Each
//! case is named for what it asserts.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

#[allow(dead_code)]
#[path = "../build.rs"]
mod build;

/// The build script under attack, copied into the probe crate of each scratch repository.
const BUILD_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs");

/// `git <args>` in `tree`, the caller's redirecting git environment left out; its trimmed
/// standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "advice.addEmbeddedRepo=false",
        ])
        .args(args)
        .current_dir(tree)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A scratch workspace on `main` whose one commit holds a root manifest, a `README.md`, a
/// `.gitignore` of `/target` and `*.log`, and the probe crate, whose `main` prints
/// `TAKT_REVISION` and then whether `PROBE_CONFIGURED` was set when it was compiled.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("revision-build-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let probe = dir.join("crates/probe");
    fs::create_dir_all(probe.join("src")).expect("a directory");
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/probe\"]\nresolver = \"3\"\n",
    )
    .expect("a root manifest");
    fs::write(dir.join("README.md"), "a probe\n").expect("a readme");
    fs::write(dir.join(".gitignore"), "/target\n*.log\n").expect("an ignore file");
    fs::write(
        probe.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .expect("a manifest");
    fs::copy(BUILD_RS, probe.join("build.rs")).expect("the build script is copied");
    fs::write(
        probe.join("src/main.rs"),
        "fn main() {\n    println!(\"{}\", env!(\"TAKT_REVISION\"));\n    println!(\"{:?}\", option_env!(\"PROBE_CONFIGURED\"));\n}\n",
    )
    .expect("a source");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// The probe crate's directory in `root`.
fn probe(root: &Path) -> PathBuf {
    root.join("crates/probe")
}

/// Builds the workspace at `root` the plain way and runs the probe: the revision it printed,
/// and whether `PROBE_CONFIGURED` was compiled in.
fn build_and_run(root: &Path) -> (String, String) {
    let built = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .args(["build", "--offline", "-q"])
        .current_dir(root)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("cargo runs");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let ran = Command::new(root.join("target/debug/probe"))
        .output()
        .expect("the probe runs");
    let stdout = String::from_utf8(ran.stdout).expect("utf-8");
    let mut lines = stdout.lines();
    (
        lines.next().unwrap_or_default().to_owned(),
        lines.next().unwrap_or_default().to_owned(),
    )
}

/// Sets the mtime of `path` far in the past, so `status` finds its stat changed and reads the
/// file again, through any filter its attributes name.
fn touch_old(path: &Path) {
    fs::File::options()
        .write(true)
        .open(path)
        .expect("a tracked file")
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000))
        .expect("its mtime is set");
}

/// Item 3. A checked-out submodule is read by `status` in a git of its own, which reads the
/// submodule's own configuration and its own `info/attributes`: a filter driver defined there
/// is not one the top repository's `git config` lists, so no `-c filter.<driver>.clean=` turns
/// it off, and its clean command runs while the build reads the revision.
#[test]
fn a_build_runs_no_command_of_a_filter_a_submodule_configures_for_itself() {
    let dir = repository("submodule-filter");
    let upstream = dir.with_extension("upstream");
    let _ = fs::remove_dir_all(&upstream);
    fs::create_dir_all(&upstream).expect("a directory");
    fs::write(upstream.join("lib.txt"), "a library\n").expect("a file");
    git(&upstream, &["init", "-q", "-b", "main"]);
    git(&upstream, &["add", "-A"]);
    git(&upstream, &["commit", "-q", "-m", "start"]);
    git(
        &dir,
        &[
            "submodule",
            "add",
            "-q",
            upstream.to_str().expect("utf-8"),
            "vendor/lib",
        ],
    );
    git(&dir, &["commit", "-q", "-m", "a submodule"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let sub = dir.join("vendor/lib");
    let ran = dir.join("filter-ran.log");
    git(
        &sub,
        &[
            "config",
            "filter.subonly.clean",
            &format!("echo clean >> '{}'; cat", ran.display()),
        ],
    );
    let attributes = PathBuf::from(git(
        &sub,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "info/attributes",
        ],
    ));
    fs::create_dir_all(attributes.parent().expect("a parent")).expect("a directory");
    fs::write(&attributes, "* filter=subonly\n").expect("the submodule's own attributes");
    touch_old(&sub.join("lib.txt"));
    let revision = build::revision(&probe(&dir));
    let _ = fs::remove_dir_all(&upstream);
    assert!(
        !ran.exists(),
        "the submodule's filter ran while the revision was read: {:?} (revision {revision})",
        fs::read_to_string(&ran)
    );
    assert_eq!(revision, head);
}

/// Item 3, the shape of git-lfs: a `process` filter that is `required`, named in the
/// repository's own `info/attributes` for a file under `crates/`, with a driver name holding
/// dots and capitals. None of its commands runs, and the file is read as it is.
#[test]
fn a_build_runs_no_required_process_filter_whatever_its_name() {
    let dir = repository("required-process");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let ran = dir.join("filter-ran.log");
    for driver in ["Lfs.Like.v2", "lfs"] {
        git(
            &dir,
            &[
                "config",
                &format!("filter.{driver}.process"),
                &format!("sh -c 'echo process >> \"{}\"; exit 1'", ran.display()),
            ],
        );
        git(
            &dir,
            &["config", &format!("filter.{driver}.required"), "true"],
        );
    }
    fs::create_dir_all(dir.join(".git/info")).expect("a directory");
    fs::write(
        dir.join(".git/info/attributes"),
        "*.rs filter=Lfs.Like.v2\n*.toml filter=lfs\n",
    )
    .expect("the repository's own attributes");
    touch_old(&probe(&dir).join("src/main.rs"));
    touch_old(&probe(&dir).join("Cargo.toml"));
    let revision = build::revision(&probe(&dir));
    assert!(
        !ran.exists(),
        "a required process filter ran: {:?} (revision {revision})",
        fs::read_to_string(&ran)
    );
    assert_eq!(revision, head);
}

/// Item 4. A root `.cargo/config.toml` made after a clean build, whose `[env]` cargo compiles
/// into the binary on the next build: a binary built with an untracked configuration must not
/// print the clean revision. The script does not watch a `.cargo/` that did not exist when it
/// last ran, so cargo rebuilds the crate with the new `[env]` and does not run the script.
#[test]
#[ignore = "known limit: a root .cargo created after the build script last ran is seen at its next rerun; see build.rs"]
fn a_root_cargo_config_made_after_a_build_is_not_compiled_in_under_a_clean_revision() {
    let dir = repository("cargo-after");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(build_and_run(&dir), (head.clone(), "None".to_owned()));
    fs::create_dir_all(dir.join(".cargo")).expect("a directory");
    fs::write(
        dir.join(".cargo/config.toml"),
        "[env]\nPROBE_CONFIGURED = \"1\"\n",
    )
    .expect("a cargo configuration");
    assert_eq!(build::revision(&probe(&dir)), format!("{head}-dirty"));
    let (printed, configured) = build_and_run(&dir);
    assert!(
        configured == "None" || printed == format!("{head}-dirty"),
        "the binary was compiled with the untracked .cargo/config.toml ({configured}) and prints {printed}"
    );
}

/// Item 4. An untracked `.cargo` that is a link to a directory elsewhere is compiled in as
/// much as a directory is: cargo reads its `config.toml` through the link.
#[test]
fn an_untracked_root_cargo_link_makes_the_revision_dirty() {
    let dir = repository("cargo-link");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let elsewhere = dir.with_extension("cargo-config");
    let _ = fs::remove_dir_all(&elsewhere);
    fs::create_dir_all(&elsewhere).expect("a directory");
    fs::write(
        elsewhere.join("config.toml"),
        "[env]\nPROBE_CONFIGURED = \"1\"\n",
    )
    .expect("a cargo configuration");
    std::os::unix::fs::symlink(&elsewhere, dir.join(".cargo")).expect("a link");
    let revision = build::revision(&probe(&dir));
    let _ = fs::remove_dir_all(&elsewhere);
    assert_eq!(revision, format!("{head}-dirty"));
}
