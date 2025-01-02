use opentelemetry::{
    metrics::{Meter, MeterProvider},
    trace::{Tracer, TracerProvider as _},
    InstrumentationScope, KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    export::trace::SpanExporter,
    metrics::SdkMeterProvider,
    runtime::Tokio,
    trace::{Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions as otconv;
use prometheus::{Encoder, Registry as PromRegistry, TextEncoder};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use thiserror::Error as ThisError;
use tracing::{Dispatch, Subscriber};
use tracing_opentelemetry::{OpenTelemetryLayer, PreSampledTracer};
use tracing_subscriber::{
    filter::EnvFilter,
    layer::SubscriberExt,
    registry::LookupSpan,
    reload::{self, Handle},
    Layer, Registry,
};

use super::{OtelLayer, TelemetryBuildError};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Tracing {
    /// Disable tracing
    None,

    /// Enable tracing to the standard output
    StdOut,

    /// Enable Jaeger tracing (https://www.jaegertracing.io)
    #[cfg(feature = "ot_otlp")]
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
    allow_reconfigure: bool,
    enable_console_log: bool,
    metrics: bool,
    tracing: Tracing,
    default_level: Option<String>,
}

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
    L: 'static + Layer<S> + From<EnvFilter> + Send + Sync,
    S: Subscriber,
{
    handle: Handle<L, S>,
    config: DynConfig,
}

impl<L, S> DynHandle for WrapHandle<L, S>
where
    L: 'static + Layer<S> + From<EnvFilter> + Send + Sync,
    S: Subscriber,
{
    fn set_configuration(&mut self, mut new_config: DynConfig) -> Result<(), String> {
        new_config.filter.retain(|c| !c.is_whitespace());
        let new_filter = new_config.filter.parse::<EnvFilter>().map_err(|e| format!("{}", e))?;
        self.handle.reload(new_filter).map_err(|e| format!("{}", e))?;
        self.config = new_config;
        Ok(())
    }

    fn get_configuration(&self) -> &DynConfig {
        &self.config
    }
}

#[derive(Debug, ThisError)]
#[error("Failed to perform trace configuration operation: {0}")]
pub struct TraceReconfigureError(String);

#[derive(Clone)]
pub struct Metrics {
    registry: PromRegistry,
    provider: SdkMeterProvider,
    service_meter: Meter,
}

#[derive(Clone)]
pub struct TelemetryService {
    reconfigure: Option<Arc<RwLock<dyn DynHandle>>>,
    metrics: Option<Metrics>,
}

impl TelemetryService {
    pub async fn new(service_name: &'static str, config: &TelemetryConfig) -> Result<Self, TelemetryBuildError> {
        let mut service = TelemetryService {
            reconfigure: None,
            metrics: None,
        };
        service.install_telemetry(service_name, config)?;
        Ok(service)
    }

    fn set_global_tracing<L>(&mut self, tracing_pipeline: L) -> Result<(), TelemetryBuildError>
    where
        L: Into<Dispatch>,
    {
        tracing::dispatcher::set_global_default(tracing_pipeline.into())?;
        Ok(())
    }

    fn install_tracing_layer_with_filter<T>(
        &mut self,
        config: &TelemetryConfig,
        pipeline: T,
    ) -> Result<(), TelemetryBuildError>
    where
        T: for<'a> LookupSpan<'a> + Subscriber + Send + Sync,
    {
        let filter = config.default_level.as_deref().unwrap_or("warn");
        let env_filter = EnvFilter::builder().parse(filter)?;

        if config.allow_reconfigure {
            // enable filtering with reconfiguration capabilities
            let (reload_env_filter, reload_handle) = reload::Layer::new(env_filter);
            let pipeline = pipeline.with(reload_env_filter);
            let reload_handle = WrapHandle {
                handle: reload_handle,
                config: DynConfig {
                    filter: filter.to_string(),
                },
            };
            self.reconfigure = Some(Arc::new(RwLock::new(reload_handle)));

            self.set_global_tracing(pipeline)?;
            Ok(())
        } else {
            // enable filtering from the environment variables
            let pipeline = pipeline.with(env_filter);

            self.set_global_tracing(pipeline)?;
            Ok(())
        }
    }

    fn install_tracing_layer<L>(&mut self, config: &TelemetryConfig, layer: L) -> Result<(), TelemetryBuildError>
    where
        L: Layer<Registry> + Send + Sync,
    {
        let pipeline = tracing_subscriber::registry().with(layer);
        if config.enable_console_log {
            let console_layer = tracing_subscriber::fmt::Layer::new().pretty();
            let pipeline = pipeline.with(console_layer);
            self.install_tracing_layer_with_filter(config, pipeline)
        } else {
            self.install_tracing_layer_with_filter(config, pipeline)
        }
    }

    fn ot_layer<T>(tracer: T) -> OpenTelemetryLayer<Registry, T>
    where
        T: 'static + Tracer + PreSampledTracer + Send + Sync,
    {
        tracing_opentelemetry::layer()
            .with_tracked_inactivity(true)
            .with_tracer(tracer)
    }

    fn install_ot_tracing<E>(
        &mut self,
        config: &TelemetryConfig,
        exporter: E,
        resource: &Resource,
        scope: &InstrumentationScope,
    ) -> Result<(), TelemetryBuildError>
    where
        E: SpanExporter + 'static,
    {
        let provider = TracerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_resource(resource.clone())
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let tracer = provider.tracer_with_scope(scope.clone());
        self.install_tracing_layer(config, Self::ot_layer(tracer))?;
        Ok(())
    }

    fn install_tracing(
        &mut self,
        config: &TelemetryConfig,
        resource: &Resource,
        scope: &InstrumentationScope,
    ) -> Result<(), TelemetryBuildError> {
        match &config.tracing {
            Tracing::StdOut => {
                log::info!("Registering StdOut tracing...");
                let exporter = opentelemetry_stdout::SpanExporter::default();
                self.install_ot_tracing(config, exporter, resource, scope)?;
            }
            #[cfg(feature = "ot_otlp")]
            Tracing::OpenTelemetryProtocol { endpoint } => {
                log::info!("Registering OpenTelemetryProtocol tracing...");
                let exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .build()?;
                self.install_ot_tracing(config, exporter, resource, scope)?;
            }
            #[cfg(feature = "ot_zipkin")]
            Tracing::Zipkin => {
                log::info!("Registering Zipkin tracing...");
                let exporter = opentelemetry_zipkin::new_pipeline().init_exporter()?;
                self.install_ot_tracing(config, exporter, resource, scope)?;
            }
            #[cfg(feature = "ot_app_insight")]
            Tracing::AppInsight { connection_string } => {
                log::info!("Registering AppInsight tracing...");
                let exporter = opentelemetry_application_insights::Exporter::new_from_connection_string(
                    connection_string,
                    reqwest::Client::new(),
                )
                .map_err(TelemetryBuildError::AppInsightConfigError)?;
                self.install_ot_tracing(config, exporter, resource, scope)?;
            }
            Tracing::None => {
                log::info!("Registering no tracing...");
                self.install_tracing_layer(config, EmptyLayer)?;
            }
        };

        Ok(())
    }

    fn install_metrics(
        &mut self,
        config: &TelemetryConfig,
        resource: &Resource,
        scope: &InstrumentationScope,
    ) -> Result<(), TelemetryBuildError> {
        // Install meter provider for opentelemetry
        if config.metrics {
            log::info!("Registering metrics...");
            let registry = prometheus::Registry::new();
            let exporter = opentelemetry_prometheus::exporter()
                .with_registry(registry.clone())
                .build()?;
            let provider = SdkMeterProvider::builder()
                .with_resource(resource.clone())
                .with_reader(exporter)
                .build();
            let service_meter = provider.meter_with_scope(scope.clone());
            self.metrics = Some(Metrics {
                registry,
                provider,
                service_meter,
            });
        }
        Ok(())
    }

    fn install_telemetry(
        &mut self,
        service_name: &'static str,
        config: &TelemetryConfig,
    ) -> Result<(), TelemetryBuildError> {
        let resource = Resource::new(vec![KeyValue::new(
            otconv::resource::SERVICE_NAME,
            service_name.to_string(),
        )]);
        let scope = InstrumentationScope::builder("opentelemetry-instrumentation-shine")
            .with_version(env!("CARGO_PKG_VERSION"))
            .build();

        self.install_metrics(config, &resource, &scope)?;
        self.install_tracing(config, &resource, &scope)?;

        Ok(())
    }

    pub fn set_configuration(&self, config: DynConfig) -> Result<(), TraceReconfigureError> {
        if let Some(reconfigure) = &self.reconfigure {
            reconfigure
                .write()
                .unwrap()
                .set_configuration(config)
                .map_err(TraceReconfigureError)?
        }
        Ok(())
    }

    pub fn get_configuration(&self) -> Result<DynConfig, TraceReconfigureError> {
        if let Some(reconfigure) = &self.reconfigure {
            let reconfigure = reconfigure.read().unwrap();
            let config = reconfigure.get_configuration();
            Ok(config.clone())
        } else {
            Err(TraceReconfigureError("Reconfigure is not enabled".to_string()))
        }
    }

    pub fn create_meter(&self, metrics_scope: &'static str) -> Option<Meter> {
        self.metrics.as_ref().map(|m| m.provider.meter(metrics_scope))
    }

    pub fn service_meter(&self) -> Option<&Meter> {
        self.metrics.as_ref().map(|m| &m.service_meter)
    }

    pub fn metrics(&self) -> String {
        if let Some(metrics) = &self.metrics {
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            let metric_families = metrics.registry.gather();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            String::from_utf8(buffer).unwrap()
        } else {
            String::new()
        }
    }

    pub fn create_layer(&self) -> OtelLayer {
        //todo: read route filtering from config
        let mut layer = OtelLayer::default();
        if let Some(metrics) = &self.metrics {
            layer = layer.meter(metrics.service_meter.clone())
        }
        layer
    }
}

struct EmptyLayer;
impl<S: Subscriber> Layer<S> for EmptyLayer {}
