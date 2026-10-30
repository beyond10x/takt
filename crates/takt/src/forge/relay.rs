//! What every git line to a loopback relay is held to, `takt run --forge connectors`'s push, read
//! and fetch and `takt scenario run`'s trial fetch and push alike: the relay holds the forge's
//! credential, so a line to it carries none of its own, follows no redirect and goes through no
//! proxy.
//!
//! A line ([`admit`]) is started with a home of its own, an empty directory takt makes for it
//! under the run's state directory ([`Homes`]) and removes after it: `HOME` and
//! `XDG_CONFIG_HOME` name it, `GIT_CONFIG_GLOBAL` is `/dev/null` and `GIT_CONFIG_NOSYSTEM` is
//! set, so git reads neither the operator's global nor the system configuration, and curl
//! reads no `~/.netrc`. Git needs nothing from a home for a fetch, a push or an `ls-remote`
//! over `http://`. The repository's own configuration, what it includes and what the
//! environment hands git still apply: they are read once, as the line reads them, before it
//! runs, and a line they would hand a credential or carry off the relay is refused, naming the
//! key and where it is set ([`refusing`]). On the command line ([`CONFIG`]) the line empties
//! `http.proxy` and `http.extraHeader` after [`crate::git::NO_REDIRECT`]; git lets a URL-scoped
//! `http.<url>.*` outrank all three, so those are refused rather than overridden.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::git::RemoteTransport;

/// What a git line to a loopback relay is given on its command line after
/// [`crate::git::NO_REDIRECT`]: no proxy (an empty `http.proxy`) and no extra header (an empty
/// `http.extraHeader` empties the list the configuration read before it built). Each outranks
/// the same key unscoped in any configuration file, which git reads before the command line; a
/// URL-scoped one outranks it, and [`admit`] refuses those.
pub const CONFIG: [&str; 4] = ["-c", "http.proxy=", "-c", "http.extraHeader="];

/// The directory a line's empty home is made in: `<state>/relay-home` for the state directory a
/// run is given (`--state-dir`), else `$XDG_STATE_HOME/takt/relay-home`
/// (`~/.local/state/takt/relay-home`).
#[derive(Debug, Clone)]
pub struct Homes(PathBuf);

impl Homes {
    /// The directory of the run whose state directory is `state` (none: takt's own).
    pub fn at(state: Option<&Path>) -> Result<Self> {
        Ok(Self(match state {
            Some(dir) => std::path::absolute(dir)
                .unwrap_or_else(|_| dir.to_path_buf())
                .join("relay-home"),
            None => crate::promotion::state_dir()
                .context("find the state directory a line to the loopback relay makes its empty home in; nothing was sent")?
                .join("takt/relay-home"),
        }))
    }

    /// A new empty home under it, readable by this user only.
    fn make(&self) -> Result<Home> {
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::fs::create_dir_all(&self.0)
            .with_context(|| format!("make {}; nothing was sent", self.0.display()))?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let dir = self.0.join(format!(
            "{}-{nanos}-{}",
            std::process::id(),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        if dir.to_str().is_none() {
            bail!(
                "the empty home {} of a line to the loopback relay would be no UTF-8 path; nothing was sent",
                dir.display()
            );
        }
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&dir).with_context(|| {
            format!(
                "make the empty home {} of a line to the loopback relay; nothing was sent",
                dir.display()
            )
        })?;
        Ok(Home(dir))
    }
}

/// An empty home of one line, removed with what is in it when dropped.
#[derive(Debug)]
struct Home(PathBuf);

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A line to a loopback relay [`admit`] admitted: the URL it goes to and the home it runs in.
#[derive(Debug)]
pub struct Line {
    /// The relay's URL, without userinfo ([`super::connectors::relay_of`]).
    pub url: String,
    home: Home,
}

impl Line {
    /// What the line's git is started with in its environment: its empty home as `HOME` and
    /// `XDG_CONFIG_HOME`, no global configuration (`GIT_CONFIG_GLOBAL=/dev/null`) and no system
    /// configuration (`GIT_CONFIG_NOSYSTEM=1`).
    #[must_use]
    pub fn env(&self) -> Vec<(&'static str, &str)> {
        let home = self.home.0.to_str().unwrap_or_default();
        vec![
            ("HOME", home),
            ("XDG_CONFIG_HOME", home),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
        ]
    }
}

/// One entry of `git config --list --show-origin -z`: where it is set, its key as git lists it
/// (section and variable in lower case) and its value, none for a key given without `=`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Where git read it: `file:<path>`, `command line:`.
    pub origin: String,
    /// The key.
    pub key: String,
    /// The value; none for a key without one, which git reads as `true`.
    pub value: Option<String>,
}

/// The entries of `listed`, `git config --list --show-origin -z`: `<origin>\0<key>\n<value>\0`
/// each, or `<origin>\0<key>\0` for a key without a value.
#[must_use]
pub fn entries(listed: &[u8]) -> Vec<Entry> {
    let mut fields = listed.split(|byte| *byte == 0);
    let mut entries = Vec::new();
    while let (Some(origin), Some(entry)) = (fields.next(), fields.next()) {
        let entry = String::from_utf8_lossy(entry);
        let (key, value) = match entry.split_once('\n') {
            Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
            None => (entry.into_owned(), None),
        };
        entries.push(Entry {
            origin: String::from_utf8_lossy(origin).into_owned(),
            key,
            value,
        });
    }
    entries
}

