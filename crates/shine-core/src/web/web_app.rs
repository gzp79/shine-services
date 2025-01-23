use crate::{
    telemetry::TelemetryService,
    web::{
        controllers::{self, ApiUrl},
        PoweredBy, ProblemConfig, UserSessionCacheReader, WebAppConfig,
    },
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    Extension,
};
use axum_server::Handle;
use serde::de::DeserializeOwned;
use std::{env, fmt::Debug, fs, future::Future, net::SocketAddr, time::Duration as StdDuration};
use tokio::{
    net::TcpListener,
    runtime::{Handle as RtHandle, Runtime},
    signal,
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{Dispatch, Level};
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

async fn graceful_shutdown(handle: Handle) {
    shutdown_signal().await;
    handle.graceful_shutdown(Some(StdDuration::from_secs(10)));
}

pub trait WebApplication {
    type AppConfig: DeserializeOwned + Debug + Send + Sync + 'static;
    type AppState: Clone + Send + Sync + 'static;

    fn feature_name(&self) -> &'static str;
    fn create_state(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
    ) -> impl Future<Output = Result<Self::AppState, AnyError>> + Send;
    fn create_routes(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
    ) -> impl Future<Output = Result<OpenApiRouter<Self::AppState>, AnyError>> + Send;
}

async fn start_web_app<A: WebApplication>(_rt_handle: RtHandle, app: A) -> Result<(), AnyError> {
    let args: Vec<String> = env::args().collect();
    let stage = args.get(1).ok_or(anyhow!("Missing config stage parameter"))?.clone();

    let (config, telemetry_service) = {
        // initialize a pre-init logger
        let pre_init_log = {
            let _ = tracing_log::LogTracer::init();
            let env_filter = EnvFilter::from_default_env();
            let pre_init_log = tracing_subscriber::fmt().with_env_filter(env_filter).compact().finish();
            Dispatch::new(pre_init_log)
        };
        let _pre_init_log_guard = tracing::dispatcher::set_default(&pre_init_log);

        log::trace!("init-trace - ok");
        log::debug!("init-debug - ok");
        log::info!("init-info  - ok");
        log::warn!("init-warn  - ok");
        log::error!("init-error - ok");

        let config = WebAppConfig::<A::AppConfig>::load_config(&stage).await?;
        let telemetry_manager = TelemetryService::new(app.feature_name(), &config.telemetry).await?;
        log::info!("pre-init completed");
        (config, telemetry_manager)
    };

    log::trace!("Creating services...");
    log::trace!("trace - ok:log");
    log::debug!("debug - ok:log");
    log::info!("info  - ok:log");
    log::warn!("warn  - ok:log");
    log::error!("error - ok:log");
    tracing::trace!("trace - tracing:ok");
    tracing::debug!("debug - tracing:ok");
    tracing::info!("info  - tracing:ok");
    tracing::warn!("warn  - tracing:ok");
    tracing::error!("error - tracing:ok");

    let cors_layer = {
        let allowed_origins = {
            let allowed_origins = config
                .service
                .allowed_origins
                .iter()
                .map(|r| regex::bytes::Regex::new(r))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| anyhow!("Cords config error: {err}"))?;
            AllowOrigin::predicate(move |origin, _| {
                let origin = origin.as_bytes();
                allowed_origins.iter().any(|r| r.is_match(origin))
            })
        };
        CorsLayer::default()
            .allow_origin(allowed_origins)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
            .allow_credentials(true)
    };
    let powered_by_layer = PoweredBy::from_service_info(app.feature_name(), &config.core.version)?;

    let mut doc = ApiDoc::with_default_components();

    let log_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));
    let problem_detail_layer = {
        let problem_config = ProblemConfig {
            include_internal: config.service.full_problem_response,
        };
        problem_config.into_layer()
    };
    let telemetry_layer = telemetry_service.create_layer();
    let user_session_layer = {
        // todo: make it a read only access to the redis
        log::info!("Creating user session cache reader...");
        let redis = crate::db::create_redis_pool(config.service.session_redis_cns.as_str()).await?;
        UserSessionCacheReader::new(None, &config.service.session_secret, "", redis)?.into_layer()
    };

    log::info!("Creating application state...");
    let mut router = OpenApiRouter::new();
    let app_state = app.create_state(&config).await?;

    log::info!("Creating common routes...");
    let health_controller = controllers::HealthController::new(app.feature_name(), &config)?.into_routes();
    router = router.nest(&format!("/{}", app.feature_name()), health_controller);

    log::info!("Creating application routes...");
    let app_controller = app.create_routes(&config).await?;
    router = router.nest(&format!("/{}", app.feature_name()), app_controller);

    let (router, router_api) = router.split_for_parts();
    doc.merge(router_api);

    log::info!("Creating swagger-ui...");
    let swagger = SwaggerUi::new(format!("/{}/doc/swagger-ui", app.feature_name()))
        .url(format!("/{}/doc/openapi.json", app.feature_name()), doc)
        .config(
            SwaggerConfig::default()
                .with_credentials(true)
                .show_common_extensions(true),
        );

    let router = router
        .merge(swagger)
        .layer(user_session_layer)
        .layer(problem_detail_layer)
        .layer(powered_by_layer)
        .layer(cors_layer)
        .layer(telemetry_layer)
        .layer(Extension(telemetry_service))
        .layer(log_layer)
        .with_state(app_state);

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
    if let Err(err) = handle.block_on(start_web_app(handle.clone(), app)) {
        eprintln!("[ERROR] {}", err);
        if let Some(cause) = err.source() {
            eprintln!();
            eprintln!("Caused by:");
            let mut cause = Some(cause);
            let mut i = 0;
            while let Some(e) = cause {
                eprintln!("   {}: {}", i, e);
                cause = e.source();
                i += 1;
            }
        }
        panic!();
    }
}
