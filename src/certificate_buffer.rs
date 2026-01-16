use crate::config::CERT_BUFFER_SIZE;
use crate::types::*;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct CertificateBuffer {
    certificates: Arc<RwLock<Vec<CertificateUpdate>>>,
    processed_count: Arc<AtomicU64>,
}

impl CertificateBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            certificates: Arc::new(RwLock::new(Vec::new())),
            processed_count: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn add_certificates(&self, certs: &[CertificateData]) {
        let cert_count = certs.len() as u64;
        self.processed_count
            .fetch_add(cert_count, Ordering::Relaxed);

        // Convert to CertificateUpdate without DER and chain
        let updates: Vec<CertificateUpdate> = certs
            .iter()
            .map(|cert| {
                let mut cert = cert.clone();
                // Remove DER from leaf cert
                cert.leaf_cert.as_der = None;
                // Remove chain
                cert.chain.clear();

                CertificateUpdate {
                    message_type: "certificate_update".to_string(),
                    data: cert,
                }
            })
            .collect();

        let mut buffer = self.certificates.write();

        // Add to front and keep only the latest CERT_BUFFER_SIZE
        for update in updates.into_iter().rev() {
            buffer.insert(0, update);
        }

        if buffer.len() > CERT_BUFFER_SIZE {
            buffer.truncate(CERT_BUFFER_SIZE);
        }
    }

    pub fn get_example(&self) -> Option<CertificateUpdate> {
        self.certificates.read().first().cloned()
    }

    pub fn get_latest(&self) -> Vec<CertificateUpdate> {
        self.certificates.read().clone()
    }

    pub fn get_processed_count(&self) -> u64 {
        self.processed_count.load(Ordering::Relaxed)
    }
}
