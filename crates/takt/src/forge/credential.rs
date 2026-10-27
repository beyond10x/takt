//! A forge's credential, named by the run file and held in memory only: `credential: {env: NAME}`
//! or `credential: {file: PATH}`. The run file names where the value is, never the value; a source
//! that is unset, readable by others, owned by another user or inside the repository's tree is
//! refused before the case opens, naming the source and never the value. [`Secret`] never prints
//! its value, and replaces it in any text that quotes the host's answer ([`Secret::redact`]).

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

/// Where a credential comes from, by reference.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum CredentialSource {
    /// A variable of takt's environment.
    Env(String),
    /// A regular file of mode 600 owned by the user, outside the tree.
    File(PathBuf),
}

impl fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Env(name) => write!(f, "the credential source `env: {name}`"),
            Self::File(path) => write!(f, "the credential source `file: {}`", path.display()),
        }
    }
}

/// A credential's value. Not `Display`, and its `Debug` is `Secret(<redacted>)`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl Secret {
    /// The text a credential is replaced with.
    pub const REDACTED: &str = "<redacted>";

    /// The value, for the one place that sends it: a request header, or the environment of one
    /// credential-helper line.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// `text` with the value, wherever it occurs, replaced.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        text.replace(&self.0, Self::REDACTED)
    }

    #[cfg(test)]
    pub(crate) fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    /// A value a header can carry: printable ASCII with no space.
    fn of(source: &CredentialSource, value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            bail!("{source} is empty");
        }
        if !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            bail!(
                "{source} holds more than a token: a token is printable characters with no space"
            );
        }
        Ok(Self(value.to_owned()))
    }
}

impl CredentialSource {
    /// Resolves the source for a run in `tree`: the value, or a refusal naming the source.
    pub fn resolve(&self, tree: &Path) -> Result<Secret> {
        match self {
            Self::Env(name) => {
                if name.is_empty() || name.contains(['=', '\0']) {
                    bail!("{self} names no variable");
                }
                let value = std::env::var_os(name).ok_or_else(|| {
                    anyhow!("{self} is not set: the variable {name} is unset in takt's environment")
                })?;
                let value = value
                    .into_string()
                    .map_err(|_| anyhow!("{self} is not text"))?;
                Secret::of(self, &value)
            }
            Self::File(path) => {
                let value = self.read_file(path, tree)?;
                Secret::of(self, &value)
            }
        }
    }

    /// The text of the credential file `path`: a regular file owned by the user, no group or
    /// other permission bit set, outside `tree`.
    fn read_file(&self, path: &Path, tree: &Path) -> Result<String> {
        if !path.is_absolute() {
            bail!("{self} is not an absolute path");
        }
        let real = path
            .canonicalize()
            .map_err(|error| anyhow!("{self} cannot be read: {}", error.kind()))?;
        let inside = tree.canonicalize().is_ok_and(|tree| real.starts_with(tree));
        if inside {
            bail!("{self} is inside the repository's tree: a credential file lives outside it");
        }
        let meta = std::fs::metadata(&real)
            .map_err(|error| anyhow!("{self} cannot be read: {}", error.kind()))?;
        if !meta.is_file() {
            bail!("{self} is not a regular file");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let mode = meta.mode() & 0o777;
            if mode & 0o077 != 0 {
                bail!(
                    "{self} is readable by others (mode {mode:o}): its mode must be 600, and nothing is read"
                );
            }
            // SAFETY: `geteuid` takes nothing and touches no memory of this process.
            if meta.uid() != unsafe { libc::geteuid() } {
                bail!("{self} is owned by another user: it must be owned by the user running takt");
            }
        }
        std::fs::read_to_string(&real)
            .map_err(|error| anyhow!("{self} cannot be read: {}", error.kind()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints_and_is_redacted_from_text() {
        let secret = Secret("tok-123".to_owned());
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert_eq!(
            secret.redact("bad tok-123 and tok-123"),
            "bad <redacted> and <redacted>"
        );
    }
}
