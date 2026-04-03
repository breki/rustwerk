use std::fmt;

use serde::{Deserialize, Serialize};

use super::error::DomainError;

/// Unique identifier for a developer — a short
/// alphanumeric username (lowercase).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct DeveloperId(String);

impl DeveloperId {
    /// Create a new developer ID. Must be non-empty,
    /// ASCII alphanumeric plus hyphens and underscores.
    /// Lowercased on creation.
    pub fn new(id: &str) -> Result<Self, DomainError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(DomainError::ValidationError(
                "developer ID must not be empty".into(),
            ));
        }
        if !id.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || c == '-'
                || c == '_'
        }) {
            return Err(DomainError::ValidationError(
                format!(
                    "developer ID must contain only \
                     ASCII alphanumeric characters, \
                     hyphens, and underscores: {id}"
                ),
            ));
        }
        Ok(Self(id.to_lowercase()))
    }

    /// Return the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeveloperId {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A developer on the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Developer {
    /// Full name.
    pub name: String,
    /// Email address (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Role on the project (optional, e.g.
    /// "project-lead", "developer").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Areas of expertise (optional).
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub specialties: Vec<String>,
}

impl Developer {
    /// Create a new developer with the given name.
    pub fn new(
        name: &str,
    ) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::ValidationError(
                "developer name must not be empty".into(),
            ));
        }
        Ok(Self {
            name: name.to_string(),
            email: None,
            role: None,
            specialties: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn developer_id_valid() {
        let id = DeveloperId::new("igor").unwrap();
        assert_eq!(id.as_str(), "igor");
    }

    #[test]
    fn developer_id_lowercased() {
        let id = DeveloperId::new("Igor").unwrap();
        assert_eq!(id.as_str(), "igor");
    }

    #[test]
    fn developer_id_with_hyphens() {
        let id =
            DeveloperId::new("john-doe").unwrap();
        assert_eq!(id.as_str(), "john-doe");
    }

    #[test]
    fn developer_id_empty_rejected() {
        assert!(DeveloperId::new("").is_err());
        assert!(DeveloperId::new("   ").is_err());
    }

    #[test]
    fn developer_id_spaces_rejected() {
        assert!(DeveloperId::new("john doe").is_err());
    }

    #[test]
    fn developer_id_unicode_rejected() {
        assert!(DeveloperId::new("игорь").is_err());
    }

    #[test]
    fn developer_new_valid() {
        let dev = Developer::new("Igor").unwrap();
        assert_eq!(dev.name, "Igor");
        assert!(dev.email.is_none());
        assert!(dev.role.is_none());
        assert!(dev.specialties.is_empty());
    }

    #[test]
    fn developer_new_trims_name() {
        let dev =
            Developer::new("  Alice  ").unwrap();
        assert_eq!(dev.name, "Alice");
    }

    #[test]
    fn developer_new_empty_rejected() {
        assert!(Developer::new("").is_err());
        assert!(Developer::new("   ").is_err());
    }
}
