//! Jira REST client with HTTP Basic auth and Platform
//! API Gateway fallback for scoped tokens.
//!
//! The module is split around an [`HttpClient`] trait so
//! gateway-fallback logic can be unit-tested with a
//! recording fake — real Jira traffic goes through the
//! [`UreqClient`] implementation.
//!
//! Fallback flow (mirrors marketplace-v2 Python):
//!
//! 1. Attempt the operation against the direct Jira URL
//!    (`{jira_url}/rest/api/3/…`).
//! 2. If the response is **401** or **404**, fetch
//!    `{jira_url}/_edge/tenant_info` and extract
//!    `cloudId`.
//! 3. Retry the operation against the gateway URL
//!    `https://api.atlassian.com/ex/jira/{cloudId}/rest/api/3/…`.
//!
//! Any other HTTP status from the first call is returned
//! verbatim; only 401/404 triggers fallback.

use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64_STANDARD;
use base64::Engine;
use serde::Deserialize;
use thiserror::Error;

use crate::config::JiraConfig;

/// Errors surfaced by [`HttpClient`] implementations
/// and by [`create_issue`]. Implements
/// [`std::fmt::Display`] so call sites can format the
/// error once without re-prefixing classifications
/// (previously task messages accumulated two `"HTTP
/// error:"` prefixes).
#[derive(Debug, Error)]
pub(crate) enum HttpError {
    #[error("HTTP transport error: {0}")]
    Transport(String),
    #[error("tenant_info returned HTTP {0} while resolving cloudId")]
    TenantInfo(u16),
    #[error("tenant_info JSON parse error: {0}")]
    TenantInfoDecode(serde_json::Error),
}

/// Errors produced by [`parse_created_issue`] when a
/// 2xx body cannot be turned into a usable
/// [`CreatedIssue`]. Each variant has a distinct
/// `Display` so `task_result_from_create_outcome` can surface
/// the *reason* plugin state wasn't recorded — silent
/// `Option::None` previously let broken responses cause
/// unlimited duplicate Jira issues on repeat pushes.
#[derive(Debug, Error)]
pub(crate) enum ParseIssueError {
    #[error("response body was empty")]
    EmptyBody,
    #[error("response body could not be parsed: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("response field `{field}` was empty")]
    EmptyField { field: &'static str },
    #[error("response `self` URL has unsupported scheme: {0:.256}")]
    InvalidSelfUrl(String),
    #[error("response `key` is not a valid Jira issue key: {0:.64}")]
    InvalidIssueKey(String),
}

/// Validated Jira issue key (e.g. `"PROJ-142"`).
/// Constructed only through [`IssueKey::parse`], which
/// enforces Jira's documented issue-key grammar
/// `[A-Z][A-Z0-9_]*-[0-9]+`. Every URL builder and
/// per-issue verb takes `&IssueKey`, so a malformed or
/// attacker-controlled value from stored state cannot
/// reach [`format!`]-based URL construction.
///
/// Closes RT-121 (path-traversal via poisoned
/// `plugin_state.jira.key`) and is the type-safe
/// encoding of AQ-105 (C-NEWTYPE from the Rust API
/// Guidelines).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueKey(String);

impl IssueKey {
    /// Parse a candidate string into an [`IssueKey`].
    /// Returns `None` for anything that does not match
    /// `[A-Z][A-Z0-9_]*-[0-9]+` — and also bounds the
    /// length at 64 chars so a hostile stored value
    /// cannot blow up error messages via
    /// [`Display`]-propagation.
    pub fn parse(candidate: &str) -> Option<Self> {
        if candidate.is_empty() || candidate.len() > 64 {
            return None;
        }
        let (project, number) = candidate.split_once('-')?;
        if project.is_empty() || number.is_empty() {
            return None;
        }
        // Project part: leading ASCII upper, then upper / digit / `_`.
        let mut project_chars = project.chars();
        let first = project_chars.next()?;
        if !first.is_ascii_uppercase() {
            return None;
        }
        if !project_chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            return None;
        }
        // Number part: ASCII digits only.
        if !number.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(Self(candidate.to_string()))
    }

    /// Borrow the wrapped key as `&str`. No allocation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IssueKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Base URL for Atlassian Platform API Gateway used when
/// a scoped token cannot authenticate against the direct
/// site URL.
pub(crate) const GATEWAY_BASE: &str = "https://api.atlassian.com";

/// Per-call connect timeout. Each `UreqClient` call uses
/// this same value for connect, read, and write so a
/// hung Jira cannot stall the host indefinitely.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of bytes of an HTTP response body
/// retained by the client. Larger bodies are truncated.
/// Small enough that embedding a failed-response body
/// into a per-task error message stays bounded even for
/// a batch of tasks.
pub(crate) const MAX_RESPONSE_BODY_BYTES: usize = 4 * 1024;

/// Minimal HTTP response shape used by the client. Kept
/// small so a test fake can synthesize values without
/// dragging in real `ureq` types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpResponse {
    /// HTTP status code (e.g. 200, 401, 404).
    pub status: u16,
    /// Response body as UTF-8 text. Binary bodies are not
    /// expected from the Jira REST API.
    pub body: String,
}

