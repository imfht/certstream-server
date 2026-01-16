use anyhow::Result;
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

mod certificate_buffer;
mod client_manager;
mod config;
mod ct_parser;
mod ct_watcher;
mod types;
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

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("Starting CertStream Server v{}", env!("CARGO_PKG_VERSION"));

    // Initialize shared state
    let certificate_buffer = certificate_buffer::CertificateBuffer::new();
    let client_manager = client_manager::ClientManager::new();

    // Start CT log watchers
    let watchers = ct_watcher::start_watchers(client_manager.clone(), certificate_buffer.clone());

    // Start web server
    let web_server = web_server::start_server(client_manager.clone(), certificate_buffer.clone());

    // Wait for both tasks
    tokio::select! {
        result = watchers => {
            match result {
                Ok(()) => warn!("CT watcher task exited unexpectedly."),
                Err(err) => error!("CT watcher task exited: {}", err),
            }
        }
        result = web_server => {
            match result {
                Ok(()) => warn!("Web server exited unexpectedly."),
                Err(err) => error!("Web server exited: {}", err),
            }
        }
    }

    Ok(())
}
