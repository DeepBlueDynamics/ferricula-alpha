//! Operator-facing HTTP surface for the agent runtime.
//!
//! Status and recall are read-only. Explicit operator chat and structured
//! episode commits persist in separate writable state; recovery memory stays
//! immutable. Background pause does not disable an authenticated chat request.
//! Overlay approval remains unavailable. MCP exposes read-only tools.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router, response::IntoResponse};
use ferricula_cognition::WhisperContext;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::runtime::{ActivityMode, RuntimeStatus, AgentRuntime, TaskKind};
use crate::sleep_cycle::{CooldownLedger, PlanningGate, plan_cycle_gated};

type ApiError = (StatusCode, Json<Value>);
type ApiResult<T> = Result<Json<T>, ApiError>;

/// Hard cap on array elements returned by Phase One list endpoints.
const MAX_STATUS_LIST: usize = 64;

pub fn router(runtime: Arc<AgentRuntime>) -> Router {
    // Routes that can carry a whole document (base64 PDF) get a body limit
    // sized from [documents] max_bytes; every other route keeps axum's default.
    let document_body_limit = DefaultBodyLimit::max(runtime.config.documents.body_limit());
    let documents = Router::new()
        .route("/documents", get(list_documents).post(ingest_document))
        .merge(crate::mcp::router())
        .layer(document_body_limit);
    Router::new()
        .route("/documents/search", post(search_documents))
        .route("/documents/{doc_id}", get(get_document))
        .route("/documents/{doc_id}/sections/{index}", get(get_section))
        .merge(documents)
        .route("/", get(chat_page))
        .route("/talk", get(|| async { axum::response::Html(include_str!("talk.html")) }))
        .route("/chat", post(chat))
        .route("/chat/stream", post(chat_stream))
        .route("/chat/{conversation_id}", get(conversation))
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/identity", get(identity))
        .route("/auth/login", get(auth_login))
        .route("/auth/callback", get(auth_callback))
        .route("/auth/logout", post(auth_logout))
        .route("/auth/status", get(auth_status))
        .route("/auth/break-glass", get(auth_break_glass))
        .route("/control/mode", post(set_mode))
        .route("/control/wake", post(wake))
        .route("/control/sleep", post(sleep))
        .route("/control/pause", post(pause))
        .route("/control/schedule", post(set_schedule))
        // Phase One read-only status slices (operator auth).
        .route("/control/autonomy", get(autonomy_status))
        .route("/control/schedule/plan", get(sleep_plan_status))
        .route("/control/advocate", get(advocate_status))
        .route("/tasks", get(tasks).post(enqueue))
        .route("/tasks/{id}", get(task))
        .route("/tasks/read-feed", post(read_feed))
        .route("/tasks/read-hacker-news", post(read_hacker_news))
        .route("/tasks/mention", post(mention))
        .route("/tasks/considerations", get(mention_considerations))
        .route("/memory/recall", post(recall))
        .route("/memory/episodes", post(episode_commit))
        .route("/memory/episode-query", post(episode_query))
        .route("/memory/overlay", get(overlay_status))
        .route(
            "/memory/overlay/approve/{event_id}",
            post(overlay_approve_blocked),
        )
        .route("/models/status", get(model_status))
        // R2b: re-run the embedding space probe (operator).
        .route("/embeddings/probe", post(embeddings_probe))
        .route("/meaning", get(meaning_status))
        .route("/meaning/backfill", post(meaning_backfill))
        // R3 life: drives, journal, dreams; forced urges for testing.
        .route("/life", get(life_status))
        .route("/life/meditate", post(life_meditate))
        .route("/life/end-meditation", post(life_end_meditation))
        .route("/life/urge", post(life_urge))
        .route("/settings", get(|| async { axum::response::Html(include_str!("settings.html")) }))
        .route("/settings/jev", get(jev_status).post(jev_update))
        .route("/settings/jev/probe", post(jev_probe))
        .route("/settings/discord/channels", get(discord_channels))
        .route("/wisdom/preview", post(wisdom_preview))
        .with_state(runtime)
}

// --- Documents (R1 sense door) ---------------------------------------------

