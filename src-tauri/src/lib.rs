mod clipboard;
mod discovery;
mod server;
mod sync;

use discovery::PeerDevice;
use once_cell::sync::OnceCell;
use sync::{get_history, set_sync_enabled, ClipKind, ClipPayload, HistoryEntry};
use tauri::Manager;
use tracing::{error, info};
use uuid::Uuid;

static LOCAL_PORT: OnceCell<u16> = OnceCell::new();

// ── Tauri commands ────────────────────────────────────────────

#[tauri::command]
fn get_devices() -> Vec<PeerDevice> {
    discovery::get_peers()
}

#[tauri::command]
fn get_clip_history() -> Vec<HistoryEntry> {
    get_history()
}

#[tauri::command]
fn toggle_sync(enabled: bool) {
    set_sync_enabled(enabled);
    info!("Sync enabled: {enabled}");
}

#[tauri::command]
async fn connect_peer(addr: String) {
    server::connect_to_peer(addr).await;
}

#[tauri::command]
fn get_local_info() -> serde_json::Value {
    let port = LOCAL_PORT.get().copied().unwrap_or(0);
    let ip = local_ip();
    serde_json::json!({ "ip": ip, "port": port })
}

/// 移动端前端发现剪贴板变化后，调此命令把内容广播给所有桌面端 peer
#[tauri::command]
fn broadcast_text(text: String, from: String) {
    let payload = ClipPayload {
        kind: ClipKind::Text,
        from,
        payload: text,
        width: None,
        height: None,
        filename: None,
    };
    sync::push_history_pub(&payload);
    server::broadcast_clip(&payload);
}

/// 告知前端当前是否运行在移动端（用于决定是否启用 JS 剪贴板轮询）
#[tauri::command]
fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

/// 按时间戳获取图片 base64 数据（供前端点击历史记录时按需获取）
#[tauri::command]
fn get_image_data(timestamp: u64) -> Option<String> {
    sync::get_image_data(timestamp)
}

/// 获取应用数据目录路径（用于查找保存的图片）
#[tauri::command]
fn get_app_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    let app_data_dir = app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    Ok(app_data_dir.to_string_lossy().to_string())
}

/// 检查文件是否存在
#[tauri::command]
fn file_exists(file_path: String) -> bool {
    std::path::Path::new(&file_path).exists()
}

/// 列出应用数据目录中的所有图片文件
#[tauri::command]
fn list_saved_images(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let app_data_dir = app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    
    let mut images = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&app_data_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "png" || ext == "jpg" || ext == "jpeg" {
                            if let Some(name) = path.file_name() {
                                images.push(name.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    images.sort();
    images.reverse(); // 最新的在前
    Ok(images)
}

/// 移动端写入图片到剪贴板（通过 Rust 后端，如果插件不支持）
#[tauri::command]
fn write_image_mobile(base64_data: String, width: usize, height: usize) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // 桌面端：直接使用 clipboard 模块
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        match B64.decode(&base64_data) {
            Ok(bytes) => {
                clipboard::write_image(width, height, bytes);
                Ok(())
            }
            Err(e) => Err(format!("Base64 decode failed: {e}")),
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // 移动端：目前不支持，返回错误提示使用 Web API
        Err("Mobile image write not supported via Rust backend. Use Web Clipboard API.".to_string())
    }
}

/// 打开文件（用于查看图片）- 桌面端使用，移动端由前端直接调用 opener 插件
#[tauri::command]
async fn open_file(file_path: String) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // 桌面端：使用系统默认程序打开
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &file_path])
                .spawn()
                .map_err(|e| format!("Failed to open file: {e}"))?;
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&file_path)
                .spawn()
                .map_err(|e| format!("Failed to open file: {e}"))?;
        }
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&file_path)
                .spawn()
                .map_err(|e| format!("Failed to open file: {e}"))?;
        }
        Ok(())
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // 移动端：由前端使用 opener 插件打开，这里不做处理
        Ok(())
    }
}

