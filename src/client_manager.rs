use crate::types::*;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

pub type ClientId = usize;

#[derive(Clone)]
pub enum StreamType {
    Lite,      // Default - no DER, no chain
    Full,      // Full stream with DER and chain
    DomainsOnly, // Only domain names
}

pub struct ClientInfo {
    pub id: ClientId,
    pub stream_type: StreamType,
    pub sender: mpsc::UnboundedSender<String>,
}

pub struct ClientManager {
    clients: Arc<DashMap<ClientId, ClientInfo>>,
    next_id: Arc<std::sync::atomic::AtomicUsize>,
}

impl ClientManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            clients: Arc::new(DashMap::new()),
            next_id: Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        })
    }

    pub fn add_client(&self, stream_type: StreamType) -> (ClientId, mpsc::UnboundedReceiver<String>) {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = mpsc::unbounded_channel();

        let client = ClientInfo {
            id,
            stream_type,
            sender: tx,
        };

        self.clients.insert(id, client);
        info!("Client {} connected. Total clients: {}", id, self.clients.len());

        (id, rx)
    }

    pub fn remove_client(&self, id: ClientId) {
        self.clients.remove(&id);
        info!("Client {} disconnected. Total clients: {}", id, self.clients.len());
    }

    pub fn get_client_count(&self) -> usize {
        self.clients.len()
    }

    pub async fn broadcast_certificates(&self, certs: &[CertificateData]) {
        debug!("Broadcasting {} certificates to {} clients", certs.len(), self.clients.len());

        // Prepare different versions
        let full_updates: Vec<String> = certs
            .iter()
            .map(|cert| {
                let update = CertificateUpdate {
                    message_type: "certificate_update".to_string(),
                    data: cert.clone(),
                };
                serde_json::to_string(&update).unwrap()
            })
            .collect();

        let lite_updates: Vec<String> = certs
            .iter()
            .map(|cert| {
                let mut cert = cert.clone();
                cert.leaf_cert.as_der = None;
                cert.chain.clear();

                let update = CertificateUpdate {
                    message_type: "certificate_update".to_string(),
                    data: cert,
                };
                serde_json::to_string(&update).unwrap()
            })
            .collect();

        let domain_updates: Vec<String> = certs
            .iter()
            .map(|cert| {
                let msg = DnsEntriesMessage {
                    message_type: "dns_entries".to_string(),
                    data: cert.leaf_cert.all_domains.clone(),
                };
                serde_json::to_string(&msg).unwrap()
            })
            .collect();

        // Broadcast to each client based on their stream type
        for entry in self.clients.iter() {
            let client = entry.value();
            let messages = match client.stream_type {
                StreamType::Full => &full_updates,
                StreamType::Lite => &lite_updates,
                StreamType::DomainsOnly => &domain_updates,
            };

            for msg in messages {
                if let Err(e) = client.sender.send(msg.clone()) {
                    warn!("Failed to send to client {}: {}", client.id, e);
                }
            }
        }
    }

    pub fn get_clients_info(&self) -> serde_json::Value {
        let mut clients_map = serde_json::Map::new();
        
        for entry in self.clients.iter() {
            let client = entry.value();
            let stream_type = match client.stream_type {
                StreamType::Full => "full",
                StreamType::Lite => "lite",
                StreamType::DomainsOnly => "domains_only",
            };

            let info = serde_json::json!({
                "id": client.id,
                "stream_type": stream_type,
            });

            clients_map.insert(format!("client_{}", client.id), info);
        }

        serde_json::Value::Object(clients_map)
    }
}
