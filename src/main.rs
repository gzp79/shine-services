mod app_config;
mod app_error;
mod auth;
mod db;
mod services;

use crate::{
    app_config::{AppConfig, SERVICE_NAME},
    auth::AuthServiceBuilder,
    db::{DBPool, IdentityManager, NameGenerator, SessionManager, SettingsManager},
    services::IdentityServiceBuilder,
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    routing::get,
    Extension, Router,
};
use chrono::Duration;
use shine_service::{
    axum::{
        tracing::{OtelAxumLayer, TracingService},
        PoweredBy,
    },
    service::{UserSessionMeta, UserSessionValidator, DOMAIN_NAME},
};
use std::{net::SocketAddr, sync::Arc};
use tera::Tera;
use tokio::{
    runtime::{Handle as RtHandle, Runtime},
    signal,
};
use tower_http::cors::CorsLayer;
use tracing::Dispatch;

async fn health_check(Extension(db): Extension<DBPool>) -> String {
    format!(
        "Postgres: {:#?}\nRedis: {:#?}\nOk",
        db.postgres.state(),
        db.redis.state()
    )
}

async fn shutdown_signal() {
    signal::ctrl_c().await.expect("expect tokio signal ctrl-c");
    log::warn!("Signal shutdown");
}

fn service_path(path: &str) -> String {
    format!("/{SERVICE_NAME}{path}")
}

async fn async_main(_rt_handle: RtHandle) -> Result<(), AnyError> {
    let (config, tracing_service) = {
        // initialize a pre-init logger
        let pre_init_log = {
            let _ = tracing_log::LogTracer::init();
            let pre_init_log = tracing_subscriber::fmt().with_env_filter("info").compact().finish();
            Dispatch::new(pre_init_log)
        };
        let _pre_init_log_guard = tracing::dispatcher::set_default(&pre_init_log);

        log::trace!("init-trace - ok");
        log::debug!("init-debug - ok");
        log::info!("init-info  - ok");
        log::warn!("init-warn  - ok");
        log::error!("init-error - ok");

        let config = AppConfig::new().await?;
        let tracing_service = TracingService::new(SERVICE_NAME, &config.tracing).await?;
        log::info!("pre-init completed");
        (config, tracing_service)
    };

    log::trace!("Creating services...");
    log::trace!("trace - ok");
    log::debug!("debug - ok");
    log::info!("info  - ok");
    log::warn!("warn  - ok");
    tracing::warn!("warn  - ok(tracing)");
    log::error!("error - ok");

    let allow_origins = config
        .allow_origins
        .iter()
        .map(|r| r.parse())
        .collect::<Result<Vec<_>, _>>()?;
    let cors = CorsLayer::default()
        .allow_origin(allow_origins)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_credentials(true);
    let powered_by = PoweredBy::from_service_info(SERVICE_NAME, &config.core.version)?;

    let tracing_router = tracing_service.into_router();
    let tracing_layer = OtelAxumLayer::default();

    let tera = {
        let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
        tera.autoescape_on(vec![".html"]);
        tera
    };

    let db_pool = DBPool::new(&config.db).await?;

    let user_session = UserSessionMeta::new(&config.cookie_secret)?
        .with_cookie_name("sid")
        .with_domain(DOMAIN_NAME);
    let user_session_validator = UserSessionValidator::new(db_pool.redis.clone());

    let settings_manager = SettingsManager::new(&config);
    let identity_manager = IdentityManager::new(&db_pool).await?;
    let session_max_duration = Duration::seconds(i64::try_from(config.session_max_duration)?);
    let session_manager = SessionManager::new(&db_pool, session_max_duration).await?;
    let name_generator = NameGenerator::new();

    let (auth_pages, auth_api) = AuthServiceBuilder::new(&config.auth, &config.cookie_secret)
        .await?
        .into_router();
    let identity_api = IdentityServiceBuilder.into_router();

    let app = Router::new()
        .route(&service_path("/info/ready"), get(health_check))
        .nest(&service_path("/auth"), auth_pages)
        .nest(&service_path("/api/tracing"), tracing_router)
        .nest(&service_path("/api/identities"), identity_api)
        .nest(&service_path("/api/auth"), auth_api)
        .layer(user_session.into_layer())
        .layer(user_session_validator.into_layer())
        .layer(Extension(Arc::new(tera)))
        .layer(Extension(identity_manager))
        .layer(Extension(session_manager))
        .layer(Extension(name_generator))
        .layer(Extension(settings_manager))
        .layer(Extension(db_pool))
        .layer(powered_by)
        .layer(cors)
        .layer(tracing_layer);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.control_port));

    if let Some(tls_config) = config.tls {
        log::info!("Starting service on {addr:?} using tls");
        let cert = tls_config.cert.as_bytes().to_vec();
        let key = tls_config.key.as_bytes().to_vec();
        let config = axum_server::tls_rustls::RustlsConfig::from_pem(cert, key)
            .await
            .map_err(|e| anyhow!(e))?;
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            //.with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow!(e))
    } else {
        log::info!("Starting service on {addr:?}");
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow!(e))
    }
}

pub fn main() {
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
