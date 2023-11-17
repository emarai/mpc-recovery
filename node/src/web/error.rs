use axum::extract::rejection::JsonRejection;
use reqwest::StatusCode;

use crate::protocol::{ConsensusError, CryptographicError, MpcMessage};

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// This enum error type serves as one true source of all futures in sign-node
/// crate. It is used to unify all errors that can happen in the application.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
    #[error(transparent)]
    Protocol(#[from] ConsensusError),
    #[error(transparent)]
    Cryptography(#[from] CryptographicError),
    #[error(transparent)]
    Message(#[from] tokio::sync::mpsc::error::SendError<MpcMessage>),
    #[error(transparent)]
    Rpc(#[from] near_fetch::Error),
    #[error("node is not running")]
    NotRunning,
}

// We implement `IntoResponse` so MpcSignError can be used as a response
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Error::JsonExtractorRejection(json_rejection) => {
                (json_rejection.status(), json_rejection.body_text())
            }
            Error::Protocol(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            Error::Cryptography(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            Error::Message(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            Error::Rpc(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            err @ Error::NotRunning => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        (status, axum::Json(message)).into_response()
    }
}
