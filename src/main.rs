use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
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

    // Start CT log watchers (restart on unexpected exit)
    let watcher_supervisor = tokio::spawn({
        let client_manager = client_manager.clone();
        let certificate_buffer = certificate_buffer.clone();
        async move {
            loop {
                match ct_watcher::start_watchers(client_manager.clone(), certificate_buffer.clone())
                    .await
                {
                    Ok(()) => warn!(
                        "CT watcher task exited unexpectedly. Restarting in {} seconds...",
                        config::INIT_RETRY_DELAY_SECS
                    ),
                    Err(err) => error!(
                        "CT watcher task exited: {}. Restarting in {} seconds...",
                        err,
                        config::INIT_RETRY_DELAY_SECS
                    ),
                }

                sleep(Duration::from_secs(config::INIT_RETRY_DELAY_SECS)).await;
            }
        }
    });

    // Start web server
    let web_server = web_server::start_server(client_manager.clone(), certificate_buffer.clone());

    // Wait for the web server (watcher supervisor should run indefinitely)
    tokio::select! {
        result = web_server => {
            match result {
                Ok(()) => warn!("Web server exited unexpectedly."),
                Err(err) => error!("Web server exited: {}", err),
            }
        }
        _ = watcher_supervisor => {
            warn!("CT watcher supervisor exited unexpectedly.");
        }
    }

    Ok(())
}
