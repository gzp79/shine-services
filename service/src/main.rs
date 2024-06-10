mod app_config;
mod auth;
mod identity;
mod openapi;
mod repositories;

use crate::{
    app_config::{AppConfig, SERVICE_NAME},
    auth::{AuthServiceBuilder, AuthServiceDependencies},
    identity::{IdentityServiceBuilder, IdentityServiceDependencies},
    repositories::{AutoNameManager, DBPool, IdentityManager, SessionManager},
};
use anyhow::{anyhow, Error as AnyError};
use axum::{
    http::StatusCode,
    http::{header, Method},
    Router,
};
use axum_server::Handle;
use chrono::Duration;
use openapi::ApiKind;
use shine_service::{
    axum::{
        add_default_components, telemetry::TelemetryManager, ApiEndpoint, ApiMethod, ApiPath, ApiRoute, PoweredBy,
        ProblemConfig,
    },
    service::UserSessionValidator,
};
use std::{env, fs, net::SocketAddr, time::Duration as StdDuration};
use tera::Tera;
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

#[derive(OpenApi)]
#[openapi(paths(), components(), tags())]
struct ApiDoc;

async fn health_check() -> String {
    "Ok".into()
}

fn ep_health_check() -> ApiEndpoint<()> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Absolute("/info/ready"), health_check)
        .with_operation_id("health_check")
        .with_tag("status")
        .with_status_response(StatusCode::OK, "Ok.")
}

async fn graceful_shutdown(handle: Handle) {
    signal::ctrl_c().await.expect("expect tokio signal ctrl-c");
    log::warn!("Shutting down the server...");
    handle.graceful_shutdown(Some(StdDuration::from_secs(10)));
}

async fn async_main(_rt_handle: RtHandle) -> Result<(), AnyError> {
    let args: Vec<String> = env::args().collect();
    let stage = args.get(1).ok_or(anyhow!("Missing stage parameter"))?.clone();

    let (config, telemetry_manager) = {
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
        let telemetry_manager = TelemetryManager::new(SERVICE_NAME, &config.telemetry).await?;
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

    let cors = CorsLayer::default()
        .allow_origin(config.service.cors_allowed_origin()?)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_credentials(true);
    let powered_by = PoweredBy::from_service_info(SERVICE_NAME, &config.core.version)?;

    let mut doc = ApiDoc::openapi();
    add_default_components(&mut doc);

    let auth_config = &config.auth.auth_session;
    let tera = {
        let mut tera = Tera::new("tera_templates/**/*").map_err(|e| anyhow!(e))?;
        tera.autoescape_on(vec![".html"]);
        tera
    };

    let db_pool = DBPool::new(&config.db).await?;
    let user_session = UserSessionValidator::new(None, &auth_config.session_secret, "", db_pool.redis.clone())?;
    let problem_config = ProblemConfig::new(config.service.full_problem_response);
    let identity_manager = IdentityManager::new(&db_pool.postgres).await?;
    let ttl_session = Duration::seconds(i64::try_from(auth_config.ttl_session)?);
    let session_manager = SessionManager::new(&db_pool.redis, String::new(), ttl_session).await?;
    let auto_name_manager = AutoNameManager::new(&config.user_name, &db_pool.postgres).await?;

    let log_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));
    let telemetry_layer = telemetry_manager.to_layer();

    let health_check = Router::new().add_api(ep_health_check(), &mut doc);

    let (auth_pages, auth_api) = {
        let auth_state = AuthServiceDependencies {
            tera: tera.clone(),
            identity_manager: identity_manager.clone(),
            session_manager: session_manager.clone(),
            auto_name_manager: auto_name_manager.clone(),
        };
        AuthServiceBuilder::new(auth_state, &config.auth)
            .await?
            .into_router(&mut doc)
    };

    let identity_api = {
        let identity_state = IdentityServiceDependencies {
            telemetry_manager,
            identity_manager: identity_manager.clone(),
            session_manager: session_manager.clone(),
            auto_name_manager: auto_name_manager.clone(),
            db: db_pool.clone(),
        };
        IdentityServiceBuilder::new(identity_state, config.auth.super_user_api_key_hash.as_deref())
            .into_router(&mut doc)
    };

    let swagger = SwaggerUi::new(ApiKind::Doc("/swagger-ui").path())
        .url(ApiKind::Doc("/openapi.json").path(), doc)
        .config(
            SwaggerConfig::default()
                .with_credentials(true)
                .show_common_extensions(true),
        );

    let app = Router::new()
        .merge(health_check)
        .merge(auth_pages)
        .merge(identity_api)
        .merge(auth_api)
        .merge(swagger)
        .layer(user_session.into_layer())
        .layer(problem_config.into_layer())
        .layer(powered_by)
        .layer(cors)
        .layer(telemetry_layer)
        .layer(log_layer);

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    //log::trace!("{app:#?}");
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));

    if let Some(tls_config) = &config.service.tls {
        log::info!("Starting service on {addr:?} using tls");
        let cert = fs::read(&tls_config.cert)?;
        let key = fs::read(&tls_config.key)?;
        let config = axum_server::tls_rustls::RustlsConfig::from_pem(cert, key)
            .await
            .map_err(|e| anyhow!(e))?;
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .map_err(|e| anyhow!(e))
    } else {
        log::info!("Starting service on {addr:?}");
        let listener = TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app).await.map_err(|e| anyhow!(e))
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
