//! Authentication and session management for ferricula-server.
//!
//! Supports:
//! - nuts-auth JWT RS256 verification using cached JWKS (offline-capable via state_dir/auth/jwks.json)
//! - `ahp_` long-lived machine token validation (via `/api/validate` with SHA-256 memory cache)
//! - `ferricula_session` cookie session storage (persisted to `state_dir/auth/sessions.json`)
//! - Operator allowlist enforcement based on nuts-auth `user_id` UUID
//! - Break-glass 5-minute local login links for container recovery
//! - Phase A0 coexistence (`mode = "both"`): static operator bearer token OR nuts-auth session/JWT

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use axum::http::{HeaderMap, header};
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::{AuthConfig, AuthMode};

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    for i in 0..left.len().max(right.len()) {
        diff |= usize::from(*left.get(i).unwrap_or(&0) ^ *right.get(i).unwrap_or(&0));
    }
    diff == 0
}

/// JWT Claims from nuts-auth (RS256).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NutsClaims {
    /// Subject (usually operator email)
    pub sub: String,
    /// Stable user ID UUID
    pub user_id: String,
    /// User's display name
    #[serde(default)]
    pub name: Option<String>,
    /// Issued at timestamp
    #[serde(default)]
    pub iat: Option<u64>,
    /// Expiration timestamp (seconds since Unix epoch)
    pub exp: u64,
    /// Issuer (optional in nuts-auth today)
    #[serde(default)]
    pub iss: Option<String>,
    /// Audience (optional in nuts-auth today)
    #[serde(default)]
    pub aud: Option<String>,
    /// Scopes (optional)
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Authenticated identity resulting from verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthIdentity {
    pub user_id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    pub role: String, // "operator" or "reader"
    pub via: String,  // "static", "session", "jwt", "ahp", "break-glass", "none"
}

impl AuthIdentity {
    pub fn static_operator() -> Self {
        Self {
            user_id: "operator".to_string(),
            email: None,
            name: Some("Operator".to_string()),
            actor: None,
            role: "operator".to_string(),
            via: "static".to_string(),
        }
    }

    pub fn unauthenticated() -> Self {
        Self {
            user_id: "anonymous".to_string(),
            email: None,
            name: None,
            actor: None,
            role: "operator".to_string(),
            via: "none".to_string(),
        }
    }

    pub fn is_operator(&self) -> bool {
        self.role == "operator"
    }

    pub fn is_reader(&self) -> bool {
        self.role == "reader"
    }
}

/// Hash a raw session token using SHA-256 for secure storage at rest.
pub fn hash_session_id(session_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Persisted Ferricula session created after successful login.
/// Session IDs are never stored raw at rest; only their SHA-256 hashes are persisted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FerriculaSession {
    pub session_hash: String,
    pub user_id: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    pub via: String,
    pub jwt_exp: u64,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthCheckResult {
    Authorized(AuthIdentity),
    Forbidden(String),    // HTTP 403: valid user, but not on operator allowlist
    Unauthorized(String), // HTTP 401: missing, invalid, or expired credentials
    Unreachable(String),  // HTTP 503: remote nuts-auth service required and unreachable
}

#[derive(Debug)]
pub enum JwtError {
    Expired,
    Unreachable,
    Invalid(String),
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expired => write!(f, "token expired"),
            Self::Unreachable => write!(f, "nuts-auth unreachable"),
            Self::Invalid(msg) => write!(f, "invalid token: {msg}"),
        }
    }
}

impl std::error::Error for JwtError {}

/// Session storage backed by `<state_dir>/auth/sessions.json`.
pub struct SessionStore {
    sessions_path: PathBuf,
    sessions: RwLock<HashMap<String, FerriculaSession>>,
    session_hours: u64,
}

