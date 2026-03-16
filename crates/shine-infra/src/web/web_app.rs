use crate::{
    health::HealthService,
    session::CurrentUserService,
    telemetry::TelemetryService,
    web::{
        middlewares::{PoweredBy, SecurityHeaders},
        responses::ProblemConfig,
        ApiUrl, FeatureConfig, WebAppConfig,
    },
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    routing::Router,
};
use axum_server::Handle;
use regex::bytes::Regex;
use serde::de::DeserializeOwned;
use std::{env, fmt::Debug, fs, future::Future, net::SocketAddr, time::Duration as StdDuration};
use tokio::{net::TcpListener, runtime::Runtime, signal, time::sleep};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{instrument, level_filters::LevelFilter, Level};
use tracing_log::LogTracer;
use tracing_subscriber::EnvFilter;
use utoipa::{
    openapi::{ComponentsBuilder, OpenApi as OpenApiDoc, OpenApiBuilder},
    OpenApi, ToResponse,
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::{Config as SwaggerConfig, SwaggerUi};

#[derive(OpenApi)]
#[openapi(paths(), components(), tags())]
struct ApiDoc;

impl ApiDoc {
    pub fn with_default_components() -> OpenApiDoc {
        #[derive(ToResponse)]
        #[allow(dead_code)]
        struct Problem {
            r#type: String,
            detail: Option<serde_json::Value>,
            instance: Option<ApiUrl>,
        }

        let mut doc = ApiDoc::openapi();

        let components: utoipa::openapi::Components = ComponentsBuilder::new()
            .schema_from::<ApiUrl>()
            .response_from::<Problem>()
            .build();
        let comp_doc = OpenApiBuilder::new().components(Some(components)).build();
        doc.merge(comp_doc);

        doc
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
        log::warn!("Received ctrl-c, shutting down the server...")
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
        log::warn!("Received SIGTERM, shutting down the server...")
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn graceful_shutdown(handle: Handle<SocketAddr>) {
    shutdown_signal().await;
    handle.graceful_shutdown(Some(StdDuration::from_secs(10)));

    loop {
        sleep(StdDuration::from_secs(1)).await;
        log::info!("alive connections: {}", handle.connection_count());
    }
}

pub trait WebApplication {
    type AppConfig: FeatureConfig + DeserializeOwned + Debug + Send + Sync + 'static;
    type AppState: Clone + Send + Sync + 'static;

    fn feature_name(&self) -> &'static str {
        Self::AppConfig::NAME
    }

    fn create(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
        health_service: &mut HealthService,
        router: &mut OpenApiRouter<Self::AppState>,
    ) -> impl Future<Output = Result<Self::AppState, AnyError>> + Send;
}

fn create_cors_layer(allowed_origins: &[String]) -> Result<CorsLayer, AnyError> {
    let allowed_origins = allowed_origins
        .iter()
        .map(|r| Regex::new(r))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| anyhow!("Cors config error: {err}"))?;
    let cors = CorsLayer::default()
        .allow_origin(AllowOrigin::predicate(move |origin, _| {
            let origin = origin.as_bytes();
            allowed_origins.iter().any(|r| r.is_match(origin))
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_credentials(true);
    Ok(cors)
}

async fn load_app_config<A: WebApplication>() -> Result<WebAppConfig<A::AppConfig>, AnyError> {
    let args: Vec<String> = env::args().collect();
    let stage = args.get(1).ok_or(anyhow!("Missing config stage parameter"))?.clone();

    // initialize a pre-init logger
    let _pre_init_log_guard = {
        LogTracer::init().expect("Failed to set log tracer");
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy();
        let pre_init_log = tracing_subscriber::fmt().with_env_filter(env_filter).compact().finish();
        tracing::dispatcher::set_default(&pre_init_log.into())
    };

    log::trace!("init-trace - log:ok");
    log::debug!("init-debug - log:ok");
    log::info!("init-info  - log:ok");
    log::warn!("init-warn  - log:ok");
    log::error!("init-error - log:ok");
    tracing::trace!("init-trace - tracing:ok");
    tracing::debug!("init-debug - tracing:ok");
    tracing::info!("init-info  - tracing:ok");
    tracing::warn!("init-warn  - tracing:ok");
    tracing::error!("init-error - tracing:ok");

    let config = WebAppConfig::<A::AppConfig>::load(&stage, None).await?;
    log::info!("pre-init completed");

    Ok(config)
}

#[instrument(skip(config, app))]
async fn create_web_app<A: WebApplication>(
    config: &WebAppConfig<A::AppConfig>,
    app: &A,
) -> Result<Router<()>, AnyError> {
    let telemetry_service = TelemetryService::new(app.feature_name(), &config.telemetry).await?;
    let mut health_service = HealthService::new(app.feature_name(), config)?;
    let problem_service = ProblemConfig::new(config.service.full_problem_response);
    let in_flight_service = crate::health::InFlightService::new();
    let current_user_service = CurrentUserService::from_config(&config.service).await?;

    let cors_layer = create_cors_layer(&config.service.allowed_origins)?;
    let powered_by_layer = if config.service.expose_powered_by {
        Some(PoweredBy::from_service_info(app.feature_name(), &config.core.version)?)
    } else {
        None
    };
    let log_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    // Register built-in status providers
    health_service.add_provider(crate::health::UptimeStatus::new());
    health_service.add_provider(in_flight_service.clone());

    let mut router = OpenApiRouter::new();
    router = router.nest(&format!("/{}", app.feature_name()), health_service.create_router());
    router = router.nest(&format!("/{}", app.feature_name()), telemetry_service.create_router());

    let app_state = app.create(config, &mut health_service, &mut router).await?;

    let (router, open_api) = router.split_for_parts();
    let router = if config.service.expose_api_docs {
        let mut doc = ApiDoc::with_default_components();
        doc.merge(open_api);

        let swagger = SwaggerUi::new(format!("/{}/doc/swagger-ui", app.feature_name()))
            .url(format!("/{}/doc/openapi.json", app.feature_name()), doc)
            .config(
                SwaggerConfig::default()
                    .with_credentials(true)
                    .show_common_extensions(true),
            );
        router.merge(swagger)
    } else {
        router
    };

    Ok(router
        .layer(current_user_service.create_layer())
        .layer(problem_service.into_layer())
        .layer(in_flight_service.create_layer())
        .layer(tower::util::option_layer(powered_by_layer))
        .layer(SecurityHeaders)
        .layer(cors_layer)
        .layer(telemetry_service.create_layer())
        .layer(log_layer)
        .with_state(app_state))
}

async fn start_web_app<A: WebApplication>(app: A) -> Result<(), AnyError> {
    let config = load_app_config::<A>().await?;
    let router = create_web_app(&config, &app).await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));

    if let Some(tls_config) = &config.service.tls {
        log::info!("Starting service on https://{addr:?} ...");
        let cert = fs::read(&tls_config.cert)?;
        let key = fs::read(&tls_config.key)?;
        //todo: workaround for https://github.com/programatik29/axum-server/issues/153
        // when fixed remove explicit dependency on rustls from Cargo.toml
        let config = axum_server::tls_rustls::RustlsConfig::from_pem(cert, key)
            .await
            .map_err(|e| anyhow!(e))?;

        let handle = Handle::new();
        tokio::spawn(graceful_shutdown(handle.clone()));

        axum_server::bind_rustls(addr, config)
            .handle(handle)
            .serve(router.into_make_service())
            .await
            .map_err(|e| anyhow!(e))
    } else {
        log::info!("Starting service on http://{addr:?} ...");
        let listener = TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow!(e))
    }
}

pub fn run_web_app<A: WebApplication>(app: A) {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let rt = Runtime::new().unwrap();
    let handle = rt.handle();

    if let Err(err) = handle.block_on(start_web_app(app)) {
        eprintln!("[ERROR] {err}");
        if let Some(cause) = err.source() {
            eprintln!();
            eprintln!("Caused by:");
            let mut cause = Some(cause);
            let mut i = 0;
            while let Some(e) = cause {
                eprintln!("   {i}: {e}");
                cause = e.source();
                i += 1;
            }
        }
        panic!();
    }
}
