# PLG-JIRA: Jira plugin crate (cdylib) with REST client

## Why

The Jira plugin is the first concrete integration,
proving the plugin architecture works end-to-end. It
pushes local tasks as Jira issues using the same auth
patterns as the existing Python code in marketplace-v2.

## What

### Crate setup

- `crates/rustwerk-jira-plugin/` with
  `crate-type = ["cdylib"]`
- Depends on `rustwerk-plugin-api`, `serde`,
  `serde_json`, `ureq`, `base64`
- Lint override: `unsafe_code = "allow"`

### FFI exports

Implement the 4 required `extern "C"` functions:
- `rustwerk_plugin_api_version` -> returns
  `API_VERSION`
- `rustwerk_plugin_info` -> returns PluginInfo with
  name "jira", capabilities ["push_tasks"]
- `rustwerk_plugin_push_tasks` -> main entry point
- `rustwerk_plugin_free_string` -> frees CStrings
  allocated by this plugin

### Jira REST client (`jira_client.rs`)

Uses `ureq` (synchronous HTTP, no async runtime).

**Auth**: HTTP Basic Auth
- Username: `config.username` (git user.email)
- Password: `config.jira_token` (scoped API token)
- Header: `Authorization: Basic base64(user:token)`

**Gateway fallback** (matching Python implementation):
1. Try direct URL: `{jira_url}/rest/api/3/issue`
2. If 401/404, get cloud ID from
   `{jira_url}/_edge/tenant_info`
3. Retry via gateway:
   `https://api.atlassian.com/ex/jira/{cloudId}/rest/api/3/issue`

**Issue creation**: `POST /rest/api/3/issue` with
JSON payload.

### Config expectations

The plugin reads from the config JSON passed by the
host:
- `jira_url` (required): Jira instance base URL
- `jira_token` (required): scoped API token
- `username` (required): email for Basic Auth
- `project_key` (required): Jira project key

Returns clear error if any required field is missing.

## How

- `src/lib.rs`: FFI exports + plugin entry point
- `src/jira_client.rs`: HTTP client with auth +
  gateway fallback
- `src/mapping.rs`: see PLG-MAP

## Acceptance criteria

- [ ] Plugin compiles to .dll/.so/.dylib
- [ ] `rustwerk plugin list` discovers it
- [ ] Auth works with scoped Jira Cloud tokens
- [ ] Gateway fallback activates on 401/404
- [ ] Clear error messages for missing config
- [ ] Unit tests for auth header construction
- [ ] Unit tests for gateway fallback logic (mocked)
- [ ] `cargo xtask clippy` passes
