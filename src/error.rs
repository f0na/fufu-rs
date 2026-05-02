use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Worker(worker::Error),
    BadRequest(String),
    WrongCredentials,
    TotpRequired,
    NotFound(String),
    Conflict(String),
    RateLimited,
    TotpInvalid,
    EmailCodeInvalid,
    TempTokenExpired,
    Internal(String),
    ExternalApiFailure(String),
}

impl AppError {
    fn business_code(&self) -> u16 {
        match self {
            Self::Worker(_) | Self::Internal(_) => 5001,
            Self::BadRequest(_) => 1001,
            Self::WrongCredentials => 1002,
            Self::TotpRequired => 1003,
            Self::NotFound(_) => 1004,
            Self::Conflict(_) => 1005,
            Self::RateLimited => 1006,
            Self::TotpInvalid => 2001,
            Self::EmailCodeInvalid => 2002,
            Self::TempTokenExpired => 2003,
            Self::ExternalApiFailure(_) => 5002,
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            Self::Worker(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::WrongCredentials
            | Self::TotpRequired
            | Self::TotpInvalid
            | Self::EmailCodeInvalid
            | Self::TempTokenExpired => StatusCode::UNAUTHORIZED,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::ExternalApiFailure(_) => StatusCode::BAD_GATEWAY,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Worker(e) => format!("Worker error: {}", e),
            Self::BadRequest(msg) => msg.clone(),
            Self::WrongCredentials => "邮箱或密码错误".into(),
            Self::TotpRequired => "需要 TOTP 第二步验证".into(),
            Self::NotFound(msg) => msg.clone(),
            Self::Conflict(msg) => msg.clone(),
            Self::RateLimited => "请求频率超限".into(),
            Self::TotpInvalid => "TOTP 验证码错误".into(),
            Self::EmailCodeInvalid => "邮箱验证码错误或已过期".into(),
            Self::TempTokenExpired => "临时令牌已过期".into(),
            Self::Internal(msg) => msg.clone(),
            Self::ExternalApiFailure(msg) => msg.clone(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl From<worker::Error> for AppError {
    fn from(e: worker::Error) -> Self {
        Self::Worker(e)
    }
}

impl From<worker::KvError> for AppError {
    fn from(e: worker::KvError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => Self::WrongCredentials,
            _ => Self::Internal(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.http_status();
        let body = Json(json!({
            "error": {
                "code": self.business_code(),
                "message": self.message()
            }
        }));
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
