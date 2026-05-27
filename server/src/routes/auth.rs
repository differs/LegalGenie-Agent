use crate::api::ApiEnvelope;
use crate::context::RequestMeta;
use crate::errors::{AppError, AppResult};
use crate::oplog::{spawn_operation_log, OperationLogNew};
use crate::state::AppState;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm as JwtAlgorithm, DecodingKey, EncodingKey, Header, Validation,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::time::Duration as StdDuration;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/password", put(change_password))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum TokenType {
    Access,
    Refresh,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    roles: Vec<String>,
    token_type: TokenType,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token_version: Option<i64>,
    exp: usize,
    iat: usize,
}

fn jwt_validation() -> Validation {
    let mut v = Validation::new(JwtAlgorithm::HS256);
    v.validate_exp = true;
    // Allow small clock skew.
    v.leeway = 60;
    v
}

fn encode_jwt(secret: &str, claims: &Claims) -> AppResult<String> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::internal(format!("jwt encode failed: {e}")))
}

fn decode_jwt(secret: &str, token: &str) -> AppResult<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &jwt_validation(),
    )
    .map_err(|_| AppError::unauthorized_code(401001, "invalid token"))?;
    Ok(data.claims)
}

fn build_access_claims(
    user_id: Uuid,
    username: &str,
    roles: Vec<String>,
    expire_minutes: i64,
) -> Claims {
    let now = Utc::now();
    let exp = now + Duration::minutes(expire_minutes);
    Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        roles,
        token_type: TokenType::Access,
        refresh_token_version: None,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    }
}

fn build_refresh_claims(user_id: Uuid, expire_days: i64, refresh_token_version: i64) -> Claims {
    let now = Utc::now();
    let exp = now + Duration::days(expire_days);
    Claims {
        sub: user_id.to_string(),
        username: String::new(),
        roles: Vec::new(),
        token_type: TokenType::Refresh,
        refresh_token_version: Some(refresh_token_version),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    }
}

fn hash_password(password: &str) -> AppResult<String> {
    validate_password_strength(password)?;

    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(19_456, 2, 1, None)
        .map_err(|e| AppError::internal(format!("argon2 params init failed: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::internal(format!("password hash failed: {e}")))?;

    Ok(hash.to_string())
}

fn validate_password_strength(password: &str) -> AppResult<()> {
    let p = password;
    if p.len() < 8 {
        return Err(AppError::bad_request_code(
            400105,
            "password too short (min 8 chars)",
        ));
    }

    if !p.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AppError::bad_request_code(
            400105,
            "password too weak (must include uppercase letter)",
        ));
    }
    if !p.chars().any(|c| c.is_ascii_lowercase()) {
        return Err(AppError::bad_request_code(
            400105,
            "password too weak (must include lowercase letter)",
        ));
    }
    if !p.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::bad_request_code(
            400105,
            "password too weak (must include a digit)",
        ));
    }

    let weak = ["123456", "password", "12345678", "qwerty"];
    let lower = p.to_ascii_lowercase();
    if weak.iter().any(|w| *w == lower.as_str()) {
        return Err(AppError::bad_request_code(400105, "password too weak"));
    }

    Ok(())
}

fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(password_hash)
        .map_err(|_| AppError::unauthorized_code(400101, "invalid credentials"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
    #[serde(default)]
    real_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
    #[serde(default)]
    remember_me: bool,
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

#[derive(Debug, Serialize)]
struct UserInfo {
    id: String,
    username: String,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    real_name: Option<String>,
    roles: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LoginResponseData {
    user: UserInfo,
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Serialize)]
struct RefreshResponseData {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: String,
    username: String,
    email: String,
    password_hash: String,
    real_name: Option<String>,
    status: String,
}

async fn fetch_refresh_token_version(state: &AppState, user_id: Uuid) -> AppResult<i64> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT refresh_token_version FROM users WHERE id = ?1 LIMIT 1")
            .bind(user_id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((refresh_token_version,)) = row else {
        return Err(AppError::unauthorized_code(401001, "invalid token"));
    };

    Ok(refresh_token_version)
}

async fn bump_refresh_token_version(state: &AppState, user_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE users
        SET refresh_token_version = refresh_token_version + 1,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?1
        "#,
    )
    .bind(user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(())
}

async fn register(
    State(state): State<AppState>,
    meta: RequestMeta,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<ApiEnvelope<LoginResponseData>>> {
    let username = req.username.trim();
    let email = req.email.trim();
    if username.is_empty() || email.is_empty() {
        return Err(AppError::bad_request("username/email required"));
    }

    let exists: (i64,) =
        sqlx::query_as("SELECT COUNT(1) FROM users WHERE username = ?1 OR email = ?2")
            .bind(username)
            .bind(email)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    if exists.0 > 0 {
        return Err(AppError::conflict("username or email already exists"));
    }

    let user_id = Uuid::new_v4();
    let password_hash = hash_password(&req.password)?;

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, real_name, status) VALUES (?1, ?2, ?3, ?4, ?5, 'active')",
    )
    .bind(user_id.to_string())
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(req.real_name.clone())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    // Default role: host_lawyer
    sqlx::query("INSERT INTO user_roles (id, user_id, role_code) VALUES (?1, ?2, ?3)")
        .bind(Uuid::new_v4().to_string())
        .bind(user_id.to_string())
        .bind("host_lawyer")
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let roles = vec!["host_lawyer".to_string()];

    // Audit log (do not include tokens).
    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user_id.to_string(),
            user_name: username.to_string(),
            case_id: None,
            action: "CREATE".to_string(),
            module: "user".to_string(),
            target_type: "user".to_string(),
            target_id: Some(user_id.to_string()),
            target_title: Some(username.to_string()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": user_id.to_string(),
                "username": username,
                "email": email,
                "real_name": req.real_name.clone(),
                "roles": roles.clone(),
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    let access_claims = build_access_claims(
        user_id,
        username,
        roles.clone(),
        state.config.access_token_expire_minutes,
    );
    let refresh_claims = build_refresh_claims(user_id, state.config.refresh_token_expire_days, 0);
    let access_token = encode_jwt(&state.config.jwt_secret, &access_claims)?;
    let refresh_token = encode_jwt(&state.config.jwt_secret, &refresh_claims)?;

    let data = LoginResponseData {
        user: UserInfo {
            id: user_id.to_string(),
            username: username.to_string(),
            email: email.to_string(),
            real_name: req.real_name,
            roles,
        },
        access_token,
        refresh_token,
        expires_in: state.config.access_token_expire_minutes * 60,
    };

    Ok(Json(ApiEnvelope::ok(data)))
}

async fn login(
    State(state): State<AppState>,
    meta: RequestMeta,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<ApiEnvelope<LoginResponseData>>> {
    let ident = req.username.trim();
    if ident.is_empty() || req.password.is_empty() {
        return Err(AppError::bad_request("username/password required"));
    }

    let ip = meta.ip_address.as_deref().unwrap_or("unknown").to_string();
    let key = format!("login:{}:{}", ip, ident.to_ascii_lowercase());
    let allowed = state
        .rate_limiter
        .check_and_record(&key, 5, StdDuration::from_secs(15 * 60))
        .await;
    if !allowed {
        return Err(AppError::too_many_requests(
            "too many login attempts, please try again later",
        ));
    }

    let user: Option<UserRow> = sqlx::query_as(
        "SELECT id, username, email, password_hash, real_name, status FROM users WHERE username = ?1 OR email = ?1 LIMIT 1",
    )
    .bind(ident)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some(user) = user else {
        // Do not leak which field failed.
        return Err(AppError::unauthorized_code(400101, "invalid credentials"));
    };

    if user.status != "active" {
        return Err(AppError::forbidden_code(400103, "user inactive"));
    }

    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::unauthorized_code(400101, "invalid credentials"));
    }

    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT role_code FROM user_roles WHERE user_id = ?1 ORDER BY role_code",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let user_id = Uuid::parse_str(&user.id).map_err(|_| AppError::internal("corrupt user id"))?;

    let access_claims = build_access_claims(
        user_id,
        &user.username,
        roles.clone(),
        state.config.access_token_expire_minutes,
    );
    let refresh_token_version = fetch_refresh_token_version(&state, user_id).await?;
    let refresh_claims = build_refresh_claims(
        user_id,
        state.config.refresh_token_expire_days,
        refresh_token_version,
    );
    let access_token = encode_jwt(&state.config.jwt_secret, &access_claims)?;
    let refresh_token = encode_jwt(&state.config.jwt_secret, &refresh_claims)?;

    let data = LoginResponseData {
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            real_name: user.real_name,
            roles,
        },
        access_token,
        refresh_token,
        expires_in: state.config.access_token_expire_minutes * 60,
    };

    // A successful login proves the credentials are valid now, so clear the
    // transient failure window for this identity/IP pair.
    state.rate_limiter.reset(&key).await;

    // remember_me is reserved for future: could lengthen refresh token or set cookie.
    let _ = req.remember_me;

    // Audit log (do not include tokens).
    let log_user_id = data.user.id.clone();
    let log_username = data.user.username.clone();
    let log_email = data.user.email.clone();
    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: log_user_id.clone(),
            user_name: log_username.clone(),
            case_id: None,
            action: "LOGIN".to_string(),
            module: "user".to_string(),
            target_type: "user".to_string(),
            target_id: Some(log_user_id.clone()),
            target_title: Some(log_username.clone()),
            old_value: None,
            new_value: Some(serde_json::json!({
                "id": log_user_id,
                "username": log_username,
                "email": log_email,
            })),
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(data)))
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> AppResult<Json<ApiEnvelope<RefreshResponseData>>> {
    let claims = decode_jwt(&state.config.jwt_secret, &req.refresh_token)?;
    if claims.token_type != TokenType::Refresh {
        return Err(AppError::unauthorized_code(401001, "invalid token"));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::unauthorized_code(401001, "invalid token"))?;

    // Verify user still exists and is active.
    let row: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT username, status, refresh_token_version FROM users WHERE id = ?1 LIMIT 1",
    )
    .bind(user_id.to_string())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((username, status, refresh_token_version)) = row else {
        return Err(AppError::unauthorized_code(401001, "invalid token"));
    };
    if status != "active" {
        return Err(AppError::forbidden_code(400103, "user inactive"));
    }
    if claims.refresh_token_version.unwrap_or(0) != refresh_token_version {
        return Err(AppError::unauthorized_code(401001, "invalid token"));
    }

    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT role_code FROM user_roles WHERE user_id = ?1 ORDER BY role_code",
    )
    .bind(user_id.to_string())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let access_claims = build_access_claims(
        user_id,
        &username,
        roles,
        state.config.access_token_expire_minutes,
    );
    let access_token = encode_jwt(&state.config.jwt_secret, &access_claims)?;

    Ok(Json(ApiEnvelope::ok(RefreshResponseData {
        access_token,
        expires_in: state.config.access_token_expire_minutes * 60,
    })))
}

