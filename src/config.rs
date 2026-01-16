pub const DEFAULT_PORT: u16 = 4000;
pub const DEFAULT_USER_AGENT: &str = concat!("Certstream Server v", env!("CARGO_PKG_VERSION"));
pub const FULL_STREAM_URL: &str = "/full-stream";
pub const DOMAINS_ONLY_URL: &str = "/domains-only";
pub const CT_LOG_LIST_URL: &str = "https://www.gstatic.com/ct/log_list/v3/all_logs_list.json";
pub const CERT_BUFFER_SIZE: usize = 25;
pub const POBOX_BUFFER_SIZE: usize = 500;
pub const CT_UPDATE_INTERVAL_SECS: u64 = 10;
pub const MAX_CONCURRENT_FETCHES: usize = 5;

pub fn get_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub fn get_user_agent() -> String {
    std::env::var("USER_AGENT")
        .unwrap_or_else(|_| DEFAULT_USER_AGENT.to_string())
}

pub fn get_stats_url() -> String {
    std::env::var("STATS_URL")
        .unwrap_or_else(|_| "stats".to_string())
}
