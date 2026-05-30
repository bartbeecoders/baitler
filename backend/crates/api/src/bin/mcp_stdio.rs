//! `baitler-mcp` — a stdio↔HTTP MCP bridge.
//!
//! Baitler serves MCP natively over Streamable HTTP at `POST /mcp`. Some MCP
//! clients only launch **stdio** servers (a command they spawn and talk to over
//! stdin/stdout). This tiny bridge fills that gap: it reads newline-delimited
//! JSON-RPC messages from stdin, forwards each to a running Baitler server's
//! `/mcp` endpoint, and writes the JSON-RPC responses back to stdout.
//!
//! Configuration (all optional):
//! - positional arg 1, or `BAITLER_MCP_URL`, or `BAITLER_API_URL` — the server
//!   base URL or full `/mcp` URL (default `http://127.0.0.1:8080`).
//! - `BAITLER_MCP_TOKEN` — bearer token, if the server sets `MCP_AUTH_TOKEN`.
//!
//! Example (Claude Code): `claude mcp add baitler -- baitler-mcp`.

use std::env;

use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let endpoint = resolve_endpoint();
    let token = env::var("BAITLER_MCP_TOKEN")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());

    eprintln!("baitler-mcp: forwarding stdio MCP to {endpoint}");

    let client = reqwest::Client::new();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut request = client
            .post(&endpoint)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream")
            .body(line.to_string());
        if let Some(token) = &token {
            request = request.bearer_auth(token);
        }

        let reply: Option<String> = match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    // 202 (notification ack) and empty bodies produce no reply.
                    let trimmed = body.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                } else {
                    // HTTP-level failure (e.g. 401): synthesize a JSON-RPC error
                    // so the client doesn't hang waiting on its request id.
                    error_reply(
                        line,
                        &format!("MCP HTTP transport returned {status}: {}", body.trim()),
                    )
                }
            }
            Err(e) => {
                eprintln!("baitler-mcp: request to {endpoint} failed: {e}");
                error_reply(line, &format!("could not reach Baitler MCP endpoint: {e}"))
            }
        };

        if let Some(reply) = reply {
            stdout.write_all(reply.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}

/// Resolve the `/mcp` endpoint URL from arg/env, defaulting to localhost.
fn resolve_endpoint() -> String {
    let base = env::args()
        .nth(1)
        .or_else(|| env::var("BAITLER_MCP_URL").ok())
        .or_else(|| env::var("BAITLER_API_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/mcp") {
        base.to_string()
    } else {
        format!("{base}/mcp")
    }
}

/// Build a JSON-RPC error response carrying the request's id, so a failed
/// forward surfaces as an error rather than a hang. Returns `None` for inputs
/// without an id (notifications) or that don't parse.
fn error_reply(input: &str, message: &str) -> Option<String> {
    let value: Value = serde_json::from_str(input).ok()?;
    let id = value.get("id")?.clone();
    if id.is_null() {
        return None;
    }
    Some(
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32001, "message": message },
        })
        .to_string(),
    )
}
