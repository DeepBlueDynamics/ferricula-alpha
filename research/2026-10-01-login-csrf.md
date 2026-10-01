# Investigation: Login CSRF (Session Planting) in Ferricula Server

**Date:** 2026-10-01  
**Base:** `v3/r0` at `dc5a42a`  
**Branch:** `v3/r-login-csrf`  
**Scope:** Architectural investigation of pre-existing login-CSRF / session planting risks across `GET /auth/callback` and `GET /auth/break-glass`. No code changes in this branch.  
**Deliverable:** Technical assessment covering flow mechanics, state/nonce feasibility, mitigation options with tradeoffs, and recommendations.

---

## 1. Flow Mechanics and Vulnerability Analysis

In PR #30 and PR #31, CSRF protection was hardened for cookie-authenticated mutating requests by ensuring safe HTTP methods (`GET`, `HEAD`, `OPTIONS`) remain exempt to allow normal browser navigation (e.g. typing `/settings` or `/talk`), while unsafe methods (`POST`, `PUT`, `PATCH`, `DELETE`) fail closed.

However, Steve flagged a pre-existing **login-CSRF (session planting)** risk: both the OAuth callback and break-glass authentication create an authenticated session and issue a `ferricula_session` cookie via top-level `GET` navigation. Consequently, an attacker holding a valid operator token can force a victim's browser to adopt the attacker's session.

### 1.1 The `/talk` UI Login Trigger
- In `crates/ferricula-server/src/talk.html:27`, the user interface displays the login link:
  ```html
  <a id="login" href="/auth/login" hidden>Sign in with nuts.services</a>
  ```
- In `crates/ferricula-server/src/talk.html:103,147`, client-side script probes `/auth/status`. If `login_enabled` is true and `signed_in` is false, the link is revealed.
- Clicking the link triggers a top-level browser `GET /auth/login` request.

### 1.2 Redirect to nuts-auth (`/auth/login`)
- Handled in `crates/ferricula-server/src/api.rs:1181-1197` (`auth_login`):
  ```rust
  // crates/ferricula-server/src/api.rs:1181-1197
  async fn auth_login(
      State(runtime): State<Arc<AgentRuntime>>,
      headers: HeaderMap,
  ) -> Result<axum::response::Response, ApiError> {
      if !runtime.auth.login_enabled() {
          return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "nuts-auth login is disabled" }))));
      }
      let origin = get_public_base_url(&runtime, &headers);
      let return_url = format!("{origin}/auth/callback");
      let redirect_url = runtime.auth.build_login_url(&return_url);
      let mut response = axum::response::Redirect::to(&redirect_url).into_response();
      *response.status_mut() = StatusCode::SEE_OTHER;
      Ok(response)
  }
  ```
- URL construction occurs in `crates/ferricula-server/src/auth.rs:741-746` (`build_login_url`):
  ```rust
  // crates/ferricula-server/src/auth.rs:741-746
  pub fn build_login_url(&self, return_url: &str) -> String {
      let url = &self.config.login_url;
      let delimiter = if url.contains('?') { '&' } else { '?' };
      let encoded_return = url_encode(return_url);
      format!("{url}{delimiter}return_url={encoded_return}")
  }
  ```
- The server responds with `HTTP 303 See Other` to `https://auth.nuts.services/login?return_url=https%3A%2F%2Fagent.example.com%2Fauth%2Fcallback`.

### 1.3 Callback and Session Creation (`/auth/callback`)
- After authentication at `nuts-auth`, the user's browser is redirected back to the agent via `GET /auth/callback?token={jwt}`.
- Handled in `crates/ferricula-server/src/api.rs:1204-1269` (`auth_callback`):
  - **Parameter extraction (`api.rs:1207-1214`)**: Extracts `token` from `axum::extract::Query(params)`.
  - **JWT Verification (`api.rs:1216-1236`)**: Calls `runtime.auth.verify_jwt(&token)` (`auth.rs:374-411`), validating cryptographic signature against cached JWKS (`auth.rs:388-393`) and token expiration.
  - **Operator Check (`api.rs:1238-1246`)**: Verifies that `claims.user_id` is in `auth.config.operators` (`auth.rs:360-363`).
  - **Session Persistence (`api.rs:1248-1257`)**: Calls `runtime.auth.create_session(...)` (`auth.rs:726-735`), generating a UUIDv4 session ID, hashing it with SHA-256 for on-disk persistence (`state/auth/sessions.json`), and returning the raw token.
  - **Cookie Formatting (`api.rs:1259-1261`)**: Calls `format_session_cookie` (`auth.rs:807-812`):
    ```
    ferricula_session={raw_session_id}; HttpOnly; SameSite=Strict; Path=/; Max-Age={session_hours*3600}{; Secure}
    ```
  - **Redirection (`api.rs:1263-1268`)**: Emits `HTTP 303 See Other` to `/talk` with the `Set-Cookie` header.

