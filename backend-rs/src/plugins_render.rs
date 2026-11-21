//! Camoufox MCP render client (D' plan, 2026-11-20).
//!
//! A self-hosted camoufox MCP server (Streamable HTTP: JSON-RPC request in,
//! SSE `data:` line out) drives a real Firefox past anti-scraping walls that
//! reject plain HTTP clients — Zhihu's zse-ck being the canonical case.
//! Verified against camofox-mcp v2.4.5: stateless (no session id), 30s
//! server-side cap on `create_tab` whose tab keeps loading anyway (recover
//! via `list_tabs`), selector wait timeout in milliseconds.
//!
//! The MCP base URL and token come from the *plugin's own KV namespace*
//! (`camoufox_url` / `camoufox_token`) — the user configures their own
//! self-hosted service, which is the trust decision; no request-supplied
//! host is ever fetched.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

/// Stage progress of one async render job. Percent reflects completed
/// stages (10/30/60/75/90/100) — a linear percentage would be fiction; the
/// stage text carries the actual meaning.
#[derive(Debug, Clone, Serialize)]
pub struct RenderJobState {
    pub percent: u8,
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<RenderedPage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// In-memory async render jobs (per-instance; jobs are short-lived and
/// polled only by their creator).
pub type RenderJobs = std::sync::Arc<
    tokio::sync::Mutex<std::collections::HashMap<String, RenderJobState>>,
>;

/// One rendered page: extracted HTML plus the evaluated page title.
#[derive(Debug, Clone, Serialize)]
pub struct RenderedPage {
    /// Extracted page HTML, scoped by the caller's selector when given.
    pub html: String,
    /// `document.title` of the settled page.
    pub title: String,
    /// Final URL after challenge redirects — the clipper stores this as source.
    pub final_url: String,
}

#[derive(Debug)]
pub struct CamoufoxConfig {
    /// MCP endpoint, e.g. `http://camofox-mcp:8080/mcp`. Normalized to end
    /// with `/mcp` (the SiYuan plugin appends it too, so users paste either
    /// the bare base or the full endpoint).
    pub base_url: String,
    /// Optional bearer token; empty when the service runs without auth.
    pub token: Option<String>,
    /// Selector wait budget in seconds (user-configurable). Drives both the
    /// selector wait and the overall HTTP timeout.
    pub wait_seconds: u64,
}

impl CamoufoxConfig {
    /// Reads the config out of a plugin KV map. `None` when the user has not
    /// configured an endpoint yet.
    pub fn from_kv(map: &std::collections::HashMap<String, serde_json::Value>) -> Option<Self> {
        let enabled = map.get("camoufox_enabled").and_then(|v| v.as_bool());
        if enabled == Some(false) {
            return None;
        }
        let base_url = map
            .get("camoufox_url")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?
            .to_string();
        let base_url = if base_url.ends_with("/mcp") {
            base_url
        } else {
            format!("{}/mcp", base_url.trim_end_matches('/'))
        };
        let token = map
            .get("camoufox_token")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let wait_seconds = map
            .get("camoufox_wait_seconds")
            .and_then(|v| v.as_u64())
            .filter(|s| *s > 0 && *s <= 300)
            .unwrap_or(30);
        Some(Self {
            base_url,
            token,
            wait_seconds,
        })
    }
}

/// Fetch/render failure with a user-facing message (surfaced through the
/// plugin error notice).
#[derive(Debug)]
pub struct RenderError(pub String);

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One JSON-RPC response extracted from the SSE stream.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct McpResponse {
    #[serde(default)]
    error: Option<McpRpcError>,
    #[serde(default)]
    result: Option<McpToolResult>,
}

#[derive(Debug, Deserialize)]
struct McpRpcError {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McpToolResult {
    #[serde(default, rename = "isError")]
    is_error: bool,
    #[serde(default)]
    content: Vec<McpContent>,
}

#[derive(Debug, Deserialize)]
struct McpContent {
    #[serde(default)]
    text: Option<String>,
}

/// Minimal MCP-over-HTTP client for one render session.
struct McpClient {
    http: reqwest::Client,
    config: CamoufoxConfig,
    next_id: u64,
}

impl McpClient {
    fn new(config: CamoufoxConfig) -> Self {
        // The selector wait runs server-side inside camoufox; the HTTP budget
        // must cover it plus the 30s create_tab cap plus transfer time.
        let timeout = Duration::from_secs(config.wait_seconds + 75);
        let http = reqwest::Client::builder()
            .timeout(timeout)
            // MCP streamable HTTP requires both content types or the server
            // answers 406 (verified against camofox-mcp v2.4.5).
            .default_headers({
                use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
                let mut headers = HeaderMap::new();
                headers.insert(
                    ACCEPT,
                    HeaderValue::from_static("application/json, text/event-stream"),
                );
                headers
            })
            .build()
            .expect("static client config");
        Self {
            http,
            config,
            next_id: 0,
        }
    }