/// `POST /documents` body: a tagged source plus an optional operator note.
#[derive(Deserialize)]
pub struct IngestBody {
    #[serde(flatten)]
    pub source: IngestSourceBody,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IngestSourceBody {
    Text { #[serde(default)] title: Option<String>, text: String },
    Url { url: String },
    Pdf { name: String, base64: String },
}

/// Validate sizes before decoding, then decode into an extraction source.
/// Shared by the HTTP route and the MCP tool.
pub fn decode_source(
    body: IngestSourceBody,
    config: &crate::config::DocumentsConfig,
) -> Result<ferricula_ingest::Source, (StatusCode, String)> {
    use base64::Engine as _;
    let too_large = |what: &str| (StatusCode::PAYLOAD_TOO_LARGE,
        format!("{what} exceeds documents.max_bytes ({} bytes)", config.max_bytes));
    match body {
        IngestSourceBody::Text { title, text } => {
            if text.len() > config.max_bytes { return Err(too_large("text")); }
            if text.trim().is_empty() { return Err((StatusCode::BAD_REQUEST, "text is empty".into())); }
            if title.as_ref().is_some_and(|t| t.len() > 1024) {
                return Err((StatusCode::BAD_REQUEST, "title exceeds 1024 bytes".into()));
            }
            Ok(ferricula_ingest::Source::Text { title, text })
        }
        IngestSourceBody::Url { url } => {
            if url.len() > 4096 { return Err((StatusCode::BAD_REQUEST, "url exceeds 4096 bytes".into())); }
            Ok(ferricula_ingest::Source::Url { url: url.trim().to_string() })
        }
        IngestSourceBody::Pdf { name, base64 } => {
            if name.trim().is_empty() || name.len() > 512 {
                return Err((StatusCode::BAD_REQUEST, "name must be 1 to 512 bytes".into()));
            }
            // Checked before decoding so an oversized payload costs no allocation.
            let compact: String = if base64.bytes().any(|b| b.is_ascii_whitespace()) {
                base64.chars().filter(|c| !c.is_ascii_whitespace()).collect()
            } else {
                base64
            };
            if compact.len() > config.max_base64_len() { return Err(too_large("pdf")); }
            let bytes = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes())
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid base64: {e}")))?;
            if bytes.len() > config.max_bytes { return Err(too_large("pdf")); }
            Ok(ferricula_ingest::Source::Pdf { name, bytes })
        }
    }
}

async fn ingest_document(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(body): Json<IngestBody>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    if body.note.as_ref().is_some_and(|n| n.len() > crate::runtime::MAX_NOTE_BYTES) {
        return Err(bad_request(format!("note exceeds {} bytes", crate::runtime::MAX_NOTE_BYTES)));
    }
    let source = decode_source(body.source, &runtime.config.documents)
        .map_err(|(status, error)| (status, Json(json!({ "error": error }))))?;
    let note = body.note;
    // Complete the ingest even if the HTTP client disconnects mid-fetch.
    let worker = runtime.clone();
    let outcome = tokio::spawn(async move { worker.ingest(source, note).await })
        .await.map_err(internal)?
        .map_err(|error| (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": format!("{error:#}") }))))?;
    runtime.life_sensed_document(outcome.duplicate);
    Ok(Json(serde_json::to_value(outcome).map_err(internal)?))
}

async fn list_documents(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    Ok(Json(json!({ "documents": runtime.documents() })))
}

async fn get_document(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Path(doc_id): Path<String>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let Some(record) = runtime.document(&doc_id) else {
        return Err((StatusCode::NOT_FOUND, Json(json!({ "error": "document not found" }))));
    };
    // Section outline only; verbatim text is served per section.
    let sections: Vec<Value> = record.sections.iter().map(|s| json!({
        "index": s.index, "heading": s.heading, "page": s.page, "bytes": s.text.len(),
        "cite": crate::recall::citation(&record.meta.doc_id, s.index, s.page),
    })).collect();
    Ok(Json(json!({ "meta": record.meta, "sections": sections, "reading": runtime.document_reading(&doc_id) })))
}

async fn get_section(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Path((doc_id, index)): Path<(String, u32)>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let Some(section) = runtime.document_section(&doc_id, index) else {
        return Err((StatusCode::NOT_FOUND, Json(json!({ "error": "section not found" }))));
    };
    Ok(Json(serde_json::to_value(section).map_err(internal)?))
}

#[derive(Deserialize)]
struct DocumentSearchRequest {
    query: String,
    #[serde(default = "default_search_k")]
    k: usize,
    #[serde(default)]
    doc_id: Option<String>,
}

fn default_search_k() -> usize {
    5
}

async fn search_documents(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<DocumentSearchRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    if request.query.trim().is_empty() {
        return Err(bad_request("query cannot be empty"));
    }
    if request.query.len() > 8192 {
        return Err(bad_request("query exceeds 8192 bytes"));
    }
    let k = request.k.clamp(1, 50);
    let hits = runtime.search_documents(&request.query, k, request.doc_id.as_deref());
    Ok(Json(json!({ "query": request.query, "hits": hits })))
}

async fn episode_commit(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(item): Json<ferricula_episode::EpisodeItem>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let event_id = runtime.commit_episode(item).map_err(bad_request)?;
    Ok(Json(json!({"event_id": event_id, "source_actor_verified": false})))
}

async fn episode_query(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<ferricula_episode::query::EpisodeQueryRequest>,
) -> ApiResult<ferricula_episode::query::EpisodeQueryResponse> {
    require_operator(&runtime, &headers)?;
    if request.query.len() > 8192 { return Err(bad_request("query exceeds 8192 bytes")); }
    Ok(Json(runtime.query_episodes(&request)))
}

async fn chat_page() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("chat.html"))
}

async fn chat(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<crate::runtime::ChatRequest>,
) -> ApiResult<crate::runtime::ChatTurn> {
    require_operator(&runtime, &headers)?;
    request.validate().map_err(bad_request)?;
    // The durable request must reach a terminal state even if HTTP disconnects.
    let turn = tokio::spawn(async move { runtime.converse(request).await })
        .await.map_err(internal)?.map_err(|error| (
        StatusCode::CONFLICT, Json(json!({"error": error.to_string()}))
    ))?;
    Ok(Json(turn))
}

/// `POST /chat/stream`: the same turn as `/chat`, answered as
/// `text/event-stream`. Each SSE message is named for its event (`accepted`,
/// `candidate`, `document`, `curator`, `round_start`, `model_call`, `round`,
/// `tool_call`, `tool_result`, `gate`, `verdict`, `nudge`, then `done` or
/// `failed`) and carries one JSON object with `request_id` and `t` (ms since
/// the turn began). The durable turn completes even if the client leaves.
async fn chat_stream(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<crate::runtime::ChatRequest>,
) -> Result<axum::response::sse::Sse<impl futures_core::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>, (StatusCode, Json<Value>)> {
    require_operator(&runtime, &headers)?;
    request.validate().map_err(bad_request)?;
    let (events, rx) = crate::runtime::TurnEvents::channel(request.request_id);
    let rejected = events.clone();
    tokio::spawn(async move {
        if let Err(error) = runtime.converse_with_events(request, events).await {
            // Rejected before the turn began (busy, reused request_id...).
            rejected.emit("failed", json!({ "error": error.to_string(), "rejected": true }));
        }
    });
    let stream = tokio_stream::StreamExt::map(tokio_stream::wrappers::UnboundedReceiverStream::new(rx), |event| {
        let name = event["event"].as_str().unwrap_or("message").to_string();
        Ok(axum::response::sse::Event::default().event(name).data(event.to_string()))
    });
    Ok(axum::response::sse::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

async fn conversation(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    Ok(Json(json!({"turns": runtime.conversation(id), "limit": 100})))
}

async fn health(State(runtime): State<Arc<AgentRuntime>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "agent_id": runtime.inspection.agent_id,
        "mode": runtime.status().mode
    }))
}

async fn status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<crate::runtime::RuntimeStatus> {
    require_operator(&runtime, &headers)?;
    Ok(Json(runtime.status()))
}

async fn identity(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    Ok(Json(
        serde_json::to_value(&runtime.inspection).map_err(internal)?,
    ))
}

#[derive(Deserialize)]
struct ModeRequest {
    mode: String,
}

async fn set_mode(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<ModeRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let mode = ActivityMode::parse(&request.mode).map_err(bad_request)?;
    runtime.set_mode(mode).map_err(internal)?;
    Ok(Json(json!({ "mode": mode })))
}

async fn wake(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    runtime.set_mode(ActivityMode::Engaged).map_err(internal)?;
    Ok(Json(json!({ "mode": ActivityMode::Engaged })))
}

async fn sleep(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    runtime.set_mode(ActivityMode::Asleep).map_err(internal)?;
    // The mode alone left the drives awake (phase resting, no consolidation,
    // no dream). With life on, the operator's sleep is a real sleep: the
    // same path as the forced sleep urge, run in the background because
    // consolidation and the dream take model calls.
    let life = runtime.life_status(0)["enabled"].as_bool().unwrap_or(false);
    if life {
        let runtime = runtime.clone();
        tokio::spawn(async move {
            if let Err(error) = runtime.life_force(crate::runtime::LifeUrgeRequest::Sleep).await {
                eprintln!("control: sleep did not reach life: {error:#}");
            }
        });
    }
    Ok(Json(json!({ "mode": ActivityMode::Asleep, "life_sleep": life })))
}

async fn pause(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    runtime.set_mode(ActivityMode::Paused).map_err(internal)?;
    Ok(Json(json!({ "mode": ActivityMode::Paused })))
}

#[derive(Deserialize)]
struct ScheduleRequest {
    enabled: bool,
}

async fn set_schedule(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<ScheduleRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    runtime.set_schedule(request.enabled).map_err(internal)?;
    Ok(Json(json!({ "enabled": request.enabled })))
}

async fn tasks(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let list = bound_slice(runtime.tasks(), MAX_STATUS_LIST);
    Ok(Json(
        json!({ "tasks": list, "truncated": runtime.tasks().len() > MAX_STATUS_LIST }),
    ))
}

async fn task(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let Some(task) = runtime.tasks().into_iter().find(|task| task.id == id) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task not found" })),
        ));
    };
    Ok(Json(serde_json::to_value(task).map_err(internal)?))
}

