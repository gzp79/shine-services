use crate::telemetry::log_metrics::LogMetricsLayer;
use crate::telemetry::otel_http::TRACING_TARGET;
use crate::telemetry::{
    ConsoleLogFormat, Logging, Metering, OtelLayer, TelemetryBuildError, TelemetryConfig, TelemetryError, Tracing,
};
use opentelemetry::{
    global,
    metrics::{Meter, MeterProvider},
    trace::TracerProvider,
    InstrumentationScope,
};
use opentelemetry_sdk::{
    logs::SdkLoggerProvider,
    metrics::SdkMeterProvider,
    trace::{Sampler, SdkTracerProvider},
    Resource,
};
use std::sync::{Arc, RwLock};
use tracing::{Dispatch, Subscriber};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    filter::{filter_fn, EnvFilter, LevelFilter},
    layer::{Filter, SubscriberExt},
    registry::LookupSpan,
    reload, Layer,
};

#[cfg(feature = "ot_otlp")]
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
#[cfg(feature = "ot_otlp")]
use opentelemetry_otlp::WithExportConfig;

#[derive(Debug, Clone)]
pub struct DynConfig {
    pub filter: String,
}

trait DynHandle: Send + Sync {
    fn set_configuration(&mut self, config: DynConfig) -> Result<(), String>;
    fn get_configuration(&self) -> &DynConfig;
}

struct WrapHandle<L, S>
where
    L: From<EnvFilter> + Send + Sync + 'static,
    S: Subscriber,
{
    handle: reload::Handle<L, S>,
    config: DynConfig,
}

impl<L, S> DynHandle for WrapHandle<L, S>
where
    L: From<EnvFilter> + Send + Sync + 'static,
    S: Subscriber,
{
    fn set_configuration(&mut self, mut new_config: DynConfig) -> Result<(), String> {
        new_config.filter.retain(|c| !c.is_whitespace());
        let new_filter = new_config.filter.parse::<EnvFilter>().map_err(|e| format!("{e}"))?;
        self.handle.reload(new_filter).map_err(|e| format!("{e}"))?;
        self.config = new_config;
        Ok(())
    }

    fn get_configuration(&self) -> &DynConfig {
        &self.config
    }
}

#[derive(Clone)]
struct Metrics {
    provider: SdkMeterProvider,
    meter: Meter,
}

/// Telemetry service.
/// Metrics:
///  - Uses opentelemetry_sdk to provide metrics in Prometheus format.
///  - The Meter type can be used to define new metrics.
///
/// Tracings:
///  - The tracing crate is used as the frontend and some configured opentelemetry exporter is used as the backend.
///
/// Logs
///  - The trace::trace,debug,info,warn,error! macros can be used
///  - For convenience, the log::trace,debug,info,warn,error! macros are also available and channelled to the tracing layer
///  - Log records can be disabled, formatted for the console, or exported over OTLP independently from traces
#[derive(Clone)]
pub struct TelemetryService {
    tracer_provider: Option<SdkTracerProvider>,
    logger_provider: Option<SdkLoggerProvider>,
    metrics: Option<Metrics>,
    reconfigure: Option<Arc<RwLock<dyn DynHandle>>>,
}

impl TelemetryService {
    pub async fn new(service_name: &'static str, config: &TelemetryConfig) -> Result<Self, TelemetryBuildError> {
        let mut service = TelemetryService {
            tracer_provider: None,
            logger_provider: None,
            metrics: None,
            reconfigure: None,
        };
        service.install_telemetry(service_name, config)?;

        log::trace!("trace - log: ok");
        log::debug!("debug - log: ok");
        log::info!("info  - log: ok");
        log::warn!("warn  - log: ok");
        log::error!("error - log: ok");
        tracing::trace!("trace - tracing: ok");
        tracing::debug!("debug - tracing: ok");
        tracing::info!("info  - tracing: ok");
        tracing::warn!("warn  - tracing: ok");
        tracing::error!("error - tracing: ok");

        Ok(service)
    }