/// Whether `value` reads as git's `false`: `false`, `no`, `off`, an empty value or a number
/// that is 0, in any case.
fn is_false(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim().to_ascii_lowercase();
        matches!(value.as_str(), "false" | "no" | "off" | "")
            || value.parse::<i64>().is_ok_and(|number| number == 0)
    })
}

/// Whether the proxy `value` names (`[<scheme>://][<userinfo>@]<host>[:<port>][/...]`) is a
/// loopback host.
fn proxy_on_loopback(value: &str) -> bool {
    let rest = value.split_once("://").map_or(value, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_port)| host_port);
    let host = if host_port.starts_with('[') {
        host_port
            .split_once(']')
            .map_or(host_port, |(host, _)| &host_port[..=host.len()])
    } else {
        host_port.split(':').next().unwrap_or_default()
    };
    crate::harness::loopback(host)
}

/// Whether a line to a loopback relay is refused for `entry`, matched on its key in any case:
/// a credential helper (`credential.helper`, `credential.<url>.helper`); a URL-scoped extra
/// header (`http.<url>.extraHeader`), which outranks the empty one on the command line
/// ([`CONFIG`]); a cookie file (`http.cookieFile`, `http.<url>.cookieFile`), whose cookies
/// would go with the line; a resolve entry (`http.curloptResolve`, `http.<url>.curloptResolve`),
/// which can send the relay's host to another address; a URL-scoped `followRedirects` that is
/// not `false`, which outranks [`crate::git::NO_REDIRECT`]; a URL-scoped proxy, which outranks
/// the empty `http.proxy`; and an `http.proxy` off loopback. An empty value, which git reads
/// as a reset, refuses none but `followRedirects`, where it is `false`.
#[must_use]
pub fn refusing(entry: &Entry) -> bool {
    let key = entry.key.to_ascii_lowercase();
    let Some((section, rest)) = key.split_once('.') else {
        return false;
    };
    let (scoped, variable) = rest
        .rsplit_once('.')
        .map_or((false, rest), |(_, variable)| (true, variable));
    let value = entry.value.as_deref();
    let set = value.is_none_or(|value| !value.is_empty());
    match (section, variable) {
        ("credential", "helper") | ("http", "cookiefile" | "curloptresolve") => set,
        ("http", "extraheader") => scoped && set,
        ("http", "followredirects") => scoped && !is_false(value),
        ("http", "proxy") => set && (scoped || !value.is_some_and(proxy_on_loopback)),
        _ => false,
    }
}

/// `key` as a refusal names it: a URL in it without its userinfo, which may hold a password,
/// and every control character escaped.
fn shown(key: &str) -> String {
    let named = match key.split_once('.') {
        Some((section, rest)) => match rest.rsplit_once('.') {
            Some((scope, variable)) => {
                format!("{section}.{}.{variable}", super::without_userinfo(scope))
            }
            None => key.to_owned(),
        },
        None => key.to_owned(),
    };
    named.escape_debug().to_string()
}

/// Admits a line from `tree` to the loopback relay `relay` (a URL without userinfo,
/// [`super::connectors::relay_of`]), its empty home made under `homes`: the tree's
/// configuration is read once as the line reads it, its home and its environment's
/// configuration included (`git config --list --show-origin -z`, includes followed). Refused,
/// before anything is sent, when git would reach the relay's URL at a URL that is not `http://`
/// to a loopback host or that holds a userinfo ([`super::connectors::reached_by`]), and when an
/// entry is one [`refusing`] names, naming its key and where it is set.
pub fn admit(tree: &Path, relay: &str, homes: &Homes) -> Result<Line> {
    let line = Line {
        url: relay.to_owned(),
        home: homes.make()?,
    };
    let listed = crate::git::bytes_as_remote_git_with(
        tree,
        &["config", "--list", "--show-origin", "-z"],
        &line.env(),
    )
    .context("read git's configuration of the tree before a line to the loopback relay; nothing was sent")?;
    let entries = entries(&listed);
    let plain: Vec<u8> = entries
        .iter()
        .filter_map(|entry| {
            let value = entry.value.as_ref()?;
            Some(format!("{}\n{value}\0", entry.key).into_bytes())
        })
        .flatten()
        .collect();
    for url in super::connectors::reached_by(relay, &plain) {
        if !matches!(
            crate::git::remote_transport(&url),
            Ok(RemoteTransport::LoopbackHttp)
        ) {
            bail!(
                "a line to the loopback relay {relay} would go to {}, no loopback host, as git's configuration rewrites it (`url.<base>.insteadOf`, `url.<base>.pushInsteadOf`, a remote's `pushurl`): the relay's lines go to the relay alone, so nothing was sent",
                super::without_userinfo(&url)
            );
        }
        if super::without_userinfo(&url) != url {
            bail!(
                "a line to the loopback relay {relay} would go to {} with a userinfo, as git's configuration rewrites it: the relay's lines carry no credential, so nothing was sent",
                super::without_userinfo(&url)
            );
        }
    }
    if let Some(entry) = entries.iter().find(|entry| refusing(entry)) {
        bail!(
            "a line to the loopback relay {relay} would go under `{}`, set in {}, which would hand it a credential or carry it off the relay: the relay holds the forge's credential itself, so nothing was sent",
            shown(&entry.key),
            entry.origin.escape_debug()
        );
    }
    Ok(line)
}
