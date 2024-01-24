mod db;
mod router;

#[cfg(feature = "std")]
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use router::route;

#[cfg(feature = "std")]
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "coordinator=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, route())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "coordinator=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    Ok(route().into())
}

#[cfg(feature = "std")]
async fn shutdown_signal() {
    use tracing::info;

    tokio::signal::ctrl_c()
        .await
        .expect("Expect shutdown signal handler");
    info!("Shutdown...");
}
