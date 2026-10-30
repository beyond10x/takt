//! story:git-through-the-connectors-git-relay, acceptance 4, at the library: which git
//! configuration keys refuse a line to a loopback relay (`takt::forge::relay::refusing`), how
//! `git config --list --show-origin -z` is read (`takt::forge::relay::entries`), and what an
//! admitted line is started with and a refused one names (`takt::forge::relay::admit`). Nothing
//! here opens a connection.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::forge::relay::{Entry, Homes, admit, entries, refusing};

const RELAY: &str = "http://127.0.0.1:9/owner/repo.git";

fn entry(key: &str, value: Option<&str>) -> Entry {
    Entry {
        origin: "file:.git/config".to_owned(),
        key: key.to_owned(),
        value: value.map(str::to_owned),
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("relay-line-config-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
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

fn tree(scratch: &Scratch) -> PathBuf {
    let tree = scratch.0.join("tree");
    fs::create_dir_all(&tree).expect("a tree");
    git(&tree, &["init", "-q", "-b", "main"]);
    tree
}

/// The keys a line to the relay is refused for, in any case, and the ones it is not: an empty
/// value is a reset, an unscoped extra header and redirect setting are outranked by the command
/// line, and a proxy on loopback goes nowhere else.
#[test]
fn the_refused_keys() {
    for (key, value) in [
        ("credential.helper", Some("store")),
        ("credential.http://127.0.0.1:9.helper", Some("store")),
        ("Credential.Helper", Some("store")),
        ("http.http://127.0.0.1:9.extraheader", Some("A: b")),
        ("http.http://127.0.0.1:9.extraHeader", None),
        ("http.cookiefile", Some("jar")),
        ("http.http://127.0.0.1:9.cookiefile", Some("jar")),
        ("http.curloptresolve", Some("127.0.0.1:9:192.0.2.1")),
        ("http.http://127.0.0.1:9.followredirects", Some("true")),
        ("http.http://127.0.0.1:9.followredirects", Some("initial")),
        ("http.http://127.0.0.1:9.followredirects", None),
        (
            "http.http://127.0.0.1:9.proxy",
            Some("http://127.0.0.1:3128"),
        ),
        ("http.proxy", Some("proxy.example:3128")),
        ("http.proxy", Some("socks5://u@[2001:db8::1]:1080")),
    ] {
        assert!(refusing(&entry(key, value)), "{key} {value:?}");
    }
    for (key, value) in [
        ("credential.helper", Some("")),
        ("credential.username", Some("someone")),
        ("http.extraheader", Some("A: b")),
        ("http.followredirects", Some("true")),
        ("http.http://127.0.0.1:9.followredirects", Some("false")),
        ("http.http://127.0.0.1:9.followredirects", Some("0")),
        ("http.http://127.0.0.1:9.proxy", Some("")),
        ("http.proxy", Some("")),
        ("http.proxy", Some("http://user@127.0.0.1:3128")),
        ("http.proxy", Some("[::1]:3128")),
        ("http.postbuffer", Some("1")),
        ("url.http://127.0.0.1:9/.insteadof", Some("x")),
    ] {
        assert!(!refusing(&entry(key, value)), "{key} {value:?}");
    }
}

/// `--show-origin -z` is read entry by entry, a key without a value kept as one.
#[test]
fn entries_are_read_with_their_origin() {
    let listed = b"file:.git/config\0core.bare\nfalse\0file:.git/x.inc\0http.http://a.proxy\0command line:\0credential.helper\n\0";
    assert_eq!(
        entries(listed),
        [
            Entry {
                origin: "file:.git/config".to_owned(),
                key: "core.bare".to_owned(),
                value: Some("false".to_owned()),
            },
            Entry {
                origin: "file:.git/x.inc".to_owned(),
                key: "http.http://a.proxy".to_owned(),
                value: None,
            },
            Entry {
                origin: "command line:".to_owned(),
                key: "credential.helper".to_owned(),
                value: Some(String::new()),
            },
        ]
    );
}

/// An admitted line runs in an empty home of its own under the run's state directory, with no
/// global or system configuration, and the home is gone once the line is.
#[test]
fn an_admitted_line_has_an_empty_home_of_its_own() {
    let scratch = Scratch::new("admitted");
    let tree = tree(&scratch);
    let state = scratch.0.join("state");
    let line = admit(&tree, RELAY, &Homes::at(Some(&state)).expect("homes")).expect("admitted");
    assert_eq!(line.url, RELAY);
    let env = line.env();
    let home = env
        .iter()
        .find(|(name, _)| *name == "HOME")
        .map(|(_, value)| PathBuf::from(value))
        .expect("a HOME");
    assert!(home.starts_with(state.join("relay-home")), "{home:?}");
    assert_eq!(fs::read_dir(&home).expect("the home").count(), 0);
    for pair in [
        ("XDG_CONFIG_HOME", home.to_str().expect("utf-8")),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
    ] {
        assert!(env.contains(&pair), "{pair:?} in {env:?}");
    }
    drop(line);
    assert!(!home.exists(), "{home:?}");
}

/// A refused line names the key with its URL's userinfo left out, and the file that sets it.
#[test]
fn a_refusal_names_the_key_without_its_userinfo() {
    let scratch = Scratch::new("refused");
    let tree = tree(&scratch);
    git(
        &tree,
        &[
            "config",
            "http.http://operator:pw-stand-in@127.0.0.1:9/owner/repo.git.extraHeader",
            "X: y",
        ],
    );
    let state = scratch.0.join("state");
    let said = format!(
        "{:#}",
        admit(&tree, RELAY, &Homes::at(Some(&state)).expect("homes")).expect_err("refused")
    );
    assert!(
        said.contains("`http.http://127.0.0.1:9/owner/repo.git.extraheader`")
            && said.contains("/.git/config")
            && said.contains("nothing was sent"),
        "{said}"
    );
    assert!(!said.contains("pw-stand-in"), "{said}");
}
