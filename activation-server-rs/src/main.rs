use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{ConnectInfo, Form, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, NaiveDate, SecondsFormat, Utc, FixedOffset};
use ed25519_dalek::{Signer, SigningKey};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, SqlitePool};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    cfg: Config,
    signing_key: SigningKey,
    rate_limiter: Arc<SimpleRateLimiter>,
    write_semaphore: Arc<Semaphore>,
}

#[derive(Clone)]
struct Config {
    bind_addr: String,
    database_url: String,
    /// 允许的 app_id 列表（逗号分隔的 `APP_IDS`，或单值 `APP_ID`），与 UClaw 一致：
    /// iClaw、UClaw、if2Ai 等多客户端共用同一激活服务。
    app_ids: Vec<String>,
    admin_username: String,
    admin_password: String,
    key_id: String,
    admin_ui_base: String,
    license_valid_days: i64,
    offline_grace_hours: i64,
    refresh_after_sec: i64,
    req_limit_per_min: u32,
    redeem_limit_per_min: u32,
    refresh_limit_per_min: u32,
    admin_login_limit_per_min: u32,
    max_inflight_writes: u32,
    readiness_min_inflight: u32,
}

impl Config {
    fn from_env() -> Self {
        Self {
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:78789".into()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://./activation.db".into()),
            app_ids: {
                // APP_IDS takes priority (comma-separated), falls back to APP_ID, then defaults
                let raw = env::var("APP_IDS")
                    .or_else(|_| env::var("APP_ID"))
                    .unwrap_or_else(|_| {
                        "com.wt.iClaw,ai.if2.UClaw,ai.if2.if2Ai".into()
                    });
                raw.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            },
            admin_username: env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".into()),
            admin_password: env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "change-me".into()),
            key_id: env::var("SIGNING_KEY_ID").unwrap_or_else(|_| "k1".into()),
            admin_ui_base: env::var("ADMIN_UI_BASE").unwrap_or_else(|_| "/license-api/admin".into()),
            license_valid_days: env::var("LICENSE_VALID_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7),
            offline_grace_hours: env::var("OFFLINE_GRACE_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(72),
            refresh_after_sec: env::var("REFRESH_AFTER_SEC")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86_400),
            req_limit_per_min: env::var("REQ_LIMIT_PER_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            redeem_limit_per_min: env::var("REDEEM_LIMIT_PER_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            refresh_limit_per_min: env::var("REFRESH_LIMIT_PER_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            admin_login_limit_per_min: env::var("ADMIN_LOGIN_LIMIT_PER_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            max_inflight_writes: env::var("MAX_INFLIGHT_WRITES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            readiness_min_inflight: env::var("READINESS_MIN_INFLIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
        }
    }
}

#[derive(Default)]
struct SimpleRateLimiter {
    // key -> (window_start_unix_sec, count)
    buckets: Mutex<HashMap<String, (i64, u32)>>,
}

#[derive(Deserialize)]
struct ActivationRequestBody {
    installation_id: String,
    app_id: String,
    app_version: String,
    build: String,
    platform: String,
}

#[derive(Serialize)]
struct ActivationRequestResponse {
    request_id: String,
    device_request_code: String,
    status: String,
    expires_at: String,
    server_time: String,
}

#[derive(Serialize)]
struct ActivationStatusResponse {
    request_id: String,
    status: String,
    can_redeem: bool,
    server_time: String,
}

#[derive(Deserialize)]
struct RedeemBody {
    request_id: String,
    installation_id: String,
    app_id: String,
    #[serde(rename = "app_version")]
    _app_version: String,
    build: String,
    platform: String,
}

#[derive(Deserialize)]
struct RedeemByCodeBody {
    invite_code: String,
    installation_id: String,
    app_id: String,
    #[serde(rename = "app_version")]
    _app_version: String,
    build: String,
    platform: String,
}

#[derive(Serialize)]
struct RedeemResponse {
    license_jws: String,
    refresh_token: String,
    license_id: String,
    server_time: String,
    refresh_after_sec: i64,
}

#[derive(Deserialize)]
struct RefreshBody {
    refresh_token: String,
    installation_id: String,
    license_id: String,
}

#[derive(Serialize)]
struct RefreshResponse {
    license_jws: String,
    refresh_token: String,
    server_time: String,
    refresh_after_sec: i64,
}

#[derive(Deserialize)]
struct RevokeCheckBody {
    license_id: String,
}

#[derive(Serialize)]
struct RevokeCheckResponse {
    revoked: bool,
    server_time: String,
}

#[derive(Deserialize)]
struct TelemetryEventBody {
    installation_id: Option<String>,
    error_code: Option<String>,
    http_status: Option<i32>,
    retry_count: Option<i32>,
    latency_bucket: Option<String>,
}

#[allow(dead_code)]
#[derive(FromRow)]
struct RequestRow {
    id: String,
    installation_id: String,
    app_id: String,
    app_version: String,
    build: String,
    platform: String,
    status: String,
    expires_at: String,
    invite_id: Option<String>,
}

#[allow(dead_code)]
#[derive(FromRow)]
struct RequestRowWithCode {
    id: String,
    device_request_code: String,
    installation_id: String,
    app_id: String,
    app_version: String,
    build: String,
    platform: String,
    status: String,
    expires_at: String,
    invite_id: Option<String>,
}

#[derive(Serialize)]
struct ErrorResp {
    error: String,
}

#[derive(Deserialize)]
struct AdminDecisionForm {
    note: Option<String>,
}

#[derive(Deserialize)]
struct AdminRevokeForm {
    note: Option<String>,
}

#[derive(Deserialize)]
struct AdminLoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AdminInviteCreateForm {
    installation_id: String,
    max_uses: Option<i64>,
    expires_days: Option<i64>,
}

#[derive(Deserialize)]
struct AdminPasswordChangeForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}

#[derive(Deserialize)]
struct AdminSettingsForm {
    auto_approve_quota: Option<i64>,
    invite_expires_days: Option<i64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let cfg = Config::from_env();
    let signing_key_b64 = env::var("SIGNING_KEY_B64")
        .expect("SIGNING_KEY_B64 is required (base64url/no-pad 32-byte ed25519 secret)");
    let secret = URL_SAFE_NO_PAD
        .decode(signing_key_b64.as_bytes())
        .expect("invalid SIGNING_KEY_B64");
    let secret_arr: [u8; 32] = secret
        .try_into()
        .expect("SIGNING_KEY_B64 must decode to exactly 32 bytes");
    let signing_key = SigningKey::from_bytes(&secret_arr);

    let pool = SqlitePoolOptions::new()
        .max_connections(12)
        .connect(&cfg.database_url)
        .await?;

    init_db(&pool).await?;
    ensure_default_admin_user(&pool, &cfg).await?;
    info!("activation db initialized");

    let state = AppState {
        pool,
        cfg: cfg.clone(),
        signing_key,
        rate_limiter: Arc::new(SimpleRateLimiter::default()),
        write_semaphore: Arc::new(Semaphore::new(cfg.max_inflight_writes as usize)),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/system/readiness", get(readiness))
        .route("/v1/activations/request", post(create_activation_request))
        .route("/v1/activations/request/:id", get(get_activation_request_status))
        .route("/v1/activations/redeem", post(redeem))
        .route("/v1/activations/redeem-by-code", post(redeem_by_code))
        .route("/v1/licenses/refresh", post(refresh_license))
        .route("/v1/licenses/revoke-check", post(revoke_check))
        .route("/v1/telemetry/events", post(telemetry_event))
        .route("/admin/login", get(admin_login_page).post(admin_login_submit))
        .route("/admin/logout", post(admin_logout))
        .route("/admin", get(admin_index))
        .route("/admin/requests", get(admin_requests_page))
        .route("/admin/invites", get(admin_invites_page))
        .route("/admin/licenses", get(admin_licenses_page))
        .route("/admin/audit", get(admin_audit_page))
        .route("/admin/settings", get(admin_settings_page).post(admin_settings_update))
        .route("/admin/invites/create", post(admin_create_invite))
        .route("/admin/password/change", post(admin_change_password))
        .route("/admin/requests/:id/approve", post(admin_approve_request))
        .route("/admin/requests/:id/reject", post(admin_reject_request))
        .route("/admin/licenses/:id/revoke", post(admin_revoke_license))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA journal_mode=WAL;").execute(pool).await?;
    sqlx::query("PRAGMA busy_timeout=5000;").execute(pool).await?;
    sqlx::query("PRAGMA synchronous=NORMAL;")
        .execute(pool)
        .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS activation_requests (
            id TEXT PRIMARY KEY,
            installation_id TEXT NOT NULL,
            app_id TEXT NOT NULL,
            app_version TEXT NOT NULL,
            build TEXT NOT NULL,
            platform TEXT NOT NULL,
            status TEXT NOT NULL,
            device_request_code TEXT NOT NULL,
            decision_note TEXT,
            invite_id TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS invite_codes (
            id TEXT PRIMARY KEY,
            code_hash TEXT NOT NULL,
            bound_installation_id TEXT NOT NULL,
            max_uses INTEGER NOT NULL,
            used_count INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS licenses (
            id TEXT PRIMARY KEY,
            installation_id TEXT NOT NULL,
            tier TEXT NOT NULL,
            status TEXT NOT NULL,
            app_id TEXT NOT NULL,
            min_build TEXT NOT NULL,
            issued_at TEXT NOT NULL,
            exp TEXT NOT NULL,
            offline_grace_exp TEXT NOT NULL,
            last_refresh_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS refresh_tokens (
            id TEXT PRIMARY KEY,
            token_hash TEXT NOT NULL,
            license_id TEXT NOT NULL,
            installation_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            revoked_at TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            result TEXT NOT NULL,
            message TEXT,
            ip TEXT,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS admin_sessions (
            token_hash TEXT PRIMARY KEY,
            username TEXT NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS admin_users (
            username TEXT PRIMARY KEY,
            password_hash TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS global_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS telemetry_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            installation_id TEXT,
            error_code TEXT,
            http_status INTEGER,
            retry_count INTEGER,
            latency_bucket TEXT,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;
    let now = now_rfc3339();
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO global_settings (key, value, updated_at) VALUES ('auto_approve_quota', '0', ?)",
    )
    .bind(&now)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO global_settings (key, value, updated_at) VALUES ('invite_expires_days', '7', ?)",
    )
    .bind(&now)
    .execute(pool)
    .await;
    Ok(())
}

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query_as::<_, (i64,)>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let available = state.write_semaphore.available_permits();
    let capacity = state.cfg.max_inflight_writes as usize;
    let overloaded = available == 0;
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "ok": db_ok,
            "degraded": overloaded || !db_ok,
            "time": now_rfc3339(),
            "capacity": {
                "inflight_available": available,
                "inflight_max": capacity
            }
        })),
    )
}

async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query_as::<_, (i64,)>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let available = state.write_semaphore.available_permits();
    let capacity = state.cfg.max_inflight_writes as usize;
    let min_inflight = state.cfg.readiness_min_inflight as usize;
    let ready = db_ok && available >= min_inflight;
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(serde_json::json!({
            "ready": ready,
            "db_ok": db_ok,
            "time": now_rfc3339(),
            "capacity": {
                "inflight_available": available,
                "inflight_max": capacity,
                "readiness_min_inflight": min_inflight
            },
            "reason": if !db_ok {
                "db_unavailable"
            } else if available < min_inflight {
                "capacity_low"
            } else {
                "ok"
            }
        })),
    )
}

fn too_many_response(message: &str) -> axum::response::Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, "2")],
        Json(serde_json::json!({
            "error": message,
            "code": "rate_limited",
            "retry_after_sec": 2
        })),
    )
        .into_response()
}

fn overloaded_response() -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "2")],
        Json(serde_json::json!({
            "error": "server overloaded, retry later",
            "code": "server_overloaded",
            "retry_after_sec": 2
        })),
    )
        .into_response()
}

fn acquire_write_permit(state: &AppState) -> Result<OwnedSemaphorePermit, axum::response::Response> {
    state
        .write_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| overloaded_response())
}

fn check_rate_limit(state: &AppState, key: String, max_per_min: u32) -> bool {
    let now = Utc::now().timestamp();
    let window_start = now - (now % 60);
    let mut map = state
        .rate_limiter
        .buckets
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (bucket_start, count) = map.entry(key).or_insert((window_start, 0));
    if *bucket_start != window_start {
        *bucket_start = window_start;
        *count = 0;
    }
    if *count >= max_per_min {
        return false;
    }
    *count += 1;
    true
}

