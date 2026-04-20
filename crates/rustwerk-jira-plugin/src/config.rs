//! Plugin configuration parsing and validation.
//!
//! The host supplies plugin config as a JSON object
//! (see [`crate::lib`] FFI docs). [`JiraConfig::from_json`]
//! enforces that every required field is present and
//! non-empty so downstream HTTP code can rely on the
//! shape.

use std::collections::HashMap;

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
    /// Jira-side issue type to use when a task has no
    /// `issue_type` set. Written as Jira sees it (e.g.
    /// `"Task"`, `"Story"`). `None` falls through to the
    /// built-in `"Task"` so an unconfigured plugin stays
    /// backwards-compatible.
    #[serde(default)]
    pub default_issue_type: Option<String>,
    /// Overrides for the kebab-case rustwerk issue-type
    /// names → the exact string Jira expects. Exists
    /// because some Jira sites rename `"Sub-task"` to
    /// `"Subtask"` or localize names. Keys are the wire
    /// names emitted by rustwerk
    /// (`epic` / `story` / `task` / `sub-task`);
    /// omitted keys fall through to the built-in
    /// defaults in [`BUILT_IN_ISSUE_TYPE_NAMES`].
    #[serde(default)]
    pub issue_type_map: HashMap<String, String>,
}

/// Built-in mapping from rustwerk's kebab-case wire name
/// to the exact string Jira uses out of the box. Applied
/// when the user's `issue_type_map` omits an entry.
const BUILT_IN_ISSUE_TYPE_NAMES: &[(&str, &str)] = &[
    ("epic", "Epic"),
    ("story", "Story"),
    ("task", "Task"),
    ("sub-task", "Sub-task"),
];

/// Upper bound on the length of an incoming
/// `TaskDto.issue_type` wire string. 64 chars is more
/// than any canonical Jira issue-type name; anything
/// longer is almost certainly a corrupted or hostile
/// `project.json` and should not be forwarded.
const MAX_ISSUE_TYPE_WIRE_LEN: usize = 64;

/// Normalize an incoming kebab-case issue-type string
/// before map lookup. Collapses CLI-level aliases (e.g.
/// `"subtask"` → `"sub-task"`) so a user who writes
/// `issue_type_map: { "subtask": "Subtask" }` in their
/// config gets the expected override.
fn canonicalize_issue_type_kebab(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "subtask" => "sub-task".to_string(),
        _ => lower,
    }
}

/// Reject issue-type wire strings that are absurdly long
/// or contain control characters. `serde_json` already
/// escapes any payload, so there is no JSON-breakout
/// vector here — but forwarding junk to Jira yields
/// confusing errors, so fail fast and make the problem
/// visible.
fn is_plausible_issue_type_wire(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ISSUE_TYPE_WIRE_LEN
        && s.chars().all(|c| !c.is_control())
}

impl JiraConfig {
    /// Resolve the Jira-side issue-type name for a task.
    ///
    /// Fallback chain:
    /// 1. The task's own `issue_type`, looked up in
    ///    [`Self::issue_type_map`] (after alias
    ///    normalization) and falling through to
    ///    [`BUILT_IN_ISSUE_TYPE_NAMES`].
    /// 2. If the task carries an *unknown* kebab name
    ///    (something the plugin doesn't recognize from
    ///    either the map or the built-in table), fall
    ///    through to the config default — better to push
    ///    the user's configured safety net than to
    ///    forward a string Jira will reject.
    /// 3. [`Self::default_issue_type`] when the task has
    ///    no issue-type at all.
    /// 4. The literal `"Task"`.
    pub(crate) fn resolve_issue_type_name(
        &self,
        task_issue_type: Option<&str>,
    ) -> String {
        if let Some(raw) = task_issue_type {
            if is_plausible_issue_type_wire(raw) {
                let kebab = canonicalize_issue_type_kebab(raw);
                if let Some(mapped) = self.issue_type_map.get(&kebab) {
                    return mapped.clone();
                }
                if let Some((_, default)) = BUILT_IN_ISSUE_TYPE_NAMES
                    .iter()
                    .find(|(k, _)| *k == kebab)
                {
                    return (*default).to_string();
                }
            }
            // Unknown / implausible kebab — fall through
            // to the config default rather than forwarding
            // a value Jira is almost certain to reject.
        }
        self.default_issue_type
            .clone()
            .unwrap_or_else(|| "Task".to_string())
    }
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
        let mut cfg: Self = serde_json::from_str(json)?;
        cfg.validate()?;
        // Rewrite user-supplied `issue_type_map` keys into
        // their canonical kebab form so that a config
        // written with `"subtask"` resolves to the same
        // entry as one written with `"sub-task"`.
        cfg.issue_type_map = cfg
            .issue_type_map
            .into_iter()
            .map(|(k, v)| (canonicalize_issue_type_kebab(&k), v))
            .collect();
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

    // --- issue-type resolution ---

