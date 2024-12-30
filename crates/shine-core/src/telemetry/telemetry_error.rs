use opentelemetry::trace::TraceError;
use opentelemetry_sdk::metrics::MetricError;

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
    #[error(transparent)]
    TraceError(#[from] TraceError),
    #[error(transparent)]
    MetricsError(#[from] MetricError),
}