async fn get_setting_i64(pool: &SqlitePool, key: &str, default_value: i64) -> i64 {
    sqlx::query_as::<_, (String,)>("SELECT value FROM global_settings WHERE key = ? LIMIT 1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|(v,)| v.parse::<i64>().ok())
        .unwrap_or(default_value)
}

async fn set_setting_i64(pool: &SqlitePool, key: &str, value: i64) {
    let _ = sqlx::query(
        "INSERT INTO global_settings (key, value, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value.to_string())
    .bind(now_rfc3339())
    .execute(pool)
    .await;
}

async fn create_activation_request(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<ActivationRequestBody>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !check_rate_limit(
        &state,
        format!("req:ip:{}", addr.ip()),
        state.cfg.req_limit_per_min,
    ) {
        return too_many_response("request rate limit exceeded");
    }
    if !check_rate_limit(
        &state,
        format!("req:installation:{}", &body.installation_id),
        state.cfg.req_limit_per_min,
    ) {
        return too_many_response("installation request rate limit exceeded");
    }
    if !state.cfg.app_ids.contains(&body.app_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp {
                error: "invalid app_id".into(),
            }),
        )
            .into_response();
    }
    let now = Utc::now();
    let expires_at = now + Duration::hours(24);

    let existing = sqlx::query_as::<_, RequestRowWithCode>(
        r#"
        SELECT id, device_request_code, installation_id, app_id, app_version, build, platform, status, expires_at, invite_id
        FROM activation_requests
        WHERE installation_id = ? AND status = 'pending' AND expires_at > ?
        ORDER BY created_at DESC LIMIT 1
        "#,
    )
    .bind(&body.installation_id)
    .bind(now.to_rfc3339())
    .fetch_optional(&state.pool)
    .await;

    if let Ok(Some(mut row)) = existing {
        // 若已有 pending 请求，检查当前是否满足免审批条件；满足则就地批准并返回 approved
        let auto_approve_quota = get_setting_i64(&state.pool, "auto_approve_quota", 0)
            .await
            .max(0);
        if auto_approve_quota > 0 {
            let approved_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(DISTINCT installation_id) FROM activation_requests WHERE status IN ('approved','redeemed')",
            )
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
            if approved_count < auto_approve_quota {
                let _ = sqlx::query(
                    "UPDATE activation_requests SET status = 'approved', decision_note = 'auto-approved by global setting' WHERE id = ?",
                )
                .bind(&row.id)
                .execute(&state.pool)
                .await;
                row.status = "approved".into();
                write_audit(
                    &state.pool,
                    "device",
                    "request_create_auto_approved",
                    "activation_request",
                    &row.id,
                    "ok",
                    None,
                    Some(addr.ip().to_string()),
                )
                .await;
            }
        }
        return (
            StatusCode::OK,
            Json(ActivationRequestResponse {
                request_id: row.id,
                device_request_code: row.device_request_code,
                status: row.status,
                expires_at: row.expires_at,
                server_time: now_rfc3339(),
            }),
        )
            .into_response();
    }

    let req_id = Uuid::new_v4().to_string();
    let device_code = format!("REQ-{}", random_code(10));
    let auto_approve_quota = get_setting_i64(&state.pool, "auto_approve_quota", 0)
        .await
        .max(0);
    let result = sqlx::query(
        r#"
        WITH quota(v) AS (SELECT ?),
        approved(c) AS (
            SELECT COUNT(DISTINCT installation_id) FROM activation_requests WHERE status IN ('approved','redeemed')
        )
        INSERT INTO activation_requests
            (id, installation_id, app_id, app_version, build, platform, status, device_request_code, decision_note, created_at, expires_at)
        SELECT
            ?, ?, ?, ?, ?, ?,
            CASE
                WHEN (SELECT v FROM quota) > 0 AND (SELECT c FROM approved) < (SELECT v FROM quota) THEN 'approved'
                ELSE 'pending'
            END,
            ?,
            CASE
                WHEN (SELECT v FROM quota) > 0 AND (SELECT c FROM approved) < (SELECT v FROM quota) THEN 'auto-approved by global setting'
                ELSE NULL
            END,
            ?, ?
        "#,
    )
    .bind(auto_approve_quota)
    .bind(&req_id)
    .bind(&body.installation_id)
    .bind(&body.app_id)
    .bind(&body.app_version)
    .bind(&body.build)
    .bind(&body.platform)
    .bind(&device_code)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.pool)
    .await;

    if let Err(e) = result {
        warn!("create request failed: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResp {
                error: "failed to create request".into(),
            }),
        )
            .into_response();
    }

    let request_status = sqlx::query_as::<_, (String,)>(
        "SELECT status FROM activation_requests WHERE id = ? LIMIT 1",
    )
    .bind(&req_id)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    .map(|(s,)| s)
    .unwrap_or_else(|| "pending".into());
    let auto_approved = request_status == "approved";

    write_audit(
        &state.pool,
        "device",
        if auto_approved {
            "request_create_auto_approved"
        } else {
            "request_create"
        },
        "activation_request",
        &req_id,
        "ok",
        None,
        Some(addr.ip().to_string()),
    )
    .await;

    (
        StatusCode::OK,
        Json(ActivationRequestResponse {
            request_id: req_id,
            device_request_code: device_code,
            status: request_status.clone(),
            expires_at: expires_at.to_rfc3339(),
            server_time: now_rfc3339(),
        }),
    )
        .into_response()
}

async fn get_activation_request_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, RequestRow>(
        r#"
        SELECT id, installation_id, app_id, app_version, build, platform, status, expires_at, invite_id
        FROM activation_requests WHERE id = ?
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    match row {
        Ok(Some(r)) => (
            StatusCode::OK,
            Json(ActivationStatusResponse {
                request_id: r.id,
                status: r.status.clone(),
                can_redeem: r.status == "approved",
                server_time: now_rfc3339(),
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResp {
                error: "request not found".into(),
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResp {
                error: "query failed".into(),
            }),
        )
            .into_response(),
    }
}

async fn redeem(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RedeemBody>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !check_rate_limit(
        &state,
        format!("redeem:ip:{}", addr.ip()),
        state.cfg.redeem_limit_per_min,
    ) {
        return too_many_response("redeem rate limit exceeded");
    }
    if !check_rate_limit(
        &state,
        format!("redeem:installation:{}", &body.installation_id),
        state.cfg.redeem_limit_per_min,
    ) {
        return too_many_response("installation redeem rate limit exceeded");
    }
    if !state.cfg.app_ids.contains(&body.app_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp {
                error: "invalid app_id".into(),
            }),
        )
            .into_response();
    }

    let req = sqlx::query_as::<_, RequestRow>(
        r#"
        SELECT id, installation_id, app_id, app_version, build, platform, status, expires_at, invite_id
        FROM activation_requests WHERE id = ?
        "#,
    )
    .bind(&body.request_id)
    .fetch_optional(&state.pool)
    .await;

    let req = match req {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResp {
                    error: "request not found".into(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp {
                    error: "query failed".into(),
                }),
            )
                .into_response()
        }
    };

    if req.status != "approved" || req.installation_id != body.installation_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResp {
                error: "request not approved".into(),
            }),
        )
            .into_response();
    }

    let now = Utc::now();
    let exp = now + Duration::days(state.cfg.license_valid_days);
    let grace = exp + Duration::hours(state.cfg.offline_grace_hours);
    let license_id = format!("lic_{}", random_code(12));
    let refresh_token = format!("rt_{}", random_code(32));
    let refresh_hash = sha256_hex(&refresh_token);

    let jws = sign_license_jws(
        &state,
        &license_id,
        &body.installation_id,
        &body.app_id,
        "beta",
        &body.build,
        now.timestamp(),
        exp.timestamp(),
        grace.timestamp(),
    );

    let tx = state.pool.begin().await;
    let mut tx = match tx {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp {
                    error: "transaction failed".into(),
                }),
            )
                .into_response()
        }
    };

    let invite_id = req.invite_id.unwrap_or_default();
    if !invite_id.is_empty() {
        let _ = sqlx::query(
            "UPDATE invite_codes SET used_count = used_count + 1 WHERE id = ? AND used_count < max_uses",
        )
        .bind(invite_id)
        .execute(&mut *tx)
        .await;
    }

    let _ = sqlx::query(
        r#"
        INSERT INTO licenses
            (id, installation_id, tier, status, app_id, min_build, issued_at, exp, offline_grace_exp, last_refresh_at)
        VALUES (?, ?, 'beta', 'active', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&license_id)
    .bind(&body.installation_id)
    .bind(&body.app_id)
    .bind(&body.build)
    .bind(now.to_rfc3339())
    .bind(exp.to_rfc3339())
    .bind(grace.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&mut *tx)
    .await;

    let _ = sqlx::query(
        r#"
        INSERT INTO refresh_tokens
            (id, token_hash, license_id, installation_id, created_at, expires_at, revoked_at)
        VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(refresh_hash)
    .bind(&license_id)
    .bind(&body.installation_id)
    .bind(now.to_rfc3339())
    .bind((now + Duration::days(30)).to_rfc3339())
    .execute(&mut *tx)
    .await;

    let _ = sqlx::query("UPDATE activation_requests SET status = 'redeemed' WHERE id = ?")
        .bind(&body.request_id)
        .execute(&mut *tx)
        .await;

    if tx.commit().await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResp {
                error: "commit failed".into(),
            }),
        )
            .into_response();
    }

    write_audit(
        &state.pool,
        "device",
        "redeem",
        "license",
        &license_id,
        "ok",
        Some(format!("platform={}", body.platform)),
        Some(addr.ip().to_string()),
    )
    .await;

    (
        StatusCode::OK,
        Json(RedeemResponse {
            license_jws: jws,
            refresh_token,
            license_id,
            server_time: now_rfc3339(),
            refresh_after_sec: state.cfg.refresh_after_sec,
        }),
    )
        .into_response()
}

#[derive(FromRow)]
struct InviteRow {
    id: String,
    max_uses: i64,
    used_count: i64,
    expires_at: Option<String>,
}

async fn redeem_by_code(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RedeemByCodeBody>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !check_rate_limit(
        &state,
        format!("redeem:ip:{}", addr.ip()),
        state.cfg.redeem_limit_per_min,
    ) {
        return too_many_response("redeem rate limit exceeded");
    }
    if !check_rate_limit(
        &state,
        format!("redeem:installation:{}", &body.installation_id),
        state.cfg.redeem_limit_per_min,
    ) {
        return too_many_response("installation redeem rate limit exceeded");
    }
    if !state.cfg.app_ids.contains(&body.app_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp {
                error: "invalid app_id".into(),
            }),
        )
            .into_response();
    }

    let code_hash = sha256_hex(&body.invite_code);
    let now = Utc::now();
    let invite = sqlx::query_as::<_, InviteRow>(
        r#"
        SELECT id, max_uses, used_count, expires_at
        FROM invite_codes
        WHERE code_hash = ? AND bound_installation_id = ?
        "#,
    )
    .bind(&code_hash)
    .bind(&body.installation_id)
    .fetch_optional(&state.pool)
    .await;

    let invite = match invite {
        Ok(Some(i)) => i,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResp {
                    error: "invalid invite code or not bound to this device".into(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp {
                    error: "query failed".into(),
                }),
            )
                .into_response()
        }
    };

    if invite.used_count >= invite.max_uses {
        return (
            StatusCode::GONE,
            Json(ErrorResp {
                error: "invite code already used".into(),
            }),
        )
            .into_response();
    }
    if let Some(ref exp) = invite.expires_at {
        if exp.as_str() < now.to_rfc3339().as_str() {
            return (
                StatusCode::GONE,
                Json(ErrorResp {
                    error: "invite code expired".into(),
                }),
            )
                .into_response();
        }
    }

    let exp = now + Duration::days(state.cfg.license_valid_days);
    let grace = exp + Duration::hours(state.cfg.offline_grace_hours);
    let license_id = format!("lic_{}", random_code(12));
    let refresh_token = format!("rt_{}", random_code(32));
    let refresh_hash = sha256_hex(&refresh_token);

    let jws = sign_license_jws(
        &state,
        &license_id,
        &body.installation_id,
        &body.app_id,
        "beta",
        &body.build,
        now.timestamp(),
        exp.timestamp(),
        grace.timestamp(),
    );

    let tx = state.pool.begin().await;
    let mut tx = match tx {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp {
                    error: "transaction failed".into(),
                }),
            )
                .into_response()
        }
    };

    let _ = sqlx::query(
        "UPDATE invite_codes SET used_count = used_count + 1 WHERE id = ? AND used_count < max_uses",
    )
    .bind(&invite.id)
    .execute(&mut *tx)
    .await;

    let _ = sqlx::query(
        r#"
        INSERT INTO licenses
            (id, installation_id, tier, status, app_id, min_build, issued_at, exp, offline_grace_exp, last_refresh_at)
        VALUES (?, ?, 'beta', 'active', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&license_id)
    .bind(&body.installation_id)
    .bind(&body.app_id)
    .bind(&body.build)
    .bind(now.to_rfc3339())
    .bind(exp.to_rfc3339())
    .bind(grace.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&mut *tx)
    .await;

    let _ = sqlx::query(
        r#"
        INSERT INTO refresh_tokens
            (id, token_hash, license_id, installation_id, created_at, expires_at, revoked_at)
        VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(refresh_hash)
    .bind(&license_id)
    .bind(&body.installation_id)
    .bind(now.to_rfc3339())
    .bind((now + Duration::days(30)).to_rfc3339())
    .execute(&mut *tx)
    .await;

    if tx.commit().await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResp {
                error: "commit failed".into(),
            }),
        )
            .into_response();
    }

    write_audit(
        &state.pool,
        "device",
        "redeem_by_code",
        "license",
        &license_id,
        "ok",
        Some(format!("platform={}, invite_id={}", body.platform, invite.id)),
        Some(addr.ip().to_string()),
    )
    .await;

    (
        StatusCode::OK,
        Json(RedeemResponse {
            license_jws: jws,
            refresh_token,
            license_id,
            server_time: now_rfc3339(),
            refresh_after_sec: state.cfg.refresh_after_sec,
        }),
    )
        .into_response()
}

