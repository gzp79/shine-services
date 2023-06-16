mod app_config;
mod app_error;
mod app_session;
mod db;
mod services;
mod utils;

use crate::{
    app_config::{AppConfig, SERVICE_NAME},
    app_session::{AppSessionMeta, ExternalLoginMeta},
    db::{DBPool, IdentityManager, SessionManager},
    services::{AuthServiceBuilder, IdentityServiceBuilder},
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    routing::get,
    Extension, Router,
};
use chrono::Duration;
use shine_service::{
    axum::tracing::{tracing_layer, TracingService},
    DOMAIN_NAME,
};
use std::{net::SocketAddr, sync::Arc};
use tera::Tera;
use tokio::{
    runtime::{Handle as RtHandle, Runtime},
    signal,
};
use tower_http::cors::CorsLayer;
use tracing::Dispatch;

async fn health_check(Extension(pool): Extension<DBPool>) -> String {
    format!(
        "Postgres: {:#?}\nRedis: {:#?}\nOk",
        pool.postgres.state(),
        pool.redis.state()
    )
}

async fn shutdown_signal() {
    signal::ctrl_c().await.expect("expect tokio signal ctrl-c");
    log::warn!("Signal shutdown");
}

fn service_path(path: &str) -> String {
    format!("/{SERVICE_NAME}{path}")
}

async fn async_main(rt_handle: RtHandle) -> Result<(), AnyError> {
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

        let config = AppConfig::new(&rt_handle)?;
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

    let tracing_router = tracing_service.into_router();
    let tracing_layer = tracing_layer();

    let tera = {
        let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
        tera.autoescape_on(vec![".html"]);
        tera
    };

    let db_pool = DBPool::new(&config.db).await?;
    let identity_manager = IdentityManager::new(&db_pool).await?;
    let session_max_duration = Duration::seconds(i64::try_from(config.session_max_duration)?);
    let session_manager = SessionManager::new(&db_pool, session_max_duration).await?;
    let session_cookie = AppSessionMeta::new(&config.cookie_secret)?
        .with_cookie_name("sid")
        .with_domain(DOMAIN_NAME);
    let external_login_cookie = ExternalLoginMeta::new(&config.cookie_secret)?
        .with_cookie_name("exl")
        .with_domain(DOMAIN_NAME);

    let oauth = AuthServiceBuilder::new(&config.oauth, &config.home_url, &identity_manager, &session_manager)
        .await?
        .into_router();
    let identity = IdentityServiceBuilder::new(&identity_manager).into_router();

    let app = Router::new()
        .route(&service_path("/info/ready"), get(health_check))
        .nest(&service_path("/oauth"), oauth)
        .nest(&service_path("/tracing"), tracing_router)
        .nest(&service_path("/api/identities"), identity)
        .layer(Extension(Arc::new(tera)))
        .layer(Extension(db_pool))
        .layer(session_cookie.into_layer())
        .layer(external_login_cookie.into_layer())
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