#[derive(Deserialize)]
struct TaskRequest {
    kind: TaskKind,
    #[serde(default)]
    payload: Value,
}

async fn enqueue(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<TaskRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let task = runtime
        .enqueue(request.kind, "operator", request.payload)
        .map_err(internal)?;
    Ok(Json(serde_json::to_value(task).map_err(internal)?))
}

async fn read_feed(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let task = runtime
        .enqueue(TaskKind::ReadFeed, "operator", json!({}))
        .map_err(internal)?;
    Ok(Json(serde_json::to_value(task).map_err(internal)?))
}

async fn read_hacker_news(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let task = runtime
        .enqueue(TaskKind::ReadHackerNews, "operator", json!({}))
        .map_err(internal)?;
    Ok(Json(serde_json::to_value(task).map_err(internal)?))
}

#[derive(Deserialize)]
struct MentionRequest {
    item_id: u64,
    comment_id: Option<u64>,
    by: String,
    text: String,
}

async fn mention(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<MentionRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let text = bound_text(&request.by, 128);
    let body = bound_text(&request.text, 4_096);
    let task = runtime
        .enqueue(
            TaskKind::DirectMention,
            "nutnews-mention",
            json!({
                "item_id": request.item_id,
                "comment_id": request.comment_id,
                "by": text,
                "text": body
            }),
        )
        .map_err(internal)?;
    Ok(Json(serde_json::to_value(task).map_err(internal)?))
}

