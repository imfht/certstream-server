# CertStream Server (Rust Edition)

This is a high-performance Rust rewrite of the CertStream server, maintaining 100% API compatibility with the original Elixir version while delivering superior performance and resource efficiency.

## Features

- **100% API Compatible**: Drop-in replacement for the original CertStream server
- **High Performance**: Built with Rust for maximum speed and minimal resource usage
- **Concurrent**: Leverages Tokio for efficient async I/O and concurrent CT log processing
- **WebSocket Streaming**: Real-time certificate updates via WebSocket connections
- **Multiple Stream Types**: Support for full, lite, and domains-only streams
- **HTTP API**: RESTful endpoints for latest certificates and statistics

## API Compatibility

### WebSocket Endpoints

- `/` - Lite stream (no DER encoding, no chain)
- `/full-stream` - Full stream with DER encoding and certificate chain
- `/domains-only` - Only domain names from certificates

### HTTP Endpoints

- `/latest.json` - Get the most recent 25 certificates
- `/example.json` - Get the most recent certificate
- `/stats` - Server statistics (processed certificates, connected clients)

## Performance Improvements

The Rust implementation offers several performance advantages over the original Elixir version:

- **Lower Memory Usage**: Efficient memory management with zero-copy parsing where possible
- **Faster Certificate Parsing**: Native performance without VM overhead
- **Better Concurrency**: Tokio runtime provides excellent async performance
- **Optimized Builds**: Release builds with LTO and aggressive optimizations

## Building

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Build Release Binary

```bash
cargo build --release
```

The optimized binary will be available at `target/release/certstream-server`.

### Build Development Binary

```bash
cargo build
```

## Running

### Using Cargo

```bash
cargo run --release
```

### Direct Execution

```bash
./target/release/certstream-server
```

### Configuration

Configure the server using environment variables:

- `PORT` - HTTP server port (default: 4000)
- `LOG_LEVEL` - Logging level: trace, debug, info, warn, error (default: info)
- Logs are written to stdout/stderr. Increase `LOG_LEVEL` for more detail, and check for shutdown messages if the server stops unexpectedly.
- `USER_AGENT` - Custom user agent for CT log requests (default: "Certstream Server v{VERSION}")
- `CT_LOG_LIST_URL` - Override CT log list endpoint (default: https://www.gstatic.com/ct/log_list/v3/all_logs_list.json)
- `STATS_URL` - Custom stats endpoint path (default: "stats")

Example:

```bash
PORT=8080 LOG_LEVEL=debug ./target/release/certstream-server
```

## Architecture

The Rust implementation follows the same architecture as the original:

### Components

1. **CT Watchers**: Poll Certificate Transparency logs for new certificates
2. **Certificate Parser**: Parse X.509 certificates from CT log entries
3. **Certificate Buffer**: Ring buffer maintaining the latest 25 certificates
4. **Client Manager**: Manage WebSocket connections and broadcast updates
5. **Web Server**: Axum-based HTTP/WebSocket server

### Data Flow

```
CT Log Servers → CT Watchers → Certificate Parser → Client Manager → WebSocket Clients
                                      ↓
                              Certificate Buffer → HTTP API
```

## Dependencies

Key dependencies include:

- `tokio` - Async runtime
- `axum` - Web framework
- `x509-parser` - Certificate parsing
- `reqwest` - HTTP client for CT logs
- `serde` / `serde_json` - JSON serialization
- `dashmap` - Concurrent HashMap
- `tracing` - Logging

## Comparison with Original

| Feature | Elixir Original | Rust Rewrite |
|---------|----------------|--------------|
| Language | Elixir | Rust |
| Runtime | BEAM VM | Native |
| Memory Usage | Higher | Lower |
| CPU Usage | Moderate | Lower |
| Startup Time | Slower | Faster |
| API Compatibility | N/A | 100% |
| Concurrency Model | Actor-based | Async/await |

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## License

MIT License - Same as the original CertStream server

## Credits

- Original CertStream server by CaliDog
- Rust rewrite maintains the same architecture and API design
