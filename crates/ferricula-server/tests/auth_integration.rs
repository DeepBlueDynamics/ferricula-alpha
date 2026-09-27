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
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::Value;
use tower::ServiceExt;

use ferricula_core::{DurableEngine, MemoryRecord, Row};
use ferricula_server::api;
use ferricula_server::auth::{NutsClaims, now_secs};
use ferricula_server::config::{AuthConfig, AuthMode, RuntimeConfig};
use ferricula_server::inspect_data_dir;
use ferricula_server::runtime::AgentRuntime;

const TEST_RSA_PRIV_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCRZIBzpYPlAATx
bO1ePQDGyxGrbkikpj11V4K0pAgXsfSMnFlnpFfx8qcLxQbYCdxlmwN0mEKjSfo8
OdemVIdtvXM4Oxr5GD/Srqli0WcXvWKKRovgJ3ww91LeA4fl4uHQvBk4Xoz+r1lo
1E8F8khqjvTe6uSrH0ZcGzS5qF7X6y0csjPCSxTVrstps5Ot0M1leoQ2yjMz4JE8
NDcgTdse2zfcnu2eDZCozTclLogSiBZEZfEWYurEd/yhdHzK9GNXIeYOeXj7gjxu
53hXMHfVU7xf5+k1uMC0P3HfLs0ivnZi1ycEdT2E/VZQ+oWgX72WyIyBQuPwHKuF
44NcM0whAgMBAAECggEADI8O3W51pylKRARxuIsybvmJjDJZigdmFaW6f5oPrORN
SHwU6PP0OM0KCuU5Ax5O8GPkd4TTdMFmIR+p9g8lJ6COvFO2r7+de7hkn0mRSyar
xV/0oycl/iDPAqhxa7UeZmZpE0pviWxiH8D/FndoM/QnSL4nhHSOIlSjSEo37xSb
ES7RXYoHNjWks/ita8bFofvkUEISHrnp4yreADDRQ3OZsV01CFzXgs9MshoLBTIe
ohn6QK+fteJ7cfGC3wAqqEwgQqksTqM6mNooYK22fahWKC+gw/BPNPWo5fXWHa1q
vwXXJErxCTrb9o7Clf1jq8ZDGI85cm7TXx+HEf7cxQKBgQDKp5A2VUKPKq/r2myk
mFr+gxG4NxJlFk0J+MiiwAukZ/zYgQx7QgKQfN3Kv+ym+0jdd7Bp3Jxapva9pjzJ
WKPlFy1rjjMoW+UbVQQ51MzI/mYtIC0NS4WyJtEyZvlnew+wo4anhSagZvabKXwp
5FecI3Qom0swShJTYGnjM9XKuwKBgQC3qi7adBHwhByK7A4G/E7mI6+OLI9+2eC9
QSABrJngx+xieseQc5Bdes95WyO1+9YW1AHkQw05/XqsiMNxwvFoS9ur+7XVRGww
VlCgA8O55vG6dQ5XKd5CdG3Y7IfgxTvMvJ6tzFxvzCJxeCLp4KmWYurNrg8gVPIR
sXxrrUZc0wKBgFOFT7g012Ot0idANDp52DbAyhLED769vC557Ca3Q5UUjm6kcQJz
qB7od3hSNTR0qAkuhPR8SaxK17I9yxuofpOyQ7PqPUdK6FelaEJ1Y5kK0A2VDzxF
fep4eQtuySdO3p6MJrjv9YVyKfy00klppHnjWsJJjmlufbMDL2DGQjx1AoGAa+8V
Tcf8au4YoAONUsmfzFuYZeMGCTQdgNru5kz6uUCESHODJ/7iDi2IE/ddiysOa6f3
3J8S/Mtb9l0BDq/TkslRtUZdW5G0SsvO4dqUgYGY+UylOtAeD8vAakTGrW77b5xB
XD3G7OR4MVq6mdsvjnNfLbRmq0eAYql9RwIzYYUCgYBAaX+M7ztt6gtW2GoYiKRZ
u7+7UDpRC9EJDIEtoFYlTpRWMMGwmko+Kx06048kQPblDDLxlaToQr+uGGa0ULTE
LWPnW1V+fIzCt7H0n/TIfMlfv+2JbqrsS8LoZXjEY2Iq//qdFkKhb/Rwn/wz8w8L
Hai0+5NPEoRUgOqxoUKL8w==
-----END PRIVATE KEY-----"#;

const TEST_JWKS_JSON: &str = r#"{"keys":[{"kty":"RSA","kid":"nuts-auth-key-1","use":"sig","alg":"RS256","n":"kWSAc6WD5QAE8WztXj0AxssRq25IpKY9dVeCtKQIF7H0jJxZZ6RX8fKnC8UG2AncZZsDdJhCo0n6PDnXplSHbb1zODsa-Rg_0q6pYtFnF71iikaL4Cd8MPdS3gOH5eLh0LwZOF6M_q9ZaNRPBfJIao703urkqx9GXBs0uahe1-stHLIzwksU1a7LabOTrdDNZXqENsozM-CRPDQ3IE3bHts33J7tng2QqM03JS6IEogWRGXxFmLqxHf8oXR8yvRjVyHmDnl4-4I8bud4VzB31VO8X-fpNbjAtD9x3y7NIr52YtcnBHU9hP1WUPqFoF-9lsiMgULj8ByrheODXDNMIQ","e":"AQAB"}]}"#;

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
    let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIV_KEY.as_bytes()).unwrap();
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
    fs::write(auth_dir.join("jwks.json"), TEST_JWKS_JSON).unwrap();

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

    let res = app
        .oneshot(
            Request::builder()
                .uri("/auth/login")
                .header(header::HOST, "127.0.0.1:18875")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get(header::LOCATION).unwrap().to_str().unwrap();
    assert!(location.starts_with("https://auth.nuts.services/login?return_url="));
    assert!(location.contains("127.0.0.1%3A18875%2Fauth%2Fcallback") || location.contains("127.0.0.1:18875/auth/callback"));
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
