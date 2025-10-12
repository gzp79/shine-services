use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Metering {
    /// Disable metrics
    None,

    /// Enable OpenTelemetry exporter
    #[cfg(feature = "ot_otlp")]
    #[serde(rename = "otlp")]
    OpenTelemetryProtocol { endpoint: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Tracing {
    /// Disable tracing
    None,

    /// Enable OpenTelemetry tracing to the standard output
    StdOut,

    /// Enable OpenTelemetry exporter (for example: https://www.jaegertracing.io)
    #[cfg(feature = "ot_otlp")]
    #[serde(rename = "otlp")]
    OpenTelemetryProtocol { endpoint: String },

    /// Enable Zipkin tracing (https://zipkin.io/)
    #[cfg(feature = "ot_zipkin")]
    Zipkin,

    /// Enable AppInsight tracing
    #[cfg(feature = "ot_app_insight")]
    AppInsight { connection_string: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryConfig {
    pub enable_console_log: bool,
    pub default_level: Option<String>,
    pub allow_reconfigure: bool,
    pub metrics: Metering,
    pub tracing: Tracing,
}