/// 保存图片到相册（移动端）
#[tauri::command]
async fn save_image_to_gallery(
    base64_data: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use std::io::Write;

    // 解码 base64 数据
    let bytes = B64.decode(&base64_data)
        .map_err(|e| format!("Base64 decode failed: {e}"))?;

    // 生成文件名（使用时间戳）
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let filename = format!("Hlink_{}.png", timestamp);

    // Android / iOS / 桌面 均写到 app 缓存目录（有写权限）
    // Android 端前端会用 <a download> 触发系统下载管理器保存到 Downloads
    // 桌面端写到 Pictures/Hlink
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let pictures_dir = app.path()
            .picture_dir()
            .map_err(|e| format!("Failed to get picture dir: {e}"))?;
        let hlink_dir = pictures_dir.join("Hlink");
        std::fs::create_dir_all(&hlink_dir)
            .map_err(|e| format!("Failed to create Hlink dir: {e}"))?;
        let file_path = hlink_dir.join(&filename);
        let mut file = std::fs::File::create(&file_path)
            .map_err(|e| format!("Failed to create file: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed to write file: {e}"))?;
        let path_str = file_path.to_string_lossy().to_string();
        info!("Image saved to Pictures: {}", path_str);
        sync::update_latest_image_history(&path_str);
        return Ok(path_str);
    }

    // Android / iOS: 写到 app 缓存目录，供前端读取后触发下载
    #[allow(unreachable_code)]
    {
        let cache_dir = app.path()
            .app_cache_dir()
            .map_err(|e| format!("Failed to get cache dir: {e}"))?;
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache dir: {e}"))?;
        let file_path = cache_dir.join(&filename);
        let mut file = std::fs::File::create(&file_path)
            .map_err(|e| format!("Failed to create file: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed to write file: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Failed to sync file: {e}"))?;
        let path_str = file_path.to_string_lossy().to_string();
        info!("Image saved to cache: {}", path_str);
        sync::update_latest_image_history(&path_str);
        Ok(path_str)
    }
}

/// 保存图片到文件系统（移动端降级方案 - 保留用于兼容）
#[tauri::command]
async fn save_image_to_file(
    base64_data: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    // 移动端优先保存到相册
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        save_image_to_gallery(base64_data, app).await
    }
    
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        save_image_to_gallery(base64_data, app).await
    }
}

// ── App 入口 ─────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hlink=info".into()),
        )
        .init();

    let device_id = Uuid::new_v4().to_string();
    let hostname = hostname_or_default();

    info!("Starting Hlink  device_id={device_id}  name={hostname}");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            let id_clone = device_id.clone();
            let name_clone = hostname.clone();
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                // 启动 WebSocket 服务器（失败时记录错误但继续运行）
                match server::start().await {
                    Ok(port) => {
                        LOCAL_PORT.set(port).ok();
                        info!("Backend services started successfully on port {port}");
                        discovery::start(id_clone.clone(), name_clone, port);
                    }
                    Err(e) => {
                        error!("Failed to start WebSocket server: {e}");
                        error!("App will continue but sync features may be unavailable");
                        // 即使服务启动失败，也设置端口为 0，表示服务未启动
                        LOCAL_PORT.set(0).ok();
                    }
                }
                // 即使服务启动失败，也启动接收循环（用于处理手动连接）
                sync::start_poll(id_clone, app_handle);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_devices,
            get_clip_history,
            toggle_sync,
            connect_peer,
            get_local_info,
            broadcast_text,
            is_mobile,
            write_image_mobile,
            save_image_to_file,
            save_image_to_gallery,
            open_file,
            get_app_data_dir,
            file_exists,
            list_saved_images,
            get_image_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn local_ip() -> String {
    use std::net::UdpSocket;
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn hostname_or_default() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| {
            std::fs::read_to_string("/etc/hostname")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "Hlink-Device".to_string())
        })
}