async fn refresh_license(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RefreshBody>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !check_rate_limit(
        &state,
        format!("refresh:ip:{}", addr.ip()),
        state.cfg.refresh_limit_per_min,
    ) {
        return too_many_response("refresh rate limit exceeded");
    }
    if !check_rate_limit(
        &state,
        format!("refresh:installation:{}", &body.installation_id),
        state.cfg.refresh_limit_per_min,
    ) {
        return too_many_response("installation refresh rate limit exceeded");
    }
    let token_hash = sha256_hex(&body.refresh_token);
    let now = Utc::now();
    let token_row = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT id, license_id, installation_id
        FROM refresh_tokens
        WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > ?
        "#,
    )
    .bind(token_hash)
    .bind(now.to_rfc3339())
    .fetch_optional(&state.pool)
    .await;

    let (token_id, license_id, installation_id) = match token_row {
        Ok(Some(v)) => v,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResp {
                    error: "invalid refresh token".into(),
                }),
            )
                .into_response()
        }
    };

    if license_id != body.license_id || installation_id != body.installation_id {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResp {
                error: "token binding mismatch".into(),
            }),
        )
            .into_response();
    }

    let lic = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, installation_id, status, app_id, min_build FROM licenses WHERE id = ?",
    )
    .bind(&body.license_id)
    .fetch_optional(&state.pool)
    .await;

    let (_, lic_installation_id, lic_status, lic_app_id, min_build) = match lic {
        Ok(Some(v)) => v,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResp {
                    error: "license not found".into(),
                }),
            )
                .into_response()
        }
    };

    if lic_status == "revoked" || lic_installation_id != body.installation_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResp {
                error: "license revoked or installation mismatch".into(),
            }),
        )
            .into_response();
    }

    let exp = now + Duration::days(state.cfg.license_valid_days);
    let grace = exp + Duration::hours(state.cfg.offline_grace_hours);
    let new_refresh_token = format!("rt_{}", random_code(32));
    let new_refresh_hash = sha256_hex(&new_refresh_token);
    let jws = sign_license_jws(
        &state,
        &body.license_id,
        &body.installation_id,
        &lic_app_id,
        "beta",
        &min_build,
        now.timestamp(),
        exp.timestamp(),
        grace.timestamp(),
    );

    let mut tx = match state.pool.begin().await {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp {
                    error: "transaction failed".into(),
                }),
            )
                .into_response()
        }
    };

    let _ = sqlx::query("UPDATE refresh_tokens SET revoked_at = ? WHERE id = ?")
        .bind(now.to_rfc3339())
        .bind(token_id)
        .execute(&mut *tx)
        .await;

    let _ = sqlx::query(
        r#"
        INSERT INTO refresh_tokens (id, token_hash, license_id, installation_id, created_at, expires_at, revoked_at)
        VALUES (?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(new_refresh_hash)
    .bind(&body.license_id)
    .bind(&body.installation_id)
    .bind(now.to_rfc3339())
    .bind((now + Duration::days(30)).to_rfc3339())
    .execute(&mut *tx)
    .await;

    let _ = sqlx::query(
        "UPDATE licenses SET exp = ?, offline_grace_exp = ?, last_refresh_at = ? WHERE id = ?",
    )
    .bind(exp.to_rfc3339())
    .bind(grace.to_rfc3339())
    .bind(now.to_rfc3339())
    .bind(&body.license_id)
    .execute(&mut *tx)
    .await;

    let _ = tx.commit().await;

    write_audit(
        &state.pool,
        "device",
        "refresh",
        "license",
        &body.license_id,
        "ok",
        None,
        Some(addr.ip().to_string()),
    )
    .await;

    (
        StatusCode::OK,
        Json(RefreshResponse {
            license_jws: jws,
            refresh_token: new_refresh_token,
            server_time: now_rfc3339(),
            refresh_after_sec: state.cfg.refresh_after_sec,
        }),
    )
        .into_response()
}

async fn revoke_check(
    State(state): State<AppState>,
    Json(body): Json<RevokeCheckBody>,
) -> impl IntoResponse {
    let status = sqlx::query_as::<_, (String,)>("SELECT status FROM licenses WHERE id = ?")
        .bind(&body.license_id)
        .fetch_optional(&state.pool)
        .await;
    let revoked = matches!(status, Ok(Some((s,))) if s == "revoked");
    (
        StatusCode::OK,
        Json(RevokeCheckResponse {
            revoked,
            server_time: now_rfc3339(),
        }),
    )
}

async fn telemetry_event(
    State(state): State<AppState>,
    Json(body): Json<TelemetryEventBody>,
) -> impl IntoResponse {
    let inst = body.installation_id.as_deref().unwrap_or("");
    let err = body.error_code.as_deref().unwrap_or("");
    let http = body.http_status.unwrap_or(-1);
    let retry = body.retry_count.unwrap_or(0);
    let bucket = body.latency_bucket.as_deref().unwrap_or("");
    let now = now_rfc3339();
    let _ = sqlx::query(
        "INSERT INTO telemetry_events (installation_id, error_code, http_status, retry_count, latency_bucket, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(inst)
    .bind(err)
    .bind(http)
    .bind(retry)
    .bind(bucket)
    .bind(&now)
    .execute(&state.pool)
    .await;
    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

async fn admin_login_page(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let msg = params.get("msg").map(|v| match v.as_str() {
        "pwd_updated" => "密码已更新，请使用新密码登录",
        _ => "",
    }).filter(|m| !m.is_empty());
    Html(render_admin_login(msg, &state.cfg.admin_ui_base))
}

async fn admin_login_submit(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(form): Form<AdminLoginForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !check_rate_limit(
        &state,
        format!("admin_login:ip:{}", addr.ip()),
        state.cfg.admin_login_limit_per_min,
    ) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Html(render_admin_login(
                Some("登录请求过于频繁，请稍后再试"),
                &state.cfg.admin_ui_base,
            )),
        )
            .into_response();
    }
    if !verify_admin_credentials(&state.pool, &form.username, &form.password).await {
        return Html(render_admin_login(Some("用户名或密码错误"), &state.cfg.admin_ui_base))
            .into_response();
    }
    let token = format!("as_{}", random_code(48));
    let token_hash = sha256_hex(&token);
    let now = Utc::now();
    let expires_at = now + Duration::hours(12);

    let _ = sqlx::query(
        "INSERT INTO admin_sessions (token_hash, username, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(token_hash)
    .bind(&form.username)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.pool)
    .await;

    write_audit(
        &state.pool,
        "admin",
        "admin_login",
        "admin_session",
        &form.username,
        "ok",
        None,
        Some(addr.ip().to_string()),
    )
    .await;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::LOCATION,
        HeaderValue::from_str(&state.cfg.admin_ui_base)
            .unwrap_or_else(|_| HeaderValue::from_static("/license-api/admin")),
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "iclaw_admin_session={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=43200"
        ))
        .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    (StatusCode::SEE_OTHER, headers).into_response()
}

async fn admin_logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(token) = get_cookie_value(&headers, "iclaw_admin_session") {
        let _ = sqlx::query("DELETE FROM admin_sessions WHERE token_hash = ?")
            .bind(sha256_hex(&token))
            .execute(&state.pool)
            .await;
    }
    let mut out = HeaderMap::new();
    out.insert(
        header::LOCATION,
        HeaderValue::from_str(&format!("{}/login", state.cfg.admin_ui_base))
            .unwrap_or_else(|_| HeaderValue::from_static("/license-api/admin/login")),
    );
    out.insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "iclaw_admin_session=deleted; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0",
        ),
    );
    (StatusCode::SEE_OTHER, out).into_response()
}

