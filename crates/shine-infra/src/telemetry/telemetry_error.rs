use crate::web::responses::Problem;
use opentelemetry_sdk::{metrics::MetricError, trace::TraceError};
use std::error::Error as StdError;
use thiserror::Error as ThisError;
use tracing::subscriber::SetGlobalDefaultError;
use tracing_subscriber::filter::ParseError;

#[derive(Debug, ThisError)]
pub enum TelemetryBuildError {
    #[error(transparent)]
    SetGlobalTracing(#[from] SetGlobalDefaultError),
    #[error("Default log format could not be parsed")]
    DefaultLogError(#[from] ParseError),
    #[cfg(feature = "ot_app_insight")]
    #[error(transparent)]
    AppInsightConfigError(Box<dyn StdError + Send + Sync + 'static>),
    #[cfg(feature = "ot_otlp")]
    #[error(transparent)]
    OtlpBuildError(#[from] opentelemetry_otlp::ExporterBuildError),
    #[cfg(feature = "ot_zipkin")]
    #[error(transparent)]
    ZipkinBuildError(#[from] opentelemetry_zipkin::ExporterBuildError),
    #[error(transparent)]
    TraceError(#[from] TraceError),
    #[error(transparent)]
    MetricsError(#[from] MetricError),
}

#[derive(Debug, ThisError)]
pub enum TelemetryError {
    #[error("Failed to update trace configuration")]
    TraceUpdateConfig(String),
    #[error("Reconfigure is not enabled")]
    TraceNoReconfigure,
}

impl From<TelemetryError> for Problem {
    fn from(value: TelemetryError) -> Self {
        Problem::internal_error()
            .with_detail(value.to_string())
            .with_sensitive_dbg(value)
    }
}
