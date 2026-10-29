//! story:an-unrelated-program-install-refuses-no-step, acceptance 1: a test that runs steps
//! under the step guard reads program directories of its own, so a program another process
//! installs into the real `~/.cargo/bin`, or any directory on the user's `PATH`, while the test
//! runs does not reach it. The guard reads them from the process's `PATH` and `CARGO_HOME`
//! (`program_dirs` in `crates/takt/src/run.rs`); `support/own_home.rs` gives each test binary
//! its own before `main`, and every test file that opens a run or starts `takt` includes it.

#[path = "support/own_home.rs"]
mod own_home;

use std::path::{Path, PathBuf};

/// The private directory `own_home` made for this process.
fn own_root() -> PathBuf {
    let root = std::env::var_os("TAKT_TEST_OWN_HOME")
        .map(PathBuf::from)
        .expect("TAKT_TEST_OWN_HOME: this test binary has no program directories of its own");
    std::fs::canonicalize(&root).expect("the private directory exists")
}

unsafe extern "C" {
    fn access(path: *const std::ffi::c_char, mode: std::ffi::c_int) -> std::ffi::c_int;
}

/// Whether this user can write `dir`, as the step guard asks: `access(2)` with `W_OK`, nothing
/// written.
fn writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a valid NUL-terminated string that outlives the call.
    unsafe { access(path.as_ptr(), 2) == 0 }
}

#[test]
fn no_program_directory_the_guard_reads_is_shared_with_another_process() {
    let root = own_root();
    let path = std::env::var_os("PATH").expect("a PATH");
    let mut shared = Vec::new();
    for dir in std::env::split_paths(&path).filter(|dir| dir.is_absolute()) {
        let Ok(real) = std::fs::canonicalize(&dir) else {
            shared.push(format!("{} (does not exist yet)", dir.display()));
            continue;
        };
        if real.is_dir() && !real.starts_with(&root) && writable(&real) {
            shared.push(real.display().to_string());
        }
    }
    let cargo = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .expect("a CARGO_HOME of this binary's own");
    let cargo = std::fs::canonicalize(&cargo).expect("the cargo home exists");
    assert!(
        cargo.starts_with(&root),
        "CARGO_HOME is {}, outside {}",
        cargo.display(),
        root.display()
    );
    if let Ok(bin) = std::fs::canonicalize(cargo.join("bin")) {
        assert!(
            bin.starts_with(&root),
            "$CARGO_HOME/bin is {}, outside {}",
            bin.display(),
            root.display()
        );
    }
    assert!(
        shared.is_empty(),
        "PATH holds directories another process can write: {shared:?}"
    );
}

#[test]
fn the_real_cargo_bin_is_not_on_the_path() {
    let root = own_root();
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let Ok(real) = std::fs::canonicalize(home.join(".cargo/bin")) else {
        return;
    };
    let path = std::env::var_os("PATH").expect("a PATH");
    let on_path: Vec<PathBuf> = std::env::split_paths(&path)
        .filter_map(|dir| std::fs::canonicalize(dir).ok())
        .filter(|dir| *dir == real)
        .collect();
    assert!(
        on_path.is_empty(),
        "{} is on PATH, not a copy under {}",
        real.display(),
        root.display()
    );
}

/// A test file runs steps under the guard when it opens a run in this process or starts the
/// `takt` binary.
const RUNS_STEPS: &[&str] = &[
    "Run::open",
    "Run::resume",
    "Run::take_next",
    "Run::dispatch",
    "CARGO_BIN_EXE_takt",
];

#[test]
fn every_test_file_that_runs_steps_has_a_home_of_its_own() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory");
    let mut missing = Vec::new();
    let mut counted = 0;
    for dir in [crates.join("takt/tests"), crates.join("takt-cli/tests")] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
            .map(|entry| entry.expect("an entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file).expect("a test file");
            if RUNS_STEPS.iter().any(|needle| text.contains(needle)) {
                counted += 1;
                if !text.contains("support/own_home.rs\"]\nmod own_home;") {
                    missing.push(
                        file.strip_prefix(crates)
                            .unwrap_or(&file)
                            .display()
                            .to_string(),
                    );
                }
            }
        }
    }
    assert!(
        counted > 0,
        "no test file runs steps: the patterns find nothing"
    );
    assert!(
        missing.is_empty(),
        "{} of {counted} test files that run steps have no home of their own: {missing:#?}",
        missing.len()
    );
}
