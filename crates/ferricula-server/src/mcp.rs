//! MCP Streamable HTTP endpoint for the agent runtime (`POST /mcp`).
//!
//! Protocol: MCP **2025-06-18** JSON-RPC 2.0, JSON response mode (no SSE).
//! Advertise only this revision. Older initialize proposals receive
//! **2025-06-18**; the client decides compatibility. Subsequent
//! `MCP-Protocol-Version` headers other than **2025-06-18** are rejected.
//! Tools: `ferricula_status`, `ferricula_recall` (read-only hybrid recall
//! with verbatim document sections), `ferricula_chat` (operator chat),
//! `ferricula_ingest` (hand the agent a document), `ferricula_documents`
//! (list read documents), `ferricula_read_section` (one verbatim section),
//! `ferricula_life` (drives, life journal, last dream; read-only). The v2 names `steve_status`/`steve_recall`/`steve_chat`
//! are still accepted by `tools/call` as deprecated aliases for one release,
//! but are not advertised in `tools/list`.
//! Operator bearer auth required on `POST /mcp`. Damascus is not a launch blocker.
//!
//! 2025-03-26 Streamable HTTP allowed JSON-RPC **batches** on POST.
//! 2025-06-18: POST body **MUST** be a **single** JSON-RPC message.
//! This server follows 2025-06-18 and rejects arrays (do not negotiate
//! 2025-03-26 while rejecting its permitted batch transport).

use std::sync::Arc;

use axum::extract::State;
use uuid::Uuid;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::runtime::AgentRuntime;

/// Advertised MCP protocol revision (only).
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// JSON-RPC / MCP endpoint path.
pub const MCP_PATH: &str = "/mcp";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const MAX_RECALL_LIMIT: usize = 64;
const MAX_QUERY_BYTES: usize = 8192;
const MAX_CHAT_MESSAGE_BYTES: usize = 8192;
const PROTOCOL_HEADER: &str = "mcp-protocol-version";

/// Unstated router for Goldfish to merge onto `Arc<AgentRuntime>`.
pub fn router() -> Router<Arc<AgentRuntime>> {
    Router::new().route(MCP_PATH, post(mcp_post).get(mcp_get).delete(mcp_delete))
}

#[derive(Debug)]
enum McpHttp {
    Unauthorized,
    ForbiddenOrigin,
    Accepted,
    MethodNotAllowed,
    Json(StatusCode, Value),
}

impl IntoResponse for McpHttp {
    fn into_response(self) -> Response {
        match self {
            McpHttp::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "operator authorization required" })),
            )
                .into_response(),
            McpHttp::ForbiddenOrigin => (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "origin not allowed" })),
            )
                .into_response(),
            McpHttp::Accepted => StatusCode::ACCEPTED.into_response(),
            McpHttp::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                [(header::ALLOW, "POST")],
                Json(json!({
                    "error": "SSE GET/DELETE sessions are not offered; POST JSON-RPC to /mcp"
                })),
            )
                .into_response(),
            McpHttp::Json(status, value) => (
                status,
                [(header::CONTENT_TYPE, "application/json")],
                value.to_string(),
            )
                .into_response(),
        }
    }
}

async fn mcp_get() -> McpHttp {
    McpHttp::MethodNotAllowed
}

async fn mcp_delete() -> McpHttp {
    McpHttp::MethodNotAllowed
}

async fn mcp_post(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    body: String,
) -> McpHttp {
    handle_post(runtime, &headers, &body).await
}

async fn handle_post(runtime: Arc<AgentRuntime>, headers: &HeaderMap, body: &str) -> McpHttp {
    match classify_origin(headers) {
        OriginCheck::Absent | OriginCheck::Allowed => {}
        OriginCheck::Denied | OriginCheck::Invalid => return McpHttp::ForbiddenOrigin,
    }
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let Some(identity) = runtime.authenticate(authorization) else {
        return McpHttp::Unauthorized;
    };
    if body.trim().is_empty() {
        return McpHttp::Json(
            StatusCode::BAD_REQUEST,
            rpc_error(Value::Null, PARSE_ERROR, "Parse error"),
        );
    }
    let parsed: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return McpHttp::Json(
                StatusCode::BAD_REQUEST,
                rpc_error(Value::Null, PARSE_ERROR, "Parse error"),
            );
        }
    };
    if parsed.is_array() {
        return McpHttp::Json(
            StatusCode::BAD_REQUEST,
            rpc_error(
                Value::Null,
                INVALID_REQUEST,
                "JSON-RPC batches are not accepted (MCP 2025-06-18 POST must be a single message)",
            ),
        );
    }
    if let Err(response) = check_protocol_header(headers) {
        return response;
    }
    match handle_rpc(runtime, &parsed, &identity).await {
        RpcOut::Accepted => McpHttp::Accepted,
        RpcOut::Body(value) => McpHttp::Json(StatusCode::OK, value),
        RpcOut::BadRequest(value) => McpHttp::Json(StatusCode::BAD_REQUEST, value),
    }
}

