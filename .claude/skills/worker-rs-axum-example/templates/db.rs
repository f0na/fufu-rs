// src/db.rs - Database Connection Helper

use std::sync::Arc;
use worker::{D1Database, Env};
use crate::error::AppError;

/// Get D1 database connection from environment
///
/// # Arguments
/// * `env` - Arc-wrapped Worker environment containing bindings
///
/// # Returns
/// * `Result<D1Database, AppError>` - Database connection or error
///
/// # Example
/// ```
/// let db = get_db(&env)?;
/// let results = db.prepare("SELECT * FROM users").all().await?;
/// ```
pub fn get_db(env: &Arc<Env>) -> Result<D1Database, AppError> {
    env.d1("db").map_err(AppError::Worker)
}