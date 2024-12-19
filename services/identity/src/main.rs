mod app_config;
mod controllers;
mod repositories;
mod services;

use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    Router,
};
use axum_server::Handle;
use shine_service::{
    axum::{add_default_components, telemetry::TelemetryService, ApiPath, PoweredBy},
    service::UserSessionCacheReader,
};
use std::{env, fs, net::SocketAddr, time::Duration as StdDuration};
use tokio::{
    net::TcpListener,
    runtime::{Handle as RtHandle, Runtime},
    signal,
};
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{Dispatch, Level};
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;
use utoipa_swagger_ui::{Config as SwaggerConfig, SwaggerUi};

use self::{
    app_config::{AppConfig, SERVICE_NAME},
    controllers::{auth, health::HealthController, identity, ApiKind, AppState},
};

#[derive(OpenApi)]
#[openapi(paths(), components(), tags())]
struct ApiDoc;

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

async fn async_main(_rt_handle: RtHandle) -> Result<(), AnyError> {
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

        let config = AppConfig::new(&stage).await?;
        let telemetry_manager = TelemetryService::new(SERVICE_NAME, &config.telemetry).await?;
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

    let cors_layer = CorsLayer::default()
        .allow_origin(config.service.cors_allowed_origin()?)
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
    let powered_by_layer = PoweredBy::from_service_info(SERVICE_NAME, &config.core.version)?;

    let mut doc = ApiDoc::openapi();
    add_default_components(&mut doc);

    // let identity_manager = PgIdentityManager::new(&db_pool.postgres).await?;
    // let ttl_session = Duration::seconds(i64::try_from(auth_config.ttl_session)?);
    // let session_manager = SessionManager::new(&db_pool.redis, String::new(), ttl_session).await?;
    // let auto_name_manager = AutoNameManager::new(&config.user_name, &db_pool.postgres).await?;

    let app_state = AppState::new(&config, &telemetry_service).await?;

    let name = app_state.identity_service().generate_user_name().await?;
    log::error!("Generated name: {}", name);
    log::error!("Generated name: {}", name);
    log::error!("Generated name: {}", name);
    log::error!("Generated name: {}", name);
    log::error!("Generated name: {}", name);
    log::error!("Generated name: {}", name);
    //let captcha_validator = CaptchaValidator::new(config.service.captcha_secret.clone());

    let log_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));
    let problem_detail_layer = app_state.problem_config().clone().into_layer();
    let telemetry_layer = telemetry_service.create_layer();
    let user_session_layer = UserSessionCacheReader::new(
        None,
        &config.auth.auth_session.session_secret,
        "",
        app_state.db().redis.clone(),
    )?
    .into_layer();

    let health_controller = HealthController::new().into_router(&mut doc);
    let identity_controller = identity::IdentityController::new().into_router(&mut doc);
    let auth_controller = auth::AuthController::new(&config).await?.into_router(&mut doc);

    let swagger = SwaggerUi::new(ApiKind::Doc("/swagger-ui").path())
        .url(ApiKind::Doc("/openapi.json").path(), doc)
        .config(
            SwaggerConfig::default()
                .with_credentials(true)
                .show_common_extensions(true),
        );

    let app = Router::new()
        .merge(health_controller)
        .merge(identity_controller)
        .merge(auth_controller)
        .merge(swagger)
        .layer(user_session_layer)
        .layer(problem_detail_layer)
        .layer(powered_by_layer)
        .layer(cors_layer)
        .layer(telemetry_layer)
        .layer(log_layer)
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));

    if let Some(tls_config) = &config.service.tls {
        log::info!("Starting service on https: //{addr:?}");
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
            .serve(app.into_make_service())
            .await
            .map_err(|e| anyhow!(e))
    } else {
        log::info!("Starting service on http://{addr:?}");
        let listener = TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow!(e))
    }
}

pub fn main() {
    rustls::crypto::ring::default_provider().install_default().unwrap();

    let rt = Runtime::new().unwrap();

    let handle = rt.handle();
    if let Err(err) = handle.block_on(async_main(handle.clone())) {
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
