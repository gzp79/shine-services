mod app_config;
mod app_error;
mod app_session;
mod auth;
mod db;
mod utils;

use crate::{
    app_config::{AppConfig, SERVICE_NAME},
    app_session::{AppSessionMeta, ExternalLoginMeta},
    auth::AuthServiceBuilder,
    db::{IdentityManager, SessionManager},
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::{header, Method},
    routing::get,
    Extension, Router,
};
use shine_service::axum::tracing::{tracing_layer, TracingService};
use std::{net::SocketAddr, sync::Arc};
use tera::Tera;
use tokio::{
    runtime::{Handle as RtHandle, Runtime},
    signal,
};
use tower_http::cors::CorsLayer;
use tracing::Dispatch;

async fn health_check() -> String {
    "Ok".to_string()
}

async fn shutdown_signal() {
    signal::ctrl_c().await.expect("expect tokio signal ctrl-c");
    log::warn!("Signal shutdown");
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

    let db_pool = db::create_pool(&config.db.connection_string).await?;
    let identity_manager = IdentityManager::new(db_pool);
    let session_manager = SessionManager::new();
    let session_cookie = AppSessionMeta::new(&config.cookie_secret)?.with_cookie_name("sid");
    let external_login_cookie = ExternalLoginMeta::new(&config.cookie_secret)?.with_cookie_name("exl");

    let oauth = AuthServiceBuilder::new(&config.oauth, &config.home_url, &identity_manager, &session_manager)
        .await?
        .into_router();

    let app = Router::new()
        .route("/info/ready", get(health_check))
        .nest("/oauth", oauth)
        .nest("/tracing", tracing_router)
        .layer(Extension(Arc::new(tera)))
        .layer(session_cookie.into_layer())
        .layer(external_login_cookie.into_layer())
        .layer(cors)
        .layer(tracing_layer);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.control_port));
    log::info!("listening on {addr:?}");

    /*
    let config = axum_server::tls_rustls::RustlsConfig::from_pem_file("temp/_wildcard.playcrey.com.pem", "temp/_wildcard.playcrey.com-key.pem")
        .await
        .map_err(|e| anyhow!(e))?;
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .map_err(|e| anyhow!(e))
        */

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow!(e))
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