    async fn call(
        &mut self,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, RenderError> {
        self.next_id += 1;
        let payload = json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "tools/call",
            "params": {"name": tool, "arguments": args},
        });
        let mut request = self.http.post(&self.config.base_url).json(&payload);
        if let Some(token) = &self.config.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RenderError(format!("camoufox {tool}: {}", e)))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| RenderError(format!("camoufox {tool} read: {e}")))?;
        if status == 401 || status == 403 {
            return Err(RenderError(format!(
                "camoufox {tool} rejected (HTTP {status}): check the API token"
            )));
        }
        if !(200..300).contains(&status) {
            return Err(RenderError(format!("camoufox {tool}: HTTP {status}")));
        }
        let data = parse_sse_data(&text)
            .ok_or_else(|| RenderError(format!("camoufox {tool}: empty SSE stream")))?;
        let parsed: McpResponse = serde_json::from_str(&data)
            .map_err(|e| RenderError(format!("camoufox {tool}: bad JSON-RPC payload ({e})")))?;
        if let Some(err) = parsed.error {
            return Err(RenderError(format!(
                "camoufox {tool} error: {}",
                err.message.unwrap_or_else(|| "unknown".into())
            )));
        }
        let result = parsed
            .result
            .ok_or_else(|| RenderError(format!("camoufox {tool}: no result")))?;
        let text = result
            .content
            .first()
            .and_then(|c| c.text.clone())
            .ok_or_else(|| RenderError(format!("camoufox {tool}: no content")))?;
        if result.is_error {
            return Err(RenderError(format!(
                "camoufox {tool} failed: {}",
                first_line(&text)
            )));
        }
        serde_json::from_str(&text)
            .map_err(|e| RenderError(format!("camoufox {tool}: bad result JSON ({e})")))
    }

    async fn initialize(&self) -> Result<(), RenderError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "pergame-plugins", "version": "1"},
            },
        });
        let mut request = self.http.post(&self.config.base_url).json(&payload);
        if let Some(token) = &self.config.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RenderError(format!("camoufox initialize: {e}")))?;
        if !response.status().is_success() {
            return Err(RenderError(format!(
                "camoufox initialize: HTTP {}",
                response.status().as_u16()
            )));
        }
        Ok(())
    }
}

/// First line of a tool error payload — the camoufox server packs detail into
/// one JSON line; trailing context is noise for the user.
fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or(text)
        .chars()
        .take(160)
        .collect()
}