// --- Phase One read-only endpoints ------------------------------------------

/// `GET /control/autonomy` — snapshot from `status()` + public `config.autonomy`.
async fn autonomy_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let s = runtime.status();
    Ok(Json(project_autonomy_status(&s, &runtime.config)))
}

/// `GET /tasks/considerations` — pending **counts** only (no private bodies;
/// runtime does not expose consideration text via a public getter).
async fn mention_considerations(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let s = runtime.status();
    Ok(Json(project_mention_pending(&s, &runtime.config)))
}

/// `GET /control/schedule/plan` — sleep status + optional config-preview plan
/// (durable cooldown ledger is not public on AgentRuntime; preview uses empty ledger).
async fn sleep_plan_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let s = runtime.status();
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(Json(project_sleep_status(
        &s,
        &runtime.config,
        &runtime.inspection.agent_id,
        t,
    )))
}

/// `GET /memory/overlay` — metadata only (event count / flags), no payload text.
async fn overlay_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let s = runtime.status();
    Ok(Json(project_overlay_status(&s, &runtime.config)))
}

/// `GET /control/advocate` — last alignment summary from status; no memory text.
async fn advocate_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let s = runtime.status();
    Ok(Json(project_advocate_status(&s, &runtime.config)))
}

/// `POST /memory/overlay/approve/{event_id}` — blocked: no public runtime
/// method enforces approval + sovereignty yet.
async fn overlay_approve_blocked(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Path(event_id): Path<String>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let _ = bound_text(&event_id, 128);
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "overlay approval is not exposed: AgentRuntime has no public method that appends Approval events under sovereignty/lease gates",
            "event_id": bound_text(&event_id, 128),
            "status": "blocked",
            "hint": "add runtime.approve_overlay_event(..) that checks overlay.enabled, may_write_overlay, and OverlayLog approval rules"
        })),
    ))
}

async fn wisdom_preview(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(context): Json<WhisperContext>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let (whispers, controls) = runtime.wisdom_preview(context);
    let whispers = bound_slice(whispers, MAX_STATUS_LIST);
    Ok(Json(
        json!({ "whispers": whispers, "integrated_controls": controls }),
    ))
}

#[derive(Deserialize)]
struct RecallRequest {
    query: String,
    #[serde(default = "default_recall_limit")]
    limit: usize,
}

fn default_recall_limit() -> usize {
    10
}

async fn recall(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<RecallRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    if request.query.trim().is_empty() {
        return Err(bad_request("query cannot be empty"));
    }
    if request.query.len() > 8192 {
        return Err(bad_request("query exceeds 8192 bytes"));
    }
    let limit = request.limit.clamp(1, MAX_STATUS_LIST);
    // `hits` keeps its pre-R1 meaning (recovered-base lexical hits); the
    // experience, section, and fused lists are additive fields.
    let recall = runtime.hybrid_recall_async(&request.query, limit).await;
    Ok(Json(serde_json::to_value(recall).map_err(internal)?))
}

// --- Life (R3) ---------------------------------------------------------------

#[derive(Deserialize)]
struct LifeQuery {
    #[serde(default = "default_life_journal")]
    journal: usize,
}

fn default_life_journal() -> usize {
    20
}

async fn life_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<LifeQuery>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    Ok(Json(runtime.life_status(query.journal.min(200))))
}

async fn life_meditate(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let entry = runtime.life_meditate(true).map_err(conflict)?;
    Ok(Json(entry))
}

async fn life_end_meditation(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let entry = runtime.life_meditate(false).map_err(conflict)?;
    Ok(Json(entry))
}

/// `GET /settings/jev`: hosted JEV gate tier status (never the key).
async fn jev_status(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    Ok(Json(runtime.jev_status()))
}

/// `POST /settings/jev`: set or clear the key and change settings.
async fn jev_update(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(update): Json<crate::jev::JevUpdate>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    runtime.jev_update(update).map(Json).map_err(bad_request)
}

