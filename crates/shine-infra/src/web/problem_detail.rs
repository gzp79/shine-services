use crate::serde::serde_status_code;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt;
use url::Url;

pub trait ProblemType {
    const TYPE: &'static str;
}

pub mod problems {
    pub const INPUT_PATH: &str = "input-path-format";
    pub const INPUT_QUERY: &str = "input-query-format";
    pub const INPUT_BODY: &str = "input-body-format";
    pub const INPUT_VALIDATION: &str = "input-validation";
}

/// Implementation of a Problem Details response for HTTP APIs as of
/// the specification [RFC-7807](https://datatracker.ietf.org/doc/html/rfc7807).
#[derive(Debug, Serialize)]
pub struct Problem {
    #[serde(rename = "status", with = "serde_status_code")]
    pub status: StatusCode,
    #[serde(rename = "type")]
    pub ty: &'static str,
    #[serde(rename = "instance")]
    pub instance: Option<Url>,
    #[serde(rename = "detail")]
    pub detail: String,

    #[serde(rename = "extension")]
    pub extension: JsonValue,
    // This property is returned only if service configuration allows it
    #[serde(rename = "sensitive")]
    pub sensitive: JsonValue,
}

impl Problem {
    pub fn new(status: StatusCode, ty: &'static str) -> Self {
        Problem {
            status,
            ty,
            instance: None,
            detail: String::new(),
            extension: JsonValue::Null,
            sensitive: JsonValue::Null,
        }
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not-found")
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    pub fn unauthorized_ty(ty: &'static str) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, ty)
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }

    pub fn internal_error() -> Self {
        Problem::new(StatusCode::INTERNAL_SERVER_ERROR, "server-error")
    }

    pub fn internal_error_ty(ty: &'static str) -> Self {
        Problem::new(StatusCode::INTERNAL_SERVER_ERROR, ty)
    }

    pub fn bad_request(ty: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ty)
    }

    pub fn conflict(ty: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, ty)
    }

    pub fn precondition_failed(ty: &'static str) -> Self {
        Self::new(StatusCode::PRECONDITION_FAILED, ty)
    }

    pub fn with_detail<S: ToString>(self, detail: S) -> Self {
        Self {
            detail: detail.to_string(),
            ..self
        }
    }

    pub fn with_instance_str<S: AsRef<str>>(self, instance: S) -> Self {
        self.with_instance(Url::parse(instance.as_ref()).ok())
    }

    pub fn with_instance(self, instance: Option<Url>) -> Self {
        Self { instance, ..self }
    }

    pub fn with_extension<S: Serialize>(self, extension: S) -> Self {
        Self {
            extension: serde_json::to_value(extension).unwrap(),
            ..self
        }
    }

    pub fn with_extension_dbg<S>(self, extension: S) -> Self
    where
        S: fmt::Debug,
    {
        self.with_extension(format!("{:#?}", extension))
    }

    pub fn with_sensitive<S>(self, extension: S) -> Self
    where
        S: Serialize,
    {
        Self {
            sensitive: serde_json::to_value(extension).unwrap(),
            ..self
        }
    }

    pub fn with_sensitive_dbg<S>(self, extension: S) -> Self
    where
        S: fmt::Debug,
    {
        self.with_sensitive(format!("{:#?}", extension))
    }
}

#[derive(Clone)]
pub struct ProblemConfig {
    include_internal: bool,
}

impl ProblemConfig {
    pub fn new(include_internal: bool) -> Self {
        Self { include_internal }
    }

    pub fn into_layer(self) -> Extension<Self> {
        Extension(self)
    }

    pub fn transform<P>(&self, problem: P) -> Problem
    where
        P: Into<Problem>,
    {
        let problem = problem.into();
        if !self.include_internal {
            Problem {
                sensitive: Default::default(),
                ..problem
            }
        } else {
            problem
        }
    }
}

pub trait IntoProblemResponse {
    fn into_response(self, config: &ProblemConfig) -> ProblemResponse;
}

impl<T> IntoProblemResponse for T
where
    T: Into<Problem>,
{
    fn into_response(self, config: &ProblemConfig) -> ProblemResponse {
        ProblemResponse::new(config, self)
    }
}

/// Problem response
pub struct ProblemResponse {
    pub config: ProblemConfig,
    pub problem: Problem,
}

impl ProblemResponse {
    pub fn new<P>(config: &ProblemConfig, problem: P) -> Self
    where
        P: Into<Problem>,
    {
        Self {
            config: config.clone(),
            problem: problem.into(),
        }
    }
}

impl IntoResponse for ProblemResponse {
    fn into_response(self) -> Response {
        let ProblemResponse { problem, config } = self;
        log::info!("problem response: {:#?}", problem);
        let problem = config.transform(problem);
        (problem.status, Json(problem)).into_response()
    }
}

/// Problem response that preserves the original problem type.
pub struct ErrorResponse<E>
where
    E: Into<Problem>,
{
    pub config: ProblemConfig,
    pub problem: E,
}

impl<E> ErrorResponse<E>
where
    E: Into<Problem>,
{
    pub fn new(config: &ProblemConfig, problem: E) -> Self {
        Self {
            config: config.clone(),
            problem,
        }
    }
}

impl<E> IntoResponse for ErrorResponse<E>
where
    E: Into<Problem>,
{
    fn into_response(self) -> Response {
        let ErrorResponse { config, problem } = self;
        ProblemResponse::new(&config, problem).into_response()
    }
}
