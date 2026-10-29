//! A test binary's own program directories: before `main`, while the process has one thread,
//! every directory on `PATH` that this user can write is replaced by a private directory of
//! links to its programs, and `CARGO_HOME` is set to a private directory that links to every
//! entry of the cargo home but `bin`, which is that private copy too. A run's step guard reads
//! the program directories from this process's environment (`program_dirs` in
//! `crates/takt/src/run.rs`), as does every `takt` this binary starts, so a program another
//! process installs into the real `~/.cargo/bin` or any other directory on the user's `PATH`
//! while a test runs reaches no step of it
//! (story:an-unrelated-program-install-refuses-no-step). A step that writes into one of the
//! private directories is still seen, as it was in the real one. Every test file that runs a
//! step under the guard includes this module (`tests_have_a_home_of_their_own.rs`).
//!
//! The private directories are under this crate's `CARGO_TARGET_TMPDIR`, or the system temporary
//! directory for unit tests, and removed when the process exits. Any failure to make them aborts
//! the binary before a test runs.
#![cfg(unix)]

/// The rest of what a test binary reads of the operator's state: git's automatic maintenance
/// and aep's protocol cache (story:gate-tests-are-deterministic).
#[path = "own_state.rs"]
mod own_state;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The private directory of this process, removed at exit.
static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The environment variable that names the private directory, so a test can tell it is in
/// place. A test binary that inherits it is a child an isolated test started (itself again,
/// under a `HOME`, `PATH` or `CARGO_HOME` that test built), and keeps the environment it was
/// given.
const MARK: &str = "TAKT_TEST_OWN_HOME";

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[used]
#[cfg_attr(target_os = "linux", unsafe(link_section = ".init_array"))]
#[cfg_attr(target_os = "macos", unsafe(link_section = "__DATA,__mod_init_func"))]
static BEFORE_MAIN: extern "C" fn() = before_main;

/// `access(2)`'s `W_OK`, the same on Linux and macOS.
const W_OK: std::ffi::c_int = 2;

extern "C" fn before_main() {
    if std::env::var_os(MARK).is_some() {
        return;
    }
    if let Err(error) = isolate() {
        eprintln!("own_home: the test binary's own program directories: {error}");
        std::process::abort();
    }
}

extern "C" fn at_exit() {
    if let Some(root) = ROOT.get() {
        let _ = std::fs::remove_dir_all(root);
    }
}

/// Whether this user can write `dir`, as the step guard asks (`writable` in `run.rs`).
fn writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a valid NUL-terminated string that outlives the call.
    unsafe { libc::access(path.as_ptr(), W_OK) == 0 }
}

/// Whether the entry at `path` is one a lookup by name may find, as the step guard lists a
/// program directory: a link, or an executable file.
fn a_program(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::symlink_metadata(path).is_ok_and(|meta| {
        meta.file_type().is_symlink() || (meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// Makes `copy` a directory of links, one to each program directly in `dir` by its path there.
fn copy_programs(dir: &Path, copy: &Path) -> std::io::Result<()> {
    std::fs::create_dir(copy)?;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if a_program(&path)
            && let Some(name) = path.file_name()
        {
            std::os::unix::fs::symlink(&path, copy.join(name))?;
        }
    }
    Ok(())
}

fn isolate() -> std::io::Result<()> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let tmp = option_env!("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let root = tmp.join(format!("own-home-{}-{nanos:09}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    std::fs::create_dir(&root)?;
    let _ = ROOT.set(root.clone());
    // SAFETY: `atexit` takes a function that lives for the whole process.
    unsafe {
        libc::atexit(at_exit);
    }
    // Each real program directory, at its real path, to its private copy.
    let mut copies: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut copy_of = |dir: &Path| -> std::io::Result<PathBuf> {
        let real = std::fs::canonicalize(dir)?;
        if let Some((_, copy)) = copies.iter().find(|(seen, _)| *seen == real) {
            return Ok(copy.clone());
        }
        let copy = root.join(format!("bin-{}", copies.len()));
        copy_programs(&real, &copy)?;
        copies.push((real, copy.clone()));
        Ok(copy)
    };

    // The cargo home the guard reads (`CARGO_HOME`, else `~/.cargo`): when there is one to
    // read, a private one in its place, every entry linked but `bin`, which is copied.
    let cargo = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    let own_cargo = match cargo {
        Some(cargo) => {
            let own = root.join("cargo-home");
            std::fs::create_dir(&own)?;
            if cargo.is_dir() {
                for entry in std::fs::read_dir(&cargo)? {
                    let path = entry?.path();
                    let Some(name) = path.file_name() else {
                        continue;
                    };
                    if name == "bin" && path.is_dir() {
                        std::os::unix::fs::symlink(copy_of(&path)?, own.join(name))?;
                    } else {
                        std::os::unix::fs::symlink(&path, own.join(name))?;
                    }
                }
            }
            Some(own)
        }
        None => None,
    };

    // `PATH` with each directory this user can write replaced by its copy; one with no `PATH`
    // keeps none.
    let path = match std::env::var_os("PATH") {
        Some(inherited) => {
            let mut path = Vec::new();
            for dir in std::env::split_paths(&inherited) {
                if !dir.is_absolute() {
                    path.push(dir);
                } else if dir.is_dir() {
                    if writable(&dir) {
                        path.push(copy_of(&dir)?);
                    } else {
                        path.push(dir);
                    }
                }
                // A directory that does not exist yet finds nothing, and one made there later
                // would be another process's: it is left out.
            }
            Some(std::env::join_paths(path).map_err(std::io::Error::other)?)
        }
        None => None,
    };
    // SAFETY: this runs before `main`, while the process has one thread.
    unsafe {
        if let Some(own_cargo) = &own_cargo {
            std::env::set_var("CARGO_HOME", own_cargo);
        }
        if let Some(path) = &path {
            std::env::set_var("PATH", path);
        }
        std::env::set_var(MARK, &root);
    }
    Ok(())
}