    fn plain_cfg() -> JiraConfig {
        JiraConfig {
            jira_url: "https://x.atlassian.net".into(),
            jira_token: "t".into(),
            username: "u".into(),
            project_key: "P".into(),
            default_issue_type: None,
            issue_type_map: HashMap::new(),
        }
    }

    #[test]
    fn resolve_falls_back_to_task_when_no_signal() {
        let cfg = plain_cfg();
        assert_eq!(cfg.resolve_issue_type_name(None), "Task");
    }

    #[test]
    fn resolve_uses_config_default_when_task_has_no_type() {
        let mut cfg = plain_cfg();
        cfg.default_issue_type = Some("Story".into());
        assert_eq!(cfg.resolve_issue_type_name(None), "Story");
    }

    #[test]
    fn resolve_uses_builtin_name_for_each_variant() {
        let cfg = plain_cfg();
        assert_eq!(cfg.resolve_issue_type_name(Some("epic")), "Epic");
        assert_eq!(cfg.resolve_issue_type_name(Some("story")), "Story");
        assert_eq!(cfg.resolve_issue_type_name(Some("task")), "Task");
        assert_eq!(cfg.resolve_issue_type_name(Some("sub-task")), "Sub-task");
    }

    #[test]
    fn resolve_applies_map_override() {
        let mut cfg = plain_cfg();
        cfg.issue_type_map
            .insert("sub-task".into(), "Subtask".into());
        cfg.issue_type_map.insert("epic".into(), "Initiative".into());
        assert_eq!(cfg.resolve_issue_type_name(Some("sub-task")), "Subtask");
        assert_eq!(cfg.resolve_issue_type_name(Some("epic")), "Initiative");
        // Unmapped entries still use the built-in name.
        assert_eq!(cfg.resolve_issue_type_name(Some("story")), "Story");
    }

    #[test]
    fn resolve_task_type_wins_over_config_default() {
        let mut cfg = plain_cfg();
        cfg.default_issue_type = Some("Ignored".into());
        assert_eq!(cfg.resolve_issue_type_name(Some("epic")), "Epic");
    }

    #[test]
    fn resolve_unknown_kebab_falls_through_to_default() {
        // Future variant on a fresh rustwerk with an older
        // plugin installed: push the configured default
        // rather than a literal string Jira will reject.
        let cfg = plain_cfg();
        assert_eq!(cfg.resolve_issue_type_name(Some("bug")), "Task");
        let mut cfg2 = plain_cfg();
        cfg2.default_issue_type = Some("Story".into());
        assert_eq!(cfg2.resolve_issue_type_name(Some("bug")), "Story");
    }

    #[test]
    fn resolve_normalizes_subtask_alias_for_map_lookup() {
        // A user-written config key using the `subtask`
        // alias must resolve the same as `sub-task`. Go
        // through `from_json` so the load-time key
        // canonicalization runs.
        let json = serde_json::json!({
            "jira_url": "https://x.atlassian.net",
            "jira_token": "t",
            "username": "u",
            "project_key": "P",
            "issue_type_map": { "subtask": "Subtask" },
        })
        .to_string();
        let cfg = JiraConfig::from_json(&json).unwrap();
        assert_eq!(cfg.resolve_issue_type_name(Some("sub-task")), "Subtask");
        // An incoming wire name "subtask" (e.g. from a
        // future client that relaxes kebab rules) must
        // also resolve through the same override.
        assert_eq!(cfg.resolve_issue_type_name(Some("subtask")), "Subtask");
    }

    #[test]
    fn resolve_rejects_implausible_wire_values() {
        // Oversized / control-char / empty wire strings
        // must not leak into the payload. The default
        // kicks in instead.
        let cfg = plain_cfg();
        assert_eq!(cfg.resolve_issue_type_name(Some("")), "Task");
        let long = "a".repeat(65);
        assert_eq!(cfg.resolve_issue_type_name(Some(&long)), "Task");
        assert_eq!(cfg.resolve_issue_type_name(Some("has\nnewline")), "Task");
    }

    #[test]
    fn config_parses_optional_issue_type_fields() {
        let json = serde_json::json!({
            "jira_url": "https://x.atlassian.net",
            "jira_token": "t",
            "username": "u",
            "project_key": "P",
            "default_issue_type": "Story",
            "issue_type_map": { "sub-task": "Subtask" },
        })
        .to_string();
        let cfg = JiraConfig::from_json(&json).unwrap();
        assert_eq!(cfg.default_issue_type.as_deref(), Some("Story"));
        assert_eq!(
            cfg.issue_type_map.get("sub-task").map(String::as_str),
            Some("Subtask")
        );
    }

    #[test]
    fn config_defaults_issue_type_fields_when_absent() {
        let cfg = JiraConfig::from_json(&full_json()).unwrap();
        assert!(cfg.default_issue_type.is_none());
        assert!(cfg.issue_type_map.is_empty());
    }
}