fn check_protocol_header(headers: &HeaderMap) -> Result<(), McpHttp> {
    match headers.get(PROTOCOL_HEADER) {
        None => Ok(()),
        Some(value) => {
            let Ok(raw) = value.to_str() else {
                return Err(McpHttp::Json(
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "invalid MCP-Protocol-Version header" }),
                ));
            };
            if raw == PROTOCOL_VERSION {
                Ok(())
            } else {
                Err(McpHttp::Json(
                    StatusCode::BAD_REQUEST,
                    json!({
                        "error": "unsupported MCP-Protocol-Version",
                        "supported": [PROTOCOL_VERSION],
                        "requested": raw
                    }),
                ))
            }
        }
    }
}

#[derive(Debug)]
enum RpcOut {
    Accepted,
    Body(Value),
    BadRequest(Value),
}

async fn handle_rpc(runtime: Arc<AgentRuntime>, msg: &Value, identity: &crate::auth::AuthIdentity) -> RpcOut {
    if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        return RpcOut::BadRequest(rpc_error(id, INVALID_REQUEST, "jsonrpc must be \"2.0\""));
    }
    let method = match msg.get("method").and_then(Value::as_str) {
        Some(method) => method,
        None => {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            return RpcOut::BadRequest(rpc_error(id, INVALID_REQUEST, "missing method"));
        }
    };
    let id = msg.get("id").cloned();
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    if method.starts_with("notifications/") || (id.is_none() && method == "initialized") {
        return RpcOut::Accepted;
    }
    let Some(id) = id else {
        return RpcOut::Accepted;
    };

    let result = match method {
        "initialize" => initialize_result(&runtime, &params),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_list(identity)),
        "tools/call" => tools_call(&runtime, &params, identity).await,
        _ => Err(rpc_error(
            id.clone(),
            METHOD_NOT_FOUND,
            &format!("Method not found: {method}"),
        )),
    };
    match result {
        Ok(value) => RpcOut::Body(rpc_ok(id, value)),
        Err(mut error) => {
            if let Some(obj) = error.as_object_mut() {
                obj.insert("id".into(), id);
            }
            RpcOut::Body(error)
        }
    }
}

fn initialize_result(runtime: &AgentRuntime, _params: &Value) -> Result<Value, Value> {
    Ok(json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "ferricula",
            "title": server_title(runtime),
            "version": env!("CARGO_PKG_VERSION"),
            "agent_id": runtime.inspection.agent_id
        },
        "instructions": format!(
            "Ferricula MCP for {}. All tools require operator auth. ferricula_ingest hands the agent a document (text, url, or base64 PDF) to read; it is stored verbatim and the agent remembers reading it. ferricula_documents lists what it has read; ferricula_read_section returns one section's exact text. ferricula_life shows the agent's life between conversations (drives, curiosity excursions, sleep, dreams). ferricula_recall is read-only hybrid recall: recovered memory and experience metadata plus verbatim document sections, fused by rank, each section with a [doc id§index p.page] citation. ferricula_chat sends one operator message through POST /chat and returns reply plus conversation_id; replies cite document sections they rely on. Damascus work-broker is not mounted.",
            server_title(runtime)
        )
    }))
}

/// MCP server title: the loaded persona's name, or "Ferricula" when the
/// persona is empty.
fn server_title(runtime: &AgentRuntime) -> String {
    let name = runtime.persona().name.trim();
    if name.is_empty() { "Ferricula".to_string() } else { name.to_string() }
}

/// Map deprecated v2 tool names onto their v3 names.
fn canonical_tool_name(name: &str) -> &str {
    match name {
        "steve_status" => "ferricula_status",
        "steve_recall" => "ferricula_recall",
        "steve_chat" => "ferricula_chat",
        other => other,
    }
}

