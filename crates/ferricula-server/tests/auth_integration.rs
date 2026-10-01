//! Integration test for Phase A0 authentication routes (§3a exit test).
//!
//! Verifies:
//! - GET /auth/status reflects login_enabled, signed_in, and user_id.
//! - GET /auth/login redirects (303) to nuts-auth with return_url when enabled, 400 when disabled.
//! - GET /auth/callback sets ferricula_session cookie and 303 redirects to /talk for allowed operators.
//! - GET /auth/callback returns 403 Forbidden for valid JWT from non-operator.
//! - GET /auth/callback returns 401 Unauthorized for expired or malformed JWT.
//! - POST /auth/logout clears the session cookie and returns {"ok": true}.
//! - Protected routes accept either the static operator token or a valid session cookie (mode = "both").
//! - Cached JWKS verifies JWTs even when nuts-auth is offline.

use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, LazyLock};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde_json::Value;
use tower::ServiceExt;

use ferricula_core::{DurableEngine, MemoryRecord, Row};
use ferricula_server::api;
use ferricula_server::auth::{NutsClaims, now_secs};
use ferricula_server::config::{AuthConfig, AuthMode, RuntimeConfig};
use ferricula_server::inspect_data_dir;
use ferricula_server::runtime::AgentRuntime;

static TEST_KEYPAIR: LazyLock<(String, String)> = LazyLock::new(|| {
    let mut rng = rand::thread_rng();
    let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("failed to generate RSA key");
    let priv_pem = priv_key
        .to_pkcs8_pem(LineEnding::LF)
        .expect("failed to export PKCS#8 PEM")
        .to_string();

    let pub_key = priv_key.to_public_key();
    let n_bytes = pub_key.n().to_bytes_be();
    let e_bytes = pub_key.e().to_bytes_be();
    let n_b64 = URL_SAFE_NO_PAD.encode(&n_bytes);
    let e_b64 = URL_SAFE_NO_PAD.encode(&e_bytes);

    let jwks_json = format!(
        r#"{{"keys":[{{"kty":"RSA","kid":"nuts-auth-key-1","use":"sig","alg":"RS256","n":"{}","e":"{}"}}]}}"#,
        n_b64, e_b64
    );

    (priv_pem, jwks_json)
});

const KORD_USER_ID: &str = "e6a86c62-3bf9-4b82-9017-0599a80b6239";
const OTHER_USER_ID: &str = "00000000-0000-0000-0000-000000000000";
const STATIC_OPERATOR_TOKEN: &str = "test-operator-secret-token-123";

fn make_test_jwt(user_id: &str, sub: &str, exp: u64, kid: Option<&str>) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = kid.map(|s| s.to_string());
    let claims = NutsClaims {
        sub: sub.to_string(),
        user_id: user_id.to_string(),
        name: Some("Test User".to_string()),
        iat: Some(now_secs()),
        exp,
        iss: None,
        aud: None,
        scopes: vec!["read".to_string(), "write".to_string()],
    };
    let key = EncodingKey::from_rsa_pem(TEST_KEYPAIR.0.as_bytes()).unwrap();
    jsonwebtoken::encode(&header, &claims, &key).unwrap()
}

fn setup_test_runtime_custom(
    mode: AuthMode,
    trust_proxy: bool,
    public_url: Option<String>,
) -> (tempfile::TempDir, Arc<AgentRuntime>) {
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

    // Cache test JWKS in state_dir/auth/jwks.json so no network calls are needed
    let auth_dir = state.join("auth");
    fs::create_dir_all(&auth_dir).unwrap();
    fs::write(auth_dir.join("jwks.json"), &TEST_KEYPAIR.1).unwrap();

    {
        let mut engine = DurableEngine::open(&memory).unwrap();
        let row = Row {
            id: 1,
            vector: vec![1.0, 0.0],
            refs: None,
            tags: BTreeMap::from([("text".to_string(), "a memory".to_string())]),
        };
        engine.remember(row, MemoryRecord::new(1)).unwrap();
        engine.checkpoint().unwrap();
    }

    let inspection = inspect_data_dir(&memory).unwrap();
    let mut config = RuntimeConfig::default();
    config.memory_dir = memory;
    config.state_dir = state;
    config.require_operator_auth = true;
    config.operator_token_env = "TEST_FERRICULA_OPERATOR_TOKEN".to_string();
    unsafe {
        std::env::set_var("TEST_FERRICULA_OPERATOR_TOKEN", STATIC_OPERATOR_TOKEN);
    }

    config.auth = AuthConfig {
        mode,
        operators: vec![KORD_USER_ID.to_string()],
        login_url: "https://auth.nuts.services/login".to_string(),
        jwks_url: "https://auth.nuts.services/.well-known/jwks.json".to_string(),
        validate_url: "https://auth.nuts.services/api/validate".to_string(),
        public_url,
        trust_proxy,
        require_iss: false,
        require_aud: false,
        expected_iss: None,
        expected_aud: None,
        session_hours: 12,
        ahp_cache_minutes: 10,
        agents: Vec::new(),
    };

    let runtime = AgentRuntime::open(config, inspection).unwrap();
    (root, runtime)
}