impl SessionStore {
    pub fn open(auth_dir: &Path, session_hours: u64) -> Result<Self> {
        let sessions_path = auth_dir.join("sessions.json");
        let now = now_secs();
        let mut map: HashMap<String, FerriculaSession> = if sessions_path.exists() {
            match fs::read_to_string(&sessions_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        eprintln!("warning: failed to parse {}: {e}", sessions_path.display());
                        HashMap::new()
                    }
                },
                Err(e) => {
                    eprintln!("warning: failed to read {}: {e}", sessions_path.display());
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        let initial_len = map.len();
        map.retain(|_, s| s.expires_at > now);
        if map.len() != initial_len {
            if let Err(e) = Self::write_disk(&sessions_path, &map) {
                eprintln!("error: failed to write sessions to {}: {e}", sessions_path.display());
            }
        }

        Ok(Self {
            sessions_path,
            sessions: RwLock::new(map),
            session_hours,
        })
    }

    fn write_disk(path: &Path, sessions: &HashMap<String, FerriculaSession>) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(sessions)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn create_session(
        &self,
        user_id: &str,
        email: &str,
        name: Option<&str>,
        via: &str,
        jwt_exp: u64,
    ) -> Result<(String, FerriculaSession)> {
        let raw_session_id = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let session_hash = hash_session_id(&raw_session_id);
        let now = now_secs();
        let session = FerriculaSession {
            session_hash: session_hash.clone(),
            user_id: user_id.to_string(),
            email: email.to_string(),
            name: name.map(|s| s.to_string()),
            via: via.to_string(),
            jwt_exp,
            created_at: now,
            expires_at: now + (self.session_hours * 3600),
        };

        let mut lock = self.sessions.write().expect("session lock poisoned");
        lock.insert(session_hash, session.clone());
        if let Err(e) = Self::write_disk(&self.sessions_path, &lock) {
            eprintln!("error: failed to write sessions to {}: {e}", self.sessions_path.display());
        }
        Ok((raw_session_id, session))
    }

    pub fn get_session(&self, raw_session_id: &str) -> Option<FerriculaSession> {
        let session_hash = hash_session_id(raw_session_id);
        let now = now_secs();
        let lock = self.sessions.read().expect("session lock poisoned");
        if let Some(s) = lock.get(&session_hash) {
            if s.expires_at > now {
                return Some(s.clone());
            }
        }
        None
    }

    pub fn delete_session(&self, raw_session_id: &str) -> bool {
        let session_hash = hash_session_id(raw_session_id);
        let mut lock = self.sessions.write().expect("session lock poisoned");
        let removed = lock.remove(&session_hash).is_some();
        if removed {
            if let Err(e) = Self::write_disk(&self.sessions_path, &lock) {
                eprintln!("error: failed to write sessions to {}: {e}", self.sessions_path.display());
            }
        }
        removed
    }

    pub fn len(&self) -> usize {
        let lock = self.sessions.read().expect("session lock poisoned");
        lock.len()
    }
}

/// Central AuthManager managing JWKS cache, JWT verification, sessions, and machine tokens.
pub struct AuthManager {
    pub config: AuthConfig,
    pub state_dir: PathBuf,
    pub auth_dir: PathBuf,
    pub jwks_path: PathBuf,
    pub sessions: SessionStore,
    jwks: RwLock<Option<JwkSet>>,
    last_jwks_fetch: Mutex<Option<Instant>>,
    last_sig_failure: Mutex<Option<Instant>>,
    ahp_cache: Mutex<HashMap<String, (AuthIdentity, Instant)>>,
    break_glass_tokens: Mutex<HashMap<String, (String, Instant)>>,
    http_agent: ureq::Agent,
}

impl AuthManager {
    pub fn new(config: &AuthConfig, state_dir: &Path) -> Result<Arc<Self>> {
        let auth_dir = state_dir.join("auth");
        fs::create_dir_all(&auth_dir)?;
        let jwks_path = auth_dir.join("jwks.json");

        let sessions = SessionStore::open(&auth_dir, config.session_hours)?;

        // Try to load cached JWKS from disk
        let initial_jwks = if jwks_path.exists() {
            match fs::read_to_string(&jwks_path) {
                Ok(content) => match serde_json::from_str::<JwkSet>(&content) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        eprintln!("warning: failed to parse cached JWKS {}: {e}", jwks_path.display());
                        None
                    }
                },
                Err(e) => {
                    eprintln!("warning: failed to read cached JWKS {}: {e}", jwks_path.display());
                    None
                }
            }
        } else {
            None
        };

