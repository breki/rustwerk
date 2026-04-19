//! Plugin configuration parsing and validation.
//!
//! The host supplies plugin config as a JSON object
//! (see [`crate::lib`] FFI docs). [`JiraConfig::from_json`]
//! enforces that every required field is present and
//! non-empty so downstream HTTP code can rely on the
//! shape.

use serde::Deserialize;
use url::Url;

/// Suffix of the only host family this plugin will talk
/// to. Anything else is rejected so a misconfigured or
/// attacker-controlled `jira_url` cannot redirect the
/// user's API token to a third party.
const ATLASSIAN_HOST_SUFFIX: &str = ".atlassian.net";

/// Configuration required to talk to a Jira Cloud site.
///
/// Fields map 1:1 to keys in the JSON object the host
/// passes through the FFI boundary.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JiraConfig {
    /// Base URL of the Jira Cloud site, e.g.
    /// `https://example.atlassian.net`.
    pub jira_url: String,
    /// Scoped API token used as the Basic-auth password.
    pub jira_token: String,
    /// User identity used as the Basic-auth username —
    /// typically the user's email.
    pub username: String,
    /// Jira project key to create issues under, e.g.
    /// `PROJ`.
    pub project_key: String,
}

/// Errors produced while parsing/validating plugin
/// config.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    /// JSON did not deserialize into the expected shape.
    #[error("invalid plugin config JSON: {0}")]
    Parse(String),
    /// A required field was present but empty, or
    /// missing entirely after defaulting.
    #[error("plugin config field '{field}' is required and must be non-empty")]
    MissingField {
        /// Name of the offending field.
        field: &'static str,
    },
    /// `jira_url` is not a syntactically valid URL.
    #[error("plugin config field 'jira_url' is not a valid URL: {0}")]
    InvalidUrl(String),
    /// `jira_url` uses a non-`https` scheme.
    #[error("plugin config field 'jira_url' must use https, got '{0}'")]
    InsecureScheme(String),
    /// `jira_url` points outside the allowed Atlassian
    /// host family. Protects the Basic-auth token from
    /// being sent to an attacker-controlled origin.
    #[error("plugin config field 'jira_url' must be a *.atlassian.net host, got '{0}'")]
    DisallowedHost(String),
}

// thiserror doesn't know serde_json's type here — we
// stringify it so the public API exposes only `String`.
impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        Self::Parse(e.to_string())
    }
}

impl JiraConfig {
    /// Parse and validate plugin config from a JSON
    /// string. Returns [`ConfigError::MissingField`] if
    /// any required field is empty.
    pub(crate) fn from_json(json: &str) -> Result<Self, ConfigError> {
        let cfg: Self = serde_json::from_str(json)?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.jira_url.trim().is_empty() {
            return Err(ConfigError::MissingField { field: "jira_url" });
        }
        if self.jira_token.trim().is_empty() {
            return Err(ConfigError::MissingField { field: "jira_token" });
        }
        if self.username.trim().is_empty() {
            return Err(ConfigError::MissingField { field: "username" });
        }
        if self.project_key.trim().is_empty() {
            return Err(ConfigError::MissingField {
                field: "project_key",
            });
        }
        validate_jira_url(&self.jira_url)?;
        Ok(())
    }
}

