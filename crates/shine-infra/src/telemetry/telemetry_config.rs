use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleLogFormat {
    #[default]
    Compact,
    Json,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Logging {
    /// Disable logging
    None,

    /// Write logs to the standard output
    Console {
        #[serde(default)]
        format: ConsoleLogFormat,
    },

    /// Export OpenTelemetry log records
    #[cfg(feature = "ot_otlp")]
    #[serde(rename = "otlp")]
    OpenTelemetryProtocol { endpoint: String },
}

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

    /// Enable OpenTelemetry exporter (for example: https://www.jaegertracing.io)
    #[cfg(feature = "ot_otlp")]
    #[serde(rename = "otlp")]
    OpenTelemetryProtocol { endpoint: String },

    /// Enable AppInsight tracing
    #[cfg(feature = "ot_app_insight")]
    AppInsight { connection_string: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryConfig {
    pub default_level: Option<String>,
    pub allow_reconfigure: bool,
    pub logging: Logging,
    pub metrics: Metering,
    pub tracing: Tracing,
}
