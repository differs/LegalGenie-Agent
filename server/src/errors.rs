use crate::api::ApiEnvelope;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest { message: String, error_code: i32 },

    #[error("{message}")]
    Unauthorized { message: String, error_code: i32 },

    #[error("{message}")]
    Forbidden { message: String, error_code: i32 },

    #[error("{message}")]
    NotFound { message: String, error_code: i32 },

    #[error("{message}")]
    Conflict { message: String, error_code: i32 },

    #[error("{message}")]
    Internal { message: String, error_code: i32 },
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
            error_code: 400000,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized {
            message: message.into(),
            error_code: 401000,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden {
            message: message.into(),
            error_code: 403000,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
            error_code: 404000,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
            error_code: 409000,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            error_code: 500000,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, error_code) = match self {
            AppError::BadRequest {
                message,
                error_code,
            } => (StatusCode::BAD_REQUEST, message, error_code),
            AppError::Unauthorized {
                message,
                error_code,
            } => (StatusCode::UNAUTHORIZED, message, error_code),
            AppError::Forbidden {
                message,
                error_code,
            } => (StatusCode::FORBIDDEN, message, error_code),
            AppError::NotFound {
                message,
                error_code,
            } => (StatusCode::NOT_FOUND, message, error_code),
            AppError::Conflict {
                message,
                error_code,
            } => (StatusCode::CONFLICT, message, error_code),
            AppError::Internal {
                message,
                error_code,
            } => (StatusCode::INTERNAL_SERVER_ERROR, message, error_code),
        };

        let body = ApiEnvelope::err(status.as_u16(), error_code, message);
        (status, Json(body)).into_response()
    }
}