/// Abstraction over the three HTTP verbs we need.
/// Kept minimal on purpose — widen only when a new
/// call site lands.
pub(crate) trait HttpClient {
    /// `GET url` with the given `Authorization` header
    /// value.
    fn get(&self, url: &str, auth: &str) -> Result<HttpResponse, HttpError>;

    /// `POST url` with the given `Authorization` header
    /// value and JSON body.
    fn post_json(
        &self,
        url: &str,
        auth: &str,
        body: &str,
    ) -> Result<HttpResponse, HttpError>;

    /// `PUT url` with the given `Authorization` header
    /// value and JSON body. Used by PLG-JIRA-UPDATE to
    /// edit issues that were previously created by this
    /// plugin.
    fn put_json(
        &self,
        url: &str,
        auth: &str,
        body: &str,
    ) -> Result<HttpResponse, HttpError>;
}

/// Build the `Authorization` header value for HTTP Basic
/// auth (`Basic base64(user:token)`).
pub(crate) fn basic_auth_header(user: &str, token: &str) -> String {
    let raw = format!("{user}:{token}");
    format!("Basic {}", B64_STANDARD.encode(raw.as_bytes()))
}

/// Build the direct `create issue` URL for a Jira site.
pub(crate) fn direct_create_issue_url(jira_url: &str) -> String {
    format!("{}/rest/api/3/issue", jira_url.trim_end_matches('/'))
}

/// Build the gateway `create issue` URL for a given
/// cloud ID.
pub(crate) fn gateway_create_issue_url(cloud_id: &str) -> String {
    format!("{GATEWAY_BASE}/ex/jira/{cloud_id}/rest/api/3/issue")
}

/// Build the `_edge/tenant_info` URL used to discover a
/// site's `cloudId`.
pub(crate) fn tenant_info_url(jira_url: &str) -> String {
    format!("{}/_edge/tenant_info", jira_url.trim_end_matches('/'))
}

/// Build the direct per-issue URL (`GET` / `PUT`
/// target) for a given issue `key`. Takes `&IssueKey`
/// rather than `&str` so malformed keys cannot reach
/// URL construction (closes RT-121).
pub(crate) fn direct_issue_url(jira_url: &str, key: &IssueKey) -> String {
    format!(
        "{}/rest/api/3/issue/{}",
        jira_url.trim_end_matches('/'),
        key.as_str()
    )
}

/// Build the gateway per-issue URL for a given
/// `cloud_id` + issue `key`. See [`direct_issue_url`]
/// for why the key is a newtype.
pub(crate) fn gateway_issue_url(cloud_id: &str, key: &IssueKey) -> String {
    format!(
        "{GATEWAY_BASE}/ex/jira/{cloud_id}/rest/api/3/issue/{}",
        key.as_str()
    )
}

/// Direct transition URL for a given issue key.
/// `POST /issue/{key}/transitions` is how Jira moves an
/// issue between workflow states — the payload carries
/// the transition ID (not a status name).
pub(crate) fn direct_transition_url(jira_url: &str, key: &IssueKey) -> String {
    format!(
        "{}/rest/api/3/issue/{}/transitions",
        jira_url.trim_end_matches('/'),
        key.as_str()
    )
}

/// Gateway transition URL. Mirrors
/// [`gateway_issue_url`] with the `/transitions` suffix.
pub(crate) fn gateway_transition_url(cloud_id: &str, key: &IssueKey) -> String {
    format!(
        "{GATEWAY_BASE}/ex/jira/{cloud_id}/rest/api/3/issue/{}/transitions",
        key.as_str()
    )
}

#[derive(Debug, Deserialize)]
struct TenantInfo {
    #[serde(rename = "cloudId")]
    cloud_id: String,
}

/// Typed view of the two fields of Jira's
/// `POST /rest/api/3/issue` success body that the
/// plugin actually persists into plugin state. Serde
/// ignores unknown fields by default, so `id` and
/// everything else in the response simply flow past.
///
/// `key` is an [`IssueKey`] rather than a raw `String`
/// so every downstream URL builder and verb gets
/// compile-time proof the value passed strict grammar
/// validation (closes RT-121 / AQ-105).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatedIssue {
    pub key: IssueKey,
    pub self_url: String,
}

/// Raw wire-shape of the create-issue response. Kept
/// private so callers see only the validated
/// [`CreatedIssue`].
#[derive(Debug, Deserialize)]
struct CreatedIssueWire {
    key: String,
    #[serde(rename = "self")]
    self_url: String,
}

