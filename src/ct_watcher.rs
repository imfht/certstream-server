use crate::certificate_buffer::CertificateBuffer;
use crate::client_manager::ClientManager;
use crate::config::*;
use crate::ct_parser::parse_ct_entry;
use crate::types::*;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

pub async fn start_watchers(
    client_manager: Arc<ClientManager>,
    cert_buffer: Arc<CertificateBuffer>,
) -> Result<()> {
    info!("Initializing CT Watchers...");

    // Fetch CT log list
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let log_list_url = get_log_list_url();
    info!("Fetching CT log list from {}", log_list_url);

    let mut log_list_retries = 0;
    let log_list: CTLogList = loop {
        match client
            .get(&log_list_url)
            .header("User-Agent", get_user_agent())
            .send()
            .await
        {
            Ok(response) => match response.json().await {
                Ok(log_list) => break log_list,
                Err(e) => {
                    log_list_retries += 1;
                    if log_list_retries >= MAX_INIT_RETRIES {
                        warn!(
                            "Failed to parse CT log list after {} attempts: {}. Restarting retry cycle and continuing every {} seconds...",
                            MAX_INIT_RETRIES, e, INIT_RETRY_DELAY_SECS
                        );
                        log_list_retries = 0;
                    } else {
                        warn!(
                            "Failed to parse CT log list (attempt {}/{}): {}. Retrying in {} seconds...",
                            log_list_retries, MAX_INIT_RETRIES, e, INIT_RETRY_DELAY_SECS
                        );
                    }
                }
            },
            Err(e) => {
                log_list_retries += 1;
                if log_list_retries >= MAX_INIT_RETRIES {
                    warn!(
                        "Failed to fetch CT log list after {} attempts: {}. Restarting retry cycle and continuing every {} seconds...",
                        MAX_INIT_RETRIES, e, INIT_RETRY_DELAY_SECS
                    );
                    log_list_retries = 0;
                } else {
                    warn!(
                        "Failed to fetch CT log list (attempt {}/{}): {}. Retrying in {} seconds...",
                        log_list_retries, MAX_INIT_RETRIES, e, INIT_RETRY_DELAY_SECS
                    );
                }
            }
        }

        sleep(Duration::from_secs(INIT_RETRY_DELAY_SECS)).await;
    };

    let mut handles = Vec::new();

    for operator in log_list.operators {
        if let Some(logs) = operator.logs {
            for log in logs {
                let operator_name = operator.name.clone();
                let client_manager = client_manager.clone();
                let cert_buffer = cert_buffer.clone();

                let handle = tokio::spawn(async move {
                    // Add random delay to stagger startup (0-3 seconds)
                    let delay = rand::random::<f64>() * 3.0;
                    sleep(Duration::from_secs_f64(delay)).await;

                    if let Err(e) =
                        watch_ct_log(operator_name, log, client_manager, cert_buffer).await
                    {
                        error!("CT watcher error: {}", e);
                    }
                });

                handles.push(handle);
            }
        }
    }

    // Wait for all watchers
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

fn normalize_log_url(url: &str) -> String {
    if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{}/", url)
    }
}

fn ct_api_url(base_url: &str, path: &str) -> String {
    format!("{}{}", normalize_log_url(base_url), path)
}

