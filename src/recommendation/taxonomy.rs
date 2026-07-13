use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

const DEFAULT_RESOURCE_PATH: &str = "resources/json/kurikulum_merdeka_structure.json";

#[derive(Debug, Clone)]
pub struct TaxonomyCatalog {
    pub raw: Value,
    pub source_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum TaxonomyError {
    #[error("taxonomy file not found: {0}")]
    NotFound(PathBuf),
    #[error("failed to read taxonomy file {0}: {1}")]
    Read(PathBuf, String),
    #[error("failed to parse taxonomy JSON {0}: {1}")]
    Parse(PathBuf, serde_json::Error),
}

impl TaxonomyCatalog {
    pub fn load_default() -> Result<Self, TaxonomyError> {
        Self::load_from_path(Self::default_resource_path())
    }

    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, TaxonomyError> {
        let path = path.into();
        if !path.exists() {
            return Err(TaxonomyError::NotFound(path));
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| TaxonomyError::Read(path.clone(), e.to_string()))?;
        let raw: Value = serde_json::from_slice(&bytes)
            .map_err(|e| TaxonomyError::Parse(path.clone(), e))?;
        Ok(Self {
            raw,
            source_path: path,
        })
    }

    fn default_resource_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_RESOURCE_PATH)
    }
}

pub type SharedTaxonomyCatalog = Arc<TaxonomyCatalog>;