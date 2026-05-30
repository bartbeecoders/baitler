//! JSON-RPC 2.0 envelope helpers for the MCP server.
//!
//! MCP messages are JSON-RPC 2.0. We keep params/results as [`serde_json::Value`]
//! (their shapes are method-specific and version-evolving) and only model the
//! envelope and the standard error codes here.

use serde_json::{json, Value};

/// JSON-RPC protocol version string.
pub const JSONRPC_VERSION: &str = "2.0";

/// Latest MCP protocol revision this server implements. We echo the client's
/// requested version when present (our core is version-agnostic), falling back
/// to this when it isn't.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

// Standard JSON-RPC error codes (https://www.jsonrpc.org/specification#error_object).
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// Build a successful JSON-RPC response.
pub fn success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result,
    })
}

/// Build a JSON-RPC error response.
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": { "code": code, "message": message },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_envelope_shape() {
        let r = success(json!(1), json!({"ok": true}));
        assert_eq!(r["jsonrpc"], "2.0");
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["ok"], true);
        assert!(r.get("error").is_none());
    }

    #[test]
    fn error_envelope_shape() {
        let r = error(Value::Null, METHOD_NOT_FOUND, "nope");
        assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(r["error"]["message"], "nope");
        assert!(r.get("result").is_none());
    }
}
