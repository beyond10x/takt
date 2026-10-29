//! What a test binary reads of the operator's state besides its program directories
//! (`own_home.rs`), made its own before `main`, while the process has one thread
//! (story:gate-tests-are-deterministic):
//!
//! - Git's automatic maintenance (`maintenance.auto=false`, `gc.auto=0`, appended to
//!   `GIT_CONFIG_COUNT`), for every git this binary and its children run but takt's own,
//!   which sets both itself. A commit starts `git maintenance run --auto --detach`, which can
//!   repack a test's repository in the background and write `.git/info/refs` while a later
//!   step of a run is guarded: two loose objects in `objects/17`, the directory git samples,
//!   are enough (`records_reach_base_adv1`, story:gate-tests-are-deterministic).
//! - aep's protocol cache (`AEP_CACHE_DIR`): the caller's when it names one, else
//!   `~/.cache/aep`, the cache CI fills before the tests and `aep` uses without
//!   `XDG_CACHE_HOME`, so a caller's `XDG_CACHE_HOME` does not move the store tests to a cache
//!   that does not hold their fixtures' pinned protocol revision.
//!
//! The rustup home the step guard also reads (`program_dirs` in `crates/takt/src/run.rs`) is
//! not made private: the confined test command runs the toolchain bound read-only by its
//! canonical path, and a toolchain made of links to the real one is not whole inside that
//! bind.
//!
//! `own_home.rs` includes this module, so every test file that has program directories of its
//! own has this state of its own too.
#![cfg(unix)]

/// The environment variable that tells a test this module is in place. A test binary that
/// inherits it is a child an isolated test started, and keeps the environment it was given, as
/// `own_home.rs` does.
const MARK: &str = "TAKT_TEST_OWN_STATE";

/// The git configuration every git this binary starts runs with, appended to the caller's
/// `GIT_CONFIG_COUNT` entries.
const GIT_CONFIG: [(&str, &str); 2] = [("maintenance.auto", "false"), ("gc.auto", "0")];

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[used]
#[cfg_attr(target_os = "linux", unsafe(link_section = ".init_array"))]
#[cfg_attr(target_os = "macos", unsafe(link_section = "__DATA,__mod_init_func"))]
static BEFORE_MAIN: extern "C" fn() = before_main;

extern "C" fn before_main() {
    if std::env::var_os(MARK).is_some() {
        return;
    }
    let count = std::env::var("GIT_CONFIG_COUNT")
        .ok()
        .and_then(|count| count.parse::<usize>().ok())
        .unwrap_or(0);
    let aep_cache = std::env::var_os("AEP_CACHE_DIR")
        .filter(|dir| !dir.is_empty())
        .is_none()
        .then(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .filter(|home| home.is_absolute())
                .map(|home| home.join(".cache/aep"))
        })
        .flatten();
    // SAFETY: this runs before `main`, while the process has one thread.
    unsafe {
        for (n, (key, value)) in GIT_CONFIG.iter().enumerate() {
            std::env::set_var(format!("GIT_CONFIG_KEY_{}", count + n), key);
            std::env::set_var(format!("GIT_CONFIG_VALUE_{}", count + n), value);
        }
        std::env::set_var("GIT_CONFIG_COUNT", (count + GIT_CONFIG.len()).to_string());
        if let Some(aep_cache) = &aep_cache {
            std::env::set_var("AEP_CACHE_DIR", aep_cache);
        }
        std::env::set_var(MARK, "1");
    }
}
