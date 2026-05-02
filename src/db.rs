use std::sync::Arc;
use worker::{D1Database, Env};

use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub enum Db {
    Core,
    Posts,
    Media,
    Bangumi,
    Social,
    Likes,
    Legal,
    Auth,
}

impl Db {
    pub fn binding_name(&self) -> &'static str {
        match self {
            Self::Core => "FUFU_CORE",
            Self::Posts => "FUFU_POSTS",
            Self::Media => "FUFU_MEDIA",
            Self::Bangumi => "FUFU_BANGUMI",
            Self::Social => "FUFU_SOCIAL",
            Self::Likes => "FUFU_LIKES",
            Self::Legal => "FUFU_LEGAL",
            Self::Auth => "FUFU_AUTH",
        }
    }

    pub fn all() -> &'static [Db] {
        &[
            Self::Core,
            Self::Posts,
            Self::Media,
            Self::Bangumi,
            Self::Social,
            Self::Likes,
            Self::Legal,
            Self::Auth,
        ]
    }
}

pub fn get_db(env: &Arc<Env>, db: Db) -> Result<D1Database, AppError> {
    env.d1(db.binding_name()).map_err(AppError::Worker)
}
