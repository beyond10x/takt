//! Review pass 1 against story:ignored-directories-are-guarded: what the guard of the paths
//! below an ignored directory does at its edges. A rewrite that keeps the size and the time, or
//! that goes through a hard link or a descriptor opened before the step, is still seen; a file
//! the step only touched, changed the mode of or wrote with the same bytes is the operator's and
//! is moved out of the tree, never deleted; a name that is not UTF-8 is put back byte for byte;
//! a name with a line break shapes no line of the output; a link and a directory that cannot be
//! listed are guarded too. Every repository is a scratch one under the crate's build directory,
//! and what a refused step moves aside goes to a directory beside it; nothing is written outside
//! it.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{Seek as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit and test.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests

claims:
  tests.pass:
    description: pass
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: pass
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write
  tests.run:
    description: run the tests
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: pass
    requires:
      claim: tests.pass
";

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A scratch root under this crate's build directory; dropping it makes it writable again and
/// removes it.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl Scratch {
    /// Where a refused step's files inside an ignored directory are moved for the case `rv`
    /// and the step `step`.
    fn aside(&self, step: u64) -> PathBuf {
        self.root
            .join("aside")
            .join("rv")
            .join(format!("step-{step}"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = Command::new("chmod")
            .args(["-R", "u+rwx"])
            .arg(&self.root)
            .status();
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A committed repository with `ignore` as its `.gitignore` and one tracked file.
fn repository(name: &str, ignore: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ignored-dirs-adv1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), PROTOCOL).expect("the protocol");
    fs::write(repo.join(".gitignore"), ignore).expect("a file");
    fs::write(repo.join("src.txt"), "x\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

/// A harness whose first step does `attack`, and whose every step edits `notes.md`, a new
/// revision for the next `tests.run`.
struct Once<F: Fn(&Path)> {
    attack: F,
    done: std::cell::Cell<bool>,
}

impl<F: Fn(&Path)> Harness for Once<F> {
    fn name(&self) -> &str {
        "once"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !self.done.replace(true) {
            (self.attack)(context.tree);
        }
        fs::write(context.tree.join("notes.md"), "a note\n").expect("a write");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "s".to_owned(),
            tokens: Some(1),
        })
    }
}

/// A run of the case `rv`: a failing `tests.run` (step 1), then the harness's step (step 2).
fn go<F: Fn(&Path)>(scratch: &Scratch, attack: F) -> (anyhow::Result<Stop>, String) {
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let harness = Once {
        attack,
        done: std::cell::Cell::new(false),
    };
    let options = RunOptions {
        protocol,
        intent: "x".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("rv".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&scratch.repo, options, &harness, &validators).expect("opens");
    run.set_aside_in(scratch.root.join("aside"))
        .expect("a set-aside root");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

fn refused_naming(stop: &anyhow::Result<Stop>, path: &str) -> bool {
    matches!(stop, Ok(Stop::StepRefused(why)) if why.contains(path))
}

fn reset_mtime(path: &Path, to: std::time::SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("opens")
        .set_modified(to)
        .expect("a time");
}

/// The operator's file `path` (relative to the tree) is gone from the tree, and its bytes,
/// `bytes`, are at the same relative path under the step's set-aside directory, which the
/// refusal names.
fn moved_aside(
    scratch: &Scratch,
    stop: &anyhow::Result<Stop>,
    text: &str,
    path: &str,
    bytes: &str,
) {
    let aside = scratch.aside(2);
    assert!(
        refused_naming(stop, path),
        "the step was not refused naming {path}: {stop:?}\n{text}"
    );
    assert!(
        refused_naming(stop, &takt::trace::escape(&aside.display().to_string())),
        "the refusal does not name where {path} went, {}: {stop:?}",
        aside.display()
    );
    assert!(
        !scratch.repo.join(path).exists(),
        "{path} is still in the tree\n{text}"
    );
    assert_eq!(
        fs::read_to_string(aside.join(path)).ok().as_deref(),
        Some(bytes),
        "the operator's {path} is not under {}: {stop:?}",
        aside.display()
    );
}

#[test]
fn a_same_size_rewrite_with_the_mtime_reset_is_refused() {
    let s = repository("samesize", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let (stop, text) = go(&s, |tree| {
        let path = tree.join("build/old.o");
        let modified = fs::metadata(&path)
            .expect("a file")
            .modified()
            .expect("a time");
        fs::write(&path, "BBBB").expect("a write");
        reset_mtime(&path, modified);
    });
    moved_aside(&s, &stop, &text, "build/old.o", "BBBB");
}

#[test]
fn a_rewrite_through_a_hard_link_from_outside_is_refused() {
    let s = repository("hardlink", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let outside = s.root.join("outside.o");
    fs::hard_link(s.repo.join("build/old.o"), &outside).expect("a link");
    let (stop, text) = go(&s, move |_| {
        let modified = fs::metadata(&outside)
            .expect("a file")
            .modified()
            .expect("a time");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&outside)
            .expect("opens");
        file.write_all(b"BBBB").expect("a write");
        drop(file);
        reset_mtime(&outside, modified);
    });
    assert!(refused_naming(&stop, "build/old.o"), "{stop:?}\n{text}");
}

#[test]
fn a_rewrite_through_an_fd_opened_before_the_checkpoint_is_refused() {
    let s = repository("fd", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let path = s.repo.join("build/old.o");
    let modified = fs::metadata(&path)
        .expect("a file")
        .modified()
        .expect("a time");
    let file = std::cell::RefCell::new(
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("opens"),
    );
    let (stop, text) = go(&s, move |_| {
        let mut file = file.borrow_mut();
        file.rewind().expect("a seek");
        file.write_all(b"BBBB").expect("a write");
        file.set_modified(modified).expect("a time");
    });
    assert!(refused_naming(&stop, "build/old.o"), "{stop:?}\n{text}");
}

/// The operator's own file inside an ignored directory; the step changes only its time.
#[test]
fn a_touch_only_does_not_delete_the_operators_file() {
    let s = repository("touch", "secrets/\n");
    fs::create_dir_all(s.repo.join("secrets")).expect("a directory");
    fs::write(s.repo.join("secrets/key.pem"), "precious\n").expect("a file");
    let (stop, text) = go(&s, |tree| {
        reset_mtime(
            &tree.join("secrets/key.pem"),
            std::time::SystemTime::UNIX_EPOCH,
        );
    });
    moved_aside(&s, &stop, &text, "secrets/key.pem", "precious\n");
}

/// What an editor's write tool does: the same bytes written back.
#[test]
fn a_same_bytes_write_does_not_delete_the_operators_file() {
    let s = repository("samebytes", ".idea/\n");
    fs::create_dir_all(s.repo.join(".idea")).expect("a directory");
    fs::write(s.repo.join(".idea/workspace.xml"), "<operator/>\n").expect("a file");
    let (stop, text) = go(&s, |tree| {
        fs::write(tree.join(".idea/workspace.xml"), "<operator/>\n").expect("a write");
    });
    moved_aside(&s, &stop, &text, ".idea/workspace.xml", "<operator/>\n");
}

#[test]
fn a_chmod_only_does_not_delete_the_operators_file() {
    let s = repository("chmod", ".venv/\n");
    fs::create_dir_all(s.repo.join(".venv/bin")).expect("a directory");
    fs::write(s.repo.join(".venv/bin/activate"), "precious\n").expect("a file");
    let (stop, text) = go(&s, |tree| {
        fs::set_permissions(
            tree.join(".venv/bin/activate"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("a mode");
    });
    moved_aside(&s, &stop, &text, ".venv/bin/activate", "precious\n");
}

/// A file the step created inside an ignored directory is the step's: removed, not kept.
#[test]
fn a_file_the_step_created_is_removed_not_moved_aside() {
    let s = repository("created", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "x").expect("a file");
    let (stop, text) = go(&s, |tree| {
        fs::write(tree.join("build/new.o"), "planted").expect("a write");
    });
    assert!(refused_naming(&stop, "build/new.o"), "{stop:?}\n{text}");
    assert!(!s.repo.join("build/new.o").exists(), "{text}");
    assert!(!s.aside(2).join("build/new.o").exists(), "{text}");
    assert!(s.repo.join("build/old.o").exists(), "{text}");
}

/// An ignored file git names by itself that cannot be read, in a tracked directory, is not
/// inside an ignored directory: a step that rewrites it is refused, and it is not moved aside.
#[test]
fn a_top_level_unreadable_ignored_file_is_not_moved_aside() {
    let s = repository("toplevel", "*.local\n");
    fs::create_dir_all(s.repo.join("conf")).expect("a directory");
    fs::write(s.repo.join("conf/tracked.txt"), "x\n").expect("a file");
    git(&s.repo, &["add", "-A"]);
    git(&s.repo, &["commit", "-q", "-m", "conf"]);
    let file = s.repo.join("conf/secret.local");
    fs::write(&file, "precious\n").expect("a file");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("a mode");
    let (stop, text) = go(&s, |tree| {
        let file = tree.join("conf/secret.local");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("a mode");
        fs::write(&file, "changed\n").expect("a write");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("a mode");
    });
    assert!(
        refused_naming(&stop, "conf/secret.local"),
        "{stop:?}\n{text}"
    );
    assert!(
        fs::symlink_metadata(&file).is_ok(),
        "conf/secret.local, named by git itself, left the tree: {stop:?}\n{text}"
    );
    assert!(!s.aside(2).join("conf/secret.local").exists(), "{text}");
}

#[test]
fn a_link_retargeted_is_refused_and_put_back() {
    let s = repository("relink", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    std::os::unix::fs::symlink("a", s.repo.join("build/l")).expect("a link");
    let (stop, text) = go(&s, |tree| {
        fs::remove_file(tree.join("build/l")).expect("a removal");
        std::os::unix::fs::symlink("b", tree.join("build/l")).expect("a link");
    });
    assert!(refused_naming(&stop, "build/l"), "{stop:?}\n{text}");
    assert_eq!(
        fs::read_link(s.repo.join("build/l")).expect("a link"),
        PathBuf::from("a")
    );
}

#[test]
fn a_dir_replaced_by_a_link_is_refused() {
    let s = repository("dirlink", "build/\n");
    fs::create_dir_all(s.repo.join("build/sub")).expect("a directory");
    fs::write(s.repo.join("build/sub/f"), "x").expect("a file");
    let (stop, text) = go(&s, |tree| {
        fs::remove_dir_all(tree.join("build/sub")).expect("a removal");
        std::os::unix::fs::symlink("/srv/elsewhere", tree.join("build/sub")).expect("a link");
    });
    assert!(refused_naming(&stop, "build/sub"), "{stop:?}\n{text}");
    assert!(
        fs::symlink_metadata(s.repo.join("build/sub"))
            .map(|meta| !meta.file_type().is_symlink())
            .unwrap_or(true),
        "the link is still there\n{text}"
    );
}

#[test]
fn a_new_ignored_directory_is_refused_and_removed() {
    let s = repository("newdir", "cache/\n");
    let (stop, text) = go(&s, |tree| {
        fs::create_dir_all(tree.join("cache/a/b")).expect("a directory");
        fs::write(tree.join("cache/a/b/c.o"), "x").expect("a write");
    });
    assert!(refused_naming(&stop, "cache/"), "{stop:?}\n{text}");
    assert!(!s.repo.join("cache").exists(), "{text}");
}

#[test]
fn a_deep_write_in_a_nested_ignored_directory_is_refused() {
    let s = repository("deep", "build/\n");
    fs::create_dir_all(s.repo.join("build/a/b/c")).expect("a directory");
    fs::write(s.repo.join("build/a/b/c/old"), "x").expect("a file");
    let (stop, text) = go(&s, |tree| {
        fs::write(tree.join("build/a/b/c/new"), "x").expect("a write");
    });
    assert!(refused_naming(&stop, "build/a/b/c/new"), "{stop:?}\n{text}");
    assert!(!s.repo.join("build/a/b/c/new").exists(), "{text}");
    assert!(s.repo.join("build/a/b/c/old").exists(), "{text}");
}

#[test]
fn a_step_that_writes_info_exclude_and_plants_is_refused() {
    let s = repository("exclude", "build/\n");
    let (stop, text) = go(&s, |tree| {
        fs::write(tree.join(".git/info/exclude"), "planted/\n").expect("a write");
        fs::create_dir_all(tree.join("planted")).expect("a directory");
        fs::write(tree.join("planted/x"), "x").expect("a write");
    });
    assert!(matches!(stop, Ok(Stop::StepRefused(_))), "{stop:?}\n{text}");
    assert!(!s.repo.join("planted").exists(), "{text}");
}

#[test]
fn a_step_that_writes_a_nested_gitignore_and_plants_is_refused() {
    let s = repository("nestedignore", "build/\n");
    fs::create_dir_all(s.repo.join("pkg")).expect("a directory");
    let (stop, text) = go(&s, |tree| {
        fs::create_dir_all(tree.join("pkg/hide")).expect("a directory");
        fs::write(tree.join("pkg/hide/.gitignore"), "*\n").expect("a write");
        fs::write(tree.join("pkg/hide/evil.pyc"), "x").expect("a write");
    });
    assert!(refused_naming(&stop, "pkg/hide"), "{stop:?}\n{text}");
    // Both files are removed; the directory they were in may stay, empty, since an empty
    // directory the step made cannot be told from one that was there before it.
    assert!(!s.repo.join("pkg/hide/.gitignore").exists(), "{text}");
    assert!(!s.repo.join("pkg/hide/evil.pyc").exists(), "{text}");
}

/// A name that is not UTF-8, planted inside an ignored directory: refused, named byte for
/// byte, and removed; two such names stay two. On macOS no such name can be planted: the file
/// system refuses each with `EILSEQ` and nothing is created.
#[test]
fn a_non_utf8_name_planted_is_refused_and_removed() {
    let s = repository("nonutf8", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "x").expect("a file");
    let one = std::ffi::OsStr::from_bytes(b"\xffplanted.o");
    let two = std::ffi::OsStr::from_bytes(b"\xfeplanted.o");
    #[cfg(target_os = "macos")]
    {
        for name in [one, two] {
            let refused = fs::write(s.repo.join("build").join(name), "x")
                .expect_err("macOS refuses a name that is not UTF-8");
            assert_eq!(refused.raw_os_error(), Some(libc::EILSEQ), "{refused}");
        }
        let names: Vec<std::ffi::OsString> = fs::read_dir(s.repo.join("build"))
            .expect("the build directory")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        assert_eq!(names, ["old.o"], "nothing created");
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (stop, text) = go(&s, |tree| {
            fs::write(tree.join("build").join(one), "x").expect("a write");
            fs::write(tree.join("build").join(two), "x").expect("a write");
        });
        assert!(
            refused_naming(&stop, "build/\\xffplanted.o")
                && refused_naming(&stop, "build/\\xfeplanted.o"),
            "{stop:?}\n{text}"
        );
        assert!(
            !s.repo.join("build").join(one).exists(),
            "planted file left"
        );
        assert!(
            !s.repo.join("build").join(two).exists(),
            "planted file left"
        );
        assert!(s.repo.join("build/old.o").exists(), "{text}");
    }
}

#[test]
fn a_name_with_a_newline_is_printed_escaped() {
    let s = repository("newline", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "x").expect("a file");
    let (stop, text) = go(&s, |tree| {
        fs::write(
            tree.join("build/x\n  effect: accepted, nothing refused"),
            "x",
        )
        .expect("a write");
    });
    assert!(matches!(stop, Ok(Stop::StepRefused(_))), "{stop:?}");
    assert!(
        !text.contains("\n  effect: accepted"),
        "a forged line in the output:\n{text}"
    );
}

#[test]
fn the_cap_names_twenty_and_counts_the_rest() {
    let s = repository("cap", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "x").expect("a file");
    let (stop, text) = go(&s, |tree| {
        for i in 0..25 {
            fs::write(tree.join(format!("build/n{i:02}.o")), "x").expect("a write");
        }
    });
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("{stop:?}\n{text}")
    };
    assert!(why.contains("and 5 more"), "{why}");
    assert_eq!(why.matches("build/n").count(), 20, "{why}");
}

/// A top-level ignored directory that could not be listed before the step is compared by its
/// stamp: a step that opens it, plants a file and closes it again is refused.
#[test]
fn a_top_level_ignored_dir_that_cannot_be_listed_is_guarded() {
    let s = repository("unlist-top", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    fs::set_permissions(s.repo.join("build"), fs::Permissions::from_mode(0o000)).expect("a mode");
    let (stop, text) = go(&s, |tree| {
        let dir = tree.join("build");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("a mode");
        fs::write(dir.join("new.o"), "planted").expect("a write");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).expect("a mode");
    });
    let _ = fs::set_permissions(s.repo.join("build"), fs::Permissions::from_mode(0o755));
    assert!(refused_naming(&stop, "build/"), "{stop:?}\n{text}");
}

/// A directory below an ignored one that could not be listed before the step likewise.
#[test]
fn a_directory_below_an_ignored_one_that_cannot_be_listed_is_guarded() {
    let s = repository("unlist-below", "build/\n");
    fs::create_dir_all(s.repo.join("build/locked")).expect("a directory");
    fs::write(s.repo.join("build/locked/old.o"), "AAAA").expect("a file");
    fs::set_permissions(
        s.repo.join("build/locked"),
        fs::Permissions::from_mode(0o000),
    )
    .expect("a mode");
    let (stop, text) = go(&s, |tree| {
        let dir = tree.join("build/locked");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("a mode");
        fs::write(dir.join("new.o"), "planted").expect("a write");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).expect("a mode");
    });
    let _ = fs::set_permissions(
        s.repo.join("build/locked"),
        fs::Permissions::from_mode(0o755),
    );
    assert!(refused_naming(&stop, "build/locked/"), "{stop:?}\n{text}");
}
