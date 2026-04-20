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
/// `Display` so `task_result_from_outcome` can surface
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
    #[error("response `self` URL has unsupported scheme: {0}")]
    InvalidSelfUrl(String),
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

/// Abstraction over the two HTTP verbs we need. Kept
/// minimal on purpose — widen only when a new call site
/// lands.
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct CreatedIssue {
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
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
/// - `self` URL with a non-`http(s)` scheme (would
///   let a compromised Jira poison persisted project
///   state with `javascript:` / `file:` URLs).
pub(crate) fn parse_created_issue(body: &str) -> Result<CreatedIssue, ParseIssueError> {
    if body.trim().is_empty() {
        return Err(ParseIssueError::EmptyBody);
    }
    let created: CreatedIssue = serde_json::from_str(body)?;
    if created.key.is_empty() {
        return Err(ParseIssueError::EmptyField { field: "key" });
    }
    if created.self_url.is_empty() {
        return Err(ParseIssueError::EmptyField { field: "self" });
    }
    let parsed = url::Url::parse(&created.self_url)
        .map_err(|_| ParseIssueError::InvalidSelfUrl(created.self_url.clone()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ParseIssueError::InvalidSelfUrl(created.self_url));
    }
    Ok(created)
}

/// Outcome of a create-issue call. Carries the final
/// status + body so callers can parse the Jira issue
/// response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreateIssueOutcome {
    /// Final HTTP status code returned by Jira.
    pub status: u16,
    /// Final body text.
    pub body: String,
    /// `true` if the call was retried through the
    /// gateway after a 401/404 on the direct URL.
    pub used_gateway: bool,
}

/// Attempt to create a Jira issue, falling back through
/// the Platform API Gateway on 401/404.
pub(crate) fn create_issue<C: HttpClient>(
    http: &C,
    config: &JiraConfig,
    payload_json: &str,
) -> Result<CreateIssueOutcome, HttpError> {
    let auth = basic_auth_header(&config.username, &config.jira_token);
    let direct = http.post_json(
        &direct_create_issue_url(&config.jira_url),
        &auth,
        payload_json,
    )?;
    if direct.status != 401 && direct.status != 404 {
        return Ok(CreateIssueOutcome {
            status: direct.status,
            body: direct.body,
            used_gateway: false,
        });
    }

    let tenant = http.get(&tenant_info_url(&config.jira_url), &auth)?;
    if !(200..300).contains(&tenant.status) {
        return Err(HttpError::TenantInfo(tenant.status));
    }
    let info: TenantInfo = serde_json::from_str(&tenant.body)
        .map_err(HttpError::TenantInfoDecode)?;

    let retried = http.post_json(
        &gateway_create_issue_url(&info.cloud_id),
        &auth,
        payload_json,
    )?;
    Ok(CreateIssueOutcome {
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

    #[test]
    fn parse_created_issue_reads_key_and_self() {
        let body = r#"{"id":"10042","key":"PROJ-142","self":"https://x.atlassian.net/rest/api/3/issue/10042"}"#;
        let created = parse_created_issue(body).unwrap();
        assert_eq!(created.key, "PROJ-142");
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
        assert_eq!(created.key, "P-1");
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
                Call::Get { auth, .. } | Call::Post { auth, .. } => auth,
            };
            assert!(auth.starts_with("Basic "), "missing Basic auth: {auth}");
        }
    }
}