async fn admin_index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }

    let pending = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        r#"
        SELECT id, device_request_code, installation_id, app_version, platform, created_at
        FROM activation_requests
        WHERE status = 'pending'
        ORDER BY created_at DESC
        LIMIT 80
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let invites = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>)>(
        r#"
        SELECT id, bound_installation_id, used_count, max_uses, created_at, expires_at
        FROM invite_codes
        ORDER BY created_at DESC
        LIMIT 80
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let recent_licenses = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, installation_id, status, exp FROM licenses ORDER BY issued_at DESC LIMIT 80",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let active_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM licenses WHERE status = 'active'",
    )
    .fetch_one(&state.pool)
    .await
    .map(|v| v.0)
    .unwrap_or(0);
    let auto_approve_quota = get_setting_i64(&state.pool, "auto_approve_quota", 0).await.max(0);
    let invite_expires_days = get_setting_i64(&state.pool, "invite_expires_days", 7)
        .await
        .clamp(1, 90);

    let revoked_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM licenses WHERE status = 'revoked'",
    )
    .fetch_one(&state.pool)
    .await
    .map(|v| v.0)
    .unwrap_or(0);
    let db_ok = sqlx::query_as::<_, (i64,)>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let inflight_available = state.write_semaphore.available_permits();
    let inflight_max = state.cfg.max_inflight_writes as usize;
    let inflight_used = inflight_max.saturating_sub(inflight_available);
    let inflight_ratio = if inflight_max == 0 {
        0.0
    } else {
        (inflight_used as f64 / inflight_max as f64) * 100.0
    };
    let (service_status_text, service_status_bg, service_status_fg) = if !db_ok {
        ("异常", "#fef2f2", "#b91c1c")
    } else if inflight_available == 0 {
        ("高负载", "#fff7ed", "#b45309")
    } else {
        ("运行中", "#ecfdf3", "#047857")
    };

    let range_days = params
        .get("range")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| [7, 14, 30].contains(v))
        .unwrap_or(14);
    let since_day = (Utc::now() - Duration::days(range_days - 1))
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let trend_rows = sqlx::query_as::<_, (String, i64, i64, i64, i64)>(
        r#"
        SELECT
            substr(created_at, 1, 10) AS day,
            SUM(CASE WHEN action = 'request_create' THEN 1 ELSE 0 END) AS requests,
            SUM(CASE WHEN action = 'approve_request' THEN 1 ELSE 0 END) AS approved,
            SUM(CASE WHEN action = 'redeem' THEN 1 ELSE 0 END) AS redeemed,
            SUM(CASE WHEN action = 'revoke_license' THEN 1 ELSE 0 END) AS revoked
        FROM audit_logs
        WHERE substr(created_at, 1, 10) >= ?
        GROUP BY substr(created_at, 1, 10)
        ORDER BY day ASC
        "#,
    )
    .bind(&since_day)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let mut trend_map: HashMap<String, (i64, i64, i64, i64)> = HashMap::new();
    for (day, requests, approved, redeemed, revoked) in trend_rows {
        trend_map.insert(day, (requests, approved, redeemed, revoked));
    }
    let mut trend_days: Vec<(String, i64, i64, i64, i64)> = Vec::new();
    for i in 0..range_days {
        let d = (Utc::now() - Duration::days(range_days - 1 - i))
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let (r1, r2, r3, r4) = trend_map.get(&d).copied().unwrap_or((0, 0, 0, 0));
        trend_days.push((d, r1, r2, r3, r4));
    }
    let trend_max = trend_days
        .iter()
        .map(|(_, a, b, c, d)| *a + *b + *c + *d)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut html = String::new();
    html.push_str(
        "<!doctype html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width, initial-scale=1'><title>iClaw Console Admin</title><link rel='icon' type='image/png' href='/iclaw-logo.png'>",
    );
    html.push_str("<style>:root{--accent:#ff9500;--accent-soft:#ffe9cc;--bg:#f5f5f7;--surface:#ffffff;--surface-soft:#fafafa;--border:#e5e7eb;--text:#111827;--text-muted:#6b7280;}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font-family:-apple-system,BlinkMacSystemFont,'SF Pro Text','PingFang SC','Helvetica Neue',sans-serif;-webkit-font-smoothing:antialiased;text-rendering:optimizeLegibility}a{color:inherit;text-decoration:none}.shell{max-width:1440px;margin:16px auto;padding:0 16px}.topbar{display:flex;align-items:center;justify-content:space-between;background:var(--surface);border:1px solid var(--border);border-radius:14px;padding:12px 14px}.brand{display:flex;align-items:center;gap:10px}.brand img{width:30px;height:30px;border-radius:9px;border:1px solid var(--border)}.brand h1{font-size:16px;line-height:1.2;margin:0}.brand .sub{font-size:11px;color:var(--text-muted);margin-top:2px}.menu{display:flex;gap:14px;font-size:12.5px;color:#4b5563}.menu .active{color:#111827;font-weight:600}.topbar-version{font-size:11px;color:var(--text-muted);padding:0 12px;margin-right:4px}.btn{display:inline-flex;align-items:center;justify-content:center;gap:4px;padding:7px 12px;border:1px solid #d8dde6;border-radius:10px;background:linear-gradient(180deg,#fff,#f7f8fa);cursor:pointer;font-weight:600;font-size:12px;color:#1f2937;box-shadow:0 1px 0 rgba(17,24,39,.04);transition:all .16s ease}.btn:hover{border-color:#c7cedb;background:linear-gradient(180deg,#fff,#f2f4f7);transform:translateY(-1px)}.btn-primary{background:linear-gradient(180deg,#ffb038,#ff9500);border-color:#e68a00;color:#111}.btn-primary:hover{background:linear-gradient(180deg,#ffb84f,#ff9f1a)}.btn-danger{background:linear-gradient(180deg,#fff4f5,#ffe9ec);border-color:#fecdd3;color:#b91c1c}.layout{display:grid;grid-template-columns:220px 1fr;gap:14px;margin-top:14px}.sidebar,.content{background:var(--surface);border:1px solid var(--border);border-radius:14px}.sidebar{padding:12px}.content{padding:12px}.section-title{font-size:11px;color:var(--text-muted);margin:8px 0 10px;text-transform:uppercase;letter-spacing:.04em}.nav-item{display:block;padding:8px 10px;border-radius:9px;font-size:12px;color:#374151;margin-bottom:6px}.nav-item.active{background:#111827;color:#fff}.metric{display:flex;justify-content:space-between;align-items:baseline;padding:9px 2px;border-bottom:1px solid var(--border);font-size:11px;color:var(--text-muted)}.metric:last-child{border-bottom:0}.metric .v{font-size:11px;font-weight:500;color:var(--text-muted)}.alert{background:var(--accent-soft);border:1px solid #f8d7a6;padding:9px 10px;border-radius:9px;font-size:12px;margin-bottom:10px}.main-grid{display:grid;grid-template-columns:1fr 250px;gap:12px}.chart-card,.stats-card,.table-card,.form-card{background:var(--surface-soft);border:1px solid var(--border);border-radius:12px;padding:11px}.chart-title{font-size:22px;font-weight:700;margin:0 0 4px}.chart-sub{font-size:11px;color:var(--text-muted)}.trend-bars{margin-top:8px;border:1px solid var(--border);border-radius:9px;padding:7px;background:#fff}.trend-row{display:grid;grid-template-columns:68px 1fr 34px;gap:7px;align-items:center;padding:3px 0}.trend-day{font-size:10.5px;color:#6b7280}.trend-track{height:10px;background:#f3f4f6;border-radius:999px;overflow:hidden;display:flex}.seg{height:100%}.s1{background:#dbeafe}.s2{background:#fef3c7}.s3{background:#d1fae5}.s4{background:#fee2e2}.trend-total{font-size:10.5px;color:#6b7280;text-align:right}.legend{display:flex;gap:10px;flex-wrap:wrap;margin-top:7px;font-size:10.5px;color:#6b7280}.dot{display:inline-block;width:9px;height:9px;border-radius:999px;margin-right:4px;vertical-align:middle}.stats-card h3,.form-card h3,.table-card h3{margin:0 0 8px;font-size:13px}.stat-line{display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid var(--border);font-size:12px}.stat-line:last-child{border-bottom:0}.grid-2{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-top:10px}.stack form{display:inline-block;margin-right:6px;margin-bottom:4px}.field{font-size:11px;color:var(--text-muted);margin-top:6px}.field input{width:100%;padding:8px 9px;border:1px solid var(--border);border-radius:9px;background:#fff;font-size:12px}.line-form{display:grid;grid-template-columns:1fr 80px 90px auto;gap:8px;align-items:end}.line-form .field{margin:0}.line-form input{width:100%}table{width:100%;border-collapse:collapse;font-size:12px}th,td{border-bottom:1px solid var(--border);padding:7px;text-align:left;vertical-align:top}th{color:#6b7280;font-weight:600;font-size:11.5px}code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:#374151}.time-text{font-family:inherit;font-size:inherit;font-weight:inherit;color:inherit;letter-spacing:0}.focus-tip{margin-top:10px;padding:10px;border:1px dashed #f3be74;border-radius:10px;background:#fff8ed;font-size:12px;color:#7c4b00}@media (max-width:1120px){.layout{grid-template-columns:1fr}.main-grid{grid-template-columns:1fr}.line-form{grid-template-columns:1fr 1fr}.menu{display:none}}</style></head><body>");
    html.push_str("<div class='shell'>");
    html.push_str(&format!(
        "<div class='topbar'><div class='brand'><img src='/iclaw-logo.png' alt='iClaw logo'><div><h1>iClaw Console</h1><div class='sub'>Activation Mission Control</div></div></div><div class='menu'><a class='active' href='{}/'>总览</a><a href='{}/requests'>申请</a><a href='{}/invites'>邀请码</a><a href='{}/licenses'>授权</a><a href='{}/audit'>审计</a><a href='{}/settings'>设置</a></div><span class='topbar-version'>v{}</span><form method='post' action='{}/logout'><button class='btn'>退出登录</button></form></div>",
        env!("CARGO_PKG_VERSION"),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base)
    ));
    html.push_str("<div class='layout'>");
    html.push_str("<aside class='sidebar'>");
    html.push_str(&format!(
        "<div class='section-title'>导航</div><a class='nav-item active' href='{0}'>激活总览</a><a class='nav-item' href='{0}/requests'>申请审批</a><a class='nav-item' href='{0}/invites'>邀请码管理</a><a class='nav-item' href='{0}/licenses'>授权吊销</a><a class='nav-item' href='{0}/audit'>审计与安全</a><a class='nav-item' href='{0}/settings'>全局设置</a>",
        escape_html(&state.cfg.admin_ui_base)
    ));
    html.push_str("<div class='section-title' style='margin-top:14px'>关键指标</div>");
    html.push_str(&format!("<div class='metric'><span>待审批申请</span><span class='v'>{}</span></div>", pending.len()));
    html.push_str(&format!("<div class='metric'><span>有效授权</span><span class='v'>{}</span></div>", active_count));
    html.push_str(&format!("<div class='metric'><span>已吊销授权</span><span class='v'>{}</span></div>", revoked_count));
    html.push_str(&format!("<div class='metric'><span>自动审批配额</span><span class='v'>{}</span></div>", auto_approve_quota));
    html.push_str("</aside>");
    html.push_str("<main class='content'>");

    if let Some(msg) = params.get("msg") {
        let code = params.get("code").cloned().unwrap_or_default();
        let localized_msg = match msg.as_str() {
            "invite_created" => "邀请码已创建",
            "approved" => "审批已通过",
            "rejected" => "申请已拒绝",
            "revoked" => "授权已吊销",
            "invalid_installation" => "installation_id不能为空",
            "pwd_updated" => "管理员密码更新成功，请使用新密码重新登录",
            "pwd_mismatch" => "两次输入的新密码不一致",
            "pwd_too_short" => "新密码至少 8 位",
            "pwd_wrong_current" => "当前密码错误",
            _ => "操作完成",
        };
        html.push_str(&format!(
            "<div class='alert'><b>{}</b>{}</div>",
            escape_html(localized_msg),
            if code.is_empty() {
                "".to_string()
            } else {
                format!("，一次性邀请码：<code>{}</code>（仅展示一次）", escape_html(&code))
            }
        ));
    }
    html.push_str("<section class='form-card' style='margin-bottom:10px'><div style='font-size:11px;color:#6b7280'>控制台 / 激活总览</div><div style='font-size:18px;font-weight:700;margin:3px 0 2px'>激活总览</div><div style='font-size:12px;color:#6b7280'>聚合激活转化、运行状态与常用管理操作</div></section>");

    html.push_str("<div class='main-grid'>");
    html.push_str("<section class='chart-card'>");
    html.push_str("<div class='chart-title'>激活转化趋势</div>");
    html.push_str(&format!(
        "<div class='chart-sub'>最近 {} 天：展示申请 -> 审批 -> 激活 -> 吊销流转</div>",
        range_days
    ));
    html.push_str(&format!(
        "<div style='margin-top:8px;display:flex;gap:8px'><a class='btn {}' href='{}?range=7'>7d</a><a class='btn {}' href='{}?range=14'>14d</a><a class='btn {}' href='{}?range=30'>30d</a><a class='btn' href='{}/audit'>查看审计日志</a></div>",
        if range_days == 7 { "btn-primary" } else { "" },
        escape_html(&state.cfg.admin_ui_base),
        if range_days == 14 { "btn-primary" } else { "" },
        escape_html(&state.cfg.admin_ui_base),
        if range_days == 30 { "btn-primary" } else { "" },
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base)
    ));
    html.push_str("<div class='trend-bars'>");
    for (day, requests, approved, redeemed, revoked) in &trend_days {
        let total = requests + approved + redeemed + revoked;
        let width = ((total as f64) / (trend_max as f64) * 100.0).max(0.0);
        let req_w = if total > 0 {
            (*requests as f64 / total as f64) * width
        } else {
            0.0
        };
        let app_w = if total > 0 {
            (*approved as f64 / total as f64) * width
        } else {
            0.0
        };
        let red_w = if total > 0 {
            (*redeemed as f64 / total as f64) * width
        } else {
            0.0
        };
        let rev_w = if total > 0 {
            (*revoked as f64 / total as f64) * width
        } else {
            0.0
        };
        let short_day = NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .ok()
            .map(|d| d.format("%m-%d").to_string())
            .unwrap_or_else(|| day.clone());
        html.push_str(&format!(
            "<div class='trend-row'><div class='trend-day'>{}</div><div class='trend-track'><span class='seg s1' style='width:{:.2}%'></span><span class='seg s2' style='width:{:.2}%'></span><span class='seg s3' style='width:{:.2}%'></span><span class='seg s4' style='width:{:.2}%'></span></div><div class='trend-total'>{}</div></div>",
            escape_html(&short_day),
            req_w,
            app_w,
            red_w,
            rev_w,
            total
        ));
    }
    html.push_str("</div><div class='legend'><span><span class='dot s1'></span>请求</span><span><span class='dot s2'></span>审批</span><span><span class='dot s3'></span>激活</span><span><span class='dot s4'></span>吊销</span></div>");
    html.push_str("</section>");
    html.push_str("<aside class='stats-card'><h3>当前统计</h3>");
    html.push_str(&format!("<div class='stat-line'><span>待审批申请</span><strong>{}</strong></div>", pending.len()));
    html.push_str(&format!("<div class='stat-line'><span>有效授权</span><strong>{}</strong></div>", active_count));
    html.push_str(&format!("<div class='stat-line'><span>已吊销授权</span><strong>{}</strong></div>", revoked_count));
    html.push_str(&format!("<div class='stat-line'><span>邀请码总数</span><strong>{}</strong></div>", invites.len()));
    html.push_str("<div style='height:8px'></div><h3 style='margin:4px 0 8px'>服务 Health</h3>");
    html.push_str(&format!(
        "<div style='display:inline-flex;align-items:center;padding:4px 9px;border-radius:999px;background:{};color:{};font-size:11px;font-weight:700'>服务状态：{}</div>",
        service_status_bg,
        service_status_fg,
        service_status_text
    ));
    html.push_str(&format!(
        "<div class='stat-line'><span>数据库连接</span><strong>{}</strong></div>",
        if db_ok { "正常" } else { "异常" }
    ));
    html.push_str(&format!(
        "<div class='stat-line'><span>并发占用</span><strong>{}/{} ({:.0}%)</strong></div>",
        inflight_used,
        inflight_max,
        inflight_ratio
    ));
    html.push_str("<div style='display:flex;flex-wrap:wrap;gap:6px;margin-top:10px'>");
    html.push_str(&format!(
        "<a class='btn' target='_blank' href='/license-api/healthz'>健康检查</a>\
         <a class='btn' target='_blank' href='/license-api/v1/system/readiness'>就绪探测</a>\
         <a class='btn' href='{}/settings'>服务设置</a>\
         <a class='btn' href='{}/requests'>审批队列</a>\
         <button class='btn' type='button' onclick='window.location.reload()'>刷新状态</button>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&state.cfg.admin_ui_base)
    ));
    html.push_str("</div>");
    html.push_str("<div style='margin-top:8px;font-size:11px;color:#6b7280'>启动与重启快捷命令</div>");
    html.push_str("<div style='display:flex;flex-wrap:wrap;gap:6px;margin-top:6px'>\
        <button class='btn' type='button' data-copy='systemctl start iclaw-activation'>复制启动命令</button>\
        <button class='btn' type='button' data-copy='systemctl restart iclaw-activation'>复制重启命令</button>\
        <button class='btn' type='button' data-copy='systemctl status iclaw-activation --no-pager'>复制状态命令</button>\
    </div>");
    html.push_str("<script>(function(){document.querySelectorAll('button[data-copy]').forEach(function(btn){btn.addEventListener('click',async function(){const text=btn.getAttribute('data-copy')||'';if(!text)return;const old=btn.textContent;try{await navigator.clipboard.writeText(text);btn.textContent='已复制';setTimeout(function(){btn.textContent=old;},1200);}catch(_){btn.textContent='复制失败';setTimeout(function(){btn.textContent=old;},1200);}});});})();</script>");
    html.push_str("</aside></div>");

    html.push_str("<div class='grid-2'>");
    html.push_str("<section class='form-card'><h3>快速创建邀请码</h3>");
    html.push_str(&format!(
        "<form method='post' action='{}/invites/create' class='line-form'>",
        escape_html(&state.cfg.admin_ui_base)
    ));
    html.push_str("<div class='field'><label>绑定 installation_id</label><input name='installation_id' placeholder='sha256_xxx' required></div>");
    html.push_str("<div class='field'><label>max_uses</label><input name='max_uses' value='1'></div>");
    html.push_str(&format!(
        "<div class='field'><label>expires_days</label><input name='expires_days' value='{}'></div>",
        invite_expires_days
    ));
    html.push_str("<button class='btn btn-primary'>创建邀请码</button></form></section>");
    html.push_str("<section class='form-card'><h3>修改管理员密码</h3>");
    html.push_str(&format!(
        "<form method='post' action='{}/password/change' class='line-form' style='grid-template-columns:1fr 1fr 1fr auto'>",
        escape_html(&state.cfg.admin_ui_base)
    ));
    html.push_str("<div class='field'><label>当前密码</label><input type='password' name='current_password' required></div>");
    html.push_str("<div class='field'><label>新密码</label><input type='password' name='new_password' required></div>");
    html.push_str("<div class='field'><label>确认新密码</label><input type='password' name='confirm_password' required></div>");
    html.push_str("<button class='btn'>更新密码</button></form></section></div>");

    let base = &state.cfg.admin_ui_base;
    html.push_str(&format!(
        "<div class='focus-tip'>待审批与已审批设备列表已迁移到独立页面：<a href='{}/requests'><strong>Requests</strong></a>。Dashboard 仅保留核心趋势和关键操作。</div>",
        escape_html(base)
    ));
    html.push_str("<section class='table-card' style='margin-top:10px'><h3>最近授权与吊销</h3><table><tr><th>license_id</th><th>installation_id</th><th>状态</th><th>到期时间</th><th>动作</th></tr>");
    for (id, installation_id, status, exp) in recent_licenses {
        let license_display = compact_request_id_display(&id);
        let installation_display = format!(
            "<div><button type='button' class='btn indicator-badge' style='padding:3px 8px;font-size:11px;border-radius:999px;background:#eef3ff;border-color:#cfe0ff;color:#1e3a8a' data-copy='{}' title='点击复制设备识别码'>{}</button></div><div style='color:#6b7280'>{}</div><div><code title='{}'>{}</code></div>",
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_id_to_uuid_display(&installation_id)),
            escape_html(&installation_id),
            escape_html(&compact_installation_id_display(&installation_id))
        );
        html.push_str(&format!(
            "<tr><td><code title='{}'>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td class='stack'>\
             <form method='post' action='{}/licenses/{}/revoke'><button class='btn btn-danger'>吊销</button></form>\
             </td></tr>",
            escape_html(&id),
            escape_html(&license_display),
            installation_display,
            escape_html(&status),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&exp))),
            escape_html(base),
            escape_html(&id)
        ));
    }
    html.push_str("</table></section>");

    html.push_str("<section class='table-card section'><h3>邀请码记录（哈希存储）</h3><table><tr><th>invite_id</th><th>绑定 installation_id</th><th>使用情况</th><th>创建时间</th><th>到期时间</th></tr>");
    for (id, bound_installation_id, used_count, max_uses, created_at, expires_at) in invites {
        let invite_display = compact_request_id_display(&id);
        let installation_display = format!(
            "<div><button type='button' class='btn indicator-badge' style='padding:3px 8px;font-size:11px;border-radius:999px;background:#eef3ff;border-color:#cfe0ff;color:#1e3a8a' data-copy='{}' title='点击复制设备识别码'>{}</button></div><div style='color:#6b7280'>{}</div><div><code title='{}'>{}</code></div>",
            escape_html(&installation_device_indicator(&bound_installation_id)),
            escape_html(&installation_device_indicator(&bound_installation_id)),
            escape_html(&installation_id_to_uuid_display(&bound_installation_id)),
            escape_html(&bound_installation_id),
            escape_html(&compact_installation_id_display(&bound_installation_id))
        );
        html.push_str(&format!(
            "<tr><td><code title='{}'>{}</code></td><td>{}</td><td>{}/{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&id),
            escape_html(&invite_display),
            installation_display,
            used_count,
            max_uses,
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&created_at))),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&expires_at.unwrap_or_else(|| "-".into()))))
        ));
    }
    html.push_str("</table></section></main></div></div></body></html>");
    Html(html).into_response()
}

