use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::{error, info};

use crate::clipboard::{self, ClipContent};
use crate::server;

/// 最后一次"由远端写入本地剪贴板"的内容，用于防止轮询把它重新广播出去
#[cfg(not(any(target_os = "android", target_os = "ios")))]
static LAST_WRITTEN: Lazy<Arc<Mutex<Option<ClipContent>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// 网络传输的剪贴板消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPayload {
    #[serde(rename = "type")]
    pub kind: ClipKind,
    pub from: String,
    pub payload: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    File,
}

/// 历史记录条目（不含图片数据，图片数据单独存储）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub kind: ClipKind,
    pub from: String,
    pub preview: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// 是否有图片数据可供预览（true = 可点击放大）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_image: Option<bool>,
}

static HISTORY: Lazy<Arc<Mutex<Vec<HistoryEntry>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

/// 独立存储图片 base64 数据，key = timestamp
/// 避免每次轮询 get_clip_history 时通过 IPC 传输大数据
static IMAGE_STORE: Lazy<Arc<Mutex<HashMap<u64, String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

static SYNC_ENABLED: Lazy<Arc<Mutex<bool>>> = Lazy::new(|| Arc::new(Mutex::new(true)));

/// 按时间戳获取图片 base64 数据（供前端点击时按需获取）
pub fn get_image_data(timestamp: u64) -> Option<String> {
    let store = IMAGE_STORE.lock().unwrap();
    let result = store.get(&timestamp).cloned();
    info!("get_image_data: timestamp={}, found={}, store_size={}", timestamp, result.is_some(), store.len());
    result
}

/// 更新历史记录中最新的一条图片记录，添加文件路径
pub fn update_latest_image_history(file_path: &str) {
    let mut history = HISTORY.lock().unwrap();
    for entry in history.iter_mut() {
        if entry.kind == ClipKind::Image && entry.file_path.is_none() {
            entry.file_path = Some(file_path.to_string());
            info!("Updated history entry with file path: {}", file_path);
            break;
        }
    }
}

pub fn get_history() -> Vec<HistoryEntry> {
    HISTORY.lock().unwrap().clone()
}

pub fn set_sync_enabled(enabled: bool) {
    *SYNC_ENABLED.lock().unwrap() = enabled;
}

/// 供 lib.rs broadcast_text 命令调用
pub fn push_history_pub(payload: &ClipPayload) {
    push_history(payload);
}

