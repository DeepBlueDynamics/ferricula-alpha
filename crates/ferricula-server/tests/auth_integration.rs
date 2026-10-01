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

fn setup_test_runtime(mode: AuthMode) -> (tempfile::TempDir, Arc<AgentRuntime>) {
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
        public_url: Some("http://127.0.0.1:18875".to_string()),
        trust_proxy: false,
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
}
