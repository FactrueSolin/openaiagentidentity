use std::net::IpAddr;

use agentidentity::app::shell;
use agentidentity::web_config::{ConfigSource, WebConfig};
use agentidentity::web_server::{WebState, api_router};
use anyhow::{Context, Result};
use axum::Router;
use axum::http::{HeaderValue, header};
use axum::routing::get;
use leptos::prelude::get_configuration;
use leptos_axum::{file_and_error_handler, render_app_to_stream};
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    any_spawner::Executor::init_tokio()
        .context("failed to initialize the Leptos Tokio executor")?;
    initialize_logging();

    let config = WebConfig::load()?;
    log_configuration(&config);
    let state = WebState::from_config(&config)?;

    let mut leptos_options = get_configuration(None)
        .context("failed to load Leptos configuration")?
        .leptos_options;
    leptos_options.site_addr = format!("{}:{}", config.host, config.port)
        .parse()
        .unwrap_or(leptos_options.site_addr);

    let render_options = leptos_options.clone();
    let pages = Router::new()
        .route(
            "/",
            get(render_app_to_stream(move || shell(render_options.clone()))),
        )
        .fallback(file_and_error_handler(shell))
        .with_state(leptos_options);

    let app = pages
        .merge(api_router(state))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ));

    let address = config.socket_address();
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind web server to {address}"))?;
    info!(address = %address, "listening for HTTP requests");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("web server stopped unexpectedly")
}

fn initialize_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

fn log_configuration(config: &WebConfig) {
    info!(
        value = %config.host,
        source = source_name(config.host_source),
        "HOST"
    );
    info!(
        value = config.port,
        source = source_name(config.port_source),
        "PORT"
    );
    info!(
        value = %config.safe_proxy_display(),
        source = source_name(config.proxy_source),
        "PROXY_URL"
    );

    if !is_loopback_host(&config.host) {
        warn!(
            "non-loopback listener configured; production traffic must use an HTTPS reverse proxy that does not log request bodies"
        );
    }
}

fn source_name(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::Environment => "environment",
        ConfigSource::Dotenv => ".env",
        ConfigSource::Default => "default",
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(%error, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => warn!(%error, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!("shutdown signal received");
}