async fn logout(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    bump_refresh_token_version(&state, user.user_id).await?;

    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username.clone(),
            case_id: None,
            action: "LOGOUT".to_string(),
            module: "user".to_string(),
            target_type: "user".to_string(),
            target_id: Some(user.user_id.to_string()),
            target_title: Some(user.username),
            old_value: None,
            new_value: None,
            changed_fields: None,
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );
    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<ApiEnvelope<UserInfo>>> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT email, real_name FROM users WHERE id = ?1 LIMIT 1")
            .bind(user.user_id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((email, real_name)) = row else {
        return Err(AppError::unauthorized_code(401001, "invalid token"));
    };

    Ok(Json(ApiEnvelope::ok(UserInfo {
        id: user.user_id.to_string(),
        username: user.username,
        email,
        real_name,
        roles: user.roles,
    })))
}

async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    meta: RequestMeta,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<Json<ApiEnvelope<serde_json::Value>>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT password_hash FROM users WHERE id = ?1 LIMIT 1")
            .bind(user.user_id.to_string())
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let Some((password_hash,)) = row else {
        return Err(AppError::unauthorized_code(401001, "invalid token"));
    };

    if !verify_password(&req.old_password, &password_hash)? {
        return Err(AppError::unauthorized_code(400101, "invalid credentials"));
    }

    let new_hash = hash_password(&req.new_password)?;
    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = ?1,
            refresh_token_version = refresh_token_version + 1,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?2
        "#,
    )
    .bind(new_hash)
    .bind(user.user_id.to_string())
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    // Audit log: never include password values.
    spawn_operation_log(
        state.pool.clone(),
        OperationLogNew {
            user_id: user.user_id.to_string(),
            user_name: user.username,
            case_id: None,
            action: "UPDATE".to_string(),
            module: "user".to_string(),
            target_type: "user".to_string(),
            target_id: Some(user.user_id.to_string()),
            target_title: None,
            old_value: None,
            new_value: None,
            changed_fields: Some("password".to_string()),
            ip_address: meta.ip_address,
            user_agent: meta.user_agent,
            request_id: Some(meta.request_id),
        },
    );

    Ok(Json(ApiEnvelope::ok(serde_json::json!({}))))
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
    pub roles: Vec<String>,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;

        let claims = decode_jwt(&state.config.jwt_secret, auth_header)?;
        if claims.token_type != TokenType::Access {
            return Err(AppError::unauthorized_code(401001, "invalid token"));
        }

        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::unauthorized_code(401001, "invalid token"))?;

        Ok(Self {
            user_id,
            username: claims.username,
            roles: claims.roles,
        })
    }
}

// Ensure we don't accidentally return a 200 on a handler that forgot to wrap `AppResult`.
#[allow(dead_code)]
fn _assert_auth_router_state(_: Router<AppState>) {}
