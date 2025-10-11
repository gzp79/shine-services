use opentelemetry::{
    global,
    metrics::{Meter, MeterProvider},
    trace::TracerProvider,
    InstrumentationScope,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    metrics::SdkMeterProvider,
    trace::{Sampler, SdkTracerProvider},
    Resource,
};
use std::sync::{Arc, RwLock};
use tracing::{Dispatch, Subscriber};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, registry::LookupSpan, reload, Layer};

use super::{Metering, OtelLayer, TelemetryBuildError, TelemetryConfig, TelemetryError, Tracing};

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
    L: Layer<S> + From<EnvFilter> + Send + Sync + 'static,
    S: Subscriber,
{
    handle: reload::Handle<L, S>,
    config: DynConfig,
}

impl<L, S> DynHandle for WrapHandle<L, S>
where
    L: Layer<S> + From<EnvFilter> + Send + Sync + 'static,
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
#[derive(Clone)]
pub struct TelemetryService {
    tracer_provider: Option<SdkTracerProvider>,
    metrics: Option<Metrics>,
    reconfigure: Option<Arc<RwLock<dyn DynHandle>>>,
}

impl TelemetryService {
    pub async fn new(service_name: &'static str, config: &TelemetryConfig) -> Result<Self, TelemetryBuildError> {
        let mut service = TelemetryService {
            tracer_provider: None,
            metrics: None,
            reconfigure: None,
        };
        service.install_telemetry(service_name, config)?;
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

    fn tracing_fixed_filter<S>(&mut self, config: &TelemetryConfig) -> Result<impl Layer<S>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        log::debug!("Registering fixed filter tracing layer...");
        let filter = config.default_level.as_deref().unwrap_or("warn");
        let env_filter = EnvFilter::builder().parse(filter)?;

        Ok(env_filter)
    }

    fn tracing_dyn_filter<S>(&mut self, config: &TelemetryConfig) -> Result<impl Layer<S>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        log::debug!("Registering dynamic filter tracing layer...");

        let filter = config.default_level.as_deref().unwrap_or("warn");
        let env_filter = EnvFilter::builder().parse(filter)?;

        let (reload_env_filter, reload_handle) = reload::Layer::new(env_filter);
        self.reconfigure = Some(Arc::new(RwLock::new(WrapHandle {
            handle: reload_handle,
            config: DynConfig { filter: filter.to_string() },
        })));
        Ok(reload_env_filter)
    }

    fn tracing_console_log<S>(&mut self, _config: &TelemetryConfig) -> Result<impl Layer<S>, TelemetryBuildError>
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync + 'static,
    {
        log::debug!("Registering console log tracing layer...");
        let console_layer = tracing_subscriber::fmt::Layer::new().compact();
        Ok(console_layer)
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
            Tracing::StdOut => {
                log::info!("Registering StdOut tracing exporter...");
                let exporter = opentelemetry_stdout::SpanExporter::default();
                builder.with_simple_exporter(exporter)
            }
            #[cfg(feature = "ot_otlp")]
            Tracing::OpenTelemetryProtocol { endpoint } => {
                log::info!("Registering OpenTelemetryProtocol tracing exporter...");
                let exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .build()?;
                builder.with_batch_exporter(exporter)
            }
            #[cfg(feature = "ot_zipkin")]
            Tracing::Zipkin => {
                log::info!("Registering Zipkin tracing exporter...");
                let exporter = opentelemetry_zipkin::ZipkinExporter::builder().build()?;
                builder.with_batch_exporter(exporter)
            }
            #[cfg(feature = "ot_app_insight")]
            Tracing::AppInsight { connection_string } => {
                log::info!("Registering AppInsight tracing exporter...");
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

        if matches!(config.tracing, Tracing::None) {
            log::warn!("Trace is disabled");

            if config.enable_console_log {
                if config.allow_reconfigure {
                    Self::set_global_tracing_pipeline(
                        registry()
                            .with(self.tracing_dyn_filter(config)?)
                            .with(self.tracing_console_log(config)?),
                    )?;
                } else {
                    Self::set_global_tracing_pipeline(
                        registry()
                            .with(self.tracing_fixed_filter(config)?)
                            .with(self.tracing_console_log(config)?),
                    )?;
                }
            } else {
                log::warn!("Service is configured for silent mode");
            }
        } else {
            match (config.enable_console_log, config.allow_reconfigure) {
                (true, true) => {
                    Self::set_global_tracing_pipeline(
                        registry()
                            .with(self.tracing_dyn_filter(config)?)
                            .with(self.tracing_console_log(config)?)
                            .with(self.tracing_ot(config, resource, scope)?),
                    )?;
                }
                (true, false) => {
                    Self::set_global_tracing_pipeline(
                        registry()
                            .with(self.tracing_fixed_filter(config)?)
                            .with(self.tracing_console_log(config)?)
                            .with(self.tracing_ot(config, resource, scope)?),
                    )?;
                }
                (false, true) => {
                    Self::set_global_tracing_pipeline(
                        registry()
                            .with(self.tracing_dyn_filter(config)?)
                            .with(self.tracing_ot(config, resource, scope)?),
                    )?;
                }
                (false, false) => {
                    Self::set_global_tracing_pipeline(
                        registry()
                            .with(self.tracing_fixed_filter(config)?)
                            .with(self.tracing_ot(config, resource, scope)?),
                    )?;
                }
            }
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
                log::warn!("Metrics are disabled");
                return Ok(());
            }
            #[cfg(feature = "ot_otlp")]
            Metering::OpenTelemetryProtocol { endpoint } => {
                log::info!("Registering OpenTelemetryProtocol metric exporter...");
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

    pub fn create_layer(&self) -> OtelLayer {
        //todo: read route filtering from config
        let mut layer = OtelLayer::default();
        if let Some(metrics) = &self.metrics {
            layer = layer.meter(metrics.meter.clone())
        }
        layer
    }
}
