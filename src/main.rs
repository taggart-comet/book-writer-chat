use book_writer_chat::{app::build_router, core::config::Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let config = Config::from_env()?;
    tracing::info!(
        environment = ?config.environment,
        address = %config.bind_addr,
        data_dir = %config.data_dir.display(),
        books_root = %config.books_root.display(),
        frontend_dist_dir = %config.frontend_dist_dir.display(),
        "loaded application configuration"
    );
    let router = build_router(config.clone()).await?;
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(address = %config.bind_addr, "server listening");
    axum::serve(listener, router).await?;
    Ok(())
}
