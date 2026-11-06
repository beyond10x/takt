//! Work sources beside the planning store (`takt.work.SourceName`): where a run's work item comes
//! from when it is no story of the store. [`command`] reads items through a team's own command
//! line.

pub mod command;