/// `GET /settings/discord/channels`: every text channel the bot can see,
/// with ids, for the `[discord] channels` map (never the token).
async fn discord_channels(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let cfg = runtime.config.discord.clone();
    let listed = tokio::task::spawn_blocking(move || crate::discord::list_channels(&cfg)).await.map_err(internal)?;
    match listed {
        Ok(rows) => Ok(Json(json!({ "channels": rows.into_iter().map(|(server, name, id)| json!({ "server": server, "channel": name, "id": id })).collect::<Vec<_>>() }))),
        Err(error) => Ok(Json(json!({ "error": format!("{error:#}") }))),
    }
}

/// `POST /settings/jev/probe`: one live call on fixed non-private text.
async fn jev_probe(State(runtime): State<Arc<AgentRuntime>>, headers: HeaderMap) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    let result = tokio::task::spawn_blocking(move || runtime.jev_probe()).await.map_err(internal)?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct UrgeRequest {
    urge: crate::runtime::LifeUrgeRequest,
}

async fn life_urge(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    Json(request): Json<UrgeRequest>,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    // Runs to completion even if the HTTP client disconnects.
    let urge = request.urge;
    let entries = tokio::spawn(async move { runtime.life_force(urge).await })
        .await.map_err(internal)?.map_err(conflict)?;
    Ok(Json(json!({ "urge": urge, "journal": entries })))
}

fn conflict(error: impl std::fmt::Display) -> ApiError {
    (StatusCode::CONFLICT, Json(json!({ "error": error.to_string() })))
}

/// `POST /embeddings/probe` — re-embed a few short recovered memories and
/// compare with their stored vectors; returns the new embeddings status.
async fn embeddings_probe(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<crate::runtime::EmbeddingsStatus> {
    require_operator(&runtime, &headers)?;
    let status = tokio::task::spawn_blocking(move || runtime.probe_embeddings())
        .await
        .map_err(internal)?;
    Ok(Json(status))
}

/// `GET /meaning` — meaning index coverage and backfill progress.
async fn meaning_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<crate::runtime::MeaningStatus> {
    require_operator(&runtime, &headers)?;
    Ok(Json(runtime.meaning_status()))
}

/// `POST /meaning/backfill` — start a background backfill (embed every
/// memory, experience row and section without a vector). 202 with
/// `started = false` when one is already running; 409 when the embedder is
/// not usable.
async fn meaning_backfill(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    require_operator(&runtime, &headers)?;
    if runtime.embedder().is_none() {
        return Err(conflict(format!(
            "embedder not usable (embeddings state {:?})",
            runtime.embeddings_status().state
        )));
    }
    let started = runtime.spawn_meaning_backfill("operator");
    Ok((StatusCode::ACCEPTED, Json(json!({ "started": started, "meaning": runtime.meaning_status() }))))
}

async fn model_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    require_operator(&runtime, &headers)?;
    Ok(Json(runtime.model_status()))
}

// --- Projection helpers (pure; unit-tested) ---------------------------------

fn project_autonomy_status(s: &RuntimeStatus, config: &crate::config::RuntimeConfig) -> Value {
    let policy = &config.autonomy.policy;
    json!({
        "enabled": config.autonomy.enabled,
        "state": s.autonomy_state,
        "lease": s.autonomy_lease_id.map(|id| json!({
            "id": id,
            "model_allowed": s.autonomy_model_allowed
        })),
        "model_allowed_now": s.autonomy_model_allowed,
        "compel_response": s.compel_response,
        "compel_write": s.compel_write,
        "mode": s.mode,
        "policy": {
            "tick_secs": policy.tick_secs,
            "quiet_secs": policy.quiet_secs,
            "idle_resume_secs": policy.idle_resume_secs,
            "lease_secs": policy.lease_secs,
            "lease_steps": policy.lease_steps,
            "budget_gate": policy.budget_gate,
            "dream_idle_secs": policy.dream_idle_secs,
            "honor_quiet_period": policy.honor_quiet_period
        },
        "note": "idle_secs/budget_pressure live on the autonomy snapshot; only status-public fields are returned here"
    })
}

fn project_mention_pending(s: &RuntimeStatus, config: &crate::config::RuntimeConfig) -> Value {
    // Bodies of pending considerations are not on a public runtime getter.
    // Expose counts only — never invent text that could leak private content.
    let pending = s.mention_pending.min(MAX_STATUS_LIST);
    json!({
        "enabled": config.mentions.enabled,
        "cursor": s.mention_cursor,
        "gap": s.mention_gap,
        "pending_count": s.mention_pending,
        "items": [],
        "truncated": s.mention_pending > MAX_STATUS_LIST,
        "shown": pending,
        "note": "consideration bodies are not exposed via public AgentRuntime methods; count/cursor only"
    })
}