/// Parse a Jira create-issue response body into a
/// [`CreatedIssue`], rejecting bodies that would
/// produce an unusable idempotency anchor:
///
/// - empty body (`204 No Content`, silent skip by the
///   caller);
/// - malformed JSON or missing fields (a possible
///   Jira/proxy schema drift);
/// - empty-string `key` or `self` (would defeat
///   [`PLG-JIRA-UPDATE`]'s dispatch table);
/// - `key` that does not match Jira's documented
///   issue-key grammar (closes RT-121 — a poisoned
///   `../admin` key can no longer reach URL
///   construction);
/// - `self` URL with a non-`http(s)` scheme (would
///   let a compromised Jira poison persisted project
///   state with `javascript:` / `file:` URLs).
pub(crate) fn parse_created_issue(body: &str) -> Result<CreatedIssue, ParseIssueError> {
    if body.trim().is_empty() {
        return Err(ParseIssueError::EmptyBody);
    }
    let wire: CreatedIssueWire = serde_json::from_str(body)?;
    if wire.key.is_empty() {
        return Err(ParseIssueError::EmptyField { field: "key" });
    }
    if wire.self_url.is_empty() {
        return Err(ParseIssueError::EmptyField { field: "self" });
    }
    let parsed = url::Url::parse(&wire.self_url)
        .map_err(|_| ParseIssueError::InvalidSelfUrl(wire.self_url.clone()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ParseIssueError::InvalidSelfUrl(wire.self_url));
    }
    let key = IssueKey::parse(&wire.key)
        .ok_or_else(|| ParseIssueError::InvalidIssueKey(wire.key.clone()))?;
    Ok(CreatedIssue {
        key,
        self_url: wire.self_url,
    })
}

/// Outcome of any Jira REST operation (create, probe,
/// update). Carries the final status + body so callers
/// can parse issue responses or embed error bodies in a
/// task message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JiraOpOutcome {
    /// Final HTTP status code returned by Jira.
    pub status: u16,
    /// Final body text.
    pub body: String,
    /// `true` if the call was retried through the
    /// gateway after a 401/404 on the direct URL.
    pub used_gateway: bool,
}

/// Resolve the site's `cloudId` by calling
/// `_edge/tenant_info`. Shared by create / probe /
/// update so the fallback path stays identical across
/// verbs.
fn resolve_cloud_id<C: HttpClient>(
    http: &C,
    config: &JiraConfig,
    auth: &str,
) -> Result<String, HttpError> {
    let tenant = http.get(&tenant_info_url(&config.jira_url), auth)?;
    if !(200..300).contains(&tenant.status) {
        return Err(HttpError::TenantInfo(tenant.status));
    }
    let info: TenantInfo = serde_json::from_str(&tenant.body)
        .map_err(HttpError::TenantInfoDecode)?;
    Ok(info.cloud_id)
}

/// Attempt to create a Jira issue, falling back through
/// the Platform API Gateway on 401/404.
pub(crate) fn create_issue<C: HttpClient>(
    http: &C,
    config: &JiraConfig,
    payload_json: &str,
) -> Result<JiraOpOutcome, HttpError> {
    let auth = basic_auth_header(&config.username, &config.jira_token);
    let direct = http.post_json(
        &direct_create_issue_url(&config.jira_url),
        &auth,
        payload_json,
    )?;
    if direct.status != 401 && direct.status != 404 {
        return Ok(JiraOpOutcome {
            status: direct.status,
            body: direct.body,
            used_gateway: false,
        });
    }
    let cloud_id = resolve_cloud_id(http, config, &auth)?;
    let retried = http.post_json(
        &gateway_create_issue_url(&cloud_id),
        &auth,
        payload_json,
    )?;
    Ok(JiraOpOutcome {
        status: retried.status,
        body: retried.body,
        used_gateway: true,
    })
}

/// Outcome of a [`get_issue`] probe. Richer than
/// [`JiraOpOutcome`] because the caller needs to
/// distinguish **authoritative** absence (direct and
/// gateway both returned 404 — issue truly missing,
/// recreate is safe) from **ambiguous** absence
/// (direct 401 blocked the only trustworthy read, so
/// a gateway 404 might just mean "scoped-token project
/// restriction" rather than "deleted"). Closes RT-122.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    /// Issue is known to exist (direct or gateway
    /// returned a success body). Carries the final
    /// response status for the "updated (HTTP 200)"
    /// message and the gateway flag.
    Exists { status: u16, used_gateway: bool },
    /// Both the direct URL *and* the gateway returned
    /// 404. Safe to recreate — the direct URL is
    /// authoritative for existence when readable.
    MissingConfirmed { used_gateway: bool },
    /// The direct URL returned 401 (auth blocked) and
    /// the gateway returned 404. We cannot tell
    /// whether the issue actually exists. Must **not**
    /// recreate — that would produce a duplicate when
    /// the issue is alive but unreadable with this
    /// token. The caller fails loudly.
    MissingAmbiguous,
    /// Direct URL (and gateway retry, if any) returned
    /// something other than 2xx/401/404. Pass status +
    /// body up so the task message is usable.
    OtherStatus {
        status: u16,
        body: String,
        used_gateway: bool,
    },
}