async fn admin_requests_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }

    let pending_rows =
        sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
            r#"
            SELECT id, device_request_code, installation_id, app_version, platform, created_at, decision_note
            FROM activation_requests
            WHERE status = 'pending'
            ORDER BY created_at DESC
            LIMIT 160
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    let approved_rows =
        sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>)>(
            r#"
            SELECT id, device_request_code, installation_id, app_version, platform, created_at, decision_note
            FROM activation_requests
            WHERE status = 'approved'
            ORDER BY created_at DESC
            LIMIT 200
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    let query_text = params
        .get("q")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let pending_rows: Vec<_> = pending_rows
        .into_iter()
        .filter(|(_, _, installation_id, _, _, _, _)| {
            installation_matches_query(installation_id, &query_text)
        })
        .collect();
    let approved_rows: Vec<_> = approved_rows
        .into_iter()
        .filter(|(_, _, installation_id, _, _, _, _)| {
            installation_matches_query(installation_id, &query_text)
        })
        .collect();

    let mut content = String::new();
    if matches!(params.get("msg").map(|s| s.as_str()), Some("approved")) {
        let code = params.get("code").cloned().unwrap_or_default();
        if !code.is_empty() {
            content.push_str(&format!(
                "<div class='toast-layer'>\
                    <div id='approve-toast' class='status-toast success'>\
                      <div>\
                        <span class='badge'>审批成功</span>\
                        <div class='title'>设备申请已批准</div>\
                        <div class='desc'>一次性激活码已生成，点击下方高亮激活码可自动复制到剪贴板。</div>\
                        <button id='invite-code-chip' class='code-btn' data-code='{0}'>{0}</button>\
                      </div>\
                      <button id='approve-toast-close' type='button' class='close-btn' aria-label='关闭'>×</button>\
                    </div>\
                 </div>\
                 <script>(function(){{\
                    const chip=document.getElementById('invite-code-chip');\
                    const toast=document.getElementById('approve-toast');\
                    const closeBtn=document.getElementById('approve-toast-close');\
                    if(!chip||!toast)return;\
                    function showCopyToast(text){{\
                      const prev=document.getElementById('copy-toast-lite');\
                      if(prev) prev.remove();\
                      const el=document.createElement('div');\
                      el.id='copy-toast-lite';\
                      el.className='copy-toast';\
                      el.textContent=text;\
                      document.body.appendChild(el);\
                      setTimeout(function(){{el.style.animation='copy-out .18s ease forwards';setTimeout(function(){{el.remove();}},200);}},1200);\
                    }}\
                    function dismissToast(){{\
                      toast.style.animation='toast-out .22s ease forwards';\
                      setTimeout(function(){{const layer=toast.closest('.toast-layer');if(layer) layer.remove(); else toast.remove();}},220);\
                    }}\
                    chip.addEventListener('click',async function(){{\
                      const code=chip.getAttribute('data-code')||'';\
                      try{{await navigator.clipboard.writeText(code);showCopyToast('复制成功');chip.textContent='已复制';setTimeout(function(){{chip.textContent=code;}},1000);}}\
                      catch(_){{showCopyToast('复制失败，请手动复制');}}\
                    }});\
                    if(closeBtn) closeBtn.addEventListener('click', dismissToast);\
                    setTimeout(dismissToast, 5600);\
                 }})();</script>",
                escape_html(&code)
            ));
        }
    }
    content.push_str(&format!(
        "<div style='display:flex;gap:8px;margin-top:2px'><span class='chip'>待审批 <strong>{}</strong></span><span class='chip'>已审批 <strong>{}</strong></span></div>",
        pending_rows.len(),
        approved_rows.len()
    ));
    content.push_str(&format!(
        "<section class='panel' style='margin-top:8px'><form method='get' action='{}/requests' style='display:grid;grid-template-columns:1fr auto;gap:8px;align-items:end'><div><label style='font-size:11px;color:#6b7280'>搜索设备（设备识别码 / UUID / installation_id）</label><input name='q' value='{}' placeholder='例如 K7M2-4Q9D 或 9a8d... 或 UUID' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div><button class='btn'>搜索</button></form></section>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&query_text)
    ));
    content.push_str("<section class='panel'><h3 style='margin:0 0 4px;font-size:14px'>待审批设备列表</h3><div style='font-size:11px;color:#6b7280;margin-bottom:8px'>用于处理首启申请。你可以在此直接批准或拒绝。</div><table><tr><th>Request</th><th>申请码</th><th>installation_id</th><th>版本/平台</th><th>备注</th><th>动作</th></tr>");
    for (id, request_code, installation_id, app_version, platform, created_at, note) in pending_rows {
        let request_display = compact_request_id_display(&id);
        let installation_display = format!(
            "<div><button type='button' class='btn indicator-badge' style='padding:3px 8px;font-size:11px;border-radius:999px;background:#eef3ff;border-color:#cfe0ff;color:#1e3a8a' data-copy='{}' title='点击复制设备识别码'>{}</button></div><div style='color:#6b7280'>{}</div><div><code title='{}'>{}</code></div>",
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_id_to_uuid_display(&installation_id)),
            escape_html(&installation_id),
            escape_html(&compact_installation_id_display(&installation_id))
        );
        content.push_str(&format!(
            "<tr><td><code title='{}'>{}</code><div style='color:#6b7280'>{}</div></td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td><form style='display:inline-block;margin-right:6px' method='post' action='{}/requests/{}/approve'><button class='btn'>批准</button></form><form style='display:inline-block' method='post' action='{}/requests/{}/reject'><button class='btn btn-danger'>拒绝</button></form></td></tr>",
            escape_html(&id),
            escape_html(&request_display),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&created_at))),
            escape_html(&request_code),
            installation_display,
            escape_html(&app_version),
            escape_html(&platform),
            escape_html(&note.unwrap_or_default()),
            escape_html(&state.cfg.admin_ui_base),
            escape_html(&id),
            escape_html(&state.cfg.admin_ui_base),
            escape_html(&id)
        ));
    }
    content.push_str("</table></section>");
    content.push_str("<section class='panel'><h3 style='margin:0 0 4px;font-size:14px'>已审批设备列表</h3><div style='font-size:11px;color:#6b7280;margin-bottom:8px'>已审批请求归档。若邀请码丢失，可直接点“重新生成邀请码”。</div><table><tr><th>Request</th><th>申请码</th><th>installation_id</th><th>版本/平台</th><th>审批备注</th><th>补救</th></tr>");
    for (id, request_code, installation_id, app_version, platform, created_at, note) in approved_rows {
        let request_display = compact_request_id_display(&id);
        let installation_display = format!(
            "<div><button type='button' class='btn indicator-badge' style='padding:3px 8px;font-size:11px;border-radius:999px;background:#eef3ff;border-color:#cfe0ff;color:#1e3a8a' data-copy='{}' title='点击复制设备识别码'>{}</button></div><div style='color:#6b7280'>{}</div><div><code title='{}'>{}</code></div>",
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_id_to_uuid_display(&installation_id)),
            escape_html(&installation_id),
            escape_html(&compact_installation_id_display(&installation_id))
        );
        content.push_str(&format!(
            "<tr><td><code title='{}'>{}</code><div style='color:#6b7280'>{}</div></td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td><a class='btn' href='{}/invites?installation_id={}'>重新生成邀请码</a></td></tr>",
            escape_html(&id),
            escape_html(&request_display),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&created_at))),
            escape_html(&request_code),
            installation_display,
            escape_html(&app_version),
            escape_html(&platform),
            escape_html(&note.unwrap_or_default()),
            escape_html(&state.cfg.admin_ui_base),
            escape_html(&installation_id)
        ));
    }
    content.push_str("</table></section>");
    content.push_str(copy_badge_script());
    Html(render_admin_page_shell(
        &state.cfg.admin_ui_base,
        "requests",
        "控制台 / 设备申请",
        "设备申请管理",
        "集中处理首启申请，统一查看待审批与已审批设备列表",
        &content,
    ))
    .into_response()
}

async fn admin_invites_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }

    let invites = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>)>(
        r#"
        SELECT id, bound_installation_id, used_count, max_uses, created_at, expires_at
        FROM invite_codes
        ORDER BY created_at DESC
        LIMIT 160
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut content = String::new();
    let preset_installation_id = params
        .get("installation_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let query_text = params
        .get("q")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let invites: Vec<_> = invites
        .into_iter()
        .filter(|(_, installation_id, _, _, _, _)| {
            installation_matches_query(installation_id, &query_text)
        })
        .collect();
    if matches!(params.get("msg").map(|s| s.as_str()), Some("invite_created")) {
        let code = params.get("code").cloned().unwrap_or_default();
        if !code.is_empty() {
            content.push_str(&format!(
                "<div class='toast-layer'>\
                    <div id='invite-toast' class='status-toast success'>\
                      <div>\
                        <span class='badge'>创建成功</span>\
                        <div class='title'>新邀请码已生成</div>\
                        <div class='desc'>这是一次性展示的邀请码，请立即复制并发给用户手动输入。</div>\
                        <button id='invite-code-chip' class='code-btn' data-code='{0}'>{0}</button>\
                      </div>\
                      <button id='invite-toast-close' type='button' class='close-btn' aria-label='关闭'>×</button>\
                    </div>\
                 </div>\
                 <script>(function(){{\
                    const chip=document.getElementById('invite-code-chip');\
                    const toast=document.getElementById('invite-toast');\
                    const closeBtn=document.getElementById('invite-toast-close');\
                    if(!chip||!toast)return;\
                    function showCopyToast(text){{\
                      const prev=document.getElementById('copy-toast-lite');\
                      if(prev) prev.remove();\
                      const el=document.createElement('div');\
                      el.id='copy-toast-lite';\
                      el.className='copy-toast';\
                      el.textContent=text;\
                      document.body.appendChild(el);\
                      setTimeout(function(){{el.style.animation='copy-out .18s ease forwards';setTimeout(function(){{el.remove();}},200);}},1200);\
                    }}\
                    function dismissToast(){{\
                      toast.style.animation='toast-out .22s ease forwards';\
                      setTimeout(function(){{const layer=toast.closest('.toast-layer');if(layer) layer.remove(); else toast.remove();}},220);\
                    }}\
                    chip.addEventListener('click',async function(){{\
                      const code=chip.getAttribute('data-code')||'';\
                      try{{await navigator.clipboard.writeText(code);showCopyToast('复制成功');chip.textContent='已复制';setTimeout(function(){{chip.textContent=code;}},1000);}}\
                      catch(_){{showCopyToast('复制失败，请手动复制');}}\
                    }});\
                    if(closeBtn) closeBtn.addEventListener('click', dismissToast);\
                    setTimeout(dismissToast, 5600);\
                 }})();</script>",
                escape_html(&code)
            ));
        }
    }
    content.push_str(&format!(
        "<section class='panel'><h3 style='margin:0 0 10px;font-size:14px'>创建邀请码</h3><div style='font-size:11px;color:#6b7280;margin-bottom:8px'>补救流程：若审批后未及时复制邀请码，可在此按 installation_id 重新生成并立即复制发给用户。</div><form method='post' action='{}/invites/create' style='display:grid;grid-template-columns:1fr 100px 110px auto;gap:8px;align-items:end'><div><label style='font-size:11px;color:#6b7280'>绑定 installation_id</label><input name='installation_id' required value='{}' placeholder='sha256_xxx' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div><div><label style='font-size:11px;color:#6b7280'>max_uses</label><input name='max_uses' value='1' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div><div><label style='font-size:11px;color:#6b7280'>expires_days</label><input name='expires_days' value='7' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div><button class='btn' style='background:#ff9500;border-color:#e68a00'>创建邀请码</button></form>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&preset_installation_id)
    ));
    content.push_str(&format!(
        "<section class='panel' style='margin-top:8px'><form method='get' action='{}/invites' style='display:grid;grid-template-columns:1fr auto auto;gap:8px;align-items:end'><input type='hidden' name='installation_id' value='{}'><div><label style='font-size:11px;color:#6b7280'>搜索设备（设备识别码 / UUID / installation_id）</label><input name='q' value='{}' placeholder='例如 K7M2-4Q9D 或 UUID' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div><button class='btn'>搜索</button><a class='btn' href='{}/invites'>清空</a></form></section>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&preset_installation_id),
        escape_html(&query_text),
        escape_html(&state.cfg.admin_ui_base)
    ));
    content.push_str("<table style='margin-top:10px'><tr><th>invite_id</th><th>installation_id</th><th>使用情况</th><th>创建时间</th><th>到期时间</th></tr>");
    for (id, bound_installation_id, used_count, max_uses, created_at, expires_at) in invites {
        let invite_display = compact_request_id_display(&id);
        let installation_display = format!(
            "<div><button type='button' class='btn indicator-badge' style='padding:3px 8px;font-size:11px;border-radius:999px;background:#eef3ff;border-color:#cfe0ff;color:#1e3a8a' data-copy='{}' title='点击复制设备识别码'>{}</button></div><div style='color:#6b7280'>{}</div><div><code title='{}'>{}</code></div>",
            escape_html(&installation_device_indicator(&bound_installation_id)),
            escape_html(&installation_device_indicator(&bound_installation_id)),
            escape_html(&installation_id_to_uuid_display(&bound_installation_id)),
            escape_html(&bound_installation_id),
            escape_html(&compact_installation_id_display(&bound_installation_id))
        );
        content.push_str(&format!(
            "<tr><td><code title='{}'>{}</code></td><td>{}</td><td>{}/{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&id),
            escape_html(&invite_display),
            installation_display,
            used_count,
            max_uses,
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&created_at))),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&expires_at.unwrap_or_else(|| "-".into()))))
        ));
    }
    content.push_str("</table></section>");
    content.push_str(copy_badge_script());
    Html(render_admin_page_shell(
        &state.cfg.admin_ui_base,
        "invites",
        "控制台 / 邀请码",
        "邀请码管理",
        "邀请码采用哈希存储，明文仅在创建后展示一次",
        &content,
    ))
    .into_response()
}

