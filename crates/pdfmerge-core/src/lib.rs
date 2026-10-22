mod error;
mod merge;
mod page_spec;

pub use crate::error::{MergeError, SourceError, SpecError};
pub use crate::merge::merge;
pub use crate::page_spec::PageSpec;

pub mod output;
pub mod sources;

/// Entrée source d'une fusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub path: std::path::PathBuf,
    pub spec: PageSpec,
}

/// Requête de fusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeRequest {
    pub sources: Vec<SourceEntry>,
    pub output: std::path::PathBuf,
    pub overwrite: bool,
}

/// Résultat d'une fusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeReport {
    pub output: std::path::PathBuf,
    pub page_count: usize,
}

/// Progression d'une fusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub processed: usize,
    pub total: usize,
}