/// Probe an existing Jira issue with `GET /issue/{key}`.
/// Used by [`push_one_update`] to decide whether to
/// `PUT` (issue exists) or fall back to `POST` + state
/// overwrite (issue was deleted in Jira).
///
/// Gateway-fallback behavior matches [`create_issue`]:
/// on a direct-URL 401/404 we look up the cloud ID and
/// retry through the gateway. The caller sees a
/// [`ProbeOutcome`] that distinguishes authoritative
/// 404 (direct 404 + gateway 404) from ambiguous 404
/// (direct 401, gateway 404) so the recreate path is
/// never taken on an unverifiable absence.
pub(crate) fn get_issue<C: HttpClient>(
    http: &C,
    config: &JiraConfig,
    key: &IssueKey,
) -> Result<ProbeOutcome, HttpError> {
    let auth = basic_auth_header(&config.username, &config.jira_token);
    let direct = http.get(&direct_issue_url(&config.jira_url, key), &auth)?;
    if (200..300).contains(&direct.status) {
        return Ok(ProbeOutcome::Exists {
            status: direct.status,
            used_gateway: false,
        });
    }
    if direct.status != 401 && direct.status != 404 {
        return Ok(ProbeOutcome::OtherStatus {
            status: direct.status,
            body: direct.body,
            used_gateway: false,
        });
    }
    let direct_was_404 = direct.status == 404;
    let cloud_id = resolve_cloud_id(http, config, &auth)?;
    let retried = http.get(&gateway_issue_url(&cloud_id, key), &auth)?;
    if (200..300).contains(&retried.status) {
        return Ok(ProbeOutcome::Exists {
            status: retried.status,
            used_gateway: true,
        });
    }
    if retried.status == 404 {
        return Ok(if direct_was_404 {
            ProbeOutcome::MissingConfirmed { used_gateway: true }
        } else {
            ProbeOutcome::MissingAmbiguous
        });
    }
    Ok(ProbeOutcome::OtherStatus {
        status: retried.status,
        body: retried.body,
        used_gateway: true,
    })
}

/// Apply a Jira workflow transition by ID.
///
/// Jira does not accept `status` in the create / update
/// payload body — moving an issue between workflow
/// states requires a separate `POST
/// /rest/api/3/issue/{key}/transitions` with body
/// `{"transition":{"id":"<transition_id>"}}`. Success is
/// `204 No Content`.
///
/// Gateway-fallback mirrors the other verbs: 401 or 404
/// on the direct URL triggers a `cloudId` lookup and a
/// retry via the Platform API Gateway.
pub(crate) fn transition<C: HttpClient>(
    http: &C,
    config: &JiraConfig,
    key: &IssueKey,
    transition_id: &str,
) -> Result<JiraOpOutcome, HttpError> {
    let auth = basic_auth_header(&config.username, &config.jira_token);
    let body = serde_json::json!({ "transition": { "id": transition_id } }).to_string();
    let direct = http.post_json(
        &direct_transition_url(&config.jira_url, key),
        &auth,
        &body,
    )?;
    if direct.status != 401 && direct.status != 404 {
        return Ok(JiraOpOutcome {
            status: direct.status,
            body: direct.body,
            used_gateway: false,
        });
    }
    let cloud_id = resolve_cloud_id(http, config, &auth)?;
    let retried = http.post_json(
        &gateway_transition_url(&cloud_id, key),
        &auth,
        &body,
    )?;
    Ok(JiraOpOutcome {
        status: retried.status,
        body: retried.body,
        used_gateway: true,
    })
}

/// Update an existing Jira issue with `PUT /issue/{key}`.
/// Gateway-fallback mirrors [`create_issue`] and
/// [`get_issue`]. Jira returns `204 No Content` on
/// success; callers must not rely on a parseable body.
pub(crate) fn update_issue<C: HttpClient>(
    http: &C,
    config: &JiraConfig,
    key: &IssueKey,
    payload_json: &str,
) -> Result<JiraOpOutcome, HttpError> {
    let auth = basic_auth_header(&config.username, &config.jira_token);
    let direct = http.put_json(
        &direct_issue_url(&config.jira_url, key),
        &auth,
        payload_json,
    )?;
    if direct.status != 401 && direct.status != 404 {
        return Ok(JiraOpOutcome {
            status: direct.status,
            body: direct.body,
            used_gateway: false,
        });
    }
    let cloud_id = resolve_cloud_id(http, config, &auth)?;
    let retried = http.put_json(
        &gateway_issue_url(&cloud_id, key),
        &auth,
        payload_json,
    )?;
    Ok(JiraOpOutcome {
        status: retried.status,
        body: retried.body,
        used_gateway: true,
    })
}

/// Production [`HttpClient`] backed by `ureq`. Holds a
/// preconfigured [`ureq::Agent`] so connect/read/write
/// timeouts apply to every call; the bare
/// `ureq::get`/`post` helpers have no read timeout and
/// would let a slow server stall the host.
pub(crate) struct UreqClient {
    agent: ureq::Agent,
}

impl Default for UreqClient {
    fn default() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .timeout_connect(HTTP_TIMEOUT)
                .timeout_read(HTTP_TIMEOUT)
                .timeout_write(HTTP_TIMEOUT)
                .build(),
        }
    }
}

