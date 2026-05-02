// src/auth/jwt.rs - JWT Authentication

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::error::AppError;

/// JWT Claims structure
/// Contains user identification and token metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Username
    pub username: String,
    /// Expiration timestamp
    pub exp: usize,
    /// Issued at timestamp
    pub iat: usize,
}

/// Extractor for authenticated requests
/// Automatically validates JWT from Authorization header
impl FromRequestParts<Arc<Env>> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Env,
    ) -> Result<Self, Self::Rejection> {
        // Extract token from Authorization header
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|auth_header| auth_header.to_str().ok())
            .and_then(|auth_value| {
                // Expect "Bearer <token>" format
                if auth_value.starts_with("Bearer ") {
                    Some(auth_value[7..].to_owned())
                } else {
                    None
                }
            })
            .ok_or_else(|| AppError::Unauthorized("Missing authorization token".into()))?;

        // Get JWT secret from environment
        let secret = state.var("JWT_SECRET")
            .map_err(|_| AppError::Internal("JWT_SECRET not configured".into()))?;

        // Decode and validate token
        let decoding_key = DecodingKey::from_secret(secret.to_string().as_ref());
        let token_data = decode::<Claims>(&token, &decoding_key, &Validation::default())
            .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

        Ok(token_data.claims)
    }
}

/// Generate a new JWT token
///
/// # Arguments
/// * `user_id` - Unique user identifier
/// * `username` - User's display name
/// * `secret` - JWT signing secret
///
/// # Returns
/// * `Result<String, String>` - Encoded token or error message
pub fn generate_token(user_id: &str, username: &str, secret: &str) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = now + 24 * 60 * 60; // 24 hours expiration

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}