### 1.4 Break-Glass Link Generation and Redemption
- **Creation (`crates/ferricula-server/src/auth.rs:748-757`)**:
  ```rust
  // crates/ferricula-server/src/auth.rs:752-756
  let nonce = format!("{}", Uuid::new_v4().simple());
  let expires_at = Instant::now() + Duration::from_secs(300);
  let user_id = self.config.operators[0].clone();
  self.break_glass_tokens.lock().unwrap().insert(nonce.clone(), (user_id, expires_at));
  Ok(format!("{origin}/auth/break-glass?token={nonce}"))
  ```
  Notice that `auth.rs:754` explicitly binds the break-glass link to `self.config.operators[0]`, the senior/primary operator account.
- **Redemption (`crates/ferricula-server/src/api.rs:1320-1348`)**:
  - The operator (or anyone holding the link) visits `GET /auth/break-glass?token={nonce}`.
  - In `crates/ferricula-server/src/auth.rs:759-775` (`redeem_break_glass_token`), the token is removed from memory, a session is created for `self.config.operators[0]`, and the `ferricula_session` cookie is attached to a `303 See Other` redirect to `/talk`.

### 1.5 The Attack Mechanism
1. **The Vulnerability**: Neither `/auth/callback` nor `/auth/break-glass` binds the redemption request to the browser session that initiated it. Because both are `GET` endpoints, they are exempt from CSRF Origin/Referer enforcement.
2. **Attack Scenario (Session Planting)**:
   - Attacker (an allowed operator, or holding a valid operator JWT / break-glass token) generates a valid login link: `https://agent.example.com/auth/callback?token=ATTACKER_VALID_JWT` or `https://agent.example.com/auth/break-glass?token=ATTACKER_BREAK_GLASS_NONCE`.
   - Attacker embeds this link on a third-party webpage (e.g., `<img src="..." />`, hidden iframe, or phished link) visited by the victim.
   - The victim's browser sends a top-level `GET` request to the agent URL.
   - The agent server accepts the valid token, creates a new session belonging to the attacker's operator account (or `operators[0]` in the break-glass case), and sets `ferricula_session` in the victim's browser.
   - The victim is now browsing `/talk` or `/settings` authenticated as the attacker.
3. **Consequences in Ferricula**:
   - In traditional web applications, login CSRF allows the attacker to view private search queries or order history entered by the victim while trapped in the attacker's account.
   - In Ferricula, conversations (`POST /chat`), ingested private documents (`POST /documents`), memories recalled (`POST /memory/recall`), and meditation triggers (`POST /life/meditate`) performed by the victim will be associated with the attacker's `user_id` and logged in the agent's persistent state. When the attacker later inspects the agent history, all victim activity is exposed.
   - **Multi-Operator Privilege Contamination**: For break-glass tokens, because `auth.rs:754` redeems `operators[0]`, planting a break-glass session plants the *senior operator account* into a peer operator's browser. In a multi-operator deployment, this allows a secondary/peer operator to be planted into the primary operator identity, which is slightly wider in impact than merely "already an operator".

---

## 2. Feasibility of a State / Nonce Parameter

### 2.1 Codebase Audit (Current Support)
An audit of `crates/ferricula-server` confirms that **zero state or nonce mechanism exists today**:
1. `crates/ferricula-server/src/api.rs:1199-1202`:
   ```rust
   #[derive(Deserialize)]
   struct AuthCallbackParams {
       token: Option<String>,
   }
   ```
   Only `token` is deserialized from the query string. Any `state`, `nonce`, or other query parameter is ignored.
2. `crates/ferricula-server/src/auth.rs:741-746`:
   `build_login_url` strictly formats `{login_url}?return_url={encoded_return}`. It neither generates nor attaches a `state` parameter.
3. No pre-login cookie (such as `ferricula_oauth_state`) is generated or verified.

### 2.2 Requirements on `nuts-auth`
For a standard OAuth 2.0 / OIDC state parameter (RFC 6749 §4.1.1, §10.12) to function:
1. **Outgoing Request**: The agent server must send `state={nonce}`:
   `https://auth.nuts.services/login?return_url=...&state={random_nonce}`.