async fn watch_ct_log(
    operator_name: String,
    log: CTLog,
    client_manager: Arc<ClientManager>,
    cert_buffer: Arc<CertificateBuffer>,
) -> Result<()> {
    let url = log.url.clone();

    info!("Starting watcher for {}", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // Initialize state
    let mut state = CTLogState {
        operator_name: operator_name.clone(),
        description: log.description.clone(),
        url: url.clone(),
        tree_size: 0,
        batch_size: 256, // Will be determined on first fetch
        processed_count: 0,
    };

    // Determine batch size
    match fetch_batch_size(&client, &state).await {
        Ok(size) => {
            state.batch_size = size;
            info!("Worker with url {} found batch size of {}", url, size);
        }
        Err(e) => {
            warn!("Failed to determine batch size for {}: {}", url, e);
        }
    }

    // Get initial tree size with retries
    let mut retries = 0;
    loop {
        match get_tree_size(&client, &state).await {
            Ok(size) => {
                state.tree_size = size;
                info!("Initial tree size for {}: {}", url, size);
                break;
            }
            Err(e) => {
                retries += 1;
                if retries >= MAX_INIT_RETRIES {
                    warn!(
                        "Failed to get initial tree size for {} after {} retries: {}. Restarting retry cycle and continuing every {} seconds...",
                        url, MAX_INIT_RETRIES, e, INIT_RETRY_DELAY_SECS
                    );
                    retries = 0;
                } else {
                    warn!(
                        "Failed to get initial tree size for {} (attempt {}/{}): {}. Retrying in {} seconds...",
                        url, retries, MAX_INIT_RETRIES, e, INIT_RETRY_DELAY_SECS
                    );
                }
                sleep(Duration::from_secs(INIT_RETRY_DELAY_SECS)).await;
            }
        }
    }

    // Poll for updates
    let mut interval = interval(Duration::from_secs(CT_UPDATE_INTERVAL_SECS));

    loop {
        interval.tick().await;

        match get_tree_size(&client, &state).await {
            Ok(current_tree_size) => {
                if current_tree_size > state.tree_size {
                    let cert_count = current_tree_size - state.tree_size;
                    info!(
                        "Worker with url {} found {} certificates [{} -> {}]",
                        url, cert_count, state.tree_size, current_tree_size
                    );

                    if let Err(e) = broadcast_updates(
                        &client,
                        &mut state,
                        current_tree_size,
                        &client_manager,
                        &cert_buffer,
                    )
                    .await
                    {
                        error!("Error broadcasting updates from {}: {}", url, e);
                    }

                    state.tree_size = current_tree_size;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to get tree size for {}: {}. Will retry on next poll.",
                    url, e
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_log_url;

    #[test]
    fn normalize_log_url_preserves_trailing_slash() {
        assert_eq!(
            normalize_log_url("https://example.test/log/"),
            "https://example.test/log/"
        );
    }

    #[test]
    fn normalize_log_url_adds_trailing_slash() {
        assert_eq!(
            normalize_log_url("https://example.test/log"),
            "https://example.test/log/"
        );
    }
}

async fn fetch_batch_size(client: &reqwest::Client, state: &CTLogState) -> Result<usize> {
    let url = ct_api_url(&state.url, "ct/v1/get-entries?start=0&end=511");

    let response: CTLogEntries = client
        .get(&url)
        .header("User-Agent", get_user_agent())
        .send()
        .await?
        .json()
        .await?;

    Ok(response.entries.len())
}

async fn get_tree_size(client: &reqwest::Client, state: &CTLogState) -> Result<u64> {
    let url = ct_api_url(&state.url, "ct/v1/get-sth");

    let response: CTTreeHead = client
        .get(&url)
        .header("User-Agent", get_user_agent())
        .send()
        .await?
        .json()
        .await?;

    Ok(response.tree_size)
}

async fn broadcast_updates(
    client: &reqwest::Client,
    state: &mut CTLogState,
    current_size: u64,
    client_manager: &Arc<ClientManager>,
    cert_buffer: &Arc<CertificateBuffer>,
) -> Result<()> {
    let certificate_count = current_size - state.tree_size;
    let certificates: Vec<u64> = (state.tree_size..current_size).collect();

    info!("Certificate count - {}", certificate_count);

    // Process certificates in batches
    let chunks: Vec<_> = certificates.chunks(state.batch_size).collect();

    // Use semaphore to limit concurrent requests
    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_FETCHES));
    let mut tasks = Vec::new();

    for chunk in chunks {
        let client = client.clone();
        let state_clone = state.clone();
        let chunk = chunk.to_vec();
        let client_manager = client_manager.clone();
        let cert_buffer = cert_buffer.clone();
        let permit = semaphore.clone().acquire_owned().await?;

        let task = tokio::spawn(async move {
            let result = fetch_and_broadcast_certs(
                &client,
                &state_clone,
                &chunk,
                &client_manager,
                &cert_buffer,
            )
            .await;
            drop(permit);
            result
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        if let Err(e) = task.await? {
            error!("Error fetching and broadcasting certs: {}", e);
        }
    }

    state.processed_count += certificate_count;

    Ok(())
}

async fn fetch_and_broadcast_certs(
    client: &reqwest::Client,
    state: &CTLogState,
    ids: &[u64],
    client_manager: &Arc<ClientManager>,
    cert_buffer: &Arc<CertificateBuffer>,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut start_index = 0;

    while start_index < ids.len() {
        let batch_ids = &ids[start_index..];

        debug!("Attempting to retrieve {} entries", batch_ids.len());

        let start = batch_ids[0];
        let end = batch_ids[batch_ids.len() - 1];
        let url = ct_api_url(
            &state.url,
            &format!("ct/v1/get-entries?start={}&end={}", start, end),
        );

        let response: CTLogEntries = match client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .send()
            .await
        {
            Ok(resp) => resp.json().await?,
            Err(e) => {
                error!("Failed to fetch entries from {}: {}", url, e);
                return Err(e.into());
            }
        };

        let mut cert_updates = Vec::new();

        for (entry, cert_index) in response.entries.iter().zip(batch_ids.iter()) {
            match parse_ct_entry(entry) {
                Ok((leaf_cert, chain)) => {
                    // Convert to chain certificates for the chain
                    let chain_certs: Vec<ChainCertificate> = chain.into_iter().collect();

                    let cert_data = CertificateData {
                        update_type: "X509LogEntry".to_string(),
                        leaf_cert,
                        chain: chain_certs,
                        cert_index: *cert_index,
                        seen: chrono::Utc::now().timestamp_micros() as f64 / 1_000_000.0,
                        source: CertSource {
                            url: state.url.clone(),
                            name: state.description.clone(),
                        },
                        cert_link: Some(ct_api_url(
                            &state.url,
                            &format!("ct/v1/get-entries?start={}&end={}", cert_index, cert_index),
                        )),
                    };

                    cert_updates.push(cert_data);
                }
                Err(e) => {
                    debug!("Failed to parse certificate: {}", e);
                }
            }
        }

        // Broadcast to clients
        client_manager.broadcast_certificates(&cert_updates).await;
        cert_buffer.add_certificates(&cert_updates).await;

        // Handle case where we got fewer entries than requested
        let entry_count = response.entries.len();
        let batch_count = batch_ids.len();

        if entry_count == 0 {
            warn!(
                "Received empty response from {}, stopping batch to avoid infinite retry",
                url
            );
            break;
        }

        if entry_count < batch_count {
            debug!(
                "Didn't retrieve all entries for this batch, fetching missing {} entries",
                batch_count - entry_count
            );
        }

        start_index += entry_count;
    }

    Ok(())
}

// Add random number generation dependency
use rand::random;