        let http_agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();

        let manager = Arc::new(Self {
            config: config.clone(),
            state_dir: state_dir.to_path_buf(),
            auth_dir,
            jwks_path,
            sessions,
            jwks: RwLock::new(initial_jwks),
            last_jwks_fetch: Mutex::new(None),
            last_sig_failure: Mutex::new(None),
            ahp_cache: Mutex::new(HashMap::new()),
            break_glass_tokens: Mutex::new(HashMap::new()),
            http_agent,
        });

        // If login is enabled and we have no cached JWKS or want to refresh, attempt fetch
        if manager.login_enabled() && manager.jwks.read().unwrap().is_none() {
            let _ = manager.refresh_jwks();
        }

        Ok(manager)
    }

    pub fn login_enabled(&self) -> bool {
        self.config.login_enabled()
    }

    pub fn is_operator(&self, user_id: &str) -> bool {
        !user_id.is_empty() && self.config.operators.iter().any(|op| op == user_id)
    }

    /// Refresh JWKS from `jwks_url` and persist to `jwks_path`.
    pub fn refresh_jwks(&self) -> Result<()> {
        let resp = self.http_agent
            .get(&self.config.jwks_url)
            .call()
            .with_context(|| format!("failed to fetch JWKS from {}", self.config.jwks_url))?;

        if resp.status() != 200 {
            bail!("JWKS request returned HTTP status {}", resp.status());
        }

        let body = resp.into_string().context("failed to read JWKS response body")?;
        let jwks: JwkSet = serde_json::from_str(&body).context("failed to parse JWKS JSON")?;

        // Save to disk atomically
        let tmp = self.jwks_path.with_extension("tmp");
        fs::write(&tmp, &body)?;
        fs::rename(&tmp, &self.jwks_path)?;

        let mut lock = self.jwks.write().expect("jwks lock poisoned");
        *lock = Some(jwks);
        *self.last_jwks_fetch.lock().unwrap() = Some(Instant::now());

        Ok(())
    }

    fn find_jwk(&self, kid: Option<&str>) -> Option<Jwk> {
        let lock = self.jwks.read().expect("jwks lock poisoned");
        let jwks = lock.as_ref()?;
        if let Some(target_kid) = kid {
            jwks.find(target_kid).cloned()
        } else if jwks.keys.len() == 1 {
            jwks.keys.first().cloned()
        } else {
            None
        }
    }

    /// Verify a nuts-auth RS256 JWT against the cached JWKS (or refresh if unknown key).
    pub fn verify_jwt(&self, token: &str) -> Result<NutsClaims, JwtError> {
        let header = decode_header(token).map_err(|e| JwtError::Invalid(format!("malformed header: {e}")))?;
        if header.alg != Algorithm::RS256 {
            return Err(JwtError::Invalid(format!("unsupported algorithm: {:?}", header.alg)));
        }

        // Try to find key in cached JWKS
        let mut jwk = self.find_jwk(header.kid.as_deref());
        if jwk.is_none() {
            // Rate-limit refetches on unknown key (at most once per 10 seconds)
            let can_fetch = {
                let lock = self.last_jwks_fetch.lock().unwrap();
                match *lock {
                    Some(last) if last.elapsed() < Duration::from_secs(10) => false,
                    _ => true,
                }
            };
            if can_fetch {
                let _ = self.refresh_jwks();
                jwk = self.find_jwk(header.kid.as_deref());
            }
        }

        let jwk = match jwk {
            Some(k) => k,
            None => {
                let has_any_keys = self.jwks.read().unwrap().is_some();
                if !has_any_keys {
                    return Err(JwtError::Unreachable);
                } else {
                    return Err(JwtError::Invalid(format!("unknown key id: {:?}", header.kid)));
                }
            }
        };

        let decoding_key = DecodingKey::from_jwk(&jwk)
            .map_err(|e| JwtError::Invalid(format!("invalid JWK key parameters: {e}")))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.leeway = 60;
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp"]);

        if self.config.require_iss {
            let iss = self.config.expected_iss.as_deref().unwrap_or("https://auth.nuts.services");
            validation.set_issuer(&[iss]);
        }
        if self.config.require_aud {
            let aud = self.config.expected_aud.as_deref().unwrap_or("");
            validation.set_audience(&[aud]);
        }

        match decode::<NutsClaims>(token, &decoding_key, &validation) {
            Ok(token_data) => {
                if token_data.claims.user_id.trim().is_empty() || token_data.claims.sub.trim().is_empty() {
                    return Err(JwtError::Invalid("missing user_id or sub claim".to_string()));
                }
                Ok(token_data.claims)
            }
            Err(e) => {
                if matches!(e.kind(), jsonwebtoken::errors::ErrorKind::ExpiredSignature) {
                    return Err(JwtError::Expired);
                }
                if matches!(e.kind(), jsonwebtoken::errors::ErrorKind::InvalidSignature) {
                    // Refetch JWKS at most once per 10 minutes on signature failure
                    let can_refetch = {
                        let mut lock = self.last_sig_failure.lock().unwrap();
                        match *lock {
                            Some(last) if last.elapsed() < Duration::from_secs(600) => false,
                            _ => {
                                *lock = Some(Instant::now());
                                true
                            }
                        }
                    };
                    if can_refetch && self.refresh_jwks().is_ok() {
                        if let Some(retry_jwk) = self.find_jwk(header.kid.as_deref()) {
                            if let Ok(retry_key) = DecodingKey::from_jwk(&retry_jwk) {
                                if let Ok(retry_data) = decode::<NutsClaims>(token, &retry_key, &validation) {
                                    return Ok(retry_data.claims);
                                }
                            }
                        }
                    }
                    return Err(JwtError::Invalid(format!("signature verification failed: {e}")));
                }
                Err(JwtError::Invalid(format!("token validation failed: {e}")))
            }
        }
    }

    /// Validate an `ahp_` machine token against `/api/validate` with in-memory caching.
    pub fn validate_ahp_token(&self, token: &str) -> AuthCheckResult {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let token_hash = format!("{:x}", hasher.finalize());

        // Check memory cache
        {
            let cache = self.ahp_cache.lock().unwrap();
            if let Some((identity, expires_at)) = cache.get(&token_hash) {
                if Instant::now() < *expires_at {
                    return AuthCheckResult::Authorized(identity.clone());
                }
            }
        }

        // Call remote validate endpoint
        let res = match self.http_agent
            .post(&self.config.validate_url)
            .send_json(serde_json::json!({ "token": token }))
        {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                return AuthCheckResult::Unauthorized(format!("validate returned status {code}"));
            }
            Err(e) => {
                return AuthCheckResult::Unreachable(format!("nuts-auth unreachable: {e}"));
            }
        };

        if res.status() != 200 {
            return AuthCheckResult::Unauthorized("invalid token".to_string());
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct ValidateResp {
            valid: bool,
            #[serde(default)]
            subject: Option<String>,
            #[serde(default)]
            actor: Option<String>,
            #[serde(default)]
            user_uid: Option<String>,
            #[serde(default)]
            token_uid: Option<String>,
        }

        let parsed: ValidateResp = match res.into_json() {
            Ok(p) => p,
            Err(_) => return AuthCheckResult::Unauthorized("invalid validate response".to_string()),
        };

        if !parsed.valid {
            return AuthCheckResult::Unauthorized("invalid token".to_string());
        }

        let user_id = parsed.user_uid.unwrap_or_default();
        let is_operator_user = !user_id.is_empty() && self.is_operator(&user_id);
        let agent_rule = parsed.actor.as_ref().and_then(|act| {
            self.config.agents.iter().find(|a| a.actor == *act)
        });

        let (is_auth, role) = if is_operator_user {
            (true, "operator".to_string())
        } else if let Some(rule) = agent_rule {
            (rule.role == "operator" || rule.role == "reader", rule.role.clone())
        } else {
            (false, String::new())
        };

        if !is_auth {
            return AuthCheckResult::Forbidden("not an operator for this agent".to_string());
        }

        let identity = AuthIdentity {
            user_id,
            email: parsed.subject,
            name: None,
            actor: parsed.actor,
            role,
            via: "ahp".to_string(),
        };

        // Cache positive result
        {
            let mut cache = self.ahp_cache.lock().unwrap();
            let expires_at = Instant::now() + Duration::from_secs(self.config.ahp_cache_minutes * 60);
            cache.insert(token_hash, (identity.clone(), expires_at));
        }

        AuthCheckResult::Authorized(identity)
    }

    /// Cache an AHP token identity directly (useful for tests and fast-path caching).
    pub fn cache_ahp_token(&self, token: &str, identity: AuthIdentity) {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let token_hash = format!("{:x}", hasher.finalize());
        let mut cache = self.ahp_cache.lock().unwrap();
        let expires_at = Instant::now() + Duration::from_secs(self.config.ahp_cache_minutes * 60);
        cache.insert(token_hash, (identity, expires_at));
    }

    /// Primary authorization check for operator routes.
    pub fn check_authorization(
        &self,
        bearer: Option<&str>,
        cookie_session_id: Option<&str>,
        expected_static_token: Option<&str>,
    ) -> AuthCheckResult {
        match self.config.mode {
            AuthMode::Static => {
                let Some(expected) = expected_static_token else {
                    return AuthCheckResult::Unauthorized("operator token not configured".to_string());
                };
                let supplied = bearer.unwrap_or_default();
                if constant_time_eq(expected.as_bytes(), supplied.as_bytes()) {
                    AuthCheckResult::Authorized(AuthIdentity::static_operator())
                } else {
                    AuthCheckResult::Unauthorized("operator authorization required".to_string())
                }
            }
            AuthMode::Both => {
                // 1. Static token match
                if let Some(expected) = expected_static_token {
                    if let Some(supplied) = bearer {
                        if constant_time_eq(expected.as_bytes(), supplied.as_bytes()) {
                            return AuthCheckResult::Authorized(AuthIdentity::static_operator());
                        }
                    }
                }

                // 2. Session cookie match
                if let Some(session_id) = cookie_session_id {
                    if let Some(session) = self.sessions.get_session(session_id) {
                        if self.is_operator(&session.user_id) {
                            return AuthCheckResult::Authorized(AuthIdentity {
                                user_id: session.user_id,
                                email: Some(session.email),
                                name: session.name,
                                actor: None,
                                role: "operator".to_string(),
                                via: session.via,
                            });
                        } else {
                            return AuthCheckResult::Forbidden("not an operator for this agent".to_string());
                        }
                    }
                }

                // 3. Bearer token (JWT or ahp_)
                if let Some(token) = bearer {
                    if token.starts_with("ahp_") {
                        return self.validate_ahp_token(token);
                    } else if token.contains('.') {
                        return self.verify_bearer_jwt(token);
                    }
                }

                AuthCheckResult::Unauthorized("operator authorization required".to_string())
            }
            AuthMode::Nuts => {
                // Static token disabled
                if let Some(session_id) = cookie_session_id {
                    if let Some(session) = self.sessions.get_session(session_id) {
                        if self.is_operator(&session.user_id) {
                            return AuthCheckResult::Authorized(AuthIdentity {
                                user_id: session.user_id,
                                email: Some(session.email),
                                name: session.name,
                                actor: None,
                                role: "operator".to_string(),
                                via: session.via,
                            });
                        } else {
                            return AuthCheckResult::Forbidden("not an operator for this agent".to_string());
                        }
                    }
                }

                if let Some(token) = bearer {
                    if token.starts_with("ahp_") {
                        return self.validate_ahp_token(token);
                    } else if token.contains('.') {
                        return self.verify_bearer_jwt(token);
                    }
                }

                AuthCheckResult::Unauthorized("operator authorization required".to_string())
            }
        }
    }

    fn verify_bearer_jwt(&self, token: &str) -> AuthCheckResult {
        match self.verify_jwt(token) {
            Ok(claims) => {
                if self.is_operator(&claims.user_id) {
                    AuthCheckResult::Authorized(AuthIdentity {
                        user_id: claims.user_id,
                        email: Some(claims.sub),
                        name: claims.name,
                        actor: None,
                        role: "operator".to_string(),
                        via: "jwt".to_string(),
                    })
                } else {
                    AuthCheckResult::Forbidden("not an operator for this agent".to_string())
                }
            }
            Err(JwtError::Expired) => {
                AuthCheckResult::Unauthorized("token expired".to_string())
            }
            Err(JwtError::Unreachable) => {
                AuthCheckResult::Unreachable("nuts-auth unreachable".to_string())
            }
            Err(JwtError::Invalid(err)) => {
                AuthCheckResult::Unauthorized(format!("invalid token: {err}"))
            }
        }
    }

    pub fn create_session(
        &self,
        user_id: &str,
        email: &str,
        name: Option<&str>,
        via: &str,
        jwt_exp: u64,
    ) -> Result<(String, FerriculaSession)> {
        self.sessions.create_session(user_id, email, name, via, jwt_exp)
    }

    pub fn delete_session(&self, raw_session_id: &str) -> bool {
        self.sessions.delete_session(raw_session_id)
    }

    pub fn build_login_url(&self, return_url: &str) -> String {
        let url = &self.config.login_url;
        let delimiter = if url.contains('?') { '&' } else { '?' };
        let encoded_return = url_encode(return_url);
        format!("{url}{delimiter}return_url={encoded_return}")
    }

    pub fn create_break_glass_link(&self, origin: &str) -> Result<String> {
        if self.config.operators.is_empty() {
            bail!("no operators configured in [auth.operators]");
        }
        let nonce = format!("{}", Uuid::new_v4().simple());
        let expires_at = Instant::now() + Duration::from_secs(300);
        let user_id = self.config.operators[0].clone();
        self.break_glass_tokens.lock().unwrap().insert(nonce.clone(), (user_id, expires_at));
        Ok(format!("{origin}/auth/break-glass?token={nonce}"))
    }

    pub fn redeem_break_glass_token(&self, nonce: &str) -> Option<(String, FerriculaSession)> {
        let mut lock = self.break_glass_tokens.lock().unwrap();
        if let Some((user_id, expires_at)) = lock.remove(nonce) {
            if Instant::now() < expires_at {
                let (raw_id, session) = self.sessions.create_session(
                    &user_id,
                    "break-glass@local",
                    Some("Break-Glass Operator"),
                    "break-glass",
                    now_secs() + (self.config.session_hours * 3600),
                ).ok()?;
                eprintln!("SECURITY NOTICE: break-glass operator session created for user_id {user_id}");
                return Some((raw_id, session));
            }
        }
        None
    }
}

