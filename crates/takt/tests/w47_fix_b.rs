//! Wave 47 review fixes, the rest of each finding's class: every way git picks a transport
//! other than a URL's scheme is read by `remote_transport` (a remote helper named before `::`,
//! `remote.<name>.vcs`), `ftp://` is held to the rule `http://` is, and the credential shapes a
//! failed command commonly prints are known in their other spellings. The values are made-up;
//! none is a live credential.

use std::path::{Path, PathBuf};
use std::process::Command;

use takt::git::{RemoteTransport, remote_transport, remote_urls};
use takt::run::credential_shape;

/// git's own curl helpers fetch their address as a URL, so the address decides: plain
/// `http://` (or an address with no scheme, which curl takes as `http://`) to another host is
/// refused naming no password, and to a loopback host is a relay.
#[test]
fn a_curl_helper_is_read_by_its_address() {
    for url in [
        "http::http://example.com/o/r.git",
        "https::http://example.com/o/r.git",
        "HTTP::http://example.com/o/r.git",
        "http::example.com/o/r.git",
        "http::user:pw0rdW47fixB@example.com/o/r.git",
        "ftp::http://example.com/o/r.git",
    ] {
        let error = remote_transport(url).expect_err(url);
        let message = format!("{error:#}");
        assert!(message.contains("no loopback host"), "{url}: {message}");
        assert!(!message.contains("pw0rdW47fixB"), "{url}: {message}");
    }
    assert_eq!(
        remote_transport("http::http://127.0.0.1:8403/o/r.git").expect("admitted"),
        RemoteTransport::LoopbackHttp
    );
    assert_eq!(
        remote_transport("http::https://example.com/o/r.git").expect("admitted"),
        RemoteTransport::Https
    );
    for url in [
        "persistent-https::https://example.com/o/r.git",
        "custom::anything",
        "http://[::1]:8403/o/r.git",
    ] {
        assert_ne!(
            remote_transport(url).expect("admitted"),
            RemoteTransport::Https,
            "{url}"
        );
    }
}

/// `ftp://` carries a password in the clear as `http://` does: refused off a loopback host.
#[test]
fn ftp_to_another_host_is_refused() {
    let error = remote_transport("ftp://user:secret@example.com/o/r.git").expect_err("refused");
    let message = format!("{error:#}");
    assert!(message.contains("`ftp://`"), "{message}");
    assert!(!message.contains("secret"), "{message}");
    assert_eq!(
        remote_transport("ftp://127.0.0.1/o/r.git").expect("admitted"),
        RemoteTransport::Other
    );
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// `remote.<name>.vcs` names the helper git reaches a remote through whatever its URL says, so
/// its URLs are answered in the `<helper>::<address>` form and read by it.
#[test]
fn a_remote_s_vcs_names_the_helper_its_urls_are_read_through() {
    let tree = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w47-fix-b-vcs-{}", std::process::id()));
    let _scratch = Scratch(tree.clone());
    std::fs::create_dir_all(&tree).expect("a directory");
    git(&tree, &["init", "-q"]);
    git(&tree, &["remote", "add", "origin", "example.com/o/r.git"]);
    let (fetch, _) = remote_urls(&tree, "origin").expect("read");
    assert_eq!(fetch, ["example.com/o/r.git"]);
    assert_eq!(
        remote_transport(&fetch[0]).expect("admitted"),
        RemoteTransport::Other
    );
    git(&tree, &["config", "remote.origin.vcs", "http"]);
    let (fetch, push) = remote_urls(&tree, "origin").expect("read");
    assert_eq!(fetch, ["http::example.com/o/r.git"]);
    assert_eq!(push, ["http::example.com/o/r.git"]);
    assert!(remote_transport(&fetch[0]).is_err(), "{fetch:?}");
}

/// The new shapes in their other common spellings, and code that only builds one stays shown.
#[test]
fn the_new_shapes_are_known_in_their_other_spellings() {
    for (text, shape) in [
        (
            "< authorization: bearer 9f8e7d6c5b4a39281706",
            "authorization-header",
        ),
        (
            "Authorization: Basic dXNlcjpwYXNzd29yZA==",
            "authorization-header",
        ),
        (
            "\"Authorization\": \"Bearer 9f8e7d6c5b4a39281706\"",
            "authorization-header",
        ),
        (
            "export AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYzEXAMPLEKEY",
            "aws-secret-access-key",
        ),
        (
            "\"SecretAccessKey\": \"wJalrXUtnFEMI/K7MDENG/bPxRfiCYzEXAMPLEKEY\"",
            "aws-secret-access-key",
        ),
        (
            concat!("key ", "ASIA", "Q3EGZ7XN", "2MBV4KPL", " expired"),
            "aws-access-key-id",
        ),
        ("mysql://root:hunter2hunter2@127.0.0.1/app", "url-password"),
    ] {
        assert_eq!(credential_shape(text), Some(shape), "{text}");
    }
    for clean in [
        "Authorization: Bearer <token>",
        "headers.insert(AUTHORIZATION, format!(\"Basic {encoded}\"));",
        "see https://user@example.com/o/r.git and ssh://git@example.com:22/o/r.git",
        "AKIAQ3EGZ7XN2MBV4KPLX is one character too long",
        "the secret_access_key field is read from the profile",
    ] {
        assert_eq!(credential_shape(clean), None, "{clean}");
    }
}
