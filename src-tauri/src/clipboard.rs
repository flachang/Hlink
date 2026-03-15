use base64::{engine::general_purpose::STANDARD as B64, Engine};
use tracing::error;

/// 剪贴板内容快照（用于变更检测和网络传输）
#[derive(Debug, Clone, PartialEq)]
pub enum ClipContent {
    Text(String),
    Image {
        width: usize,
        height: usize,
        /// RGBA 原始数据
        bytes: Vec<u8>,
    },
    Empty,
}

impl ClipContent {
    /// 将图片数据编码为 base64 字符串（用于 JSON 传输）
    pub fn image_to_base64(bytes: &[u8]) -> String {
        B64.encode(bytes)
    }

    /// 从 base64 字符串解码图片数据
    pub fn image_from_base64(s: &str) -> Option<Vec<u8>> {
        B64.decode(s).ok()
    }
}

// ── Desktop（arboard）────────────────────────────────────────

#[cfg(not(target_os = "android"))]
#[cfg(not(target_os = "ios"))]
mod desktop {
    use super::ClipContent;
    use arboard::Clipboard;
    use tracing::error;

    pub fn read() -> ClipContent {
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                error!("Clipboard open failed: {e}");
                return ClipContent::Empty;
            }
        };

        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                return ClipContent::Text(text);
            }
        }

        if let Ok(img) = cb.get_image() {
            return ClipContent::Image {
                width: img.width,
                height: img.height,
                bytes: img.bytes.into_owned(),
            };
        }

        ClipContent::Empty
    }

    pub fn write_text(text: &str) {
        match Clipboard::new() {
            Ok(mut cb) => {
                if let Err(e) = cb.set_text(text) {
                    error!("Clipboard write text failed: {e}");
                }
            }
            Err(e) => error!("Clipboard open failed: {e}"),
        }
    }

    pub fn write_image(width: usize, height: usize, bytes: Vec<u8>) {
        use arboard::ImageData;
        use std::borrow::Cow;

        match Clipboard::new() {
            Ok(mut cb) => {
                let img = ImageData {
                    width,
                    height,
                    bytes: Cow::Owned(bytes),
                };
                if let Err(e) = cb.set_image(img) {
                    error!("Clipboard write image failed: {e}");
                }
            }
            Err(e) => error!("Clipboard open failed: {e}"),
        }
    }
}

// ── Mobile（stub — 通过 Tauri 插件在 JS 层读写）──────────────

#[cfg(any(target_os = "android", target_os = "ios"))]
mod desktop {
    use super::ClipContent;

    /// 移动端剪贴板在 JS 层由 tauri-plugin-clipboard-manager 处理，
    /// Rust 侧暂返回 Empty，防止编译报错。
    pub fn read() -> ClipContent {
        ClipContent::Empty
    }
    pub fn write_text(_text: &str) {}
    pub fn write_image(_w: usize, _h: usize, _b: Vec<u8>) {}
}

// ── 公共导出 ─────────────────────────────────────────────────

pub fn read() -> ClipContent {
    desktop::read()
}

pub fn write_text(text: &str) {
    desktop::write_text(text);
}

pub fn write_image(width: usize, height: usize, bytes: Vec<u8>) {
    desktop::write_image(width, height, bytes);
}