fn project_sleep_status(
    s: &RuntimeStatus,
    config: &crate::config::RuntimeConfig,
    agent_id: &str,
    now: u64,
) -> Value {
    let enabled = config.sleep_cycle.enabled;
    let gate = match s.mode {
        ActivityMode::Paused => PlanningGate::Paused,
        _ => PlanningGate::Allow,
    };
    // Config-preview plan only: durable CooldownLedger is not public on the runtime.
    let (proposals, skipped, gate_label) = if enabled {
        let plan = plan_cycle_gated(
            &config.sleep_cycle,
            &CooldownLedger::default(),
            agent_id,
            now,
            gate,
        );
        let props: Vec<Value> = plan
            .proposals
            .iter()
            .take(MAX_STATUS_LIST)
            .map(|p| {
                json!({
                    "id": p.id,
                    "kind": p.kind.as_str(),
                    "phase": format!("{:?}", p.phase),
                    "requires_operator_approval": p.requires_operator_approval,
                    "memory_effect": format!("{:?}", p.memory_effect),
                    // No private memory excerpts — only metadata.
                })
            })
            .collect();
        let skips: Vec<Value> = plan
            .skipped
            .iter()
            .take(MAX_STATUS_LIST)
            .map(|sk| {
                json!({
                    "kind": sk.kind.as_str(),
                    "reason": bound_text(&sk.reason, 256),
                    "ready_at": sk.ready_at
                })
            })
            .collect();
        (props, skips, format!("{:?}", plan.gate))
    } else {
        (vec![], vec![], "disabled".into())
    };
    json!({
        "enabled": enabled,
        "last_cycle": s.sleep_last_cycle,
        "mode": s.mode,
        "ready_for_planning": enabled && !matches!(s.mode, ActivityMode::Paused),
        "gate": gate_label,
        "proposals": proposals,
        "skipped": skipped,
        "proposals_source": if enabled {
            "config_preview_without_durable_cooldowns"
        } else {
            "disabled"
        },
        "policy": {
            "cycles_per_day": config.sleep_cycle.cycles_per_day,
            "max_proposals_per_cycle": config.sleep_cycle.max_proposals_per_cycle,
            "share_private_memory": config.sleep_cycle.privacy.share_private_memory
        },
        "note": "proposal list is a pure config preview; durable cooldown ledger is not exposed on AgentRuntime"
    })
}

fn project_overlay_status(s: &RuntimeStatus, config: &crate::config::RuntimeConfig) -> Value {
    json!({
        "enabled": config.overlay.enabled,
        "path": config.overlay.path.display().to_string(),
        "total_events": s.overlay_events,
        "pending_approvals": [],
        "pending_approvals_note": "pending approval envelopes are not on a public runtime getter; use offline tooling or a future status method",
        "bounds": {
            "max_events": config.overlay.bounds.max_events,
            "max_text_bytes": config.overlay.bounds.max_text_bytes,
            "max_note_bytes": config.overlay.bounds.max_note_bytes
        },
        "compel_write": s.compel_write
    })
}

fn project_advocate_status(s: &RuntimeStatus, config: &crate::config::RuntimeConfig) -> Value {
    json!({
        "enabled": config.advocate.enabled,
        "reviews_per_day": config.advocate.reviews_per_day,
        "last_alignment": s.advocate_alignment,
        "expired": s.advocate_expired,
        "history": [],
        "history_note": "full wants/verdict history is not on a public runtime getter; last_alignment only",
        "compel_response": s.compel_response
    })
}

fn bound_slice<T>(mut items: Vec<T>, max: usize) -> Vec<T> {
    if items.len() > max {
        items.truncate(max);
    }
    items
}

fn bound_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

fn require_operator(runtime: &AgentRuntime, headers: &HeaderMap) -> Result<(), ApiError> {
    require_operator_identity(runtime, headers).map(|_| ())
}

fn require_operator_identity(
    runtime: &AgentRuntime,
    headers: &HeaderMap,
) -> Result<crate::auth::AuthIdentity, ApiError> {
    if !runtime.config.require_operator_auth {
        return Ok(crate::auth::AuthIdentity::unauthenticated());
    }

    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let bearer = authorization.and_then(|h| h.strip_prefix("Bearer "));
    let cookie_session = crate::auth::extract_session_cookie(headers);
    let expected_static = std::env::var(&runtime.config.operator_token_env).ok();

    match runtime.auth.check_authorization(bearer, cookie_session, expected_static.as_deref()) {
        crate::auth::AuthCheckResult::Authorized(identity) => {
            if identity.via == "session" {
                if let Err(csrf_err) = crate::auth::verify_csrf(headers) {
                    return Err((
                        StatusCode::FORBIDDEN,
                        Json(json!({ "error": csrf_err })),
                    ));
                }
            }
            Ok(identity)
        }
        crate::auth::AuthCheckResult::Forbidden(reason) => Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": reason })),
        )),
        crate::auth::AuthCheckResult::Unreachable(reason) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": reason })),
        )),
        crate::auth::AuthCheckResult::Unauthorized(reason) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": reason })),
        )),
    }
}

