use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use once_cell::sync::Lazy;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info};

use crate::sync::ClipPayload;

const BROADCAST_CAP: usize = 64;

/// TX_OUT: 本机产生的消息
static TX_OUT: Lazy<broadcast::Sender<String>> = Lazy::new(|| {
    let (tx, _) = broadcast::channel(BROADCAST_CAP);
    tx
});

/// TX_IN: 从远端收到的消息
static TX_IN: Lazy<broadcast::Sender<String>> = Lazy::new(|| {
    let (tx, _) = broadcast::channel(BROADCAST_CAP);
    tx
});

/// 已建立出站连接的地址集合（防止重复出站）
static OUTBOUND_PEERS: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

/// 当前已有 TX_OUT → peer 转发任务的 IP 集合
/// 每个对端 IP 最多只有一个"发送者"，后来的连接只收不发
static FORWARDING_IPS: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

type PeerSinks = Arc<Mutex<HashMap<SocketAddr, ()>>>;

pub fn subscribe() -> broadcast::Receiver<String> {
    TX_IN.subscribe()
}

pub fn broadcast_clip(payload: &ClipPayload) {
    if let Ok(json) = serde_json::to_string(payload) {
        let _ = TX_OUT.send(json);
    }
}

pub async fn start() -> Result<u16, String> {
    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("failed to bind WebSocket server: {e}"))?;
    let port = listener.local_addr()
        .map_err(|e| format!("failed to get local address: {e}"))?
        .port();
    info!("WebSocket server listening on port {port}");

    let peers: PeerSinks = Arc::new(Mutex::new(HashMap::new()));
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(peers);

    let listener_clone = listener;
    tokio::spawn(async move {
        if let Err(e) = axum::serve(
            listener_clone,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        {
            error!("WebSocket server error: {e}");
        }
    });
    Ok(port)
}

/// 尝试声明"我来负责向 peer_ip 转发 TX_OUT"
/// 返回 true 表示声明成功（应启动转发任务），false 表示已有其他连接在转发
async fn claim_forward(peer_ip: &str) -> bool {
    let mut set = FORWARDING_IPS.lock().await;
    if set.contains(peer_ip) {
        false
    } else {
        set.insert(peer_ip.to_string());
        true
    }
}

async fn release_forward(peer_ip: &str) {
    FORWARDING_IPS.lock().await.remove(peer_ip);
}

pub async fn connect_to_peer(server_addr: String) {
    {
        let mut out = OUTBOUND_PEERS.lock().await;
        if out.contains(&server_addr) {
            return;
        }
        out.insert(server_addr.clone());
    }

    let addr = server_addr.clone();
    tokio::spawn(async move {
        let url = format!("ws://{}/ws", addr);
        info!("Connecting to peer {url}");
        match tokio_tungstenite::connect_async(&url).await {
            Ok((ws_stream, _)) => {
                info!("Connected to peer {url}");
                handle_outbound(ws_stream, addr.clone()).await;
            }
            Err(e) => {
                error!("Failed to connect to {url}: {e}");
            }
        }
        OUTBOUND_PEERS.lock().await.remove(&addr);
        info!("Outbound connection to {addr} ended");
    });
}

// ── 入站处理 ──────────────────────────────────────────────────

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(_peers): State<PeerSinks>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_inbound(socket, addr))
}

async fn handle_inbound(socket: WebSocket, addr: SocketAddr) {
    info!("Peer connected from {addr}");
    let peer_ip = addr.ip().to_string();
    let (mut sink, mut stream) = socket.split();

    // 尝试成为该 IP 的"发送者"（先到先得）
    let i_am_sender = claim_forward(&peer_ip).await;

    let forward_handle = if i_am_sender {
        info!("Inbound {addr}: I will forward TX_OUT (sender role)");
        let mut rx_out = TX_OUT.subscribe();
        Some(tokio::spawn(async move {
            while let Ok(msg) = rx_out.recv().await {
                if sink.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        }))
    } else {
        info!("Inbound {addr}: receive-only (sender role taken)");
        drop(sink);
        None
    };

    // 接收对端消息 → TX_IN
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(text) => {
                if serde_json::from_str::<Value>(&text).is_ok() {
                    let _ = TX_IN.send(text.to_string());
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    info!("Peer disconnected: {addr}");
    if i_am_sender {
        release_forward(&peer_ip).await;
    }
    if let Some(h) = forward_handle {
        h.abort();
    }
}

// ── 出站处理 ──────────────────────────────────────────────────

type OutboundStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

async fn handle_outbound(ws_stream: OutboundStream, server_addr: String) {
    use tokio_tungstenite::tungstenite::Message as TMsg;

    // 从 server_addr 提取 IP
    let peer_ip = server_addr
        .split(':')
        .next()
        .unwrap_or(&server_addr)
        .to_string();

    let (mut sink, mut stream) = ws_stream.split();

    // 尝试成为该 IP 的"发送者"
    let i_am_sender = claim_forward(&peer_ip).await;

    let forward = if i_am_sender {
        info!("Outbound {server_addr}: I will forward TX_OUT (sender role)");
        let mut rx_out = TX_OUT.subscribe();
        Some(tokio::spawn(async move {
            while let Ok(msg) = rx_out.recv().await {
                if sink.send(TMsg::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        }))
    } else {
        info!("Outbound {server_addr}: receive-only (sender role taken)");
        drop(sink);
        None
    };

    // 接收对端消息 → TX_IN
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            TMsg::Text(text) => {
                if serde_json::from_str::<Value>(&text).is_ok() {
                    let _ = TX_IN.send(text.to_string());
                }
            }
            TMsg::Close(_) => break,
            _ => {}
        }
    }

    if i_am_sender {
        release_forward(&peer_ip).await;
    }
    if let Some(h) = forward {
        h.abort();
    }
}