2. **Echo Requirement**: `nuts-auth` must store or preserve this `state` value through its authentication workflow and echo it back verbatim on redirect:
   `https://agent.example.com/auth/callback?token={jwt}&state={random_nonce}`.
3. **Alternative (Query Embedding in `return_url`)**:
   If `nuts-auth` does not natively support a `state` parameter but preserves query parameters present in `return_url`:
   - Agent generates `return_url = format!("{origin}/auth/callback?state={nonce}")`.
   - `build_login_url` URL-encodes this entire return URL.
   - If `nuts-auth` appends `token` correctly (`&token={jwt}` rather than blindly prepending `?token={jwt}`), `state` will return in the callback query string without changes to `nuts-auth`.
   - **Caveat**: If `nuts-auth` validates `return_url` against a rigid whitelist that does not allow variable query strings, embedding state in `return_url` will fail at `nuts-auth`.

---

## 3. Mitigation Options with Costs and Tradeoffs

| Option | Description | Implementation Cost | External Dependency | Protects Break-Glass? | UX Impact |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **A. State Cookie + Nonce** | Standard OAuth 2.0 state parameter stored in temporary cookie. | Low (agent-side) | High (requires `nuts-auth` echo) | No (break-glass doesn't use `nuts-auth`) | Completely seamless |
| **B. Interstitial POST Confirmation** | Convert session creation to a POST confirmation step. | Low–Medium | None (100% self-contained) | Yes (protects both flows) | 1 extra click on login |
| **C. Pre-Login Binding Cookie** | Bind session issuance to an anonymous device cookie. | Medium | Low | Yes | Seamless |
| **D. Accept the Risk** | Document threat boundary; rely on operator authorization requirement. | None | None | No | None |

### 3.1 Option A: State Cookie + Nonce (Standard OAuth 2.0 Pattern)
- **Mechanics**:
  1. In `auth_login`, generate a cryptographic random nonce (e.g. `Uuid::new_v4().to_string()`).
  2. Set a short-lived cookie: `ferricula_oauth_state={nonce}; HttpOnly; SameSite=Lax; Path=/auth/callback; Max-Age=300{; Secure}`.
  3. Pass `state={nonce}` to `nuts-auth` (either as `&state={nonce}` or embedded in `return_url`).
  4. In `auth_callback`, deserialize `struct AuthCallbackParams { token: Option<String>, state: Option<String> }`.
  5. Validate that `params.state` matches the `ferricula_oauth_state` cookie. If missing or mismatched, return `403 Forbidden ("invalid oauth state")`.
  6. Clear the `ferricula_oauth_state` cookie upon session issuance.
- **Tradeoffs**:
  - *Pros*: Follows RFC 6749 industry standard; frictionless for legitimate users; no extra user clicks.
  - *Cons*: Strictly dependent on `nuts-auth` echoing `state`; leaves `break-glass` unprotected unless break-glass is redesigned.

### 3.2 Option B: Interstitial POST Confirmation
- **Mechanics**:
  1. `GET /auth/callback?token={jwt}` does **not** create a session or emit `Set-Cookie`.
  2. Instead, it validates the JWT and returns an HTML confirmation page displaying:
     > *"Sign in to Ferricula as Operator: {name} ({email})"*  
     > `[ Confirm Sign In ]`
  3. Clicking the button submits a `POST /auth/session` request carrying the verified token.
  4. Apply the identical pattern to `GET /auth/break-glass?token={nonce}`: renders a confirmation page with a `POST /auth/break-glass` button.
- **Critical Security Implementation Note (Correction to CSRF Layer Assumption)**:
  - It is critical to recognize that **the existing `csrf_middleware` does NOT protect unauthenticated POST requests out of the box**.
  - In `crates/ferricula-server/src/api.rs:115-116`, `csrf_middleware` explicitly gates its check behind:
    ```rust
    if let Some(cookie_session) = crate::auth::extract_session_cookie(req.headers()) { ... }
    ```
  - At login time, the victim has **no session cookie yet**—that absence is the foundational premise of login CSRF.
  - Consequently, an attacker's cross-origin auto-submitting form POST to a new session-creating endpoint would present `cookie_session = None` and would **bypass** the existing `csrf_middleware` check!
  - **Required Hardening**: To make Option B secure against cross-origin auto-submission:
    1. **Direct `verify_csrf` Call in Handlers**: The new session-creating POST handlers (`POST /auth/session`, `POST /auth/break-glass`) must explicitly call `crate::auth::verify_csrf(&headers, runtime.auth.config.trust_proxy, runtime.auth.config.public_url.as_deref())` (`crates/ferricula-server/src/auth.rs:821`, `pub`) directly on every incoming request, regardless of cookie presence.
    2. **Or Middleware Route Expansion**: Alternatively, `csrf_middleware` can be extended to enforce `Origin`/`Referer` validation on explicitly designated unauthenticated session-creation POST routes.
- **Tradeoffs**:
  - *Pros*: Completely independent of `nuts-auth`; solves login CSRF for **both** `nuts-auth` and `break-glass`; zero reliance on third-party parameter echo.
  - *Cons*: Introduces an intermediate confirmation click. Requires explicit CSRF enforcement in the unauthenticated POST handlers.

### 3.3 Option C: Pre-Login Binding Cookie (Anonymous Device Fingerprint)
- **Mechanics**:
  1. Issue an anonymous tracking cookie `ferricula_client_id={uuid}` on first visit to `/talk` or `/status`.
  2. When `/auth/login` is visited, associate the generated login request with `ferricula_client_id` in server memory.
  3. When `/auth/callback` returns, verify that the browser presenting the callback holds the same `ferricula_client_id`.
- **Tradeoffs**:
  - *Pros*: Transparent to users; doesn't strictly depend on `nuts-auth` query echo if server matches by timestamp/client.
  - *Cons*: Fragile across multi-tab logins or browser cookie cleaning; adds in-memory state management.

### 3.4 Option D: Accept the Risk (Documented Threat Boundary)
- **Mechanics**:
  - Maintain the existing implementation without code changes. Document the attack vector and security boundaries in architecture documentation.
- **Threat Model Evaluation**:
  - To exploit this vulnerability, the attacker **must already possess a valid operator token**:
    - Either a signed JWT whose `user_id` is on the agent's explicit `[auth.operators]` allowlist, or
    - An unexpired one-time break-glass token generated on the server host.
  - An attacker who already possesses valid operator credentials already has full operator authority over the agent (can invoke `/chat`, `/control/mode`, mutate settings, or call `/memory` directly via Bearer auth).
  - The *only* marginal capability granted by login CSRF is **session planting**: tricking a different operator into using the attacker's session so the attacker can spy on the victim's prompt history.
  - In single-operator deployments (the primary target for local agent runtimes), the attacker and the victim are the same person; the threat is effectively void.
  - **Multi-Operator Scope**: In multi-operator environments, the risk is slightly wider than merely "already an operator". As established in Section 1.4, `auth.rs:754` binds break-glass links directly to `self.config.operators[0]`. Planting a break-glass session plants the *senior/primary operator account* into a peer operator's browser, enabling privilege escalation / account confusion across operators within the allowlist.

---

## 4. Recommendation and Team Consensus

We recommend a **pragmatic, proportionate, layered approach** (planned enhancement, not an emergency):

1. **Immediate Step: Break-Glass Interstitial (Option B)**:
   - Convert `GET /auth/break-glass` to render an HTML confirmation prompt that submits `POST /auth/break-glass`.
   - *Implementation detail*: The `POST /auth/break-glass` handler must explicitly call `verify_csrf` (`crates/ferricula-server/src/auth.rs:821`, `pub`), because the client does not yet possess a session cookie and `csrf_middleware` only inspects requests with existing session cookies.
   - *Rationale*: Break-glass links are generated locally and do not involve external services. Furthermore, because break-glass redeems `operators[0]` (`auth.rs:754`), securing it eliminates primary-account planting risks across peer operators with zero external dependencies.

2. **Probe `nuts-auth` for State Parameter Support (Option A Probe)**:
   - Probe `https://auth.nuts.services/login` to confirm whether it currently accepts and echoes `&state=...` (or preserves query parameters embedded in `return_url`).
   - If `nuts-auth` echoes `state`: Implement **Option A** using a `SameSite=Lax` state cookie (`ferricula_oauth_state`). This provides standard RFC 6749 compliance with zero user friction.
   - If `nuts-auth` does not echo `state`: Adopt **Option B** (Interstitial POST confirmation with explicit `verify_csrf`) on `/auth/callback`, ensuring the server remains self-reliant.

3. **Status Quo Viability**:
   - Because exploiting this vulnerability requires possessing valid operator credentials on `[auth.operators]`, this is not an unauthenticated remote code execution or privilege escalation risk from an outsider. It is proportionate to schedule this as a planned enhancement rather than an emergency fix.
