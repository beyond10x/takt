//! The scan seam: what a judgement step's session proposes, the change so far and the scope
//! files a prompt shows, and what the `gitlab` forge publishes pass before they leave a step or
//! the machine. A run has one scan adapter ([`Scan`]):
//!
//! | adapter | name | what it scans with |
//! |---|---|---|
//! | [`Scan::Gates`] | `gates` | [`gates`]: its program under the policy its variable names; with no policy set it scans nothing, which is never a pass |
//! | [`Scan::None`] | `none` | nothing; only ever declared in the run configuration file (`scan: none` in `takt run --config`), never by a flag |
//!
//! Not declared, the adapter is `gates`. Whatever the adapter, every proposal is checked in
//! process for the shape of a credential ([`crate::run::credential_shape`]). The `bot` forge
//! scans with its own program whatever the run's scan: it is the gates adapter's forge.

pub mod gates;

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

/// A scan adapter's name, as the run configuration file declares it (`scan: gates | none`).
/// Read from a string only ([`ScanName::deserialize`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanName {
    /// [`Scan::Gates`].
    Gates,
    /// [`Scan::None`].
    None,
}

/// The name read from a string, `gates` or `none`, and from nothing else: a derived reader would
/// take a table whose one key is the name (`scan = { none = {} }`) as the name, and a table is
/// not the declaration `scan: none`.
impl<'de> serde::Deserialize<'de> for ScanName {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        /// Takes a string and nothing else; refused inside the reader, so a refusal keeps the
        /// value's line.
        struct Name;
        impl serde::de::Visitor<'_> for Name {
            type Value = ScanName;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("`gates` or `none`")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                name: &str,
            ) -> std::result::Result<ScanName, E> {
                match name {
                    "gates" => Ok(ScanName::Gates),
                    "none" => Ok(ScanName::None),
                    other => Err(E::unknown_variant(other, &["gates", "none"])),
                }
            }
        }
        deserializer.deserialize_str(Name)
    }
}

impl ScanName {
    /// The name as the file and the record write it: `gates` or `none`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gates => "gates",
            Self::None => "none",
        }
    }
}

/// The scan adapter of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scan {
    /// The `gates` adapter ([`gates`]) under `policy`, the file [`gates::POLICY_VAR`] names;
    /// none when it is not set, and then nothing is scanned.
    Gates {
        /// The policy file.
        policy: Option<PathBuf>,
    },
    /// The `none` adapter: nothing is scanned, as the run configuration file `declared_in`
    /// declares (`scan: none`).
    None {
        /// The run configuration file that declares it.
        declared_in: PathBuf,
    },
}

impl Scan {
    /// The adapter when none is declared: `gates`, under the policy [`gates::POLICY_VAR`] names
    /// when it is set.
    #[must_use]
    pub fn from_env() -> Self {
        Self::Gates {
            policy: gates::policy_from_env(),
        }
    }

    /// The adapter of a run whose configuration file declares `declared`, as `(name, file)`:
    /// `none` declared is [`Scan::None`], refused when [`gates::POLICY_VAR`] is set too, since a
    /// run has one adapter and a policy no scan reads would read as a scan that ran; `gates`
    /// declared, or nothing, is [`Scan::from_env`].
    pub fn of_run(declared: Option<(ScanName, &Path)>) -> Result<Self> {
        match declared {
            Some((ScanName::None, file)) => {
                if gates::policy_from_env().is_some() {
                    return Err(anyhow!(
                        "the run configuration file {} declares `scan: none`, and {} is set: a run has one scan adapter, and a policy no scan reads would read as a scan that ran; unset {} to run unscanned, or leave `scan: none` out to scan under the policy; nothing was opened",
                        file.display(),
                        gates::POLICY_VAR,
                        gates::POLICY_VAR
                    ));
                }
                Ok(Self::None {
                    declared_in: file.to_path_buf(),
                })
            }
            Some((ScanName::Gates, _)) | None => Ok(Self::from_env()),
        }
    }

    /// The adapter's name, `gates` or `none`, as the `opened` line records it.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gates { .. } => ScanName::Gates.as_str(),
            Self::None { .. } => ScanName::None.as_str(),
        }
    }

    /// Whether the run declared `scan: none`.
    #[must_use]
    pub fn declared_none(&self) -> bool {
        matches!(self, Self::None { .. })
    }

    /// The gates policy, under `gates` when one is set; none otherwise.
    #[must_use]
    pub fn policy(&self) -> Option<&Path> {
        match self {
            Self::Gates { policy } => policy.as_deref(),
            Self::None { .. } => None,
        }
    }

    /// The scan a proposal and the change so far pass: `gates` under a policy; none otherwise.
    #[must_use]
    pub fn change_scan(&self) -> Option<gates::ChangeScan> {
        self.policy().map(|policy| gates::ChangeScan {
            program: PathBuf::from(gates::PROGRAM),
            policy: policy.to_path_buf(),
        })
    }

    /// Why the proposal of `action`, from a session that may have read outside its copy, is
    /// refused when no scan read it under this adapter: not scanned is never passed.
    #[must_use]
    pub fn unscanned(&self, action: &str) -> String {
        format!(
            "{action} proposes a change of a session that could read outside its copy, and the scan adapter `{}` has no policy to scan it under ({}): not scanned, which is not passed",
            self.name(),
            gates::POLICY_VAR
        )
    }

    /// Why a run of `--harness <harness>`, whose session may read outside its copy, is refused
    /// before its case opens under this adapter: under `gates` with no policy, what it proposes
    /// would not be scanned. `config` says whether the run takes a run configuration file, where
    /// `scan: none` is declared (`takt run --config`); a scenario run takes none. None under a
    /// policy or a declared `none`.
    #[must_use]
    pub fn refused_at_open(&self, harness: &str, config: bool) -> Option<String> {
        let Self::Gates { policy: None } = self else {
            return None;
        };
        let var = gates::POLICY_VAR;
        let ways = if config {
            format!(
                "set {var} to a gates policy file, or declare `scan: none` in the run configuration file given with --config (`takt run --config <file>`) to accept what it proposes unscanned, checked only for the shape of a credential; nothing was opened"
            )
        } else {
            format!(
                "set {var} to a gates policy file: a scenario run takes no run configuration file, so it cannot declare `scan: none` as `takt run --config <file>` can; nothing was opened or changed"
            )
        };
        Some(format!(
            "--harness {harness} runs a session that may read outside its copy, and the scan adapter `{}` has no policy ({var} is not set): what it proposes would not be scanned, and not scanned is never passed; {ways}",
            self.name()
        ))
    }

    /// The line a run under a declared `none` says before its first step, `harness` the name
    /// of the run's harness; none under `gates`.
    #[must_use]
    pub fn declared_line(&self, harness: &str) -> Option<String> {
        let Self::None { declared_in } = self else {
            return None;
        };
        Some(format!(
            "scan: none (declared in {}); what --harness {harness} proposes is accepted unscanned, checked only for the shape of a credential",
            declared_in.display()
        ))
    }

    /// Why nothing was scanned under a declared `none`, as a forge's
    /// [`crate::forge::IdentityScan::NotScanned`] says it; none under `gates`.
    #[must_use]
    pub fn not_scanned(&self) -> Option<String> {
        let Self::None { declared_in } = self else {
            return None;
        };
        Some(format!(
            "the scan adapter is none, `scan: none` declared in {}: nothing is scanned",
            declared_in.display()
        ))
    }
}