/// Pulls the single JSON-RPC payload out of an SSE body (`data:` lines).
fn parse_sse_data(body: &str) -> Option<String> {
    let joined = body
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("data:")
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .collect::<Vec<_>>()
        .join("\n");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// Renders `url` in a throwaway camoufox tab.
///
/// Zhihu's wall is probabilistic: the per-tab randomized fingerprint
/// sometimes passes, sometimes bounces to `/signin` (measured 2026-11-20:
/// 0/4 on a flagged streak after repeated probing, while user clips passed
/// hours earlier). So the render retries with FRESH tabs — each new tab is a
/// new fingerprint roll — and treats a signin landing as retryable.
///
/// `wait_selector` gates the extraction (challenge pages reload once their
/// cookie is computed — the selector only exists on the real page);
/// `html_selector` scopes the returned HTML; empty returns the whole document.
pub async fn render_page(
    config: CamoufoxConfig,
    url: &str,
    wait_selector: Option<&str>,
    html_selector: Option<&str>,
    cookie: Option<&str>,
    progress: &(dyn Fn(u8, &str) + Send + Sync),
) -> Result<RenderedPage, RenderError> {
    progress(10, "connecting");
    let mut client = McpClient::new(config);
    client.initialize().await?;
    // Headed/virtual display is load-bearing against the same wall as the
    // builtin engine: the container has no GUI, so request its virtual
    // display (Xvfb) instead of headless. The override persists server-side
    // ("saved as virtual display mode"); a TIMEOUT here still applied it,
    // so the error is tolerated and the tab flow proceeds.
    let _ = client
        .call(
            "toggle_display",
            json!({"headless": "virtual", "userId": "default"}),
        )
        .await;
    // A logged-in session makes the pass deterministic: import the user's
    // Zhihu cookie into the browser jar once per render (cheap), and the
    // auto-saved profile keeps it for later tabs.
    if let Some(raw) = cookie.filter(|c| !c.trim().is_empty()) {
        let cookies: Vec<serde_json::Value> = raw
            .split(';')
            .filter_map(|pair| {
                let (name, value) = pair.split_once('=')?;
                let name = name.trim();
                if name.is_empty() || value.trim().is_empty() {
                    return None;
                }
                Some(json!({
                    "name": name,
                    "value": value.trim(),
                    "domain": ".zhihu.com",
                    "path": "/",
                }))
            })
            .collect();
        if !cookies.is_empty() {
            let _ = client
                .call("import_cookies", json!({ "cookies": cookies }))
                .await;
        }
    }
    let mut last_error: Option<RenderError> = None;
    for _attempt in 0..3 {
        match render_once(&mut client, url, wait_selector, html_selector).await {
            Ok(page) if !page.final_url.contains("/signin") => {
                progress(60, "extracted");
                return Ok(page);
            }
            Ok(signin_page) => {
                last_error = Some(RenderError(format!(
                    "camoufox landed on the login wall ({})",
                    signin_page.final_url
                )));
            }
            Err(err) => last_error = Some(err),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(last_error.unwrap_or_else(|| RenderError("camoufox render failed".into())))
}

/// One render attempt in a fresh tab. create_tab carries the navigation; on
/// challenge pages the server-side 30s cap can answer TIMEOUT while the tab
/// keeps loading — recover the tabId from list_tabs (verified v2.4.5).
async fn render_once(
    client: &mut McpClient,
    url: &str,
    wait_selector: Option<&str>,
    html_selector: Option<&str>,
) -> Result<RenderedPage, RenderError> {
    // create_tab on the target URL can outlive the server's fixed 30s call
    // cap (challenge reload loops on slow passes) and the server then drops
    // the tab. Opening blank first is instant and immune; the navigate below
    // may also report TIMEOUT while the page keeps loading — the tab survives
    // that (verified against v2.4.5), so errors here are tolerated.
    let tab = client.call("create_tab", json!({})).await?;
    let tab_id = tab
        .get("tabId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RenderError("camoufox create_tab returned no tabId".into()))?
        .to_string();
    let _ = client
        .call("navigate", json!({ "tabId": tab_id, "url": url }))
        .await;

    // Wait for real content, then extract scoped HTML and the page title.
    if let Some(selector) = wait_selector.filter(|s| !s.is_empty()) {
        client
            .call(
                "camofox_wait_for_selector",
                json!({"tabId": tab_id, "selector": selector, "timeout": client.config.wait_seconds * 1000}),
            )
            .await?;
    }
    let html_args = if html_selector.filter(|s| !s.is_empty()).is_some() {
        json!({"tabId": tab_id, "selector": html_selector})
    } else {
        json!({"tabId": tab_id})
    };
    let page = client.call("camofox_get_page_html", html_args).await?;
    let html = page
        .get("html")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| RenderError("camoufox returned no html".into()))?;
    let title_value = client
        .call(
            "camofox_evaluate_js",
            json!({"tabId": tab_id, "expression": "document.title"}),
        )
        .await?;
    let final_url_value = client
        .call(
            "camofox_evaluate_js",
            json!({"tabId": tab_id, "expression": "location.href"}),
        )
        .await?;
    let title = title_value
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let final_url = final_url_value
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or(url)
        .to_string();

    // Best effort: the tab is server-side and expires on its own.
    let _ = client.call("close_tab", json!({"tabId": tab_id})).await;

    Ok(RenderedPage {
        html,
        title,
        final_url,
    })
}

// ---------------------------------------------------------------- builtin
//
// Dev engine: drive a locally installed camoufox-cli as subprocesses
// (user decision 2026-11-21: dev = local cli, production = camofox-mcp
// container). The cli speaks one command per invocation — open/wait/eval/
// cookies import — and keeps a daemon alive across commands, so the
// `--session pergame --persistent` identity (frozen fingerprint + cookie
// jar) accumulates exactly the reputation Zhihu's content-tiered wall
// rewards. No HTTP endpoint: the backend owns the process tree.

/// CLI invocation result.
struct CliRun {
    stdout: String,
    success: bool,
}

/// One command against the persistent pergame session.
async fn cli_run(cli_path: &str, args: &[&str], timeout: Duration) -> Result<CliRun, RenderError> {
    // A wedged daemon would hang the child forever; the budget caps it.
    let output = tokio::time::timeout(
        timeout,
        tokio::process::Command::new(cli_path)
            // Headed is load-bearing: Zhihu's wall fingerprints headless /
            // automation traits and soft-blocks UGC pages for them (2026-11-21
            // matrix: headed passes where headless is bounced to
            // signin/unhuman). macOS has no Xvfb equivalent, so the window
            // itself is unavoidable — but the daemon is long-lived and its
            // browser persists across renders, so a 24h idle timeout bounds
            // the pop-up to at most one per day (first render of the day).
            .args(["--session", "pergame", "--persistent", "--headed", "--timeout", "86400"])
            .args(args)
            .env("HOME", std::env::var("HOME").unwrap_or_default())
            .output(),
    )
    .await
    .map_err(|_| {
        RenderError(format!(
            "camoufox-cli {} timed out",
            args.first().unwrap_or(&"")
        ))
    })?
    .map_err(|e| RenderError(format!("camoufox-cli spawn failed: {e}")))?;
    // The cli writes results to stdout and errors to stderr; both matter —
    // a stale daemon answers "Browser not launched" on stderr with an empty
    // stdout, and the caller must see that instead of a bare EOF.
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let success = output.status.success();
    if !success && stdout.is_empty() && !stderr.is_empty() {
        return Err(RenderError(format!(
            "camoufox-cli: {}",
            first_line(&stderr)
        )));
    }
    Ok(CliRun { success, stdout })
}

/// Extracted page state parsed from the eval probe.
#[derive(Debug, serde::Deserialize)]
struct CliPageState {
    title: String,
    url: String,
    html: String,
    gated: bool,
}

/// Builtin render: local camoufox-cli subprocess flow.
///
/// Escalation inside one call: cookie import (deterministic pass) →
/// navigate → settle → eval-probe → gated pages get one explore warm-up
/// round-trip before failing (the gate is content-tiered; list pages
/// always pass and the round trip sometimes unlocks the article).
pub async fn render_page_builtin(
    cli_path: &str,
    url: &str,
    wait_selector: Option<&str>,
    html_selector: Option<&str>,
    cookie: Option<&str>,
    wait_seconds: u64,
    progress: &(dyn Fn(u8, &str) + Send + Sync),
) -> Result<RenderedPage, RenderError> {
    progress(10, "connecting");
    let settle_ms = format!("{}000", wait_seconds.max(10));
    let overall = Duration::from_secs(wait_seconds * 2 + 60);

    // Cookie bridge: the cli imports through a temp file (native command).
    if let Some(raw) = cookie.filter(|c| !c.trim().is_empty()) {
        let cookies: Vec<serde_json::Value> = raw
            .split(';')
            .filter_map(|pair| {
                let (name, value) = pair.split_once('=')?;
                let name = name.trim();
                if name.is_empty() || value.trim().is_empty() {
                    return None;
                }
                Some(json!({ "name": name, "value": value.trim(), "domain": ".zhihu.com", "path": "/" }))
            })
            .collect();
        if !cookies.is_empty() {
            let path =
                std::env::temp_dir().join(format!("pergame-cookies-{}.json", std::process::id()));
            if std::fs::write(&path, serde_json::to_string(&cookies).unwrap_or_default()).is_ok() {
                let path_str = path.to_string_lossy().to_string();
                let _ = cli_run(cli_path, &["cookies", "import", &path_str], overall).await;
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    // The cli overloads --timeout (daemon idle + navigation cap; the last
    // one parsed wins, measured). Use one large value for both: a hung
    // navigation is bounded by the outer tokio budget, not this flag.
    let open_args = ["open", url, "--timeout", "86400"];
    let open = cli_run(cli_path, &open_args, overall).await?;
    if !open.success && open.stdout.is_empty() {
        return Err(RenderError(
            "camoufox-cli open failed (is it installed?)".into(),
        ));
    }
    let _ = cli_run(cli_path, &["wait", &settle_ms], overall).await;

    let selector_js = html_selector.unwrap_or("");
    let probe = format!(
        "JSON.stringify({{title: document.title, url: location.href, \
         html: ({sel} ? (document.querySelector({sel})?.innerHTML ?? document.documentElement.outerHTML) \
         : document.documentElement.outerHTML), \
         gated: /signin|unhuman/.test(location.href) || !!document.querySelector('#zh-zse-ck')}})",
        sel = serde_json::to_string(selector_js).unwrap_or_default()
    );
    let probe_args = ["eval", &probe];
    let mut state = cli_run(cli_path, &probe_args, overall).await?;
    let mut parsed: CliPageState = parse_cli_state(&state.stdout)?;

    if parsed.gated {
        // Warm-up round trip: list pages always pass; arriving at the article
        // from inside the site occasionally unlocks it.
        let _ = cli_run(
            cli_path,
            &["open", "https://www.zhihu.com/explore", "--timeout", "45"],
            overall,
        )
        .await;
        let _ = cli_run(cli_path, &["wait", &settle_ms], overall).await;
        let _ = cli_run(cli_path, &open_args, overall).await;
        let _ = cli_run(cli_path, &["wait", &settle_ms], overall).await;
        state = cli_run(cli_path, &probe_args, overall).await?;
        parsed = parse_cli_state(&state.stdout)?;
        if parsed.gated {
            return Err(RenderError(
                "page is behind Zhihu's login gate; set zhihu_cookie in the plugin settings for a deterministic pass".into(),
            ));
        }
    }

    // wait_selector is honored implicitly by the settle wait above; a caller
    // selector only scopes the extraction probe.

    progress(60, "extracted");
    Ok(RenderedPage {
        html: parsed.html,
        title: parsed.title,
        final_url: parsed.url,
    })
}

/// The eval probe prints a bare JSON object (verified against cli 0.8.x);
/// accept either that or the --json envelope.
fn parse_cli_state(stdout: &str) -> Result<CliPageState, RenderError> {
    let trimmed = stdout.trim();
    let payload = if trimmed.starts_with('{') {
        trimmed.to_string()
    } else {
        // --json envelope: {id, success, data?} — find the first {...} that
        // parses with our keys by scanning from the first brace.
        match trimmed.find("{\"") {
            Some(i) => trimmed[i..].to_string(),
            None => trimmed.to_string(),
        }
    };
    serde_json::from_str(&payload).map_err(|e| {
        RenderError(format!(
            "camoufox-cli probe returned unexpected output ({e}): bytes={} head={:?}",
            trimmed.len(),
            trimmed.chars().take(80).collect::<Vec<_>>()
        ))
    })
}