fn setup_test_runtime(mode: AuthMode) -> (tempfile::TempDir, Arc<AgentRuntime>) {
    setup_test_runtime_custom(mode, false, Some("http://127.0.0.1:18875".to_string()))
}

#[tokio::test]
async fn test_auth_status_endpoint() {
    // 1. Mode = Static: login_enabled = false
    let (_dir, runtime_static) = setup_test_runtime(AuthMode::Static);
    let app = api::router(runtime_static);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["login_enabled"], false);
    assert_eq!(json["signed_in"], false);
    assert!(json["user_id"].is_null());

    // 2. Mode = Both: login_enabled = true
    let (_dir2, runtime_both) = setup_test_runtime(AuthMode::Both);
    let app_both = api::router(runtime_both);

    let res2 = app_both
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::OK);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["login_enabled"], true);
    assert_eq!(json2["signed_in"], false);

    // 3. Status with static bearer token -> signed_in = true
    let res3 = app_both
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/status")
                .header(header::AUTHORIZATION, format!("Bearer {STATIC_OPERATOR_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res3.status(), StatusCode::OK);
    let body3 = res3.into_body().collect().await.unwrap().to_bytes();
    let json3: Value = serde_json::from_slice(&body3).unwrap();
    assert_eq!(json3["signed_in"], true);
    assert_eq!(json3["user_id"], "operator");
}

#[tokio::test]
async fn test_auth_login_redirect() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let app = api::router(runtime);

    // Host header is evil.attacker.com, but return_url MUST be built from configured public_url / loopback!
    let res = app
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .header(header::HOST, "evil.attacker.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get(header::LOCATION).unwrap().to_str().unwrap();
    assert!(location.starts_with("https://auth.nuts.services/login?return_url="));
    assert!(location.contains("127.0.0.1%3A18875%2Fauth%2Fcallback") || location.contains("127.0.0.1:18875/auth/callback"));
    // Verify client Host header is never used to determine credential destination
    assert!(!location.contains("evil.attacker.com"));
}

#[tokio::test]
async fn test_auth_login_disabled_when_static() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Static);
    let app = api::router(runtime);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_auth_callback_flow_and_exit_criteria() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let app = api::router(runtime);

    // 1. Missing token -> 400
    let res_missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_missing.status(), StatusCode::BAD_REQUEST);

    // 2. Expired JWT -> 401
    let expired_jwt = make_test_jwt(KORD_USER_ID, "kord@test.org", now_secs() - 600, Some("nuts-auth-key-1"));
    let res_expired = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/auth/callback?token={expired_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_expired.status(), StatusCode::UNAUTHORIZED);

    // 3. Non-operator JWT -> 403 Forbidden
    let other_jwt = make_test_jwt(OTHER_USER_ID, "other@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
    let res_forbidden = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/auth/callback?token={other_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_forbidden.status(), StatusCode::FORBIDDEN);

    // 4. Kord's valid operator JWT -> 303 Redirect to /talk with ferricula_session cookie
    let valid_jwt = make_test_jwt(KORD_USER_ID, "kord@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
    let res_ok = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/auth/callback?token={valid_jwt}"))
                .header(header::HOST, "127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_ok.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        res_ok.headers().get(header::LOCATION).unwrap().to_str().unwrap(),
        "/talk"
    );

    let cookie_header = res_ok.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(cookie_header.contains("ferricula_session="));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));

    // Extract session id from Set-Cookie header
    let session_val = cookie_header
        .split(';')
        .find_map(|p| p.trim().strip_prefix("ferricula_session="))
        .unwrap();

    // Verify session ID is stored hashed on disk, never raw
    let sessions_disk = fs::read_to_string(_dir.path().join("state/auth/sessions.json")).unwrap();
    assert!(!sessions_disk.contains(session_val), "raw session ID must not be stored in sessions.json");
    let session_hash = ferricula_server::auth::hash_session_id(session_val);
    assert!(sessions_disk.contains(&session_hash), "hashed session ID must be stored in sessions.json");

    // 5. Use session cookie on protected route (e.g. GET /status)
    let res_status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::COOKIE, format!("ferricula_session={session_val}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_status.status(), StatusCode::OK);

    // 6. Logout clears cookie
    let res_logout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(header::COOKIE, format!("ferricula_session={session_val}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_logout.status(), StatusCode::OK);
    let clear_cookie = res_logout.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(clear_cookie.contains("ferricula_session="));
    assert!(clear_cookie.contains("Max-Age=0"));

    // 7. After logout, session cookie is invalid -> 401
    let res_status_after = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::COOKIE, format!("ferricula_session={session_val}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_status_after.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_phase_a0_both_modes_accept_operator_token() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let app = api::router(runtime);

    // 1. Unauthenticated -> 401
    let res_unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_unauth.status(), StatusCode::UNAUTHORIZED);

    // 2. Static operator bearer token -> 200 OK
    let res_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::AUTHORIZATION, format!("Bearer {STATIC_OPERATOR_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_auth.status(), StatusCode::OK);

    // 3. Bearer JWT for allowed operator directly on API -> 200 OK
    let valid_jwt = make_test_jwt(KORD_USER_ID, "kord@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
    let res_jwt = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::AUTHORIZATION, format!("Bearer {valid_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_jwt.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_public_url_and_proxy_trust() {
    // 1. With configured HTTPS public_url
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let mut config = runtime.config.clone();
    config.auth.public_url = Some("https://agent.deepblue.example.com".to_string());
    config.auth.trust_proxy = false;
    let inspection = inspect_data_dir(&config.memory_dir).unwrap();
    let runtime_https = AgentRuntime::open(config, inspection).unwrap();
    let app_https = api::router(runtime_https);

    // Login redirects to configured public_url (ignoring Host)
    let res_login = app_https
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .header(header::HOST, "random-host.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_login.status(), StatusCode::SEE_OTHER);
    let location = res_login.headers().get(header::LOCATION).unwrap().to_str().unwrap();
    assert!(location.contains("https%3A%2F%2Fagent.deepblue.example.com%2Fauth%2Fcallback")
        || location.contains("https://agent.deepblue.example.com/auth/callback"));

    // Callback on HTTPS public_url sets Secure cookie
    let valid_jwt = make_test_jwt(KORD_USER_ID, "kord@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
    let res_cb = app_https
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/auth/callback?token={valid_jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_cb.status(), StatusCode::SEE_OTHER);
    let cookie = res_cb.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(cookie.contains("; Secure"));

    // 2. With trust_proxy = true and loopback bind
    let (_dir2, runtime2) = setup_test_runtime(AuthMode::Both);
    let mut config2 = runtime2.config.clone();
    config2.auth.public_url = None;
    config2.auth.trust_proxy = true;
    let inspection2 = inspect_data_dir(&config2.memory_dir).unwrap();
    let runtime_proxy = AgentRuntime::open(config2, inspection2).unwrap();
    let app_proxy = api::router(runtime_proxy);

    let res_proxy_cb = app_proxy
        .oneshot(
            Request::builder()
                .uri(format!("/auth/callback?token={valid_jwt}"))
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let proxy_cookie = res_proxy_cb.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(proxy_cookie.contains("; Secure"));
}

#[tokio::test]
async fn test_reader_role_enforcement_and_operator_unchanged() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let mut config = runtime.config.clone();
    config.auth.agents = vec![
        ferricula_server::config::AgentAuthRule {
            actor: "test-reader-actor".to_string(),
            role: "reader".to_string(),
        },
        ferricula_server::config::AgentAuthRule {
            actor: "test-operator-actor".to_string(),
            role: "operator".to_string(),
        },
    ];
    let inspection = inspect_data_dir(&config.memory_dir).unwrap();
    let runtime = AgentRuntime::open(config, inspection).unwrap();

    let reader_token = "ahp_test_reader_token_xyz";
    let operator_token = "ahp_test_operator_token_abc";

    runtime.auth.cache_ahp_token(
        reader_token,
        ferricula_server::auth::AuthIdentity {
            user_id: "".to_string(),
            email: Some("reader@nuts.services".to_string()),
            name: Some("Reader Agent".to_string()),
            actor: Some("test-reader-actor".to_string()),
            role: "reader".to_string(),
            via: "ahp".to_string(),
            via_cookie: false,
        },
    );

    runtime.auth.cache_ahp_token(
        operator_token,
        ferricula_server::auth::AuthIdentity {
            user_id: "".to_string(),
            email: Some("operator@nuts.services".to_string()),
            name: Some("Operator Agent".to_string()),
            actor: Some("test-operator-actor".to_string()),
            role: "operator".to_string(),
            via: "ahp".to_string(),
            via_cookie: false,
        },
    );

    let app = api::router(runtime);

    // --- 1. Reader tests ---

    // 1a. Reader gets 200 OK on GET /status (read route)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 1b. Reader gets 200 OK on POST /memory/recall (read route)
    let recall_body = serde_json::json!({ "query": "memory" }).to_string();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memory/recall")
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(recall_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 1c. Reader gets 403 Forbidden on POST /chat (mutating route)
    let chat_body = serde_json::json!({
        "request_id": uuid::Uuid::new_v4(),
        "conversation_id": uuid::Uuid::new_v4(),
        "message": "hello agent",
        "reported_origin": "human"
    }).to_string();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/chat")
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(chat_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "read-only role");

    // 1d. Reader gets 403 Forbidden on POST /documents (ingest route)
    let ingest_body = serde_json::json!({
        "kind": "text",
        "text": "Some text to ingest"
    }).to_string();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/documents")
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(ingest_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "read-only role");

    // 1e. Reader gets 403 Forbidden on a life action: POST /life/meditate
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/life/meditate")
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "read-only role");

    // 1f. Reader gets 403 Forbidden on GET /chat/{id}
    let dummy_conv_id = uuid::Uuid::new_v4();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/chat/{dummy_conv_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "read-only role");

    // 1g. Reader gets 403 Forbidden on GET /life
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/life")
                .header(header::AUTHORIZATION, format!("Bearer {reader_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "read-only role");

    // --- 2. Operator tests (operator is unchanged) ---

    // 2a. Operator gets 200 OK on GET /status
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::AUTHORIZATION, format!("Bearer {operator_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2b. Operator gets 200 OK on POST /memory/recall
    let recall_body = serde_json::json!({ "query": "memory" }).to_string();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memory/recall")
                .header(header::AUTHORIZATION, format!("Bearer {operator_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(recall_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2c. Operator gets 200 OK on POST /documents
    let ingest_body = serde_json::json!({
        "kind": "text",
        "text": "Valid document text"
    }).to_string();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/documents")
                .header(header::AUTHORIZATION, format!("Bearer {operator_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(ingest_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2d. Static operator token still works
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .header(header::AUTHORIZATION, format!("Bearer {STATIC_OPERATOR_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2e. Operator gets 200 OK on GET /chat/{id}
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/chat/{dummy_conv_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {operator_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2f. Operator gets 200 OK on GET /life
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/life")
                .header(header::AUTHORIZATION, format!("Bearer {operator_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_session_non_operator_refused_on_write() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let app = api::router(runtime.clone());

    // Create a session for a user who is NOT in the operator allowlist
    let (raw_session_id, _session) = runtime
        .auth
        .create_session(
            OTHER_USER_ID,
            "non-operator@test.org",
            Some("Non Operator"),
            "nuts-auth",
            now_secs() + 3600,
        )
        .unwrap();

    // 1. Attempt POST /documents with the non-operator session cookie -> 403 Forbidden
    let ingest_body = serde_json::json!({
        "kind": "text",
        "text": "Attempted write by non-operator session"
    }).to_string();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/documents")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(ingest_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Attempt POST /chat with the non-operator session cookie -> 403 Forbidden
    let chat_body = serde_json::json!({
        "request_id": uuid::Uuid::new_v4(),
        "conversation_id": uuid::Uuid::new_v4(),
        "message": "hello",
        "reported_origin": "human"
    }).to_string();

    let res_chat = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/chat")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(chat_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_chat.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_csrf_protection_for_cookie_session_and_bearer_exemption() {
    let (_dir, runtime) = setup_test_runtime(AuthMode::Both);
    let app = api::router(runtime.clone());

    // 1. Create a valid operator session cookie (via "nuts-auth")
    let (raw_session_id, session) = runtime
        .auth
        .create_session(
            KORD_USER_ID,
            "kord@test.org",
            Some("Kord"),
            "nuts-auth",
            now_secs() + 3600,
        )
        .unwrap();
    assert_eq!(session.via, "nuts-auth");

    let post_body = serde_json::json!({
        "mode": "awake"
    }).to_string();

    // 1a. Cookie session POST without CSRF (cross-origin Origin: evil.com) -> 403 Forbidden
    let res_evil_origin = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://evil.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_evil_origin.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(res_evil_origin.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["error"], "cross-origin request forbidden");

    // 1b. Cookie session POST with cross-origin Referer -> 403 Forbidden
    let res_evil_referer = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::REFERER, "http://evil.com/talk")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_evil_referer.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(res_evil_referer.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["error"], "cross-origin referer forbidden");

    // 1c. Cookie session POST without Origin and without Referer -> 403 Forbidden ("missing origin or referer")
    let res_missing_both = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_missing_both.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(res_missing_both.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["error"], "missing origin or referer");

    // 1d. trust_proxy is false (default): X-Forwarded-Host is ignored, Host is enforced -> 403 Forbidden
    let res_proxy_ignored = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header("x-forwarded-host", "agent.example.com")
                .header(header::ORIGIN, "https://agent.example.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_proxy_ignored.status(), StatusCode::FORBIDDEN);

    // 2. Cookie session POST with valid CSRF (same-origin Origin) -> 200 OK
    let res_valid_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://127.0.0.1:18875")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_valid_csrf.status(), StatusCode::OK);

    // 3. Break-glass session cookie (via "break-glass") also enforces CSRF
    let link = runtime.auth.create_break_glass_link("http://127.0.0.1:18875").unwrap();
    let token = link.split("token=").nth(1).unwrap();
    let (bg_raw_id, bg_session) = runtime.auth.redeem_break_glass_token(token).unwrap();
    assert_eq!(bg_session.via, "break-glass");

    let res_bg_evil = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={bg_raw_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://evil.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bg_evil.status(), StatusCode::FORBIDDEN);

    let res_bg_ok = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={bg_raw_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://127.0.0.1:18875")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bg_ok.status(), StatusCode::OK);

    // 4. Bearer JWT POST is unaffected (CSRF exempt, even with cross-origin Origin) -> 200 OK
    let operator_jwt = make_test_jwt(KORD_USER_ID, "kord@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
    let res_bearer_jwt = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::AUTHORIZATION, format!("Bearer {operator_jwt}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://evil.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_bearer_jwt.status(), StatusCode::OK);

    // 5. Static Bearer token POST is unaffected (CSRF exempt) -> 200 OK
    let res_static_bearer = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::AUTHORIZATION, format!("Bearer {STATIC_OPERATOR_TOKEN}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://evil.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(post_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_static_bearer.status(), StatusCode::OK);

    // 6. trust_proxy = true: respects X-Forwarded-Host and public_url
    let (_proxy_dir, proxy_runtime) = setup_test_runtime_custom(AuthMode::Both, true, None);
    let proxy_app = api::router(proxy_runtime.clone());

    let (proxy_session_id, _) = proxy_runtime
        .auth
        .create_session(
            KORD_USER_ID,
            "kord@test.org",
            Some("Kord"),
            "nuts-auth",
            now_secs() + 3600,
        )
        .unwrap();

    // 6a. Matching X-Forwarded-Host gives 200 OK
    let res_proxy_ok = proxy_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={proxy_session_id}"))
                .header(header::HOST, "internal:18875")
                .header("x-forwarded-host", "agent.example.com")
                .header(header::ORIGIN, "https://agent.example.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::json!({"mode": "awake"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_proxy_ok.status(), StatusCode::OK);

    // 6b. Mismatching X-Forwarded-Host gives 403 Forbidden
    let res_proxy_mismatch = proxy_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={proxy_session_id}"))
                .header(header::HOST, "internal:18875")
                .header("x-forwarded-host", "agent.example.com")
                .header(header::ORIGIN, "https://evil.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::json!({"mode": "awake"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_proxy_mismatch.status(), StatusCode::FORBIDDEN);

    // 6c. trust_proxy = true with public_url fallback when X-Forwarded-Host absent -> 200 OK
    let (_pub_dir, pub_runtime) = setup_test_runtime_custom(
        AuthMode::Both,
        true,
        Some("https://agent.example.com".to_string()),
    );
    let pub_app = api::router(pub_runtime.clone());
    let (pub_session_id, _) = pub_runtime
        .auth
        .create_session(
            KORD_USER_ID,
            "kord@test.org",
            Some("Kord"),
            "nuts-auth",
            now_secs() + 3600,
        )
        .unwrap();

    let res_pub_url_ok = pub_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={pub_session_id}"))
                .header(header::HOST, "internal:18875")
                .header(header::ORIGIN, "https://agent.example.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::json!({"mode": "awake"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_pub_url_ok.status(), StatusCode::OK);

    // 7. POST /auth/logout CSRF gate
    // 7a. Logout with cross-origin Origin gives 403 Forbidden
    let res_logout_evil = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://evil.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_logout_evil.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(res_logout_evil.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["error"], "cross-origin request forbidden");

    // 7b. Logout with matching same-origin Origin gives 200 OK
    let res_logout_ok = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(header::COOKIE, format!("ferricula_session={raw_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_logout_ok.status(), StatusCode::OK);

    // 8. Safe methods (GET, HEAD, OPTIONS) are exempt from CSRF:
    // Create an active session (since raw_session_id was logged out above)
    let (nav_session_id, _) = runtime
        .auth
        .create_session(
            KORD_USER_ID,
            "kord@test.org",
            Some("Kord"),
            "nuts-auth",
            now_secs() + 3600,
        )
        .unwrap();

    // 8a. GET /settings with cookie session and NO Origin and NO Referer gives 200 OK (page loads on plain navigation)
    let res_get_settings = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/settings")
                .header(header::COOKIE, format!("ferricula_session={nav_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_get_settings.status(), StatusCode::OK);
    let settings_html = axum::body::to_bytes(res_get_settings.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&settings_html).to_lowercase().contains("<!doctype html>"));

    // 8b. GET /life with cookie session and NO Origin and NO Referer gives 200 OK
    let res_get_life = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/life")
                .header(header::COOKIE, format!("ferricula_session={nav_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_get_life.status(), StatusCode::OK);

    // 8c. Plain navigation to GET /talk loads without error
    let res_get_talk = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/talk")
                .header(header::COOKIE, format!("ferricula_session={nav_session_id}"))
                .header(header::HOST, "127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_get_talk.status(), StatusCode::OK);

    // 9. Pin the auth ordering: valid session cookie + junk Bearer + evil Origin on a POST gives 403.
    // The defence depends on check_authorization evaluating the cookie branch before the bearer branch.
    let res_junk_bearer_evil = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/mode")
                .header(header::COOKIE, format!("ferricula_session={nav_session_id}"))
                .header(header::AUTHORIZATION, "Bearer junk-unauthorized-bearer-token")
                .header(header::HOST, "127.0.0.1:18875")
                .header(header::ORIGIN, "http://evil.com")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::json!({"mode": "awake"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_junk_bearer_evil.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(res_junk_bearer_evil.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["error"], "cross-origin request forbidden");

    // 10. Guard the layer: a POST to a sample of routes (including /mcp and /auth/logout)
    // with a cookie and an evil Origin gets 403, verifying that all routes are wrapped by csrf_middleware.
    let sample_routes = [
        "/mcp",
        "/auth/logout",
        "/control/mode",
        "/chat",
        "/documents",
        "/memory/recall",
        "/life/meditate",
        "/settings/jev",
    ];

    for route in sample_routes {
        let res_sampled = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(route)
                    .header(header::COOKIE, format!("ferricula_session={nav_session_id}"))
                    .header(header::HOST, "127.0.0.1:18875")
                    .header(header::ORIGIN, "http://evil.com")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            res_sampled.status(),
            StatusCode::FORBIDDEN,
            "route {route} must be protected by csrf_middleware"
        );
        let bytes = axum::body::to_bytes(res_sampled.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json["error"],
            "cross-origin request forbidden",
            "route {route} must return cross-origin request forbidden"
        );
    }
}
