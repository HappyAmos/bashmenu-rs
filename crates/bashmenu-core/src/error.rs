//! Domain error types for BashMenu.

use std::path::PathBuf;
use thiserror::Error;

/// Core error type for BashMenu operations.
#[derive(Error, Debug)]
pub enum BashMenuError {
    /// Filesystem I/O error
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Target path where error occurred
        path: PathBuf,
        /// Underlying std::io::Error
        #[source]
        source: std::io::Error,
    },

    /// YAML deserialization error
    #[error("YAML deserialization error: {0}")]
    Yaml(#[from] serde_saphyr::Error),

    /// YAML serialization error
    #[error("YAML serialization error: {0}")]
    YamlSerialize(#[from] serde_saphyr::SerializeError),

    /// JSON/Value conversion error
    #[error("Value conversion error: {0}")]
    Value(#[from] serde_json::Error),

    /// Invalid configuration or menu schema
    #[error("Schema error: {0}")]
    Schema(String),

    /// Theme not found in theme definitions
    #[error("Theme '{0}' not found")]
    ThemeNotFound(String),
}

/// Convenience result type using BashMenuError
pub type Result<T> = std::result::Result<T, BashMenuError>;