/// Enforce scheme + host allowlist on the Jira base URL.
/// Kept as a free function so both the top-level
/// validator and future callers (tests, host-side
/// config assembly) can reuse it.
fn validate_jira_url(raw: &str) -> Result<(), ConfigError> {
    let url = Url::parse(raw).map_err(|e| ConfigError::InvalidUrl(e.to_string()))?;
    if url.scheme() != "https" {
        return Err(ConfigError::InsecureScheme(url.scheme().into()));
    }
    let host = url.host_str().ok_or_else(|| {
        ConfigError::InvalidUrl("missing host".into())
    })?;
    let host_lower = host.to_ascii_lowercase();
    if !host_lower.ends_with(ATLASSIAN_HOST_SUFFIX)
        || host_lower.len() <= ATLASSIAN_HOST_SUFFIX.len()
    {
        return Err(ConfigError::DisallowedHost(host.into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_json() -> String {
        serde_json::json!({
            "jira_url": "https://x.atlassian.net",
            "jira_token": "tok",
            "username": "u@example.com",
            "project_key": "PROJ",
        })
        .to_string()
    }

    #[test]
    fn parses_complete_config() {
        let cfg = JiraConfig::from_json(&full_json()).unwrap();
        assert_eq!(cfg.jira_url, "https://x.atlassian.net");
        assert_eq!(cfg.jira_token, "tok");
        assert_eq!(cfg.username, "u@example.com");
        assert_eq!(cfg.project_key, "PROJ");
    }

    #[test]
    fn rejects_missing_jira_url() {
        let json = serde_json::json!({
            "jira_token": "t",
            "username": "u",
            "project_key": "P",
        })
        .to_string();
        let err = JiraConfig::from_json(&json).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn rejects_empty_jira_url() {
        let json = serde_json::json!({
            "jira_url": "",
            "jira_token": "t",
            "username": "u",
            "project_key": "P",
        })
        .to_string();
        let err = JiraConfig::from_json(&json).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField { field: "jira_url" }
        ));
    }

    #[test]
    fn rejects_whitespace_jira_token() {
        let json = serde_json::json!({
            "jira_url": "https://x.atlassian.net",
            "jira_token": "   ",
            "username": "u",
            "project_key": "P",
        })
        .to_string();
        let err = JiraConfig::from_json(&json).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField { field: "jira_token" }
        ));
    }

    #[test]
    fn rejects_empty_username() {
        let json = serde_json::json!({
            "jira_url": "https://x.atlassian.net",
            "jira_token": "t",
            "username": "",
            "project_key": "P",
        })
        .to_string();
        let err = JiraConfig::from_json(&json).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField { field: "username" }
        ));
    }

    #[test]
    fn rejects_empty_project_key() {
        let json = serde_json::json!({
            "jira_url": "https://x.atlassian.net",
            "jira_token": "t",
            "username": "u",
            "project_key": "",
        })
        .to_string();
        let err = JiraConfig::from_json(&json).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingField {
                field: "project_key"
            }
        ));
    }

    #[test]
    fn rejects_malformed_json() {
        let err = JiraConfig::from_json("not json").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn error_display_for_missing_field() {
        let err = ConfigError::MissingField { field: "jira_url" };
        assert!(format!("{err}").contains("jira_url"));
    }

    fn with_url(url: &str) -> String {
        serde_json::json!({
            "jira_url": url,
            "jira_token": "t",
            "username": "u",
            "project_key": "P",
        })
        .to_string()
    }

    #[test]
    fn rejects_non_https_scheme() {
        let err =
            JiraConfig::from_json(&with_url("http://x.atlassian.net")).unwrap_err();
        assert!(matches!(err, ConfigError::InsecureScheme(s) if s == "http"));
    }

    #[test]
    fn rejects_host_outside_atlassian() {
        let err =
            JiraConfig::from_json(&with_url("https://evil.example")).unwrap_err();
        assert!(matches!(err, ConfigError::DisallowedHost(_)));
    }

    #[test]
    fn rejects_bare_atlassian_suffix() {
        // ".atlassian.net" with an empty label before the
        // suffix must not pass — a misconfigured empty
        // subdomain would otherwise end with the suffix.
        let err = JiraConfig::from_json(&with_url("https://.atlassian.net"))
            .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidUrl(_)
                                  | ConfigError::DisallowedHost(_)));
    }

    #[test]
    fn rejects_malformed_url() {
        let err = JiraConfig::from_json(&with_url("not a url")).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidUrl(_)));
    }

    #[test]
    fn accepts_subdomain_of_atlassian_net() {
        let cfg =
            JiraConfig::from_json(&with_url("https://acme.atlassian.net")).unwrap();
        assert_eq!(cfg.jira_url, "https://acme.atlassian.net");
    }

    #[test]
    fn host_check_is_case_insensitive() {
        let cfg =
            JiraConfig::from_json(&with_url("https://ACME.Atlassian.Net")).unwrap();
        assert!(cfg.jira_url.contains("ACME"));
    }

    #[test]
    fn error_display_for_disallowed_host_mentions_host() {
        let err = ConfigError::DisallowedHost("evil.example".into());
        assert!(format!("{err}").contains("evil.example"));
    }

    #[test]
    fn error_display_for_insecure_scheme_mentions_scheme() {
        let err = ConfigError::InsecureScheme("http".into());
        assert!(format!("{err}").contains("http"));
    }
}