fn tools_list(identity: &crate::auth::AuthIdentity) -> Value {
    let mut list = json!({
        "tools": [
            {
                "name": "ferricula_status",
                "description": "Read-only agent runtime status (identity, mode, memory counts). No memory text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            },
            {
                "name": "ferricula_recall",
                "description": "Read-only hybrid recall. Fuses (RRF) recovered-memory and experience hits (ids, scores, tags, refs) with document sections, which carry verbatim text and a [doc id§index p.page] citation. `hits` alone keeps the legacy recovered-memory list.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Non-empty lexical query"
                        },
                        "limit": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": MAX_RECALL_LIMIT,
                            "default": 10
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            },
            {
                "name": "ferricula_chat",
                "description": "Send one operator message to the agent (POST /chat). Returns the model reply and conversation_id. Does not hydrate source text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Operator message, 1 to 8192 UTF-8 bytes"
                        },
                        "conversation_id": {
                            "type": "string",
                            "description": "Optional UUID of an existing conversation. Omitted starts a new one."
                        }
                    },
                    "required": ["message"],
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": false,
                    "openWorldHint": true
                }
            },
            {
                "name": "ferricula_ingest",
                "description": "Hand the agent a document to read: inline text/markdown, a URL (web page via grub, or a PDF link), or a base64 PDF. Stored verbatim as addressable sections and remembered as an \"I read X\" experience. Identical re-ingests dedupe. Returns doc_id, title, sections, memory_id.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Inline text or markdown" },
                        "title": { "type": "string", "description": "Optional title for text" },
                        "url": { "type": "string", "description": "http(s) URL of a web page or PDF" },
                        "pdf_base64": { "type": "string", "description": "Base64-encoded PDF bytes" },
                        "name": { "type": "string", "description": "File name for pdf_base64" },
                        "note": { "type": "string", "description": "Optional: why you are giving the agent this document" }
                    },
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": false,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": true
                }
            },
            {
                "name": "ferricula_documents",
                "description": "List documents the agent has read (doc_id, title, origin, pages, sections, ingested_at).",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            },
            {
                "name": "ferricula_life",
                "description": "Read-only status of the agent's life between conversations: phase (resting/engaged/asleep/meditating), boredom, sleep_pressure, curiosity_today, life model calls today, the last journal entries (curiosity excursions with query, pages read and reflection; sleep; consolidation; dreams; wakes) and the last dream with its question. Dreams are labeled as dreams, never evidence.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "journal": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 200,
                            "default": 20,
                            "description": "Number of most recent journal entries to include"
                        }
                    },
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            },
            {
                "name": "ferricula_read_section",
                "description": "Return one document section's exact text with its citation handle [doc id§index p.page].",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "doc_id": { "type": "string" },
                        "index": { "type": "integer", "minimum": 0 }
                    },
                    "required": ["doc_id", "index"],
                    "additionalProperties": false
                },
                "annotations": {
                    "readOnlyHint": true,
                    "destructiveHint": false,
                    "idempotentHint": true,
                    "openWorldHint": false
                }
            }
        ]
    });
    if !identity.is_operator() {
        const READ_TOOLS: [&str; 4] = [
            "ferricula_status",
            "ferricula_recall",
            "ferricula_documents",
            "ferricula_read_section",
        ];
        if let Some(tools) = list.get_mut("tools").and_then(Value::as_array_mut) {
            tools.retain(|t| {
                t.get("name")
                    .and_then(Value::as_str)
                    .map(|n| READ_TOOLS.contains(&n))
                    .unwrap_or(false)
            });
        }
    }
    list
}

async fn ferricula_ingest(runtime: &Arc<AgentRuntime>, arguments: &Value) -> Result<Value, Value> {
    use crate::api::IngestSourceBody;
    let Some(obj) = arguments.as_object() else {
        return Ok(tool_text(&json!({"error": "arguments must be an object"}), true));
    };
    const KNOWN: [&str; 6] = ["text", "title", "url", "pdf_base64", "name", "note"];
    if let Some(key) = obj.keys().find(|k| !KNOWN.contains(&k.as_str())) {
        return Ok(tool_text(&json!({"error": format!("unknown argument: {key}")}), true));
    }
    let text_arg = |key: &str| obj.get(key).and_then(Value::as_str).map(str::to_string);
    let (text, url, pdf) = (text_arg("text"), text_arg("url"), text_arg("pdf_base64"));
    let body = match (text, url, pdf) {
        (Some(text), None, None) => IngestSourceBody::Text { title: text_arg("title"), text },
        (None, Some(url), None) => IngestSourceBody::Url { url },
        (None, None, Some(base64)) => IngestSourceBody::Pdf {
            name: text_arg("name").unwrap_or_else(|| "document.pdf".into()),
            base64,
        },
        _ => {
            return Ok(tool_text(
                &json!({"error": "provide exactly one of text, url, or pdf_base64"}),
                true,
            ));
        }
    };
    let note = text_arg("note");
    if note.as_ref().is_some_and(|n| n.len() > crate::runtime::MAX_NOTE_BYTES) {
        return Ok(tool_text(&json!({"error": "note exceeds 2048 bytes"}), true));
    }
    let source = match crate::api::decode_source(body, &runtime.config.documents) {
        Ok(source) => source,
        Err((_, error)) => return Ok(tool_text(&json!({ "error": error }), true)),
    };
    match runtime.ingest(source, note).await {
        Ok(outcome) => {
            runtime.life_sensed_document(outcome.duplicate);
            Ok(tool_text(&json!(outcome), false))
        }
        Err(error) => Ok(tool_text(&json!({ "error": format!("{error:#}") }), true)),
    }
}

