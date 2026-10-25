use std::{net::SocketAddr, sync::Arc};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "oxide_cms=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = oxide_cms::config::AppConfig::from_env();
    tracing::info!("Connecting to SQLite database: {}", config.database_url);

    let pool = oxide_cms::db::init_db(&config.database_url).await?;
    tracing::info!("SQLite database initialized and migrations applied successfully");

    tracing::info!("Compiling Tera templates from templates/**/*.html");
    let tera = oxide_cms::markdown::init_tera().map_err(|e| {
        tracing::error!("Tera template initialization failed: {e}");
        Box::new(e) as Box<dyn std::error::Error>
    })?;

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let state = oxide_cms::auth::AppState {
        pool,
        config,
        tera: Arc::new(tera),
    };
    let app = oxide_cms::create_app(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