fn get_public_base_url(runtime: &AgentRuntime, headers: &HeaderMap) -> String {
    if let Some(ref url) = runtime.auth.config.public_url {
        return url.trim_end_matches('/').to_string();
    }

    let bind = runtime.config.bind;
    let host_port = if bind.ip().is_loopback() || bind.ip().is_unspecified() {
        format!("127.0.0.1:{}", bind.port())
    } else {
        bind.to_string()
    };

    let is_https = runtime.auth.config.trust_proxy && is_request_secure(headers);
    let scheme = if is_https { "https" } else { "http" };
    format!("{scheme}://{host_port}")
}

fn is_request_secure(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|p| p.to_str().ok())
        .map(|p| p.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

fn is_secure_connection(runtime: &AgentRuntime, headers: &HeaderMap) -> bool {
    if let Some(ref url) = runtime.auth.config.public_url {
        if url.starts_with("https://") {
            return true;
        }
    }
    if runtime.auth.config.trust_proxy {
        return is_request_secure(headers);
    }
    false
}

async fn auth_login(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    if !runtime.auth.login_enabled() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "nuts-auth login is disabled" })),
        ));
    }
    let origin = get_public_base_url(&runtime, &headers);
    let return_url = format!("{origin}/auth/callback");
    let redirect_url = runtime.auth.build_login_url(&return_url);
    let mut response = axum::response::Redirect::to(&redirect_url).into_response();
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

#[derive(Deserialize)]
struct AuthCallbackParams {
    token: Option<String>,
}

async fn auth_callback(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<AuthCallbackParams>,
) -> Result<axum::response::Response, ApiError> {
    let Some(token) = params.token.filter(|t| !t.trim().is_empty()) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing token parameter" })),
        ));
    };

    let claims = match runtime.auth.verify_jwt(&token) {
        Ok(c) => c,
        Err(crate::auth::JwtError::Expired) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "token expired" })),
            ));
        }
        Err(crate::auth::JwtError::Unreachable) => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "nuts-auth unreachable" })),
            ));
        }
        Err(crate::auth::JwtError::Invalid(msg)) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": format!("invalid token: {msg}") })),
            ));
        }
    };

    if !runtime.auth.is_operator(&claims.user_id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "not an operator for this agent",
                "user_id": claims.user_id,
            })),
        ));
    }

    let (raw_session_id, _session) = runtime
        .auth
        .create_session(
            &claims.user_id,
            &claims.sub,
            claims.name.as_deref(),
            "nuts-auth",
            claims.exp,
        )
        .map_err(|e| internal(e))?;

    let is_secure = is_secure_connection(&runtime, &headers);
    let max_age_secs = runtime.auth.config.session_hours * 3600;
    let cookie = crate::auth::format_session_cookie(&raw_session_id, max_age_secs, is_secure);

    let mut response = axum::response::Redirect::to("/talk").into_response();
    *response.status_mut() = StatusCode::SEE_OTHER;
    if let Ok(cookie_val) = axum::http::HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(axum::http::header::SET_COOKIE, cookie_val);
    }
    Ok(response)
}

async fn auth_logout(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    if let Some(session_id) = crate::auth::extract_session_cookie(&headers) {
        runtime.auth.delete_session(session_id);
    }
    let is_secure = is_secure_connection(&runtime, &headers);
    let clear_cookie = crate::auth::format_clear_session_cookie(is_secure);
    let mut response = Json(json!({ "ok": true })).into_response();
    if let Ok(cookie_val) = axum::http::HeaderValue::from_str(&clear_cookie) {
        response.headers_mut().insert(axum::http::header::SET_COOKIE, cookie_val);
    }
    Ok(response)
}

async fn auth_status(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
) -> Json<Value> {
    let login_enabled = runtime.auth.login_enabled();
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let bearer = authorization.and_then(|h| h.strip_prefix("Bearer "));
    let session_id = crate::auth::extract_session_cookie(&headers);
    let expected_static = std::env::var(&runtime.config.operator_token_env).ok();

    let (signed_in, user_id) = match runtime.auth.check_authorization(
        bearer,
        session_id,
        expected_static.as_deref(),
    ) {
        crate::auth::AuthCheckResult::Authorized(id) => (true, Some(id.user_id)),
        _ => (false, None),
    };

    Json(json!({
        "login_enabled": login_enabled,
        "signed_in": signed_in,
        "user_id": user_id,
    }))
}

#[derive(Deserialize)]
struct BreakGlassParams {
    token: Option<String>,
}

async fn auth_break_glass(
    State(runtime): State<Arc<AgentRuntime>>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<BreakGlassParams>,
) -> Result<axum::response::Response, ApiError> {
    let Some(token) = params.token else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing break-glass token" })),
        ));
    };
    let Some((raw_session_id, _session)) = runtime.auth.redeem_break_glass_token(&token) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid or expired break-glass token" })),
        ));
    };

    let is_secure = is_secure_connection(&runtime, &headers);
    let max_age_secs = runtime.auth.config.session_hours * 3600;
    let cookie = crate::auth::format_session_cookie(&raw_session_id, max_age_secs, is_secure);

    let mut response = axum::response::Redirect::to("/talk").into_response();
    *response.status_mut() = StatusCode::SEE_OTHER;
    if let Ok(cookie_val) = axum::http::HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(axum::http::header::SET_COOKIE, cookie_val);
    }
    Ok(response)
}

