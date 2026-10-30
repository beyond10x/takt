//! The scan seam: whether what a judgement step's session proposes, and what a forge publishes,
//! may pass unscanned. A run has one scan adapter ([`Scan`]):
//!
//! | adapter | name | what it scans with |
//! |---|---|---|
//! | [`Scan::Unset`] | `unset` | nothing: no adapter is declared, and not scanned is never passed |
//! | [`Scan::None`] | `none` | nothing; only ever declared in the run configuration file (`scan: none` in `takt run --config`), never by a flag |
//!
//! Not declared, the adapter is `unset`: a harness whose session may read outside its copy is
//! refused before its case opens, and so is a forge that publishes what the run makes. Whatever
//! the adapter, every proposal is checked in process for the shape of a credential
//! ([`crate::run::credential_shape`]).

use std::path::{Path, PathBuf};

/// A scan adapter's name, as the run configuration file declares it (`scan: none`). Read from a
/// string only ([`ScanName::deserialize`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanName {
    /// [`Scan::None`].
    None,
}

/// The name read from a string, `none`, and from nothing else: a derived reader would take a
/// table whose one key is the name (`scan = { none = {} }`) as the name, and a table is not the
/// declaration `scan: none`.
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
                formatter.write_str("`none`")
            }
            fn visit_str<E: serde::de::Error>(
                self,
                name: &str,
            ) -> std::result::Result<ScanName, E> {
                match name {
                    "none" => Ok(ScanName::None),
                    other => Err(E::unknown_variant(other, &["none"])),
                }
            }
        }
        deserializer.deserialize_str(Name)
    }
}

impl ScanName {
    /// The name as the file and the record write it: `none`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
        }
    }
}

/// The scan adapter of a run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Scan {
    /// No adapter is declared: nothing is scanned, and not scanned is never passed.
    #[default]
    Unset,
    /// The `none` adapter: nothing is scanned, as the run configuration file `declared_in`
    /// declares (`scan: none`).
    None {
        /// The run configuration file that declares it.
        declared_in: PathBuf,
    },
}

impl Scan {
    /// The adapter of a run whose configuration file declares `declared`, as `(name, file)`:
    /// `none` declared is [`Scan::None`]; nothing declared is [`Scan::Unset`].
    #[must_use]
    pub fn of_run(declared: Option<(ScanName, &Path)>) -> Self {
        match declared {
            Some((ScanName::None, file)) => Self::None {
                declared_in: file.to_path_buf(),
            },
            None => Self::Unset,
        }
    }

    /// The adapter's name, `unset` or `none`, as the `opened` line records it.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::None { .. } => ScanName::None.as_str(),
        }
    }

    /// Whether the run declared `scan: none`.
    #[must_use]
    pub fn declared_none(&self) -> bool {
        matches!(self, Self::None { .. })
    }

    /// Why the proposal of `action`, from a session that may have read outside its copy, is
    /// refused when no scan read it under this adapter: not scanned is never passed.
    #[must_use]
    pub fn unscanned(&self, action: &str) -> String {
        format!(
            "{action} proposes a change of a session that could read outside its copy, and the scan adapter is `{}`: not scanned, which is not passed",
            self.name()
        )
    }

    /// Why a run of `--harness <harness>`, whose session may read outside its copy, is refused
    /// before its case opens under this adapter: under `unset`, what it proposes would not be
    /// scanned. None under a declared `none`.
    #[must_use]
    pub fn refused_at_open(&self, harness: &str) -> Option<String> {
        let Self::Unset = self else {
            return None;
        };
        Some(format!(
            "--harness {harness} runs a session that may read outside its copy, and no scan adapter is declared: what it proposes would not be scanned, and not scanned is never passed; declare `scan: none` in the run configuration file given with --config to accept what it proposes unscanned, checked only for the shape of a credential; nothing was opened"
        ))
    }

    /// Why a run whose forge `forge` publishes what the run makes is refused before its case
    /// opens under this adapter: under `unset`, what it publishes would not be scanned. None under
    /// a declared `none`.
    #[must_use]
    pub fn refused_to_publish(&self, forge: &str) -> Option<String> {
        let Self::Unset = self else {
            return None;
        };
        Some(format!(
            "the forge {forge} publishes what the run makes, and no scan adapter is declared: what it publishes would not be scanned, and not scanned is never passed; declare `scan: none` in the run configuration file given with --config to publish unscanned, or give --forge none; nothing was opened"
        ))
    }

    /// The line a run under a declared `none` says before its first step, `harness` the name
    /// of the run's harness; none under `unset`.
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

    /// Why nothing was scanned, as a forge's [`crate::forge::IdentityScan::NotScanned`] says
    /// it.
    #[must_use]
    pub fn not_scanned(&self) -> String {
        match self {
            Self::Unset => "no scan adapter is declared: nothing is scanned".to_owned(),
            Self::None { declared_in } => format!(
                "the scan adapter is none, `scan: none` declared in {}: nothing is scanned",
                declared_in.display()
            ),
        }
    }
}