async fn admin_licenses_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }

    let status = params
        .get("status")
        .map(|v| v.trim().to_string())
        .filter(|v| ["all", "active", "revoked"].contains(&v.as_str()))
        .unwrap_or_else(|| "all".into());
    let query_text = params
        .get("q")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let rows = if status == "all" {
        sqlx::query_as::<_, (String, String, String, String, String)>(
            "SELECT id, installation_id, status, issued_at, exp FROM licenses ORDER BY issued_at DESC LIMIT 180",
        )
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, String, String, String, String)>(
            "SELECT id, installation_id, status, issued_at, exp FROM licenses WHERE status = ? ORDER BY issued_at DESC LIMIT 180",
        )
        .bind(&status)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    };
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|(_, installation_id, _, _, _)| installation_matches_query(installation_id, &query_text))
        .collect();

    let mut content = String::new();
    content.push_str("<section class='panel'><div>");
    for s in ["all", "active", "revoked"] {
        content.push_str(&format!(
            "<a class='chip {}' href='{}/licenses?status={}'>{}</a>",
            if status == s { "active" } else { "" },
            escape_html(&state.cfg.admin_ui_base),
            s,
            s
        ));
    }
    content.push_str("</div>");
    content.push_str(&format!(
        "<div style='margin:8px 0 10px'><form method='get' action='{}/licenses' style='display:grid;grid-template-columns:auto 1fr auto auto;gap:8px;align-items:end'><input type='hidden' name='status' value='{}'><div style='font-size:11px;color:#6b7280;padding-bottom:8px'>按设备搜索</div><div><input name='q' value='{}' placeholder='设备识别码 / UUID / installation_id' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div><button class='btn'>搜索</button><a class='btn' href='{}/licenses?status={}'>清空</a></form></div>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&status),
        escape_html(&query_text),
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&status)
    ));
    content.push_str("<table style='margin-top:10px'><tr><th>license_id</th><th>installation_id</th><th>状态</th><th>时间</th><th>到期时间</th><th>动作</th></tr>");
    for (id, installation_id, license_status, issued_at, exp) in rows {
        let reissue_action = format!(
            "<a class='btn' style='margin-left:6px' href='{}/invites?installation_id={}'>重新生成邀请码</a>",
            escape_html(&state.cfg.admin_ui_base),
            escape_html(&installation_id)
        );
        let actions = if license_status == "active" {
            format!(
                "<form style='display:inline-block' method='post' action='{}/licenses/{}/revoke'><button class='btn' style='background:#fff1f2;border-color:#fecdd3;color:#b91c1c'>吊销</button></form>{}",
                escape_html(&state.cfg.admin_ui_base),
                escape_html(&id),
                reissue_action
            )
        } else {
            reissue_action
        };
        let license_display = compact_request_id_display(&id);
        let installation_display = format!(
            "<div><button type='button' class='btn indicator-badge' style='padding:3px 8px;font-size:11px;border-radius:999px;background:#eef3ff;border-color:#cfe0ff;color:#1e3a8a' data-copy='{}' title='点击复制设备识别码'>{}</button></div><div style='color:#6b7280'>{}</div><div><code title='{}'>{}</code></div>",
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_device_indicator(&installation_id)),
            escape_html(&installation_id_to_uuid_display(&installation_id)),
            escape_html(&installation_id),
            escape_html(&compact_installation_id_display(&installation_id))
        );
        content.push_str(&format!(
            "<tr><td><code title='{}'>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&id),
            escape_html(&license_display),
            installation_display,
            escape_html(&license_status),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&issued_at))),
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&exp))),
            actions
        ));
    }
    content.push_str("</table></section>");
    content.push_str(copy_badge_script());
    Html(render_admin_page_shell(
        &state.cfg.admin_ui_base,
        "licenses",
        "控制台 / 授权管理",
        "授权管理",
        "统一管理授权生命周期、状态过滤与吊销操作",
        &content,
    ))
    .into_response()
}

async fn admin_settings_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    let auto_approve_quota = get_setting_i64(&state.pool, "auto_approve_quota", 0)
        .await
        .max(0);
    let invite_expires_days = get_setting_i64(&state.pool, "invite_expires_days", 7)
        .await
        .clamp(1, 90);
    let approved_or_redeemed = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(DISTINCT installation_id) FROM activation_requests WHERE status IN ('approved','redeemed')",
    )
    .fetch_one(&state.pool)
    .await
    .map(|v| v.0)
    .unwrap_or(0);

    let mut content = String::new();
    if let Some(msg) = params.get("msg") {
        let text = match msg.as_str() {
            "updated" => "全局设置已更新",
            _ => "操作完成",
        };
        content.push_str(&format!(
            "<section class='panel' style='background:#fff8ed;border-color:#f3be74'><strong style='font-size:12px'>{}</strong></section>",
            escape_html(text)
        ));
    }
    content.push_str("<section class='panel'>");
    content.push_str("<h3 style='margin:0 0 8px;font-size:14px'>激活审批策略</h3>");
    content.push_str(&format!(
        "<div style='font-size:12px;color:#6b7280;margin-bottom:8px'>当前已审批/已激活用户数：<strong>{}</strong></div>",
        approved_or_redeemed
    ));
    content.push_str(&format!(
        "<form method='post' action='{}/settings' style='display:grid;grid-template-columns:1fr 1fr auto;gap:10px;align-items:end'>\
         <div><label style='font-size:11px;color:#6b7280'>自动审批配额（前 N 位用户自动审批）</label><input name='auto_approve_quota' value='{}' type='number' min='0' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div>\
         <div><label style='font-size:11px;color:#6b7280'>邀请码到期天数</label><input name='invite_expires_days' value='{}' type='number' min='1' max='90' style='width:100%;padding:8px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'></div>\
         <button class='btn' style='background:#ff9500;border-color:#e68a00'>保存设置</button>\
         </form>",
        escape_html(&state.cfg.admin_ui_base),
        auto_approve_quota,
        invite_expires_days
    ));
    content.push_str("</section>");
    Html(render_admin_page_shell(
        &state.cfg.admin_ui_base,
        "settings",
        "控制台 / 全局设置",
        "全局设置",
        "管理自动审批配额与邀请码有效期策略",
        &content,
    ))
    .into_response()
}

async fn admin_settings_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<AdminSettingsForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let quota = form.auto_approve_quota.unwrap_or(0).clamp(0, 1_000_000);
    let invite_days = form.invite_expires_days.unwrap_or(7).clamp(1, 90);
    set_setting_i64(&state.pool, "auto_approve_quota", quota).await;
    set_setting_i64(&state.pool, "invite_expires_days", invite_days).await;
    Redirect::to(&format!("{}/settings?msg=updated", state.cfg.admin_ui_base)).into_response()
}

async fn admin_audit_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }

    let action_filter = params
        .get("action")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "all")
        .unwrap_or_else(|| "all".into());
    let days = params
        .get("days")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| [1, 7, 30, 90].contains(v))
        .unwrap_or(30);
    let from_param = params
        .get("from")
        .map(|s| s.trim().to_string())
        .filter(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok());
    let to_param = params
        .get("to")
        .map(|s| s.trim().to_string())
        .filter(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok());
    let (date_from, date_to) = if let (Some(from), Some(to)) = (from_param, to_param) {
        (from, to)
    } else {
        (
            (Utc::now() - Duration::days(days - 1))
                .date_naive()
                .format("%Y-%m-%d")
                .to_string(),
            Utc::now().date_naive().format("%Y-%m-%d").to_string(),
        )
    };
    let page = params
        .get("page")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1);
    let page_size: i64 = 40;
    let offset = (page - 1) * page_size;

    let (total, rows): (i64, Vec<(i64, String, String, String, String, Option<String>, Option<String>, String)>) =
        if action_filter == "all" {
            let total = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM audit_logs WHERE substr(created_at, 1, 10) >= ? AND substr(created_at, 1, 10) <= ?",
            )
            .bind(&date_from)
            .bind(&date_to)
            .fetch_one(&state.pool)
            .await
            .map(|v| v.0)
            .unwrap_or(0);
            let rows = sqlx::query_as::<_, (i64, String, String, String, String, Option<String>, Option<String>, String)>(
                r#"
                SELECT id, actor, action, target_type, target_id, message, ip, created_at
                FROM audit_logs
                WHERE substr(created_at, 1, 10) >= ? AND substr(created_at, 1, 10) <= ?
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(&date_from)
            .bind(&date_to)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
            (total, rows)
        } else {
            let total = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM audit_logs WHERE action = ? AND substr(created_at, 1, 10) >= ? AND substr(created_at, 1, 10) <= ?",
            )
            .bind(&action_filter)
            .bind(&date_from)
            .bind(&date_to)
            .fetch_one(&state.pool)
            .await
            .map(|v| v.0)
            .unwrap_or(0);
            let rows = sqlx::query_as::<_, (i64, String, String, String, String, Option<String>, Option<String>, String)>(
                r#"
                SELECT id, actor, action, target_type, target_id, message, ip, created_at
                FROM audit_logs
                WHERE action = ? AND substr(created_at, 1, 10) >= ? AND substr(created_at, 1, 10) <= ?
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(&action_filter)
            .bind(&date_from)
            .bind(&date_to)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
            (total, rows)
        };
    let page_count = ((total + page_size - 1) / page_size).max(1);

    let actions = sqlx::query_as::<_, (String,)>("SELECT DISTINCT action FROM audit_logs ORDER BY action ASC")
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

    let mut content = String::new();
    content.push_str("<section class='panel'><div style='margin-bottom:10px'><span style='font-size:12px;color:#6b7280'>时间范围：</span>");
    for d in [1, 7, 30, 90] {
        content.push_str(&format!(
            "<a class='chip {}' href='{}/audit?days={}&action={}'>最近{}天</a> ",
            if days == d { "active" } else { "" },
            escape_html(&state.cfg.admin_ui_base),
            d,
            escape_html(&action_filter),
            d
        ));
    }
    content.push_str("</div>");
    content.push_str(&format!(
        "<div style='margin-bottom:10px'><form method='get' action='{}/audit' style='display:flex;gap:8px;align-items:center;flex-wrap:wrap'><span style='font-size:12px;color:#6b7280'>自定义：</span><input type='date' name='from' value='{}' style='padding:7px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'><span style='font-size:12px;color:#6b7280'>到</span><input type='date' name='to' value='{}' style='padding:7px 9px;border:1px solid #e5e7eb;border-radius:9px;font-size:12px'><input type='hidden' name='action' value='{}'><button class='btn'>应用</button></form></div>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&date_from),
        escape_html(&date_to),
        escape_html(&action_filter)
    ));
    content.push_str("<div style='margin-bottom:10px'><span style='font-size:12px;color:#6b7280'>Action 过滤：</span>");
    content.push_str(&format!(
        "<a class='chip {}' href='{}/audit?action=all&days={}'>all</a> ",
        if action_filter == "all" { "active" } else { "" },
        escape_html(&state.cfg.admin_ui_base),
        days
    ));
    for (act,) in actions {
        content.push_str(&format!(
            "<a class='chip {}' href='{}/audit?action={}&days={}'>{}</a> ",
            if action_filter == act { "active" } else { "" },
            escape_html(&state.cfg.admin_ui_base),
            escape_html(&act),
            days,
            escape_html(&act)
        ));
    }
    content.push_str("</div><table><tr><th>ID</th><th>时间</th><th>Actor</th><th>Action</th><th>Target</th><th>Message</th><th>IP</th></tr>");
    for (id, actor, action, target_type, target_id, message, ip, created_at) in rows {
        content.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td><code>{}:{}</code></td><td>{}</td><td>{}</td></tr>",
            id,
            format!("<span class='time-text'>{}</span>", escape_html(&format_ts_short(&created_at))),
            escape_html(&actor),
            escape_html(&action),
            escape_html(&target_type),
            escape_html(&target_id),
            escape_html(&message.unwrap_or_default()),
            escape_html(&ip.unwrap_or_default())
        ));
    }
    content.push_str("</table>");
    let prev_page = (page - 1).max(1);
    let next_page = (page + 1).min(page_count);
    content.push_str(&format!(
        "<div class='pager'><a class='btn' href='{}/audit?action={}&days={}&page={}'>上一页</a><span style='color:#6b7280;font-size:12px'>范围 {} ~ {} · 第 {}/{} 页 · 共 {} 条</span><a class='btn' href='{}/audit?action={}&days={}&page={}'>下一页</a></div>",
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&action_filter),
        days,
        prev_page,
        escape_html(&date_from),
        escape_html(&date_to),
        page,
        page_count,
        total,
        escape_html(&state.cfg.admin_ui_base),
        escape_html(&action_filter),
        days,
        next_page
    ));
    content.push_str("</div>");
    Html(render_admin_page_shell(
        &state.cfg.admin_ui_base,
        "security",
        "控制台 / 审计安全",
        "审计日志中心",
        "统一查看关键管理动作与安全审计轨迹",
        &content,
    ))
    .into_response()
}

