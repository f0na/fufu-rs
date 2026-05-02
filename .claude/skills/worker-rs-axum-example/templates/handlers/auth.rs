// src/handlers/auth.rs - Authentication Handlers

use axum::{extract::State, Json};
use std::sync::Arc;
use worker::Env;

use crate::auth::{generate_token, hash_password, verify_password, Claims};
use crate::db::get_db;
use crate::error::AppError;
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, User, UserResponse};

/// Register a new user
/// POST /api/auth/register
#[worker::send]
pub async fn register(
    State(env): State<Arc<Env>>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Validate input
    if body.username.is_empty() || body.password.is_empty() {
        return Err(AppError::BadRequest("Username and password are required".into()));
    }

    let db = get_db(&env)?;
    let user_id = uuid::Uuid::now_v7().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let secret = env.var("JWT_SECRET")?.to_string();
    let password_hash = hash_password(&body.password, &secret);

    // Check if username already exists
    let existing = db
        .prepare("SELECT id FROM users WHERE username = ?1")
        .bind(&[body.username.clone().into()])?
        .first::<serde_json::Value>(None)
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Username already exists".into()));
    }

    // Create user
    db.prepare(
        "INSERT INTO users (id, username, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)"
    )
    .bind(&[
        user_id.clone().into(),
        body.username.clone().into(),
        password_hash.into(),
        now.clone().into(),
    ])?
    .run()
    .await?;

    // Generate token
    let token = generate_token(&user_id, &body.username, &secret)
        .map_err(|e| AppError::Internal(e))?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse {
            id: user_id,
            username: body.username,
            created_at: now,
        },
    }))
}

/// Login with existing credentials
/// POST /api/auth/login
#[worker::send]
pub async fn login(
    State(env): State<Arc<Env>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Validate input
    if body.username.is_empty() || body.password.is_empty() {
        return Err(AppError::BadRequest("Username and password are required".into()));
    }

    let db = get_db(&env)?;
    let secret = env.var("JWT_SECRET")?.to_string();

    // Find user
    let user = db
        .prepare("SELECT id, username, password_hash, created_at FROM users WHERE username = ?1")
        .bind(&[body.username.clone().into()])?
        .first::<User>(None)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".into()))?;

    // Verify password
    if !verify_password(&body.password, &secret, &user.password_hash) {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    // Generate token
    let token = generate_token(&user.id, &user.username, &secret)
        .map_err(|e| AppError::Internal(e))?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse::from(user),
    }))
}

/// Get current authenticated user
/// GET /api/auth/me
/// Protected route - requires valid JWT
#[worker::send]
pub async fn me(
    claims: Claims,
    State(env): State<Arc<Env>>,
) -> Result<Json<UserResponse>, AppError> {
    let db = get_db(&env)?;

    let user = db
        .prepare("SELECT id, username, password_hash, created_at FROM users WHERE id = ?1")
        .bind(&[claims.sub.into()])?
        .first::<User>(None)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    Ok(Json(UserResponse::from(user)))
}