    fn set_global_tracing_pipeline<P>(pipeline: P) -> Result<(), TelemetryBuildError>
    where
        P: Into<Dispatch>,
    {
        //Note: pipeline.init (SubscriberInitExt::init) cannot be used as we have already installed
        // the LogTracer in the WebApplication for the pre-init phase. If we call init here,
        // it would result in a double install error from the LogTracer.
        tracing::dispatcher::set_global_default(pipeline.into())?;
        Ok(())
    }

    fn tracing_fixed_filter<S>(&mut self, config: &TelemetryConfig) -> Result<impl Filter<S>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        tracing::debug!("Registering fixed filter tracing layer...");
        let filter = config.default_level.as_deref().unwrap_or("warn");
        let env_filter = EnvFilter::builder().parse(filter)?;

        Ok(env_filter)
    }

    fn tracing_dyn_filter<S>(&mut self, config: &TelemetryConfig) -> Result<impl Filter<S>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        tracing::debug!("Registering dynamic filter tracing layer...");

        let filter = config.default_level.as_deref().unwrap_or("warn");
        let env_filter = EnvFilter::builder().parse(filter)?;

        let (reload_env_filter, reload_handle) = reload::Layer::new(env_filter);
        self.reconfigure = Some(Arc::new(RwLock::new(WrapHandle {
            handle: reload_handle,
            config: DynConfig { filter: filter.to_string() },
        })));
        Ok(reload_env_filter)
    }

    fn logging_layer<S>(
        &mut self,
        config: &TelemetryConfig,
        resource: &Resource,
    ) -> Result<Option<Box<dyn Layer<S> + Send + Sync>>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        match &config.logging {
            Logging::None => {
                tracing::warn!("Logging is disabled");
                Ok(None)
            }
            Logging::Console { format } => {
                tracing::debug!("Registering console log layer...");

                // The HTTP request span carries many attributes meant for the trace backend; keep them out of console logs.
                match format {
                    ConsoleLogFormat::Compact => Ok(Some(
                        tracing_subscriber::fmt::Layer::new()
                            .compact()
                            .with_filter(filter_fn(|meta| !(meta.is_span() && meta.target() == TRACING_TARGET)))
                            .boxed(),
                    )),
                    ConsoleLogFormat::Json => Ok(Some(
                        tracing_subscriber::fmt::Layer::new()
                            .json()
                            .with_filter(filter_fn(|meta| !(meta.is_span() && meta.target() == TRACING_TARGET)))
                            .boxed(),
                    )),
                }
            }
            #[cfg(feature = "ot_otlp")]
            Logging::OpenTelemetryProtocol { endpoint } => {
                tracing::info!("Registering OpenTelemetryProtocol log exporter...");
                let exporter = opentelemetry_otlp::LogExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .build()?;
                let provider = SdkLoggerProvider::builder()
                    .with_resource(resource.clone())
                    .with_batch_exporter(exporter)
                    .build();
                let layer = OpenTelemetryTracingBridge::new(&provider);
                self.logger_provider = Some(provider);
                Ok(Some(layer.boxed()))
            }
        }
    }

    fn tracing_ot<S>(
        &mut self,
        config: &TelemetryConfig,
        resource: &Resource,
        scope: &InstrumentationScope,
    ) -> Result<impl Layer<S>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        let mut builder = SdkTracerProvider::builder();

        builder = match &config.tracing {
            #[cfg(feature = "ot_otlp")]
            Tracing::OpenTelemetryProtocol { endpoint } => {
                tracing::info!("Registering OpenTelemetryProtocol tracing exporter...");
                let exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .build()?;
                builder.with_batch_exporter(exporter)
            }
            #[cfg(feature = "ot_app_insight")]
            Tracing::AppInsight { connection_string } => {
                tracing::info!("Registering AppInsight tracing exporter...");
                let exporter = opentelemetry_application_insights::Exporter::new_from_connection_string(
                    connection_string,
                    reqwest::Client::new(),
                )
                .map_err(TelemetryBuildError::AppInsightConfigError)?;
                builder.with_batch_exporter(exporter)
            }
            Tracing::None => unreachable!("Tracing::None should not be used in this context"),
        };

        let provider = builder
            .with_resource(resource.clone())
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let tracer = provider.tracer_with_scope(scope.clone());
        self.tracer_provider = Some(provider);

        Ok(OpenTelemetryLayer::new(tracer).with_tracked_inactivity(true))
    }

    fn install_trace(
        &mut self,
        config: &TelemetryConfig,
        resource: &Resource,
        scope: &InstrumentationScope,
    ) -> Result<(), TelemetryBuildError> {
        use tracing_subscriber::registry;

        // Kept out of the (reconfigurable) sink filter, so the counts do not depend on the log verbosity.
        let log_metrics = self
            .metrics
            .as_ref()
            .map(|m| LogMetricsLayer::new(&m.meter).with_filter(LevelFilter::WARN));

        let logging_layer = self.logging_layer(config, resource)?;
        let tracing_layer = if matches!(config.tracing, Tracing::None) {
            tracing::warn!("Tracing is disabled");
            None
        } else {
            Some(self.tracing_ot(config, resource, scope)?.boxed())
        };
        let sinks = Layer::and_then(logging_layer, tracing_layer);

        if config.allow_reconfigure {
            Self::set_global_tracing_pipeline(
                registry()
                    .with(log_metrics)
                    .with(sinks.with_filter(self.tracing_dyn_filter(config)?)),
            )?;
        } else {
            Self::set_global_tracing_pipeline(
                registry()
                    .with(log_metrics)
                    .with(sinks.with_filter(self.tracing_fixed_filter(config)?)),
            )?;
        }

        Ok(())
    }

    fn install_metrics(
        &mut self,
        config: &TelemetryConfig,
        resource: &Resource,
        scope: &InstrumentationScope,
    ) -> Result<(), TelemetryBuildError> {
        // Install meter provider for opentelemetry
        let provider = match &config.metrics {
            Metering::None => {
                tracing::warn!("Metrics are disabled");
                return Ok(());
            }
            #[cfg(feature = "ot_otlp")]
            Metering::OpenTelemetryProtocol { endpoint } => {
                tracing::info!("Registering OpenTelemetryProtocol metric exporter...");
                let exporter = opentelemetry_otlp::MetricExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .build()?;
                SdkMeterProvider::builder()
                    .with_resource(resource.clone())
                    .with_periodic_exporter(exporter)
                    .build()
            }
        };

        global::set_meter_provider(provider.clone());
        let meter = provider.meter_with_scope(scope.clone());
        self.metrics = Some(Metrics { provider, meter });
        Ok(())
    }

    fn install_telemetry(
        &mut self,
        service_name: &'static str,
        config: &TelemetryConfig,
    ) -> Result<(), TelemetryBuildError> {
        let resource = Resource::builder().with_service_name(service_name).build();
        let scope = InstrumentationScope::builder("opentelemetry-instrumentation-shine")
            .with_version(env!("CARGO_PKG_VERSION"))
            .build();

        self.install_metrics(config, &resource, &scope)?;
        self.install_trace(config, &resource, &scope)?;

        Ok(())
    }

    pub fn set_configuration(&self, config: DynConfig) -> Result<(), TelemetryError> {
        if let Some(reconfigure) = &self.reconfigure {
            reconfigure
                .write()
                .unwrap()
                .set_configuration(config)
                .map_err(TelemetryError::TraceUpdateConfig)?;
        }
        Ok(())
    }

    pub fn get_configuration(&self) -> Result<DynConfig, TelemetryError> {
        if let Some(reconfigure) = &self.reconfigure {
            let reconfigure = reconfigure.read().unwrap();
            let config = reconfigure.get_configuration();
            Ok(config.clone())
        } else {
            Err(TelemetryError::TraceNoReconfigure)
        }
    }

    pub fn create_meter(&self, metrics_scope: &'static str) -> Option<Meter> {
        self.metrics.as_ref().map(|m| m.provider.meter(metrics_scope))
    }

    pub fn service_meter(&self) -> Option<&Meter> {
        self.metrics.as_ref().map(|m| &m.meter)
    }

    pub fn create_router<S>(&self) -> utoipa_axum::router::OpenApiRouter<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        super::telemetry_router::build_router(self.clone())
    }

    pub fn create_layer(&self) -> OtelLayer {
        //todo: read route filtering from config
        let mut layer = OtelLayer::default();
        if let Some(metrics) = &self.metrics {
            layer = layer.meter(metrics.meter.clone())
        }
        layer
    }
}