impl HttpClient for UreqClient {
    fn get(&self, url: &str, auth: &str) -> Result<HttpResponse, HttpError> {
        let response = self
            .agent
            .get(url)
            .set("Authorization", auth)
            .set("Accept", "application/json")
            .call();
        ureq_to_response(response)
    }

    fn post_json(
        &self,
        url: &str,
        auth: &str,
        body: &str,
    ) -> Result<HttpResponse, HttpError> {
        let response = self
            .agent
            .post(url)
            .set("Authorization", auth)
            .set("Accept", "application/json")
            .set("Content-Type", "application/json")
            .send_string(body);
        ureq_to_response(response)
    }

    fn put_json(
        &self,
        url: &str,
        auth: &str,
        body: &str,
    ) -> Result<HttpResponse, HttpError> {
        let response = self
            .agent
            .put(url)
            .set("Authorization", auth)
            .set("Accept", "application/json")
            .set("Content-Type", "application/json")
            .send_string(body);
        ureq_to_response(response)
    }
}

/// Normalise a `ureq` response (or error carrying a
/// response) into our small [`HttpResponse`] shape.
/// Transport errors map to [`HttpError::Transport`];
/// HTTP status errors preserve the response for caller
/// inspection.
fn ureq_to_response(
    result: Result<ureq::Response, ureq::Error>,
) -> Result<HttpResponse, HttpError> {
    match result {
        Ok(resp) | Err(ureq::Error::Status(_, resp)) => Ok(to_response(resp)),
        Err(ureq::Error::Transport(t)) => {
            Err(HttpError::Transport(transport_error_message(&t)))
        }
    }
}

/// Render a `ureq::Transport` error into a host-safe
/// string. Intentionally drops the URL (which can carry
/// embedded userinfo credentials) and keeps only the
/// error kind plus the short message.
fn transport_error_message(t: &ureq::Transport) -> String {
    match t.message() {
        Some(msg) => format!("({:?}): {msg}", t.kind()),
        None => format!("({:?})", t.kind()),
    }
}

fn to_response(resp: ureq::Response) -> HttpResponse {
    let status = resp.status();
    let body = resp.into_string().unwrap_or_default();
    HttpResponse {
        status,
        body: truncate_body(body),
    }
}

