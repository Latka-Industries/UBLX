//! Main-mode pages (Snapshot / Lenses / Delta / Duplicates / Settings).

mod delta;
mod duplicates;
mod lenses;
mod settings;
mod snapshot;

pub(crate) use delta::DeltaMode;
pub(crate) use duplicates::DuplicatesMode;
pub(crate) use lenses::LensesMode;
pub(crate) use settings::SettingsMode;
pub(crate) use snapshot::SnapshotMode;
