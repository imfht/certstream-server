use crate::certificate_buffer::CertificateBuffer;
use crate::client_manager::{ClientManager, StreamType};
use crate::config::*;
use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::{Html, IntoResponse, Json, Response},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{debug, info};

#[derive(Clone)]
struct AppState {
    client_manager: Arc<ClientManager>,
    cert_buffer: Arc<CertificateBuffer>,
}

pub async fn start_server(
    client_manager: Arc<ClientManager>,
    cert_buffer: Arc<CertificateBuffer>,
) -> Result<()> {
    let port = get_port();
    info!("Starting web server on port {}...", port);

    let state = AppState {
        client_manager,
        cert_buffer,
    };

    let app = Router::new()
        .route("/", get(websocket_handler))
        .route(FULL_STREAM_URL, get(full_stream_handler))
        .route(DOMAINS_ONLY_URL, get(domains_only_handler))
        .route("/example.json", get(example_json_handler))
        .route("/latest.json", get(latest_json_handler))
        .route(&format!("/{}", get_stats_url()), get(stats_handler))
        .nest_service("/static", ServeDir::new("frontend/dist/static"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("Server listening on {}", addr);
    
    axum::serve(listener, app).await?;

    Ok(())
}

async fn websocket_handler(
    _headers: HeaderMap,
    ws: Option<WebSocketUpgrade>,
    State(state): State<AppState>,
) -> Response {
    // Check if this is a WebSocket upgrade request
    if let Some(ws) = ws {
        ws.on_upgrade(|socket| handle_websocket(socket, state, StreamType::Lite))
            .into_response()
    } else {
        // Serve index.html for regular HTTP requests
        index_handler().await.into_response()
    }
}

async fn full_stream_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state, StreamType::Full))
}

async fn domains_only_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state, StreamType::DomainsOnly))
}

async fn handle_websocket(
    socket: WebSocket,
    state: AppState,
    stream_type: StreamType,
) {
    let (mut sender, mut receiver) = socket.split();
    let (client_id, mut rx) = state.client_manager.add_client(stream_type);

    // Spawn task to send messages to client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages (just for keeping connection alive)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) => {
                    // Pong is automatically sent by axum
                }
                _ => {
                    debug!("Received message from client: {:?}", msg);
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    state.client_manager.remove_client(client_id);
}

async fn example_json_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.cert_buffer.get_example() {
        Some(cert) => Json(cert).into_response(),
        None => Json(serde_json::json!({})).into_response(),
    }
}

async fn latest_json_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let certificates = state.cert_buffer.get_latest();
    Json(serde_json::json!({
        "messages": certificates
    }))
}

async fn stats_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let processed_certs = state.cert_buffer.get_processed_count();
    let clients = state.client_manager.get_clients_info();

    Json(serde_json::json!({
        "processed_certificates": processed_certs,
        "current_users": clients,
        "workers": {}
    }))
}

// Serve index.html for root path when not a WebSocket upgrade
async fn index_handler() -> impl IntoResponse {
    match tokio::fs::read_to_string("frontend/dist/index.html").await {
        Ok(content) => Html(content).into_response(),
        Err(_) => Html("<html><body><h1>CertStream Server</h1><p>Frontend not found</p></body></html>").into_response(),
    }
}