/// Cap `body` at [`MAX_RESPONSE_BODY_BYTES`] so a large
/// Jira (or proxy) response cannot blow up per-task
/// error messages. Truncation respects UTF-8 char
/// boundaries — we never split a codepoint.
fn truncate_body(mut body: String) -> String {
    if body.len() <= MAX_RESPONSE_BODY_BYTES {
        return body;
    }
    let mut cut = MAX_RESPONSE_BODY_BYTES;
    while cut > 0 && !body.is_char_boundary(cut) {
        cut -= 1;
    }
    body.truncate(cut);
    body.push_str("…[truncated]");
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{ok, transport_err, Call, MockHttp};

    fn cfg() -> JiraConfig {
        JiraConfig {
            jira_url: "https://example.atlassian.net".into(),
            jira_token: "tok".into(),
            username: "user@example.com".into(),
            project_key: "PROJ".into(),
            default_issue_type: None,
            issue_type_map: std::collections::HashMap::new(),
            status_map: std::collections::HashMap::new(),
            assignee_map: std::collections::HashMap::new(),
            priority_map: std::collections::HashMap::new(),
            labels_from_tags: false,
            epic_link_custom_field: None,
        }
    }

    #[test]
    fn basic_auth_encodes_user_and_token() {
        let header = basic_auth_header("alice@example.com", "secret-123");
        assert!(header.starts_with("Basic "));
        let b64 = &header["Basic ".len()..];
        let decoded = B64_STANDARD.decode(b64).unwrap();
        assert_eq!(decoded, b"alice@example.com:secret-123");
    }

    #[test]
    fn basic_auth_handles_empty_token() {
        let header = basic_auth_header("u", "");
        let b64 = &header["Basic ".len()..];
        assert_eq!(B64_STANDARD.decode(b64).unwrap(), b"u:");
    }

    #[test]
    fn direct_url_strips_trailing_slash() {
        assert_eq!(
            direct_create_issue_url("https://x.atlassian.net/"),
            "https://x.atlassian.net/rest/api/3/issue"
        );
        assert_eq!(
            direct_create_issue_url("https://x.atlassian.net"),
            "https://x.atlassian.net/rest/api/3/issue"
        );
    }

    #[test]
    fn gateway_url_uses_cloud_id() {
        assert_eq!(
            gateway_create_issue_url("abc-123"),
            "https://api.atlassian.com/ex/jira/abc-123/rest/api/3/issue"
        );
    }

    #[test]
    fn tenant_info_url_strips_trailing_slash() {
        assert_eq!(
            tenant_info_url("https://x.atlassian.net/"),
            "https://x.atlassian.net/_edge/tenant_info"
        );
    }

    #[test]
    fn create_issue_direct_success_skips_fallback() {
        let http = MockHttp::new(vec![ok(201, r#"{"id":"1","key":"PROJ-1"}"#)]);
        let out = create_issue(&http, &cfg(), r#"{"fields":{}}"#).unwrap();
        assert_eq!(out.status, 201);
        assert!(!out.used_gateway);
        assert_eq!(http.calls().len(), 1);
    }

    #[test]
    fn create_issue_falls_back_on_401() {
        let http = MockHttp::new(vec![
            ok(401, "unauthorized"),
            ok(200, r#"{"cloudId":"cid-1"}"#),
            ok(201, r#"{"id":"2","key":"PROJ-2"}"#),
        ]);
        let out = create_issue(&http, &cfg(), r#"{"fields":{}}"#).unwrap();
        assert_eq!(out.status, 201);
        assert!(out.used_gateway);
        let calls = http.calls();
        assert_eq!(calls.len(), 3);
        assert!(matches!(&calls[0], Call::Post { url, .. } if url.contains("example.atlassian.net/rest/api/3/issue")));
        assert!(matches!(&calls[1], Call::Get { url, .. } if url.contains("_edge/tenant_info")));
        assert!(matches!(&calls[2], Call::Post { url, .. } if url == "https://api.atlassian.com/ex/jira/cid-1/rest/api/3/issue"));
    }

    #[test]
    fn create_issue_falls_back_on_404() {
        let http = MockHttp::new(vec![
            ok(404, "not found"),
            ok(200, r#"{"cloudId":"cid-2"}"#),
            ok(201, r#"{"id":"3","key":"PROJ-3"}"#),
        ]);
        let out = create_issue(&http, &cfg(), "{}").unwrap();
        assert!(out.used_gateway);
        assert_eq!(out.status, 201);
    }

    #[test]
    fn create_issue_no_fallback_on_403() {
        let http = MockHttp::new(vec![ok(403, "forbidden")]);
        let out = create_issue(&http, &cfg(), "{}").unwrap();
        assert_eq!(out.status, 403);
        assert!(!out.used_gateway);
        assert_eq!(http.calls().len(), 1);
    }

    #[test]
    fn create_issue_no_fallback_on_500() {
        let http = MockHttp::new(vec![ok(500, "boom")]);
        let out = create_issue(&http, &cfg(), "{}").unwrap();
        assert_eq!(out.status, 500);
        assert!(!out.used_gateway);
    }

    #[test]
    fn create_issue_errors_when_tenant_info_fails() {
        let http = MockHttp::new(vec![ok(401, "nope"), ok(500, "tenant down")]);
        let err = create_issue(&http, &cfg(), "{}").unwrap_err();
        assert!(matches!(err, HttpError::TenantInfo(500)));
    }

    #[test]
    fn create_issue_errors_when_tenant_info_missing_cloud_id() {
        let http = MockHttp::new(vec![
            ok(401, "nope"),
            ok(200, r#"{"not":"cloudId"}"#),
        ]);
        let err = create_issue(&http, &cfg(), "{}").unwrap_err();
        assert!(matches!(err, HttpError::TenantInfoDecode(_)));
    }

    #[test]
    fn create_issue_propagates_transport_error_from_first_call() {
        let http = MockHttp::new(vec![transport_err("dns failed")]);
        let err = create_issue(&http, &cfg(), "{}").unwrap_err();
        assert!(matches!(&err, HttpError::Transport(m) if m.contains("dns failed")));
    }

    #[test]
    fn truncate_body_leaves_short_body_untouched() {
        let s = "hello";
        let out = truncate_body(s.into());
        assert_eq!(out, "hello");
    }

    #[test]
    fn truncate_body_caps_long_body_and_marks_it() {
        let big = "a".repeat(MAX_RESPONSE_BODY_BYTES * 2);
        let out = truncate_body(big);
        assert!(out.len() < MAX_RESPONSE_BODY_BYTES * 2);
        assert!(out.ends_with("…[truncated]"));
    }

    #[test]
    fn truncate_body_respects_utf8_boundary() {
        // Build a body whose byte length is exactly the
        // cap, but where the cap falls inside a multi-
        // byte codepoint; `truncate_body` must step back
        // to a valid boundary rather than panic.
        let mut body = "a".repeat(MAX_RESPONSE_BODY_BYTES - 1);
        body.push('€'); // 3 bytes, straddles the cap
        body.push_str("tail");
        let out = truncate_body(body);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.ends_with("…[truncated]"));
    }

    #[test]
    fn create_issue_body_is_truncated_in_outcome() {
        let huge = "x".repeat(MAX_RESPONSE_BODY_BYTES * 3);
        // to_response is what ureq_to_response delegates
        // through; exercise truncation via the logical
        // public surface by going through HttpResponse.
        let resp = HttpResponse {
            status: 500,
            body: truncate_body(huge),
        };
        assert!(resp.body.len() < MAX_RESPONSE_BODY_BYTES * 3);
        assert!(resp.body.ends_with("…[truncated]"));
    }

    fn key(s: &str) -> IssueKey {
        IssueKey::parse(s).unwrap()
    }

    #[test]
    fn get_issue_direct_2xx_returns_exists_without_fallback() {
        let http = MockHttp::new(vec![ok(200, r#"{"key":"PROJ-1"}"#)]);
        let probe = get_issue(&http, &cfg(), &key("PROJ-1")).unwrap();
        assert!(matches!(
            probe,
            ProbeOutcome::Exists {
                status: 200,
                used_gateway: false
            }
        ));
        assert_eq!(http.calls().len(), 1);
    }

    #[test]
    fn get_issue_direct_401_then_gateway_200_returns_exists_via_gateway() {
        let http = MockHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(200, r#"{"key":"PROJ-1"}"#),
        ]);
        let probe = get_issue(&http, &cfg(), &key("PROJ-1")).unwrap();
        assert!(matches!(
            probe,
            ProbeOutcome::Exists {
                status: 200,
                used_gateway: true
            }
        ));
        let calls = http.calls();
        assert!(matches!(&calls[2], Call::Get { url, .. }
            if url == "https://api.atlassian.com/ex/jira/cid/rest/api/3/issue/PROJ-1"));
    }

    #[test]
    fn get_issue_direct_404_and_gateway_404_returns_missing_confirmed() {
        let http = MockHttp::new(vec![
            ok(404, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(404, ""),
        ]);
        let probe = get_issue(&http, &cfg(), &key("MISSING-1")).unwrap();
        assert!(matches!(
            probe,
            ProbeOutcome::MissingConfirmed { used_gateway: true }
        ));
    }

    #[test]
    fn get_issue_direct_401_and_gateway_404_returns_missing_ambiguous() {
        // RT-122: direct-URL 401 + gateway 404 is not
        // proof of absence and must NOT trigger a
        // recreate. The probe says "ambiguous" and the
        // caller fails the task.
        let http = MockHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(404, ""),
        ]);
        let probe = get_issue(&http, &cfg(), &key("PROJ-1")).unwrap();
        assert!(matches!(probe, ProbeOutcome::MissingAmbiguous));
    }

    #[test]
    fn get_issue_5xx_on_direct_returns_other_status() {
        let http = MockHttp::new(vec![ok(503, "down")]);
        let probe = get_issue(&http, &cfg(), &key("PROJ-1")).unwrap();
        assert!(matches!(
            probe,
            ProbeOutcome::OtherStatus {
                status: 503,
                used_gateway: false,
                ..
            }
        ));
    }

    #[test]
    fn update_issue_direct_2xx_skips_fallback() {
        let http = MockHttp::new(vec![ok(204, "")]);
        let out =
            update_issue(&http, &cfg(), &key("PROJ-1"), r#"{"fields":{}}"#).unwrap();
        assert_eq!(out.status, 204);
        assert!(!out.used_gateway);
        let calls = http.calls();
        assert_eq!(calls.len(), 1);
        assert!(matches!(&calls[0], Call::Put { url, .. }
            if url == "https://example.atlassian.net/rest/api/3/issue/PROJ-1"));
    }

    #[test]
    fn update_issue_falls_back_to_gateway_on_401() {
        let http = MockHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(204, ""),
        ]);
        let out = update_issue(&http, &cfg(), &key("PROJ-1"), "{}").unwrap();
        assert!(out.used_gateway);
        assert_eq!(out.status, 204);
        let calls = http.calls();
        assert!(matches!(&calls[2], Call::Put { url, .. }
            if url == "https://api.atlassian.com/ex/jira/cid/rest/api/3/issue/PROJ-1"));
    }

    #[test]
    fn parse_created_issue_reads_key_and_self() {
        let body = r#"{"id":"10042","key":"PROJ-142","self":"https://x.atlassian.net/rest/api/3/issue/10042"}"#;
        let created = parse_created_issue(body).unwrap();
        assert_eq!(created.key.as_str(), "PROJ-142");
        assert_eq!(
            created.self_url,
            "https://x.atlassian.net/rest/api/3/issue/10042"
        );
    }

    #[test]
    fn parse_created_issue_empty_body_returns_empty_body_error() {
        assert!(matches!(
            parse_created_issue(""),
            Err(ParseIssueError::EmptyBody)
        ));
        assert!(matches!(
            parse_created_issue("   \n\t "),
            Err(ParseIssueError::EmptyBody)
        ));
    }

    #[test]
    fn parse_created_issue_missing_field_returns_malformed() {
        assert!(matches!(
            parse_created_issue(r#"{"key":"P-1"}"#),
            Err(ParseIssueError::Malformed(_))
        ));
        assert!(matches!(
            parse_created_issue(r#"{"id":"1"}"#),
            Err(ParseIssueError::Malformed(_))
        ));
    }

    #[test]
    fn parse_created_issue_invalid_json_returns_malformed() {
        assert!(matches!(
            parse_created_issue("not json"),
            Err(ParseIssueError::Malformed(_))
        ));
    }

    #[test]
    fn parse_created_issue_empty_key_is_rejected() {
        let body =
            r#"{"key":"","self":"https://x.atlassian.net/rest/api/3/issue/1"}"#;
        assert!(matches!(
            parse_created_issue(body),
            Err(ParseIssueError::EmptyField { field: "key" })
        ));
    }

    #[test]
    fn parse_created_issue_empty_self_is_rejected() {
        assert!(matches!(
            parse_created_issue(r#"{"key":"P-1","self":""}"#),
            Err(ParseIssueError::EmptyField { field: "self" })
        ));
    }

    #[test]
    fn parse_created_issue_rejects_non_http_scheme() {
        let body = r#"{"key":"P-1","self":"javascript:alert(1)"}"#;
        assert!(matches!(
            parse_created_issue(body),
            Err(ParseIssueError::InvalidSelfUrl(_))
        ));
    }

    #[test]
    fn parse_created_issue_rejects_file_scheme() {
        let body = r#"{"key":"P-1","self":"file:///etc/passwd"}"#;
        assert!(matches!(
            parse_created_issue(body),
            Err(ParseIssueError::InvalidSelfUrl(_))
        ));
    }

    #[test]
    fn parse_created_issue_accepts_plain_http() {
        let body = r#"{"key":"P-1","self":"http://jira.example/rest/api/3/issue/1"}"#;
        let created = parse_created_issue(body).unwrap();
        assert_eq!(created.key.as_str(), "P-1");
    }

    #[test]
    fn parse_created_issue_rejects_invalid_issue_key() {
        // Path-traversal-shaped key from a compromised
        // Jira must be rejected at parse time
        // (RT-121): even if `self` passes scheme
        // validation, a malformed `key` is refused.
        let body = r#"{"key":"../../admin","self":"https://x.atlassian.net/rest/api/3/issue/1"}"#;
        assert!(matches!(
            parse_created_issue(body),
            Err(ParseIssueError::InvalidIssueKey(_))
        ));
    }

    // --- PLG-JIRA-FIELDS: transition verb ---

    #[test]
    fn transition_direct_success_skips_fallback() {
        let http = MockHttp::new(vec![ok(204, "")]);
        let out = transition(&http, &cfg(), &key("PROJ-1"), "11").unwrap();
        assert_eq!(out.status, 204);
        assert!(!out.used_gateway);
        let calls = http.calls();
        assert_eq!(calls.len(), 1);
        match &calls[0] {
            Call::Post { url, body, .. } => {
                assert_eq!(
                    url,
                    "https://example.atlassian.net/rest/api/3/issue/PROJ-1/transitions"
                );
                assert!(body.contains(r#""transition""#));
                assert!(body.contains(r#""id":"11""#));
            }
            other => panic!("expected POST, got {other:?}"),
        }
    }

    #[test]
    fn transition_falls_back_on_401() {
        let http = MockHttp::new(vec![
            ok(401, "nope"),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(204, ""),
        ]);
        let out = transition(&http, &cfg(), &key("PROJ-1"), "11").unwrap();
        assert_eq!(out.status, 204);
        assert!(out.used_gateway);
        let calls = http.calls();
        assert!(matches!(&calls[2], Call::Post { url, .. }
            if url == "https://api.atlassian.com/ex/jira/cid/rest/api/3/issue/PROJ-1/transitions"));
    }

    #[test]
    fn transition_no_fallback_on_500() {
        let http = MockHttp::new(vec![ok(500, "broken")]);
        let out = transition(&http, &cfg(), &key("PROJ-1"), "11").unwrap();
        assert_eq!(out.status, 500);
        assert!(!out.used_gateway);
    }

    #[test]
    fn transition_propagates_transport_error() {
        let http = MockHttp::new(vec![transport_err("dns")]);
        let err = transition(&http, &cfg(), &key("PROJ-1"), "11").unwrap_err();
        assert!(matches!(err, HttpError::Transport(m) if m.contains("dns")));
    }

    #[test]
    fn direct_transition_url_strips_trailing_slash() {
        assert_eq!(
            direct_transition_url("https://x.atlassian.net/", &key("P-1")),
            "https://x.atlassian.net/rest/api/3/issue/P-1/transitions"
        );
    }

    #[test]
    fn gateway_transition_url_embeds_cloud_id() {
        assert_eq!(
            gateway_transition_url("cid", &key("P-1")),
            "https://api.atlassian.com/ex/jira/cid/rest/api/3/issue/P-1/transitions"
        );
    }

    #[test]
    fn create_issue_sends_auth_header_on_all_calls() {
        let http = MockHttp::new(vec![
            ok(401, ""),
            ok(200, r#"{"cloudId":"cid"}"#),
            ok(201, ""),
        ]);
        create_issue(&http, &cfg(), "{}").unwrap();
        for call in http.calls() {
            let auth = match call {
                Call::Get { auth, .. }
                | Call::Post { auth, .. }
                | Call::Put { auth, .. } => auth,
            };
            assert!(auth.starts_with("Basic "), "missing Basic auth: {auth}");
        }
    }
}