pub fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

pub fn extract_session_cookie<'a>(headers: &'a HeaderMap) -> Option<&'a str> {
    for cookie_header in headers.get_all(header::COOKIE) {
        if let Ok(s) = cookie_header.to_str() {
            for part in s.split(';') {
                let trimmed = part.trim();
                if let Some(val) = trimmed.strip_prefix("ferricula_session=") {
                    return Some(val.trim());
                }
            }
        }
    }
    None
}

pub fn format_session_cookie(session_id: &str, max_age_secs: u64, is_secure: bool) -> String {
    format!(
        "ferricula_session={session_id}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age_secs}{}",
        if is_secure { "; Secure" } else { "" }
    )
}

pub fn format_clear_session_cookie(is_secure: bool) -> String {
    format!(
        "ferricula_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT{}",
        if is_secure { "; Secure" } else { "" }
    )
}

pub fn verify_csrf(headers: &HeaderMap) -> Result<(), &'static str> {
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
            return Err("missing host header");
        };
        let origin_host = origin
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default();
        if !origin_host.eq_ignore_ascii_case(host) {
            return Err("cross-origin request forbidden");
        }
    } else if let Some(referer) = headers.get(header::REFERER).and_then(|v| v.to_str().ok()) {
        let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
            return Err("missing host header");
        };
        let referer_host = referer
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default();
        if !referer_host.eq_ignore_ascii_case(host) {
            return Err("cross-origin referer forbidden");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use jsonwebtoken::{EncodingKey, Header};
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};
    use rsa::traits::PublicKeyParts;
    use rsa::RsaPrivateKey;

    fn generate_test_keypair(kid: &str) -> (String, String) {
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
            r#"{{"keys":[{{"kty":"RSA","kid":"{}","use":"sig","alg":"RS256","n":"{}","e":"{}"}}]}}"#,
            kid, n_b64, e_b64
        );

        (priv_pem, jwks_json)
    }

    fn make_test_jwt(priv_pem: &str, user_id: &str, sub: &str, exp: u64, kid: Option<&str>) -> String {
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
        let key = EncodingKey::from_rsa_pem(priv_pem.as_bytes()).unwrap();
        jsonwebtoken::encode(&header, &claims, &key).unwrap()
    }

    #[test]
    fn test_session_store_crud_and_persistence() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionStore::open(temp.path(), 12).unwrap();
        assert_eq!(store.len(), 0);

        let (raw_id, s) = store.create_session("uid-1", "user@test.org", Some("Alice"), "nuts-auth", now_secs() + 1800).unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(s.session_hash, hash_session_id(&raw_id));

        // Ensure raw session ID is never stored on disk
        let disk_content = fs::read_to_string(temp.path().join("sessions.json")).unwrap();
        assert!(!disk_content.contains(&raw_id));
        assert!(disk_content.contains(&s.session_hash));

        let retrieved = store.get_session(&raw_id).unwrap();
        assert_eq!(retrieved.user_id, "uid-1");
        assert_eq!(retrieved.email, "user@test.org");
        assert_eq!(retrieved.name.as_deref(), Some("Alice"));

        // Reopen from disk to verify persistence
        let store2 = SessionStore::open(temp.path(), 12).unwrap();
        assert_eq!(store2.len(), 1);
        assert!(store2.get_session(&raw_id).is_some());

        // Deletion
        assert!(store2.delete_session(&raw_id));
        assert_eq!(store2.len(), 0);
        assert!(store2.get_session(&raw_id).is_none());

        // Reopen after delete
        let store3 = SessionStore::open(temp.path(), 12).unwrap();
        assert_eq!(store3.len(), 0);
    }

    #[test]
    fn test_jwt_verification_against_cached_jwks() {
        let (priv_pem, jwks_json) = generate_test_keypair("nuts-auth-key-1");
        let temp = tempfile::tempdir().unwrap();
        let mut config = AuthConfig::default();
        config.mode = AuthMode::Both;
        config.operators = vec!["kord-uid-123".to_string()];

        // Write test JWKS to cache file on disk
        let auth_dir = temp.path().join("auth");
        fs::create_dir_all(&auth_dir).unwrap();
        fs::write(auth_dir.join("jwks.json"), &jwks_json).unwrap();

        let manager = AuthManager::new(&config, temp.path()).unwrap();

        // 1. Valid operator JWT
        let valid_jwt = make_test_jwt(&priv_pem, "kord-uid-123", "kord@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
        let auth_res = manager.check_authorization(Some(&valid_jwt), None, None);
        assert!(matches!(auth_res, AuthCheckResult::Authorized(ref id) if id.user_id == "kord-uid-123"));

        // 2. Valid JWT but non-operator user -> 403 Forbidden
        let other_jwt = make_test_jwt(&priv_pem, "intruder-uid-999", "intruder@test.org", now_secs() + 1800, Some("nuts-auth-key-1"));
        let auth_res2 = manager.check_authorization(Some(&other_jwt), None, None);
        assert!(matches!(auth_res2, AuthCheckResult::Forbidden(_)));

        // 3. Expired operator JWT -> 401 Unauthorized
        let expired_jwt = make_test_jwt(&priv_pem, "kord-uid-123", "kord@test.org", now_secs() - 100, Some("nuts-auth-key-1"));
        let auth_res3 = manager.check_authorization(Some(&expired_jwt), None, None);
        assert!(matches!(auth_res3, AuthCheckResult::Unauthorized(ref msg) if msg.contains("expired")));

        // 4. Corrupted signature -> 401 Unauthorized
        let mut tampered = valid_jwt.clone();
        tampered.push_str("tamper");
        let auth_res4 = manager.check_authorization(Some(&tampered), None, None);
        assert!(matches!(auth_res4, AuthCheckResult::Unauthorized(_)));
    }

    #[test]
    fn test_mode_both_static_token_and_session() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AuthConfig::default();
        config.mode = AuthMode::Both;
        config.operators = vec!["operator-uuid".to_string()];

        let manager = AuthManager::new(&config, temp.path()).unwrap();

        // 1. Static operator token succeeds
        let res_static = manager.check_authorization(Some("my-secret-token"), None, Some("my-secret-token"));
        assert!(matches!(res_static, AuthCheckResult::Authorized(ref id) if id.via == "static"));

        // 2. Wrong static token fails
        let res_bad = manager.check_authorization(Some("wrong-token"), None, Some("my-secret-token"));
        assert!(matches!(res_bad, AuthCheckResult::Unauthorized(_)));

        // 3. Create session for operator and verify session cookie succeeds
        let (raw_id, _) = manager.create_session("operator-uuid", "op@test.org", None, "nuts-auth", now_secs() + 1800).unwrap();
        let res_cookie = manager.check_authorization(None, Some(&raw_id), Some("my-secret-token"));
        assert!(matches!(res_cookie, AuthCheckResult::Authorized(ref id) if id.user_id == "operator-uuid" && id.via == "nuts-auth"));

        // 4. Session for non-operator returns 403 Forbidden
        let (non_op_raw_id, _) = manager.create_session("guest-uuid", "guest@test.org", None, "nuts-auth", now_secs() + 1800).unwrap();
        let res_non_op = manager.check_authorization(None, Some(&non_op_raw_id), Some("my-secret-token"));
        assert!(matches!(res_non_op, AuthCheckResult::Forbidden(_)));
    }

    #[test]
    fn test_mode_static_rejects_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AuthConfig::default();
        config.mode = AuthMode::Static;
        config.operators = vec!["operator-uuid".to_string()];

        let manager = AuthManager::new(&config, temp.path()).unwrap();
        let (raw_id, _) = manager.create_session("operator-uuid", "op@test.org", None, "nuts-auth", now_secs() + 1800).unwrap();

        // In static mode, session cookie alone is rejected
        let res = manager.check_authorization(None, Some(&raw_id), Some("my-secret-token"));
        assert!(matches!(res, AuthCheckResult::Unauthorized(_)));

        // But static token succeeds
        let res_static = manager.check_authorization(Some("my-secret-token"), None, Some("my-secret-token"));
        assert!(matches!(res_static, AuthCheckResult::Authorized(_)));
    }

    #[test]
    fn test_csrf_origin_verification() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:18875".parse().unwrap());
        headers.insert(header::ORIGIN, "http://127.0.0.1:18875".parse().unwrap());
        assert!(verify_csrf(&headers).is_ok());

        let mut bad_headers = HeaderMap::new();
        bad_headers.insert(header::HOST, "127.0.0.1:18875".parse().unwrap());
        bad_headers.insert(header::ORIGIN, "http://evil.com".parse().unwrap());
        assert!(verify_csrf(&bad_headers).is_err());
    }

    #[test]
    fn test_break_glass_link_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = AuthConfig::default();
        config.operators = vec!["kord-emergency-uid".to_string()];

        let manager = AuthManager::new(&config, temp.path()).unwrap();
        let link = manager.create_break_glass_link("http://127.0.0.1:18875").unwrap();
        assert!(link.starts_with("http://127.0.0.1:18875/auth/break-glass?token="));

        let token = link.split("token=").nth(1).unwrap();
        let (raw_id, session) = manager.redeem_break_glass_token(token).unwrap();
        assert_eq!(session.user_id, "kord-emergency-uid");
        assert_eq!(session.via, "break-glass");
        assert_eq!(session.session_hash, hash_session_id(&raw_id));

        // Second redemption attempt fails (single-use)
        assert!(manager.redeem_break_glass_token(token).is_none());
    }
}
