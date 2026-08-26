use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use rand::Rng;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::SharedState;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn err(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": message })))
}

#[derive(Deserialize)]
pub struct PairingCodeRequest {
    peer_id: String,
}

const PAIRING_CODE_TTL_SECONDS: i64 = 5 * 60;

pub async fn create_pairing_code(
    State(state): State<Arc<SharedState>>,
    Json(req): Json<PairingCodeRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !state.pairing_rate_limiter.check(&req.peer_id).await {
        return Err(err(StatusCode::TOO_MANY_REQUESTS, "rate limited, try again shortly"));
    }

    let conn = state
        .db
        .get_conn()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;

    let now = now_secs();
    let expires_at = now + PAIRING_CODE_TTL_SECONDS;

    let code = loop {
        let candidate = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32));
        let active: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM pairing_codes WHERE code = ?1 AND expires_at > ?2 AND consumed = 0",
                rusqlite::params![candidate, now],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
        if active.is_none() {
            break candidate;
        }
    };

    conn.execute(
        "INSERT INTO pairing_codes (code, peer_id, expires_at, consumed) VALUES (?1, ?2, ?3, 0)",
        rusqlite::params![code, req.peer_id, expires_at],
    )
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "failed to create pairing code"))?;

    Ok(Json(json!({ "code": code, "expires_at": expires_at })))
}

#[derive(Deserialize)]
pub struct LinkRequest {
    code: String,
    name: Option<String>,
}

#[derive(Serialize)]
struct ComputerView {
    id: i64,
    name: String,
    peer_id: String,
    linked_at: i64,
}

pub async fn link_computer(
    State(state): State<Arc<SharedState>>,
    auth_user: AuthUser,
    Json(req): Json<LinkRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let conn = state
        .db
        .get_conn()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;

    let now = now_secs();
    let invalid_code = || err(StatusCode::NOT_FOUND, "invalid or expired code");

    let peer_id: Option<String> = conn
        .query_row(
            "SELECT peer_id FROM pairing_codes WHERE code = ?1 AND expires_at > ?2 AND consumed = 0",
            rusqlite::params![req.code, now],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    let peer_id = peer_id.ok_or_else(invalid_code)?;

    conn.execute(
        "UPDATE pairing_codes SET consumed = 1 WHERE code = ?1",
        rusqlite::params![req.code],
    )
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "failed to consume code"))?;

    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, user_id FROM linked_computers WHERE peer_id = ?1",
            rusqlite::params![peer_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    let name = req.name.filter(|n| !n.is_empty()).unwrap_or_else(|| "New Computer".to_string());

    let computer_id = if let Some((existing_id, existing_user_id)) = existing {
        if existing_user_id != auth_user.user_id {
            return Err(err(
                StatusCode::CONFLICT,
                "this computer is already linked to another account",
            ));
        }
        existing_id
    } else {
        conn.execute(
            "INSERT INTO linked_computers (user_id, peer_id, name, linked_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![auth_user.user_id, peer_id, name, now],
        )
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "failed to link computer"))?;
        conn.last_insert_rowid()
    };

    Ok(Json(json!({
        "computer": ComputerView {
            id: computer_id,
            name,
            peer_id,
            linked_at: now,
        }
    })))
}

pub async fn list_computers(
    State(state): State<Arc<SharedState>>,
    auth_user: AuthUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let rows: Vec<(i64, String, String, i64, Option<i64>)> = {
        let conn = state
            .db
            .get_conn()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, peer_id, linked_at, last_seen_at FROM linked_computers WHERE user_id = ?1",
            )
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

        let rows = stmt
            .query_map(rusqlite::params![auth_user.user_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
        rows
    };

    let mut computers = Vec::with_capacity(rows.len());
    for (id, name, peer_id, linked_at, last_seen_at) in rows {
        let online = state.relay.is_registered(&peer_id).await;
        computers.push(json!({
            "id": id,
            "name": name,
            "peer_id": peer_id,
            "online": online,
            "linked_at": linked_at,
            "last_seen_at": last_seen_at,
        }));
    }

    Ok(Json(json!({ "computers": computers })))
}

pub async fn delete_computer(
    State(state): State<Arc<SharedState>>,
    auth_user: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let conn = state
        .db
        .get_conn()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;

    let affected = conn
        .execute(
            "DELETE FROM linked_computers WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, auth_user.user_id],
        )
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    if affected == 0 {
        return Err(err(StatusCode::NOT_FOUND, "not found"));
    }

    Ok(Json(json!({ "ok": true })))
}
