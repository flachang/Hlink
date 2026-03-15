use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// UDP 广播端口（所有设备监听同一端口）
const DISCOVERY_PORT: u16 = 45678;
/// 广播间隔
const BROADCAST_INTERVAL: Duration = Duration::from_secs(3);
/// 设备过期时间（超过此时间未收到心跳则移除）
const PEER_TTL: Duration = Duration::from_secs(12);

/// 发现到的对等设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDevice {
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub port: u16,
}

/// 内部条目，附带最后收到心跳的时间
struct PeerEntry {
    device: PeerDevice,
    last_seen: Instant,
}

/// 全局已发现设备表 { id -> PeerEntry }
static PEERS: Lazy<Arc<Mutex<HashMap<String, PeerEntry>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// 获取当前活跃的设备列表（供前端查询）
pub fn get_peers() -> Vec<PeerDevice> {
    let now = Instant::now();
    let mut map = PEERS.lock().unwrap();

    // 顺便清理过期条目
    map.retain(|_, v| now.duration_since(v.last_seen) < PEER_TTL);

    map.values().map(|e| e.device.clone()).collect()
}

// ─── UDP 广播发现包 ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct Beacon {
    id: String,
    name: String,
    port: u16,
}

/// 启动 UDP 广播发现（桌面 + 移动端通用）
///
/// * `device_id`   本机设备 ID
/// * `device_name` 本机显示名称
/// * `ws_port`     本机 WebSocket 监听端口
pub fn start(device_id: String, device_name: String, ws_port: u16) {
    let id_recv = device_id.clone();

    // ── 接收端：监听 UDP 广播 ──────────────────────────────
    std::thread::spawn(move || {
        // 使用 socket2 设置 SO_REUSEADDR/SO_REUSEPORT（仅桌面端），
        // 允许新旧进程过渡期共用同一端口
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let sock: UdpSocket = {
            use socket2::{Domain, Protocol, Socket, Type};
            let s = match Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
                Ok(s) => s,
                Err(e) => { error!("socket2::new failed: {e}"); return; }
            };
            s.set_reuse_address(true).ok();
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            s.set_reuse_port(true).ok();
            let addr: SocketAddr = format!("0.0.0.0:{DISCOVERY_PORT}").parse().unwrap();
            if let Err(e) = s.bind(&addr.into()) {
                error!("UDP bind failed: {e}");
                return;
            }
            s.into()
        };

        #[cfg(any(target_os = "android", target_os = "ios"))]
        let sock: UdpSocket = match UdpSocket::bind(format!("0.0.0.0:{DISCOVERY_PORT}")) {
            Ok(s) => s,
            Err(e) => { error!("UDP bind failed: {e}"); return; }
        };

        sock.set_read_timeout(Some(Duration::from_secs(2))).ok();
        info!("UDP discovery listening on port {DISCOVERY_PORT}");

        let mut buf = [0u8; 1024];
        loop {
            match sock.recv_from(&mut buf) {
                Ok((len, src_addr)) => {
                    let Ok(beacon) = serde_json::from_slice::<Beacon>(&buf[..len]) else {
                        continue;
                    };

                    // 跳过自己
                    if beacon.id == id_recv {
                        continue;
                    }

                    let src_ip = match src_addr.ip() {
                        IpAddr::V4(v4) => v4.to_string(),
                        IpAddr::V6(v6) => v6.to_string(),
                    };

                    let peer = PeerDevice {
                        id: beacon.id.clone(),
                        name: beacon.name,
                        addresses: vec![src_ip],
                        port: beacon.port,
                    };

                    info!("Discovered peer: {} ({}:{})", peer.id, peer.addresses[0], peer.port);

                    let is_new = {
                        let mut map = PEERS.lock().unwrap();
                        let is_new = !map.contains_key(&beacon.id);
                        map.insert(
                            beacon.id,
                            PeerEntry {
                                device: peer.clone(),
                                last_seen: Instant::now(),
                            },
                        );
                        is_new
                    };

                    // 发现新设备：触发 WebSocket 自动连接
                    // connect_to_peer 内部会检查是否已连接，防止重复
                    if is_new {
                        let addr = format!("{}:{}", peer.addresses[0], peer.port);
                        tauri::async_runtime::spawn(async move {
                            crate::server::connect_to_peer(addr).await;
                        });
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // 超时，继续循环
                }
                Err(e) => {
                    warn!("UDP recv error: {e}");
                }
            }
        }
    });

    // ── 发送端：每 3 秒广播一次心跳 ───────────────────────
    std::thread::spawn(move || {
        // 稍等片刻确保接收端已就绪
        std::thread::sleep(Duration::from_millis(500));

        let sock = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                error!("UDP sender bind failed: {e}");
                return;
            }
        };
        sock.set_broadcast(true).ok();

        let beacon = Beacon {
            id: device_id,
            name: device_name,
            port: ws_port,
        };
        let payload = serde_json::to_vec(&beacon).unwrap();
        let broadcast_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)),
            DISCOVERY_PORT,
        );

        loop {
            if let Err(e) = sock.send_to(&payload, broadcast_addr) {
                warn!("UDP broadcast failed: {e}");
            }
            std::thread::sleep(BROADCAST_INTERVAL);
        }
    });
}
