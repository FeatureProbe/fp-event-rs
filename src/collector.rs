use crate::event::PackedData;
use axum::{
    async_trait,
    extract::{Extension, Json},
    headers::HeaderName,
    http::{
        header::{self, AUTHORIZATION},
        StatusCode,
    },
    response::{IntoResponse, Response},
    TypedHeader,
};
use headers::{Error, Header, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use std::collections::VecDeque;
use thiserror::Error;

#[async_trait]
pub trait EventHandler {
    async fn handle_events(
        &self,
        sdk_key: String,
        data: VecDeque<PackedData>,
    ) -> Result<Response, FPEventError>;
}

pub async fn post_events<T>(
    Json(data): Json<VecDeque<PackedData>>,
    TypedHeader(SdkAuthorization(sdk_key)): TypedHeader<SdkAuthorization>,
    Extension(handler): Extension<T>,
) -> Result<Response, FPEventError>
where
    T: EventHandler + Clone + Send + Sync + 'static,
{
    handler.handle_events(sdk_key, data).await?;
    let status = StatusCode::OK;
    let body = "";
    Ok((status, cors_headers(), body).into_response())
}

pub fn cors_headers() -> [(HeaderName, &'static str); 4] {
    [
        (header::CONTENT_TYPE, "application/json"),
        (header::ACCESS_CONTROL_ALLOW_HEADERS, "*"),
        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        (
            header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, PUT, DELETE, OPTIONS",
        ),
    ]
}

#[derive(Debug, Error, Deserialize)]
pub enum FPEventError {
    #[error("not found {0}")]
    NotFound(String),
    #[error("user base64 decode error")]
    UserDecodeError,
    #[error("config error: {0}")]
    ConfigError(String),
}

impl IntoResponse for FPEventError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            FPEventError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            FPEventError::UserDecodeError => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, cors_headers(), body).into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct SdkAuthorization(pub String);

impl SdkAuthorization {
    pub fn encode(&self) -> HeaderValue {
        HeaderValue::from_str(&self.0).expect("valid header value")
    }
}

impl Header for SdkAuthorization {
    fn name() -> &'static HeaderName {
        &AUTHORIZATION
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i HeaderValue>,
    {
        match values.next() {
            Some(v) => match v.to_str() {
                Ok(s) => Ok(SdkAuthorization(s.to_owned())),
                Err(_) => Err(Error::invalid()),
            },
            None => Err(Error::invalid()),
        }
    }

    fn encode<E: Extend<HeaderValue>>(&self, values: &mut E) {
        if let Ok(value) = HeaderValue::from_str(&self.0) {
            values.extend(std::iter::once(value))
        }
    }
}