/// 启动剪贴板轮询 + 接收循环
pub fn start_poll(device_id: String, app: tauri::AppHandle) {
    // ── 接收远端消息 ─────────────────────────────────────────
    let recv_id = device_id.clone();
    let app_recv = app.clone();
    tokio::spawn(async move {
        let mut rx = server::subscribe();
        loop {
            match rx.recv().await {
                Ok(json) => {
                    if let Ok(payload) = serde_json::from_str::<ClipPayload>(&json) {
                        if payload.from == recv_id {
                            continue; // 跳过自身消息
                        }

                        info!("Received payload: kind={:?}, payload_len={}, from={}", payload.kind, payload.payload.len(), payload.from);
                        push_history(&payload);

                        // 桌面端：Rust 直接写剪贴板
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        apply_desktop_clip(&payload);

                        // 所有平台：emit 事件给前端
                        if let Err(e) = app_recv.emit("clip-received", &payload) {
                            error!("emit clip-received failed: {e}");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Broadcast lagged by {n} messages");
                }
                Err(_) => break,
            }
        }
    });

    // ── 桌面端：Rust 轮询本地剪贴板 ──────────────────────────
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let id = device_id.clone();
        tokio::spawn(async move {
            let mut last: Option<ClipContent> = Some(clipboard::read());
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;

                if !*SYNC_ENABLED.lock().unwrap() {
                    continue;
                }

                let current = clipboard::read();
                if current == ClipContent::Empty {
                    continue;
                }

                let changed = match &last {
                    None => true,
                    Some(prev) => prev != &current,
                };

                if changed {
                    last = Some(current.clone());

                    // 若该内容是由远端写入的（apply_desktop_clip），跳过广播和历史记录
                    {
                        let mut lw = LAST_WRITTEN.lock().unwrap();
                        if lw.as_ref() == Some(&current) {
                            info!("Desktop poll: skipping remote-written content to avoid echo");
                            *lw = None; // 消费掉，下次轮询不再跳过
                            continue;
                        }
                    }

                    if let Some(payload) = clip_to_payload(&id, current) {
                        push_history(&payload);
                        server::broadcast_clip(&payload);
                        info!("Desktop clipboard changed, broadcasted");
                    }
                }
            }
        });
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    info!("Mobile: clipboard polling handled in frontend JS");
}

// ── 内部工具 ─────────────────────────────────────────────────

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn apply_desktop_clip(payload: &ClipPayload) {
    match payload.kind {
        ClipKind::Text => {
            let content = ClipContent::Text(payload.payload.clone());
            clipboard::write_text(&payload.payload);
            // 记录写入的内容，防止轮询把它当作新内容重新广播
            *LAST_WRITTEN.lock().unwrap() = Some(content);
        }
        ClipKind::Image => {
            let (Some(w), Some(h)) = (payload.width, payload.height) else {
                return;
            };
            match B64.decode(&payload.payload) {
                Ok(bytes) => {
                    let content = ClipContent::Image { width: w, height: h, bytes: bytes.clone() };
                    clipboard::write_image(w, h, bytes);
                    *LAST_WRITTEN.lock().unwrap() = Some(content);
                }
                Err(e) => error!("Image base64 decode failed: {e}"),
            }
        }
        ClipKind::File => {
            let content = ClipContent::Text(payload.payload.clone());
            clipboard::write_text(&payload.payload);
            *LAST_WRITTEN.lock().unwrap() = Some(content);
        }
    }
}

fn clip_to_payload(device_id: &str, content: ClipContent) -> Option<ClipPayload> {
    match content {
        ClipContent::Text(text) => Some(ClipPayload {
            kind: ClipKind::Text,
            from: device_id.to_string(),
            payload: text,
            width: None,
            height: None,
            filename: None,
        }),
        ClipContent::Image { width, height, bytes } => Some(ClipPayload {
            kind: ClipKind::Image,
            from: device_id.to_string(),
            payload: B64.encode(&bytes),
            width: Some(width),
            height: Some(height),
            filename: None,
        }),
        ClipContent::Empty => None,
    }
}

fn push_history(payload: &ClipPayload) {
    push_history_with_path(payload, None);
}

/// 添加历史记录，图片数据单独存入 IMAGE_STORE
pub fn push_history_with_path(payload: &ClipPayload, file_path: Option<String>) {
    let preview = match payload.kind {
        ClipKind::Text => {
            let s = &payload.payload;
            if s.len() > 80 { format!("{}…", &s[..80]) } else { s.clone() }
        }
        ClipKind::Image => "[图片]".to_string(),
        ClipKind::File => format!(
            "[文件: {}]",
            payload.filename.as_deref().unwrap_or("unknown")
        ),
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // ── 去重：5 秒内相同 kind + payload 只记录一次 ──────────────
    {
        let history = HISTORY.lock().unwrap();
        let dedup_window = 5u64; // 秒
        for entry in history.iter().take(10) {
            if entry.kind == payload.kind
                && timestamp.saturating_sub(entry.timestamp) <= dedup_window
                && entry.preview == preview
                // 图片用 preview 去重（"[图片]" 相同），文本用 preview 去重
            {
                info!("push_history: duplicate within {}s, skipping ({:?})", dedup_window, payload.kind);
                return;
            }
        }
    }

    // 图片数据单独存储，不放入 HistoryEntry（避免 IPC 大数据传输问题）
    // 存储格式：JSON { "fmt": "png"|"rgba", "data": base64, "w": width, "h": height }
    let has_image = if payload.kind == ClipKind::Image {
        info!("push_history: Image kind, payload.len={}, width={:?}, height={:?}",
            payload.payload.len(), payload.width, payload.height);
        if payload.payload.is_empty() {
            info!("push_history: payload is EMPTY, skip IMAGE_STORE");
            None
        } else {
            // 判断数据格式：有 width/height 的是 RGBA 原始像素（arboard 读取/发送的格式）
            // 没有 width/height 的是 PNG 格式（暂不支持，保留扩展）
            let fmt = if payload.width.is_some() && payload.height.is_some() {
                "rgba"
            } else {
                "png"
            };
            let store_val = format!(
                r#"{{"fmt":"{}","data":"{}","w":{},"h":{}}}"#,
                fmt,
                payload.payload,
                payload.width.unwrap_or(0),
                payload.height.unwrap_or(0),
            );
            info!("Storing image in IMAGE_STORE: timestamp={}, fmt={}, len={}KB",
                timestamp, fmt, payload.payload.len() / 1024);
            let mut store = IMAGE_STORE.lock().unwrap();
            store.insert(timestamp, store_val);
            // 只保留最近 20 张图片
            if store.len() > 20 {
                let oldest = store.keys().copied().min();
                if let Some(k) = oldest { store.remove(&k); }
            }
            info!("IMAGE_STORE now has {} entries", store.len());
            Some(true)
        }
    } else {
        None
    };

    let entry = HistoryEntry {
        kind: payload.kind.clone(),
        from: payload.from.clone(),
        preview,
        timestamp,
        file_path,
        has_image,
    };

    let mut history = HISTORY.lock().unwrap();
    history.insert(0, entry);
    history.truncate(50);
}
