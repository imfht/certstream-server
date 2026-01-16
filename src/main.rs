use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod config;
mod types;
mod ct_parser;
mod ct_watcher;
mod certificate_buffer;
mod client_manager;
mod web_server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let log_level = std::env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .parse::<Level>()
        .unwrap_or(Level::INFO);
    
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    info!("Starting CertStream Server v{}", env!("CARGO_PKG_VERSION"));

    // Initialize shared state
    let certificate_buffer = certificate_buffer::CertificateBuffer::new();
    let client_manager = client_manager::ClientManager::new();

    // Start CT log watchers
    let watchers = ct_watcher::start_watchers(
        client_manager.clone(),
        certificate_buffer.clone(),
    );

    // Start web server
    let web_server = web_server::start_server(
        client_manager.clone(),
        certificate_buffer.clone(),
    );

    // Wait for both tasks
    tokio::select! {
        result = watchers => {
            result?;
        }
        result = web_server => {
            result?;
        }
    }

    Ok(())
}