async fn admin_approve_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Form(form): Form<AdminDecisionForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let req = sqlx::query_as::<_, (String, String)>(
        "SELECT installation_id, status FROM activation_requests WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;
    let (installation_id, status) = match req {
        Ok(Some(v)) => v,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    if status != "pending" {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let invite_plain = random_code(8);
    let invite_hash = sha256_hex(&invite_plain);
    let invite_id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let invite_expires_days = get_setting_i64(&state.pool, "invite_expires_days", 7)
        .await
        .clamp(1, 90);

    let mut tx = match state.pool.begin().await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO invite_codes (id, code_hash, bound_installation_id, max_uses, used_count, created_at, expires_at)
        VALUES (?, ?, ?, 1, 0, ?, ?)
        "#,
    )
    .bind(&invite_id)
    .bind(invite_hash)
    .bind(installation_id)
    .bind(&now)
    .bind((Utc::now() + Duration::days(invite_expires_days)).to_rfc3339())
    .execute(&mut *tx)
    .await;

    let _ = sqlx::query(
        "UPDATE activation_requests SET status = 'approved', invite_id = ?, decision_note = ? WHERE id = ?",
    )
    .bind(invite_id)
    .bind(form.note)
    .bind(&id)
    .execute(&mut *tx)
    .await;

    let _ = tx.commit().await;

    write_audit(
        &state.pool,
        "admin",
        "approve_request",
        "activation_request",
        &id,
        "ok",
        Some("approved and invite generated".into()),
        Some(addr.ip().to_string()),
    )
    .await;

    info!("invite generated (not persisted in plaintext): {}", invite_plain);
    Redirect::to(&format!(
        "{}/requests?msg=approved&code={}",
        state.cfg.admin_ui_base, invite_plain
    ))
    .into_response()
}

async fn admin_reject_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Form(form): Form<AdminDecisionForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let res =
        sqlx::query("UPDATE activation_requests SET status = 'rejected', decision_note = ? WHERE id = ?")
            .bind(form.note)
            .bind(&id)
            .execute(&state.pool)
            .await;

    if res.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    write_audit(
        &state.pool,
        "admin",
        "reject_request",
        "activation_request",
        &id,
        "ok",
        None,
        Some(addr.ip().to_string()),
    )
    .await;
    Redirect::to(&format!("{}/requests?msg=rejected", state.cfg.admin_ui_base)).into_response()
}

async fn admin_revoke_license(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Form(form): Form<AdminRevokeForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let res = sqlx::query("UPDATE licenses SET status = 'revoked' WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await;
    if res.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    write_audit(
        &state.pool,
        "admin",
        "revoke_license",
        "license",
        &id,
        "ok",
        form.note,
        Some(addr.ip().to_string()),
    )
    .await;
    Redirect::to(&format!("{}?msg=revoked", state.cfg.admin_ui_base)).into_response()
}

async fn admin_create_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(form): Form<AdminInviteCreateForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let installation_id = form.installation_id.trim().to_string();
    if installation_id.is_empty() {
        return Redirect::to(&format!("{}?msg=invalid_installation", state.cfg.admin_ui_base))
            .into_response();
    }
    let max_uses = form.max_uses.unwrap_or(1).clamp(1, 20);
    let default_expires_days = get_setting_i64(&state.pool, "invite_expires_days", 7)
        .await
        .clamp(1, 90);
    let expires_days = form.expires_days.unwrap_or(default_expires_days).clamp(1, 90);

    let invite_plain = random_code(8);
    let invite_hash = sha256_hex(&invite_plain);
    let invite_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = (now + Duration::days(expires_days)).to_rfc3339();
    let _ = sqlx::query(
        r#"
        INSERT INTO invite_codes (id, code_hash, bound_installation_id, max_uses, used_count, created_at, expires_at)
        VALUES (?, ?, ?, ?, 0, ?, ?)
        "#,
    )
    .bind(&invite_id)
    .bind(invite_hash)
    .bind(&installation_id)
    .bind(max_uses)
    .bind(now.to_rfc3339())
    .bind(expires_at)
    .execute(&state.pool)
    .await;

    write_audit(
        &state.pool,
        "admin",
        "manual_invite_create",
        "invite",
        &invite_id,
        "ok",
        Some(format!("installation={installation_id}, max_uses={max_uses}")),
        Some(addr.ip().to_string()),
    )
    .await;

    Redirect::to(&format!(
        "{}/invites?msg=invite_created&code={}&installation_id={}",
        state.cfg.admin_ui_base, invite_plain, installation_id
    ))
    .into_response()
}

async fn admin_change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(form): Form<AdminPasswordChangeForm>,
) -> impl IntoResponse {
    let _write_permit = match acquire_write_permit(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if !is_admin_authenticated(&state, &headers).await {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    }
    if !is_same_origin_mutation(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if form.new_password.len() < 8 {
        return Redirect::to(&format!("{}?msg=pwd_too_short", state.cfg.admin_ui_base)).into_response();
    }
    if form.new_password != form.confirm_password {
        return Redirect::to(&format!("{}?msg=pwd_mismatch", state.cfg.admin_ui_base)).into_response();
    }

    let username = if let Some((user, _)) = parse_basic_auth(&headers) {
        user
    } else if let Some(token) = get_cookie_value(&headers, "iclaw_admin_session") {
        let token_hash = sha256_hex(&token);
        match sqlx::query_as::<_, (String,)>(
            "SELECT username FROM admin_sessions WHERE token_hash = ? AND expires_at > ? LIMIT 1",
        )
        .bind(token_hash)
        .bind(Utc::now().to_rfc3339())
        .fetch_optional(&state.pool)
        .await
        {
            Ok(Some((name,))) => name,
            _ => return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response(),
        }
    } else {
        return Redirect::to(&format!("{}/login", state.cfg.admin_ui_base)).into_response();
    };

    if !verify_admin_credentials(&state.pool, &username, &form.current_password).await {
        return Redirect::to(&format!("{}?msg=pwd_wrong_current", state.cfg.admin_ui_base))
            .into_response();
    }

    let _ = sqlx::query("UPDATE admin_users SET password_hash = ?, updated_at = ? WHERE username = ?")
        .bind(hash_password(&form.new_password))
        .bind(now_rfc3339())
        .bind(&username)
        .execute(&state.pool)
        .await;
    let _ = sqlx::query("DELETE FROM admin_sessions WHERE username = ?")
        .bind(&username)
        .execute(&state.pool)
        .await;

    write_audit(
        &state.pool,
        "admin",
        "admin_password_change",
        "admin_user",
        &username,
        "ok",
        None,
        Some(addr.ip().to_string()),
    )
    .await;

    let mut out = HeaderMap::new();
    out.insert(
        header::LOCATION,
        HeaderValue::from_str(&format!("{}/login?msg=pwd_updated", state.cfg.admin_ui_base))
            .unwrap_or_else(|_| HeaderValue::from_static("/license-api/admin/login?msg=pwd_updated")),
    );
    out.insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "iclaw_admin_session=deleted; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0",
        ),
    );
    (StatusCode::SEE_OTHER, out).into_response()
}

async fn is_admin_authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    if let Some((user, pass)) = parse_basic_auth(headers) {
        if verify_admin_credentials(&state.pool, &user, &pass).await {
            return true;
        }
    }
    let Some(token) = get_cookie_value(headers, "iclaw_admin_session") else {
        return false;
    };
    let token_hash = sha256_hex(&token);
    matches!(
        sqlx::query_as::<_, (String,)>(
            "SELECT username FROM admin_sessions WHERE token_hash = ? AND expires_at > ? LIMIT 1",
        )
        .bind(token_hash)
        .bind(Utc::now().to_rfc3339())
        .fetch_optional(&state.pool)
        .await,
        Ok(Some(_))
    )
}

fn parse_basic_auth(headers: &HeaderMap) -> Option<(String, String)> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())?;
    if !auth.starts_with("Basic ") {
        return None;
    }
    let raw = &auth[6..];
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw.as_bytes())
        .ok()?;
    let pair = String::from_utf8(decoded)
        .ok()?;
    let mut parts = pair.splitn(2, ':');
    let user = parts.next().unwrap_or("").to_string();
    let pass = parts.next().unwrap_or("").to_string();
    Some((user, pass))
}

fn is_same_origin_mutation(headers: &HeaderMap) -> bool {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim();
    if host.is_empty() {
        return false;
    }
    let allowed_https = format!("https://{host}");
    let allowed_http = format!("http://{host}");
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let origin = origin.trim();
        if origin == allowed_https || origin == allowed_http {
            return true;
        }
    }
    if let Some(referer) = headers.get(header::REFERER).and_then(|v| v.to_str().ok()) {
        let referer = referer.trim();
        if referer.starts_with(&allowed_https) || referer.starts_with(&allowed_http) {
            return true;
        }
    }
    false
}

