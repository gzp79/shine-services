mod app_config;
mod auth;
mod db;
mod services;

use crate::{
    app_config::{AppConfig, SERVICE_NAME},
    auth::{AuthServiceBuilder, AuthServiceDependencies},
    db::{DBPool, IdentityManager, NameGenerator, SessionManager},
    services::{IdentityServiceBuilder, IdentityServiceDependencies},
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    routing::get,
    Router,
};
use chrono::Duration;
use shine_service::{
    axum::{
        tracing::{OtelAxumLayer, TracingService},
        PoweredBy,
    },
    service::UserSessionValidator,
};
use std::net::SocketAddr;
use tera::Tera;
use tokio::{
    runtime::{Handle as RtHandle, Runtime},
    signal,
};
use tower_http::cors::CorsLayer;
use tracing::Dispatch;
use tracing_subscriber::EnvFilter;

async fn health_check() -> String {
    "Ok".into()
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

        let config = AppConfig::new().await?;
        let tracing_service = TracingService::new(SERVICE_NAME, &config.tracing).await?;
        log::info!("pre-init completed");
        (config, tracing_service)
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
    let tracing_layer = OtelAxumLayer::default(); //.filter(|a| true);

    let tera = {
        let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
        tera.autoescape_on(vec![".html"]);
        tera
    };

    let auth_config = &config.auth.auth_session;

    let db_pool = DBPool::new(&config.db).await?;
    let user_session = UserSessionValidator::new(None, &auth_config.session_secret, db_pool.redis.clone())?;
    let identity_manager = IdentityManager::new(&db_pool).await?;
    let session_max_duration = Duration::seconds(i64::try_from(auth_config.session_max_duration)?);
    let session_manager = SessionManager::new(&db_pool, session_max_duration).await?;
    let name_generator = NameGenerator::new(&config.user_name, &db_pool).await?;

    let (auth_pages, auth_api) = {
        let auth_state = AuthServiceDependencies {
            tera: tera.clone(),
            identity_manager: identity_manager.clone(),
            session_manager: session_manager.clone(),
            name_generator: name_generator.clone(),
        };
        AuthServiceBuilder::new(auth_state, &config.auth).await?.into_router()
    };

    let identity_api = {
        let identity_state = IdentityServiceDependencies {
            identity_manager: identity_manager.clone(),
            name_generator: name_generator.clone(),
            db: db_pool.clone(),
        };
        IdentityServiceBuilder::new(identity_state).into_router()
    };

    let app = Router::new()
        .route(&service_path("/info/ready"), get(health_check))
        .nest(&service_path(""), auth_pages)
        .nest(&service_path("/api/tracing"), tracing_router)
        .nest(&service_path("/api"), identity_api)
        .nest(&service_path("/api"), auth_api)
        .layer(user_session.into_layer())
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