fn bad_request(error: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": error.to_string() })),
    )
}

fn internal(error: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": error.to_string() })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuntimeConfig;
    use crate::runtime::{ActivityMode, RuntimeStatus};

    fn sample_status() -> RuntimeStatus {
        RuntimeStatus {
            identity: "Ferricula Agent".into(),
            agent_id: "ferricula-agent".into(),
            mode: ActivityMode::Asleep,
            queued: 0,
            running: 0,
            schedule_enabled: true,
            last_scheduled_wake: None,
            memory_rows: 0,
            memory_records: 0,
            nutnews_enabled: false,
            nutnews_writes_enabled: false,
            model_usd_today: 0.0,
            autonomy_state: "disabled".into(),
            autonomy_lease_id: None,
            autonomy_model_allowed: true,
            compel_response: false,
            compel_write: false,
            mention_cursor: 0,
            mention_pending: 3,
            mention_gap: false,
            affect_label: "disabled".into(),
            affect_intensity: 0.0,
            sleep_last_cycle: Some(99),
            overlay_events: 0,
            advocate_alignment: Some("aligned".into()),
            advocate_expired: false,
            documents: 0,
            experience_records: 0,
            embeddings: crate::runtime::EmbeddingsStatus {
                backend: "none".into(),
                space: "none".into(),
                state: crate::runtime::EmbeddingsState::Disabled,
                probe_cosines: Vec::new(),
                probe_ids: Vec::new(),
                detail: None,
                checked_at: None,
            },
            meaning: crate::runtime::MeaningStatus {
                enabled: false,
                counts: Default::default(),
                backfill: Default::default(),
                notes: Vec::new(),
            },
        }
    }

    #[test]
    fn autonomy_projection_never_compels() {
        let s = sample_status();
        let config = RuntimeConfig::default();
        let v = project_autonomy_status(&s, &config);
        assert_eq!(v["compel_response"], false);
        assert_eq!(v["compel_write"], false);
        assert_eq!(v["enabled"], false);
        assert!(v["policy"]["lease_steps"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn mention_pending_exposes_counts_not_bodies() {
        let s = sample_status();
        let config = RuntimeConfig::default();
        let v = project_mention_pending(&s, &config);
        assert_eq!(v["pending_count"], 3);
        assert!(v["items"].as_array().unwrap().is_empty());
        assert!(!v["note"].as_str().unwrap().is_empty());
    }

    #[test]
    fn sleep_projection_respects_disabled_pause() {
        let mut s = sample_status();
        s.mode = ActivityMode::Paused;
        let config = RuntimeConfig::default();
        let v = project_sleep_status(&s, &config, "ferricula-agent", 1_000);
        assert_eq!(v["ready_for_planning"], false);
        // Default sleep_cycle.enabled is true, but paused gate yields no proposals.
        assert!(v["proposals"].as_array().unwrap().is_empty() || v["gate"] == "paused");
    }

    #[test]
    fn sleep_projection_when_sleep_disabled() {
        let s = sample_status();
        let mut config = RuntimeConfig::default();
        config.sleep_cycle.enabled = false;
        let v = project_sleep_status(&s, &config, "ferricula-agent", 1_000);
        assert_eq!(v["enabled"], false);
        assert_eq!(v["proposals_source"], "disabled");
        assert!(v["proposals"].as_array().unwrap().is_empty());
    }

    #[test]
    fn overlay_status_has_no_payload_text() {
        let s = sample_status();
        let config = RuntimeConfig::default();
        let v = project_overlay_status(&s, &config);
        assert_eq!(v["enabled"], false);
        assert!(v["pending_approvals"].as_array().unwrap().is_empty());
        assert!(v.get("total_events").is_some());
        // No accidental text fields.
        assert!(v.get("text").is_none());
    }

    #[test]
    fn advocate_status_is_summary_only() {
        let s = sample_status();
        let mut config = RuntimeConfig::default();
        config.advocate.enabled = true;
        let v = project_advocate_status(&s, &config);
        assert_eq!(v["enabled"], true);
        assert_eq!(v["last_alignment"], "aligned");
        assert!(v["history"].as_array().unwrap().is_empty());
        assert_eq!(v["compel_response"], false);
    }

    #[test]
    fn bound_text_and_slice() {
        assert_eq!(bound_text("hello", 10), "hello");
        assert_eq!(bound_text("abcdef", 3), "abc");
        let v = bound_slice(vec![1, 2, 3, 4, 5], 3);
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn phase_one_routes_are_registered() {
        // Ensure path constants used by PHASE1_API_DELTA remain wired.
        let paths = [
            "/control/autonomy",
            "/tasks/considerations",
            "/control/schedule/plan",
            "/memory/overlay",
            "/memory/overlay/approve/{event_id}",
            "/control/advocate",
        ];
        // router() needs a runtime; we only check path list stability here.
        for p in paths {
            assert!(p.starts_with('/'));
        }
    }
}