async fn tools_call(
    runtime: &Arc<AgentRuntime>,
    params: &Value,
    identity: &crate::auth::AuthIdentity,
) -> Result<Value, Value> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let canonical = canonical_tool_name(name);
    if !identity.is_operator() {
        const WRITE_TOOLS: [&str; 3] = ["ferricula_chat", "ferricula_ingest", "ferricula_life"];
        if WRITE_TOOLS.contains(&canonical) {
            return Ok(tool_text(&json!({ "error": "read-only role" }), true));
        }
    }
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    match canonical {
        "ferricula_status" => {
            let status = runtime.status();
            let payload = serde_json::to_value(&status)
                .map_err(|err| rpc_error(Value::Null, INTERNAL_ERROR, &err.to_string()))?;
            Ok(tool_text(&payload, false))
        }
        "ferricula_recall" => {
            let query = arguments
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if query.is_empty() {
                return Ok(tool_text(
                    &json!({ "error": "query cannot be empty" }),
                    true,
                ));
            }
            if query.len() > MAX_QUERY_BYTES {
                return Ok(tool_text(
                    &json!({ "error": "query exceeds 8192 bytes" }),
                    true,
                ));
            }
            let limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(10) as usize;
            let limit = limit.clamp(1, MAX_RECALL_LIMIT);
            let recall = runtime.hybrid_recall_async(query, limit).await;
            let payload = serde_json::to_value(&recall)
                .map_err(|err| rpc_error(Value::Null, INTERNAL_ERROR, &err.to_string()))?;
            Ok(tool_text(&payload, false))
        }
        "ferricula_chat" => ferricula_chat(runtime, &arguments).await,
        "ferricula_ingest" => ferricula_ingest(runtime, &arguments).await,
        "ferricula_documents" => {
            let documents = runtime.documents();
            Ok(tool_text(&json!({ "documents": documents }), false))
        }
        "ferricula_life" => {
            let journal = arguments.get("journal").and_then(Value::as_u64).unwrap_or(20).min(200) as usize;
            Ok(tool_text(&runtime.life_status(journal), false))
        }
        "ferricula_read_section" => {
            let Some(doc_id) = arguments.get("doc_id").and_then(Value::as_str) else {
                return Ok(tool_text(&json!({ "error": "doc_id is required" }), true));
            };
            let Some(index) = arguments.get("index").and_then(Value::as_u64)
                .and_then(|i| u32::try_from(i).ok()) else {
                return Ok(tool_text(&json!({ "error": "index must be a non-negative integer" }), true));
            };
            match runtime.document_section(doc_id.trim(), index) {
                Some(section) => Ok(tool_text(&json!(section), false)),
                None => Ok(tool_text(&json!({ "error": "section not found" }), true)),
            }
        }
        "" => Err(rpc_error(
            Value::Null,
            INVALID_PARAMS,
            "tools/call missing name",
        )),
        other => Err(rpc_error(
            Value::Null,
            INVALID_PARAMS,
            &format!("Unknown tool: {other}"),
        )),
    }
}

