//! Portable project identity and schema revision.

use serde::{Deserialize, Serialize};

use crate::{DomainError, ProjectId};

const MAX_PROJECT_NAME_BYTES: usize = 160;

/// Current independently versioned Shape project schema.
pub const SHAPE_PROJECT_SCHEMA_REVISION: &str = "20261018.1";

/// Durable identity and display metadata for one project bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Stable project identity independent of its filesystem path.
    pub id: ProjectId,
    /// User-visible project name.
    pub name: String,
    /// Exact schema revision required to open the bundle.
    pub schema_revision: String,
}

impl ProjectMetadata {
    /// Creates current-revision project metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when the name is empty or exceeds the portable limit.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        if name.is_empty() || name.len() > MAX_PROJECT_NAME_BYTES {
            return Err(DomainError::InvalidProjectName {
                max_bytes: MAX_PROJECT_NAME_BYTES,
            });
        }
        Ok(Self {
            id: ProjectId::new(),
            name,
            schema_revision: SHAPE_PROJECT_SCHEMA_REVISION.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests;