async fn ensure_default_admin_user(pool: &SqlitePool, cfg: &Config) -> anyhow::Result<()> {
    let exists = sqlx::query_as::<_, (String,)>("SELECT username FROM admin_users WHERE username = ?")
        .bind(&cfg.admin_username)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        sqlx::query(
            "INSERT INTO admin_users (username, password_hash, updated_at) VALUES (?, ?, ?)",
        )
        .bind(&cfg.admin_username)
        .bind(hash_password(&cfg.admin_password))
        .bind(now_rfc3339())
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn verify_admin_credentials(pool: &SqlitePool, username: &str, password: &str) -> bool {
    matches!(
        sqlx::query_as::<_, (String,)>(
            "SELECT username FROM admin_users WHERE username = ? AND password_hash = ? LIMIT 1",
        )
        .bind(username)
        .bind(hash_password(password))
        .fetch_optional(pool)
        .await,
        Ok(Some(_))
    )
}

fn hash_password(password: &str) -> String {
    sha256_hex(&format!("iclaw-admin::{password}"))
}

fn get_cookie_value(headers: &HeaderMap, key: &str) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for item in cookie.split(';') {
        let mut kv = item.trim().splitn(2, '=');
        let k = kv.next().unwrap_or("");
        let v = kv.next().unwrap_or("");
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

fn admin_nav_link(base: &str, active: &str, key: &str, label: &str, suffix: &str) -> String {
    let href = if suffix.is_empty() {
        escape_html(base)
    } else {
        format!("{}/{}", escape_html(base), suffix)
    };
    format!(
        "<a class='{}' href='{}'>{}</a>",
        if active == key { "active" } else { "" },
        href,
        escape_html(label)
    )
}

fn render_admin_page_shell(
    base: &str,
    active: &str,
    breadcrumb: &str,
    title: &str,
    subtitle: &str,
    content_html: &str,
) -> String {
    let top_menu = format!(
        "{}{}{}{}{}{}",
        admin_nav_link(base, active, "dashboard", "总览", ""),
        admin_nav_link(base, active, "requests", "申请", "requests"),
        admin_nav_link(base, active, "invites", "邀请码", "invites"),
        admin_nav_link(base, active, "licenses", "授权", "licenses"),
        admin_nav_link(base, active, "security", "审计", "audit"),
        admin_nav_link(base, active, "settings", "设置", "settings")
    );
    let side_menu = format!(
        "<a class='nav-item {}' href='{}'>激活总览</a>\
         <a class='nav-item {}' href='{}/requests'>申请审批</a>\
         <a class='nav-item {}' href='{}/invites'>邀请码管理</a>\
         <a class='nav-item {}' href='{}/licenses'>授权吊销</a>\
         <a class='nav-item {}' href='{}/audit'>审计与安全</a>\
         <a class='nav-item {}' href='{}/settings'>全局设置</a>",
        if active == "dashboard" { "active" } else { "" },
        escape_html(base),
        if active == "requests" { "active" } else { "" },
        escape_html(base),
        if active == "invites" { "active" } else { "" },
        escape_html(base),
        if active == "licenses" { "active" } else { "" },
        escape_html(base),
        if active == "security" { "active" } else { "" },
        escape_html(base),
        if active == "settings" { "active" } else { "" },
        escape_html(base),
    );
    format!(
        "<!doctype html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width, initial-scale=1'><title>iClaw Console</title><link rel='icon' type='image/png' href='/iclaw-logo.png'>\
         <style>:root{{--accent:#ff9500;--bg:#f5f5f7;--surface:#fff;--surface-soft:#fafafa;--border:#e5e7eb;--text:#111827;--text-muted:#6b7280}}*{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:var(--text);font-family:-apple-system,BlinkMacSystemFont,'SF Pro Text','PingFang SC','Helvetica Neue',sans-serif;-webkit-font-smoothing:antialiased}}a{{color:inherit;text-decoration:none}}\
         .shell{{max-width:1440px;margin:16px auto;padding:0 16px}}.topbar{{display:flex;align-items:center;justify-content:space-between;background:var(--surface);border:1px solid var(--border);border-radius:14px;padding:12px 14px}}\
         .brand{{display:flex;align-items:center;gap:10px}}.brand img{{width:30px;height:30px;border-radius:9px;border:1px solid var(--border)}}.brand h1{{font-size:16px;line-height:1.2;margin:0}}.brand .sub{{font-size:11px;color:var(--text-muted);margin-top:2px}}\
         .menu{{display:flex;gap:14px;font-size:12.5px}}.menu a{{color:#4b5563}}.menu .active{{color:#111827;font-weight:700}}.topbar-version{{font-size:11px;color:var(--text-muted);padding:0 12px;margin-right:4px}}.btn{{display:inline-flex;align-items:center;justify-content:center;gap:4px;padding:7px 12px;border:1px solid #d8dde6;border-radius:10px;background:linear-gradient(180deg,#fff,#f7f8fa);color:#111827;font-weight:600;font-size:12px;cursor:pointer;box-shadow:0 1px 0 rgba(17,24,39,.04);transition:all .16s ease}}.btn:hover{{border-color:#c7cedb;background:linear-gradient(180deg,#fff,#f2f4f7);transform:translateY(-1px)}}.btn-danger{{background:linear-gradient(180deg,#fff4f5,#ffe9ec);border-color:#fecdd3;color:#b91c1c}}\
         .layout{{display:grid;grid-template-columns:220px 1fr;gap:14px;margin-top:14px}}.sidebar,.content{{background:var(--surface);border:1px solid var(--border);border-radius:14px}}.sidebar{{padding:12px}}.content{{padding:12px}}\
         .section-title{{font-size:11px;color:var(--text-muted);margin:8px 0 10px;text-transform:uppercase;letter-spacing:.04em}}.nav-item{{display:block;padding:8px 10px;border-radius:9px;font-size:12px;color:#374151;margin-bottom:6px}}.nav-item.active{{background:#111827;color:#fff}}\
         .page-head{{background:var(--surface-soft);border:1px solid var(--border);border-radius:12px;padding:10px 12px;margin-bottom:10px}}.crumb{{font-size:11px;color:var(--text-muted)}}.page-title{{font-size:17px;font-weight:700;margin:3px 0 2px}}.page-sub{{font-size:12px;color:var(--text-muted)}}\
         .panel{{background:var(--surface-soft);border:1px solid var(--border);border-radius:12px;padding:11px;margin-top:10px}}table{{width:100%;border-collapse:collapse;font-size:12px}}th,td{{border-bottom:1px solid var(--border);padding:7px;text-align:left;vertical-align:top}}th{{color:#6b7280;font-size:11.5px}}\
         .chip{{padding:5px 9px;border:1px solid var(--border);border-radius:999px;text-decoration:none;color:#374151;margin-right:6px;font-size:11.5px}}.chip.active{{background:#111827;color:#fff;border-color:#111827}}code{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}}.time-text{{font-family:inherit;font-size:inherit;font-weight:inherit;color:inherit;letter-spacing:0}}\
         .toast-layer{{position:fixed;top:18px;right:18px;z-index:9999;pointer-events:none}}.status-toast{{min-width:360px;max-width:520px;background:#ffffff;border:2px solid #22c55e;border-radius:12px;box-shadow:0 18px 38px rgba(15,23,42,.18);display:flex;align-items:flex-start;gap:12px;padding:12px 12px 11px;pointer-events:auto;transform:translateY(-16px) scale(.98);opacity:0;animation:toast-in .24s cubic-bezier(.22,.61,.36,1) forwards}}.status-toast.error{{border-color:#ef4444}}.status-toast.warn{{border-color:#f59e0b}}.status-toast .badge{{display:inline-flex;align-items:center;justify-content:center;padding:2px 8px;border-radius:999px;font-size:11px;font-weight:800;background:#dcfce7;color:#166534;letter-spacing:.02em}}.status-toast.error .badge{{background:#fee2e2;color:#991b1b}}.status-toast.warn .badge{{background:#fef3c7;color:#92400e}}.status-toast .title{{font-size:13px;font-weight:800;color:#111827;line-height:1.2}}.status-toast .desc{{margin-top:2px;font-size:12px;color:#4b5563;line-height:1.35}}.status-toast .code-btn{{margin-top:7px;border:1px solid #16a34a;background:#22c55e;color:#052e16;border-radius:10px;padding:5px 9px;font-size:12px;font-weight:800;cursor:pointer}}.status-toast .close-btn{{margin-left:auto;border:0;background:transparent;color:#9ca3af;font-size:18px;line-height:1;cursor:pointer;padding:2px 4px;border-radius:8px}}.status-toast .close-btn:hover{{background:#f3f4f6;color:#111827}}.copy-toast{{position:fixed;right:18px;bottom:18px;z-index:9999;background:#111827;color:#fff;border-radius:10px;padding:8px 10px;font-size:12px;font-weight:700;box-shadow:0 12px 26px rgba(2,6,23,.32);opacity:0;transform:translateY(8px);animation:copy-in .18s ease forwards}}@keyframes toast-in{{to{{opacity:1;transform:translateY(0) scale(1)}}}}@keyframes toast-out{{to{{opacity:0;transform:translateY(-12px) scale(.98)}}}}@keyframes copy-in{{to{{opacity:1;transform:translateY(0)}}}}@keyframes copy-out{{to{{opacity:0;transform:translateY(8px)}}}}\
         @media (max-width:1120px){{.layout{{grid-template-columns:1fr}}.menu{{display:none}}}}</style></head><body>\
         <div class='shell'><div class='topbar'><div class='brand'><img src='/iclaw-logo.png' alt='iClaw logo'><div><h1>iClaw Console</h1><div class='sub'>Activation Mission Control</div></div></div>\
         <div class='menu'>{top_menu}</div><span class='topbar-version'>v{version}</span><form method='post' action='{base}/logout'><button class='btn'>退出登录</button></form></div>\
         <div class='layout'><aside class='sidebar'><div class='section-title'>导航</div>{side_menu}</aside><main class='content'>\
         <section class='page-head'><div class='crumb'>{breadcrumb}</div><div class='page-title'>{title}</div><div class='page-sub'>{subtitle}</div></section>\
         {content_html}</main></div></div></body></html>",
        top_menu = top_menu,
        version = env!("CARGO_PKG_VERSION"),
        base = escape_html(base),
        side_menu = side_menu,
        breadcrumb = escape_html(breadcrumb),
        title = escape_html(title),
        subtitle = escape_html(subtitle),
        content_html = content_html
    )
}

fn format_ts_short(raw: &str) -> String {
    if raw.is_empty() || raw == "-" {
        return "-".into();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        // 在控制台中统一按上海时间显示（UTC+8）
        if let Some(shanghai) = FixedOffset::east_opt(8 * 3600) {
            return dt.with_timezone(&shanghai).format("%Y-%m-%d %H:%M").to_string();
        }
        return dt.format("%Y-%m-%d %H:%M").to_string();
    }
    let normalized = raw.replace('T', " ");
    if normalized.len() >= 16 {
        return normalized[..16].to_string();
    }
    normalized
}

fn installation_id_to_uuid_display(id: &str) -> String {
    if !id.starts_with("sha256_") {
        return id.to_string();
    }
    let hex = &id[7..];
    if hex.len() < 32 {
        return id.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn compact_head_tail(raw: &str, head: usize, tail: usize) -> String {
    if raw.chars().count() <= head + tail + 1 {
        return raw.to_string();
    }
    let chars: Vec<char> = raw.chars().collect();
    let prefix: String = chars.iter().take(head).collect();
    let suffix: String = chars
        .iter()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}…{suffix}")
}

fn looks_uuid_like(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.len() != 36 {
        return false;
    }
    trimmed.chars().enumerate().all(|(i, c)| match i {
        8 | 13 | 18 | 23 => c == '-',
        _ => c.is_ascii_hexdigit(),
    })
}

fn compact_request_id_display(id: &str) -> String {
    compact_head_tail(id, 4, 4)
}

fn compact_installation_id_display(id: &str) -> String {
    if let Some(hex) = id.strip_prefix("sha256_") {
        return format!("sha256_{}", compact_head_tail(hex, 4, 4));
    }
    if looks_uuid_like(id) {
        return compact_head_tail(id, 4, 4);
    }
    id.to_string()
}

fn installation_device_indicator(id: &str) -> String {
    let digest = Sha256::digest(id.as_bytes());
    let alphabet = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut buffer: u64 = 0;
    let mut bits: usize = 0;
    let mut out = String::with_capacity(8);

    for b in digest {
        buffer = (buffer << 8) | (b as u64);
        bits += 8;
        while bits >= 5 && out.len() < 8 {
            let idx = ((buffer >> (bits - 5)) & 0x1f) as usize;
            out.push(alphabet[idx] as char);
            bits -= 5;
        }
        if out.len() == 8 {
            break;
        }
    }
    while out.len() < 8 {
        out.push('X');
    }
    format!("{}-{}", &out[0..4], &out[4..8])
}

fn normalize_token(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

fn installation_matches_query(installation_id: &str, query: &str) -> bool {
    let q = normalize_token(query);
    if q.is_empty() {
        return true;
    }
    let raw = normalize_token(installation_id);
    let uuid = normalize_token(&installation_id_to_uuid_display(installation_id));
    let indicator = normalize_token(&installation_device_indicator(installation_id));
    raw.contains(&q) || uuid.contains(&q) || indicator.contains(&q)
}

fn copy_badge_script() -> &'static str {
    "<script>(function(){\
      function showCopyToast(text){\
        const prev=document.getElementById('copy-toast-lite');\
        if(prev) prev.remove();\
        const el=document.createElement('div');\
        el.id='copy-toast-lite';\
        el.className='copy-toast';\
        el.textContent=text;\
        document.body.appendChild(el);\
        setTimeout(function(){el.style.animation='copy-out .18s ease forwards';setTimeout(function(){el.remove();},200);},1200);\
      }\
      document.querySelectorAll('button[data-copy]').forEach(function(btn){\
        if(btn.dataset.copyBound==='1') return;\
        btn.dataset.copyBound='1';\
        btn.addEventListener('click',async function(){\
          const text=btn.getAttribute('data-copy')||'';\
          if(!text) return;\
          try{\
            await navigator.clipboard.writeText(text);\
            showCopyToast('设备识别码已复制');\
            const old=btn.textContent;\
            btn.textContent='已复制';\
            setTimeout(function(){btn.textContent=old;},900);\
          }catch(_){showCopyToast('复制失败，请手动复制');}\
        });\
      });\
    })();</script>"
}

fn render_admin_login(error: Option<&str>, admin_ui_base: &str) -> String {
    let err = error
        .map(|m| format!("<div style='color:#fecaca;background:#450a0a;border:1px solid #7f1d1d;padding:10px 12px;border-radius:10px;margin-bottom:12px'>{}</div>", escape_html(m)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html><head><meta charset='utf-8'><title>iClaw Admin Login</title><link rel='icon' type='image/png' href='/iclaw-logo.png'><style>:root{{--accent:#ff9500;--bg-primary:#1c1c1e;--bg-secondary:#2c2c2e;--separator:#38383a;--text-primary:#ffffff;--text-secondary:#aeaeb2;}}body{{font-family:-apple-system,BlinkMacSystemFont,'SF Pro Text','PingFang SC','Helvetica Neue',sans-serif;background:radial-gradient(900px 500px at 10% -10%,#3d2a00,var(--bg-primary));display:flex;align-items:center;justify-content:center;height:100vh;margin:0;color:var(--text-primary)}} .card{{background:rgba(44,44,46,.92);border:1px solid var(--separator);border-radius:16px;padding:28px;min-width:360px;box-shadow:0 20px 40px rgba(2,6,23,.45)}} .brand{{display:flex;align-items:center;gap:10px;margin-bottom:8px}} .brand img{{width:34px;height:34px;border-radius:8px;border:1px solid var(--separator)}} .subtitle{{color:var(--text-secondary);font-size:13px;margin-top:-2px;margin-bottom:14px}} input{{width:100%;padding:11px;border:1px solid var(--separator);border-radius:10px;margin:6px 0 12px;background:var(--bg-primary);color:var(--text-primary)}} button{{width:100%;padding:11px;border:0;border-radius:10px;background:var(--accent);color:#1c1c1e;cursor:pointer;font-weight:700}} button:hover{{filter:brightness(1.08)}}</style></head><body><div class='card'><div class='brand'><img src='/iclaw-logo.png' alt='iClaw logo'><h2 style='margin:0'>iClaw 激活管理台</h2></div><div class='subtitle'>Console Mission Control · Activation</div>{err}<form method='post' action='{}/login'><label>用户名</label><input name='username' autocomplete='username' required><label>密码</label><input name='password' type='password' autocomplete='current-password' required><button type='submit'>登录</button></form></div></body></html>",
        escape_html(admin_ui_base)
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn random_code(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

fn sign_license_jws(
    state: &AppState,
    license_id: &str,
    installation_id: &str,
    app_id: &str,
    tier: &str,
    min_build: &str,
    iat: i64,
    exp: i64,
    offline_grace_exp: i64,
) -> String {
    let header = serde_json::json!({
        "alg": "EdDSA",
        "typ": "JWT",
        "kid": state.cfg.key_id
    });
    let payload = serde_json::json!({
        "iss": "iclaw-license",
        "aud": app_id,
        "sub": license_id,
        "installation_id": installation_id,
        "tier": tier,
        "iat": iat,
        "nbf": iat,
        "exp": exp,
        "offline_grace_exp": offline_grace_exp,
        "min_build": min_build,
        "ver": 1
    });
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap_or_default());
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap_or_default());
    let signing_input = format!("{header_b64}.{payload_b64}");
    let sig = state.signing_key.sign(signing_input.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
    format!("{signing_input}.{sig_b64}")
}

async fn write_audit(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    target_type: &str,
    target_id: &str,
    result: &str,
    message: Option<String>,
    ip: Option<String>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO audit_logs (actor, action, target_type, target_id, result, message, ip, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(actor)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(result)
    .bind(message)
    .bind(ip)
    .bind(now_rfc3339())
    .execute(pool)
    .await;
}