async fn ferricula_chat(runtime: &Arc<AgentRuntime>, arguments: &Value) -> Result<Value, Value> {
    let Some(obj) = arguments.as_object() else {
        return Ok(tool_text(&json!({"error": "arguments must be an object"}), true));
    };
    for key in obj.keys() {
        if key != "message" && key != "conversation_id" {
            return Ok(tool_text(
                &json!({"error": format!("unknown argument: {key}")}),
                true,
            ));
        }
    }
    let Some(message) = obj.get("message").and_then(Value::as_str) else {
        return Ok(tool_text(&json!({"error": "message is required"}), true));
    };
    if message.trim().is_empty() || message.len() > MAX_CHAT_MESSAGE_BYTES {
        return Ok(tool_text(
            &json!({"error": "message must contain 1 to 8192 UTF-8 bytes"}),
            true,
        ));
    }
    let conversation_id = match obj.get("conversation_id") {
        None | Some(Value::Null) => Uuid::new_v4(),
        Some(Value::String(raw)) => match Uuid::parse_str(raw.trim()) {
            Ok(id) => id,
            Err(_) => {
                return Ok(tool_text(
                    &json!({"error": "conversation_id must be a uuid"}),
                    true,
                ));
            }
        },
        Some(_) => {
            return Ok(tool_text(
                &json!({"error": "conversation_id must be a string"}),
                true,
            ));
        }
    };
    let request = crate::runtime::ChatRequest {
        request_id: Uuid::new_v4(),
        conversation_id,
        message: message.to_string(),
        reported_origin: crate::runtime::InputOrigin::Agent,
    };
    match runtime.converse(request).await {
        Ok(turn) => {
            let failed = turn.reply.is_none() || turn.status != "completed";
            Ok(tool_text(
                &json!({
                    "reply": turn.reply,
                    "conversation_id": turn.request.conversation_id.to_string(),
                }),
                failed,
            ))
        }
        Err(err) => Ok(tool_text(
            &json!({
                "error": err.to_string(),
                "conversation_id": conversation_id.to_string(),
            }),
            true,
        )),
    }
}

fn tool_text(payload: &Value, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": payload.to_string() }],
        "isError": is_error
    })
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

#[derive(Debug, PartialEq, Eq)]
enum OriginCheck {
    Absent,
    Allowed,
    Denied,
    Invalid,
}

fn classify_origin(headers: &HeaderMap) -> OriginCheck {
    let Some(value) = headers.get(header::ORIGIN) else {
        return OriginCheck::Absent;
    };
    let Ok(raw) = value.to_str() else {
        return OriginCheck::Invalid;
    };
    match parse_origin_host(raw) {
        None => OriginCheck::Invalid,
        Some(host) if host_is_loopback(&host) => OriginCheck::Allowed,
        Some(_) => OriginCheck::Denied,
    }
}

