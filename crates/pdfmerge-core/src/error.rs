use std::fmt;

/// Erreur levée par le parseur/évaluateur d'une sélection de pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    /// Position 1-based dans la chaîne analysée.
    pub position: usize,
    /// Motif lisible de l'erreur.
    pub message: String,
}

impl SpecError {
    pub fn new(position: usize, message: impl Into<String>) -> Self {
        Self {
            position,
            message: message.into(),
        }
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid page selection at position {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for SpecError {}

/// Erreur de fusion de documents.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("invalid page selection in {path:?} at position {position}: {message}")]
    Selection {
        path: std::path::PathBuf,
        position: usize,
        message: String,
    },
    #[error("encrypted PDF is not supported in v1: {0}")]
    Encrypted(std::path::PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("output already exists: {0}")]
    OutputExists(std::path::PathBuf),
    #[error("output would overwrite a source file: {0}")]
    OutputOverwritesSource(std::path::PathBuf),
    #[error("no sources to merge")]
    NoSources,
}

/// Erreur de résolution/expansion de sources.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("failed to read directory {path:?}: {source}")]
    ReadDirectory {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read source {path:?}: {source}")]
    ReadFile {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}
