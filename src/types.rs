use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateUpdate {
    pub message_type: String,
    pub data: CertificateData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateData {
    pub update_type: String,
    pub leaf_cert: LeafCertificate,
    pub chain: Vec<ChainCertificate>,
    pub cert_index: u64,
    pub seen: f64,
    pub source: CertSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafCertificate {
    pub subject: Subject,
    pub extensions: HashMap<String, String>,
    pub not_before: f64,
    pub not_after: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_der: Option<String>,
    pub all_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainCertificate {
    pub subject: Subject,
    pub extensions: HashMap<String, String>,
    pub not_before: f64,
    pub not_after: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_der: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub aggregated: String,
    #[serde(rename = "C")]
    pub c: Option<String>,
    #[serde(rename = "ST")]
    pub st: Option<String>,
    #[serde(rename = "L")]
    pub l: Option<String>,
    #[serde(rename = "O")]
    pub o: Option<String>,
    #[serde(rename = "OU")]
    pub ou: Option<String>,
    #[serde(rename = "CN")]
    pub cn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertSource {
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsEntriesMessage {
    pub message_type: String,
    pub data: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CTLogList {
    pub operators: Vec<CTOperator>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CTOperator {
    pub name: String,
    pub logs: Option<Vec<CTLog>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CTLog {
    pub description: String,
    pub url: String,
    pub maximum_merge_delay: Option<u64>,
    pub operated_by: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CTLogEntries {
    pub entries: Vec<CTLogEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CTLogEntry {
    pub leaf_input: String,
    pub extra_data: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CTTreeHead {
    pub tree_size: u64,
    pub timestamp: u64,
    pub sha256_root_hash: String,
    pub tree_head_signature: String,
}

#[derive(Debug, Clone)]
pub struct CTLogState {
    pub operator_name: String,
    pub description: String,
    pub url: String,
    pub tree_size: u64,
    pub batch_size: usize,
    pub processed_count: u64,
}