/// RFC 6454 Origin: `scheme://host[:port]` only. No userinfo, path, query, or fragment.
fn parse_origin_host(origin: &str) -> Option<String> {
    let rest = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))?;
    if rest.contains('@') || rest.contains('/') || rest.contains('?') || rest.contains('#') {
        return None;
    }
    if rest.starts_with('[') {
        let end = rest.find(']')?;
        let host = rest[..=end].to_ascii_lowercase();
        let after = &rest[end + 1..];
        if !after.is_empty() {
            let port = after.strip_prefix(':')?;
            if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
        }
        return Some(host);
    }
    let host = match rest.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => rest,
    };
    if host.is_empty() || host.contains(':') {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

fn host_is_loopback(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1" || host == "[::1]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuntimeConfig;
    use crate::inspect_data_dir;
    use std::fs;
    use uuid::Uuid;

    fn json_body(http: McpHttp) -> Value {
        match http {
            McpHttp::Json(_, value) => value,
            other => panic!("expected JSON body, got {other:?}"),
        }
    }

    fn status_of(http: &McpHttp) -> StatusCode {
        match http {
            McpHttp::Unauthorized => StatusCode::UNAUTHORIZED,
            McpHttp::ForbiddenOrigin => StatusCode::FORBIDDEN,
            McpHttp::Accepted => StatusCode::ACCEPTED,
            McpHttp::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            McpHttp::Json(status, _) => *status,
        }
    }

    fn test_runtime() -> Arc<AgentRuntime> {
        let root = std::env::temp_dir().join(format!("ferricula-mcp-{}", Uuid::new_v4()));
        let memory = root.join("memory");
        let state = root.join("state");
        fs::create_dir_all(&memory).unwrap();
        fs::create_dir_all(&state).unwrap();
        fs::write(
            memory.join("identity.json"),
            r#"{"agent_id":"ferricula-agent","name":"Test Persona"}"#,
        )
        .unwrap();
        fs::write(
            memory.join("agent.toml"),
            "name = \"Test Persona\"\nrole = \"a test fixture\"\n",
        )
        .unwrap();
        let mut config = RuntimeConfig::default();
        config.memory_dir = memory.clone();
        config.state_dir = state.clone();
        config.overlay.path = state.join("overlay.json");
        config.schedule.enabled = false;
        config.require_operator_auth = false;
        let inspection = inspect_data_dir(&memory).unwrap();
        AgentRuntime::open(config, inspection).expect("test AgentRuntime")
    }

    fn post(runtime: &Arc<AgentRuntime>, origin: Option<&str>, protocol: Option<&str>, body: &str) -> McpHttp {
        post_with_auth(runtime, origin, protocol, None, body)
    }

    fn post_with_auth(
        runtime: &Arc<AgentRuntime>,
        origin: Option<&str>,
        protocol: Option<&str>,
        auth: Option<&str>,
        body: &str,
    ) -> McpHttp {
        let mut headers = HeaderMap::new();
        if let Some(origin) = origin {
            headers.insert(header::ORIGIN, origin.parse().unwrap());
        }
        if let Some(protocol) = protocol {
            headers.insert(PROTOCOL_HEADER, protocol.parse().unwrap());
        }
        if let Some(auth) = auth {
            headers.insert(header::AUTHORIZATION, auth.parse().unwrap());
        }
        let runtime = Arc::clone(runtime);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("mcp test runtime")
            .block_on(handle_post(runtime, &headers, body))
    }

    #[test]
    fn origin_rejects_suffix_spoof_userinfo_and_opaque_header() {
        assert_eq!(
            classify_origin(&HeaderMap::new()),
            OriginCheck::Absent
        );
        for denied in [
            "http://localhost.evil",
            "http://127.0.0.1.evil",
            "https://localhost.evil.example",
            "http://user@localhost",
            "http://127.0.0.1:18875/path",
            "null",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::ORIGIN, denied.parse().unwrap());
            let check = classify_origin(&headers);
            assert!(
                matches!(check, OriginCheck::Denied | OriginCheck::Invalid),
                "{denied} => {check:?}"
            );
        }
        for allowed in [
            "http://127.0.0.1:18875",
            "http://localhost",
            "https://localhost:443",
            "http://[::1]:8875",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::ORIGIN, allowed.parse().unwrap());
            assert_eq!(classify_origin(&headers), OriginCheck::Allowed, "{allowed}");
        }
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap());
        assert_eq!(classify_origin(&headers), OriginCheck::Invalid);
    }

    #[test]
    fn handler_forbidden_on_spoofed_origin() {
        let runtime = test_runtime();
        let http = post(
            &runtime,
            Some("http://localhost.evil"),
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        );
        assert_eq!(status_of(&http), StatusCode::FORBIDDEN);
    }

    #[test]
    fn handler_ping_initialize_list_and_unknown_tool() {
        let runtime = test_runtime();
        let ping = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#,
        );
        let ping = json_body(ping);
        assert_eq!(ping["result"], json!({}));
        assert_eq!(ping["id"], "p");

        let init = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        );
        let init = json_body(init);
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(init["result"]["serverInfo"]["agent_id"], "ferricula-agent");
        assert_eq!(init["result"]["serverInfo"]["name"], "ferricula");
        assert_eq!(init["result"]["serverInfo"]["title"], "Test Persona");

        let list = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        );
        let list = json_body(list);
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, [
            "ferricula_status", "ferricula_recall", "ferricula_chat",
            "ferricula_ingest", "ferricula_documents", "ferricula_life", "ferricula_read_section",
        ]);

        let unknown = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#,
        );
        let unknown = json_body(unknown);
        assert_eq!(unknown["id"], 3);
        assert_eq!(unknown["error"]["code"], INVALID_PARAMS);
        assert!(unknown["error"]["message"].as_str().unwrap().contains("Unknown tool"));
    }

    fn tool_payload(response: &Value) -> Value {
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn ingest_then_recall_returns_verbatim_section_and_read_section_matches() {
        let runtime = test_runtime();
        let call = |name: &str, args: Value| {
            json_body(post(
                &runtime,
                None,
                Some(PROTOCOL_VERSION),
                &json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                        "params":{"name": name, "arguments": args}}).to_string(),
            ))
        };
        let text = "# Paging\nVirtual context management pages memory between tiers.\n\n# Other\nUnrelated gardening notes.\n";
        let ingested = call("ferricula_ingest", json!({"text": text, "note": "compare with your memory"}));
        assert_eq!(ingested["result"]["isError"], false, "{ingested}");
        let outcome = tool_payload(&ingested);
        let doc_id = outcome["doc_id"].as_str().unwrap().to_string();
        assert_eq!(outcome["duplicate"], false);
        assert!(outcome["memory_id"].as_u64().unwrap() >= u64::from(crate::memory::EXPERIENCE_ID_BASE));

        let both = call("ferricula_ingest", json!({"text": "a", "url": "https://x"}));
        assert_eq!(both["result"]["isError"], true);

        let recall = tool_payload(&call("ferricula_recall", json!({"query": "virtual context paging"})));
        let section = recall["candidates"].as_array().unwrap().iter()
            .find(|c| c["kind"] == "document_section").expect("section candidate");
        let verbatim = section["section"]["text"].as_str().unwrap();
        assert!(text.contains(verbatim));
        assert!(verbatim.contains("Virtual context management"));
        assert!(recall["experience_hits"].as_array().unwrap().iter()
            .any(|h| h["tags"]["doc_id"] == doc_id.as_str()));

        let listed = tool_payload(&call("ferricula_documents", json!({})));
        assert_eq!(listed["documents"][0]["doc_id"], doc_id.as_str());
        let index = section["section"]["index"].clone();
        let read = tool_payload(&call("ferricula_read_section", json!({"doc_id": doc_id, "index": index})));
        assert_eq!(read["text"].as_str().unwrap(), verbatim);
        assert!(read["cite"].as_str().unwrap().starts_with(&format!("[doc {doc_id}§")));
    }

    #[test]
    fn handler_status_and_empty_recall() {
        let runtime = test_runtime();
        let status = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"ferricula_status","arguments":{}}}"#,
        );
        let status = json_body(status);
        assert_eq!(status["result"]["isError"], false);
        let text = status["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["agent_id"], "ferricula-agent");

        let recall = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"ferricula_recall","arguments":{"query":"   "}}}"#,
        );
        let recall = json_body(recall);
        assert_eq!(recall["result"]["isError"], true);
    }

    #[test]
    fn deprecated_steve_aliases_still_dispatch() {
        let runtime = test_runtime();
        let status = json_body(post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"steve_status","arguments":{}}}"#,
        ));
        assert_eq!(status["result"]["isError"], false);
        let recall = json_body(post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"steve_recall","arguments":{"query":"   "}}}"#,
        ));
        assert_eq!(recall["result"]["isError"], true);
        let chat = json_body(post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"steve_chat","arguments":{}}}"#,
        ));
        let text = chat["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("message is required"));
    }

    #[test]
    fn handler_ferricula_chat_rejects_bad_arguments_without_calling_the_model() {
        let runtime = test_runtime();
        let missing = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"ferricula_chat","arguments":{}}}"#,
        );
        let missing = json_body(missing);
        assert_eq!(missing["result"]["isError"], true);
        let text = missing["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("message is required"));

        let bad_id = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"ferricula_chat","arguments":{"message":"hello","conversation_id":"not-a-uuid"}}}"#,
        );
        let bad_id = json_body(bad_id);
        assert_eq!(bad_id["result"]["isError"], true);
        let text = bad_id["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("conversation_id must be a uuid"));
    }

    #[test]
    fn handler_rejects_batches_and_unsupported_protocol_header() {
        let runtime = test_runtime();
        let batch = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#,
        );
        assert_eq!(status_of(&batch), StatusCode::BAD_REQUEST);
        let body = json_body(batch);
        assert_eq!(body["error"]["code"], INVALID_REQUEST);

        let bad_ver = post(
            &runtime,
            None,
            Some("1999-01-01"),
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        );
        assert_eq!(status_of(&bad_ver), StatusCode::BAD_REQUEST);

        let legacy_header = post(
            &runtime,
            None,
            Some("2025-03-26"),
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        );
        assert_eq!(status_of(&legacy_header), StatusCode::BAD_REQUEST);

        let note = post(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert_eq!(status_of(&note), StatusCode::ACCEPTED);
    }

    #[test]
    fn initialize_older_proposal_returns_supported_version() {
        let runtime = test_runtime();
        let init = post(
            &runtime,
            None,
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        );
        let init = json_body(init);
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn reader_role_enforcement_in_mcp() {
        let root = tempfile::tempdir().unwrap();
        let memory = root.path().join("memory");
        let state = root.path().join("state");
        fs::create_dir_all(&memory).unwrap();
        fs::create_dir_all(&state).unwrap();
        fs::write(
            memory.join("identity.json"),
            r#"{"agent_id":"ferricula-agent","name":"Test Persona"}"#,
        )
        .unwrap();
        fs::write(
            memory.join("agent.toml"),
            "name = \"Test Persona\"\nrole = \"a test fixture\"\n",
        )
        .unwrap();
        let mut config = RuntimeConfig::default();
        config.memory_dir = memory.clone();
        config.state_dir = state.clone();
        config.overlay.path = state.join("overlay.json");
        config.schedule.enabled = false;
        config.require_operator_auth = true;
        config.operator_token_env = "TEST_MCP_OPERATOR_TOKEN".to_string();
        config.auth.mode = crate::config::AuthMode::Both;
        unsafe {
            std::env::set_var("TEST_MCP_OPERATOR_TOKEN", "test-static-operator-token");
        }
        config.auth.agents = vec![
            crate::config::AgentAuthRule {
                actor: "reader-agent".to_string(),
                role: "reader".to_string(),
            },
            crate::config::AgentAuthRule {
                actor: "operator-agent".to_string(),
                role: "operator".to_string(),
            },
        ];

        let inspection = inspect_data_dir(&memory).unwrap();
        let runtime = AgentRuntime::open(config, inspection).expect("test AgentRuntime");

        let reader_token = "ahp_reader_token_mcp";
        let operator_token = "ahp_operator_token_mcp";

        runtime.auth.cache_ahp_token(
            reader_token,
            crate::auth::AuthIdentity {
                user_id: "".to_string(),
                email: Some("reader@nuts.services".to_string()),
                name: Some("Reader Agent".to_string()),
                actor: Some("reader-agent".to_string()),
                role: "reader".to_string(),
                via: "ahp".to_string(),
                via_cookie: false,
            },
        );

        runtime.auth.cache_ahp_token(
            operator_token,
            crate::auth::AuthIdentity {
                user_id: "".to_string(),
                email: Some("operator@nuts.services".to_string()),
                name: Some("Operator Agent".to_string()),
                actor: Some("operator-agent".to_string()),
                role: "operator".to_string(),
                via: "ahp".to_string(),
                via_cookie: false,
            },
        );

        // 1. Reader tools/list shows only read tools
        let list_resp = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {reader_token}")),
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        );
        let list_body = json_body(list_resp);
        let names: Vec<&str> = list_body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "ferricula_status",
                "ferricula_recall",
                "ferricula_documents",
                "ferricula_read_section"
            ]
        );
        assert!(!names.contains(&"ferricula_chat"));
        assert!(!names.contains(&"ferricula_ingest"));
        assert!(!names.contains(&"ferricula_life"));

        // 2. Operator tools/list shows all 7 tools
        let op_list_resp = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {operator_token}")),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        );
        let op_list_body = json_body(op_list_resp);
        let op_names: Vec<&str> = op_list_body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            op_names,
            [
                "ferricula_status",
                "ferricula_recall",
                "ferricula_chat",
                "ferricula_ingest",
                "ferricula_documents",
                "ferricula_life",
                "ferricula_read_section"
            ]
        );

        // 3. Reader tools/call for ferricula_chat is refused with clear MCP error
        let chat_call = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {reader_token}")),
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"ferricula_chat","arguments":{"message":"hello"}}}"#,
        );
        let chat_body = json_body(chat_call);
        assert_eq!(chat_body["result"]["isError"], true);
        let err_payload = tool_payload(&chat_body);
        assert_eq!(err_payload["error"], "read-only role");

        // 4. Reader tools/call for ferricula_ingest is refused
        let ingest_call = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {reader_token}")),
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"ferricula_ingest","arguments":{"text":"doc"}}}"#,
        );
        let ingest_body = json_body(ingest_call);
        assert_eq!(ingest_body["result"]["isError"], true);
        assert_eq!(tool_payload(&ingest_body)["error"], "read-only role");

        // 5. Reader tools/call for ferricula_life is refused
        let life_call = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {reader_token}")),
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"ferricula_life","arguments":{}}}"#,
        );
        let life_body = json_body(life_call);
        assert_eq!(life_body["result"]["isError"], true);
        assert_eq!(tool_payload(&life_body)["error"], "read-only role");

        // 6. Reader tools/call for ferricula_status succeeds
        let status_call = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {reader_token}")),
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"ferricula_status","arguments":{}}}"#,
        );
        let status_body = json_body(status_call);
        assert_eq!(status_body["result"]["isError"], false);

        // 7. Reader tools/call for ferricula_recall succeeds
        let recall_call = post_with_auth(
            &runtime,
            None,
            Some(PROTOCOL_VERSION),
            Some(&format!("Bearer {reader_token}")),
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"ferricula_recall","arguments":{"query":"test"}}}"#,
        );
        let recall_body = json_body(recall_call);
        assert_eq!(recall_body["result"]["isError"], false);
    }
}
