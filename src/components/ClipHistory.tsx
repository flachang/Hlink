import { useState } from "react";
import { createPortal } from "react-dom";
import { ClipboardList, FileText, Image, File, Clock, Eye, X, Loader, Download } from "lucide-react";
import { HistoryEntry } from "../types";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  history: HistoryEntry[];
  isMobile?: boolean;
}

function KindIcon({ kind }: { kind: string }) {
  if (kind === "image") return <Image size={14} />;
  if (kind === "file") return <File size={14} />;
  return <FileText size={14} />;
}

function formatTime(ts: number) {
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export default function ClipHistory({ history }: Props) {
  const [previewSrc, setPreviewSrc] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const handleViewImage = async (entry: HistoryEntry) => {
    if (entry.kind !== "image") return;
    if (!entry.has_image) {
      showToast("⚠️ 该图片记录已过期，请重新接收");
      return;
    }

    setLoading(true);
    try {
      // 按需从 Rust 端获取图片数据（JSON 格式）
      const raw = await invoke<string | null>("get_image_data", { timestamp: entry.timestamp });
      if (!raw) {
        showToast("⚠️ 图片数据已从内存中清除，请重新接收");
        return;
      }

      const info = JSON.parse(raw) as { fmt: string; data: string; w: number; h: number };

      if (info.fmt === "png") {
        // PNG / 已编码格式，直接用 <img> 显示
        setPreviewSrc(`data:image/png;base64,${info.data}`);
      } else {
        // RGBA 原始像素 → 用 Canvas 转换为 data URL
        const bytes = Uint8Array.from(atob(info.data), c => c.charCodeAt(0));
        const canvas = document.createElement("canvas");
        canvas.width = info.w;
        canvas.height = info.h;
        const ctx = canvas.getContext("2d")!;
        // 先填白色背景，避免透明区域显示为灰色棋盘格，也能让黑色内容可见
        ctx.fillStyle = "#ffffff";
        ctx.fillRect(0, 0, info.w, info.h);
        const imageData = new ImageData(new Uint8ClampedArray(bytes), info.w, info.h);
        ctx.putImageData(imageData, 0, 0);
        setPreviewSrc(canvas.toDataURL("image/png"));
      }
    } catch (e) {
      console.error("get_image_data failed:", e);
      showToast("❌ 获取图片失败");
    } finally {
      setLoading(false);
    }
  };

  // 将预览图片保存到相册（移动端）或下载（桌面端）
  const handleSaveImage = async () => {
    if (!previewSrc) return;
    setSaving(true);
    try {
      const isAndroid = navigator.userAgent.toLowerCase().includes("android");

      if (isAndroid) {
        // Android：直接触发浏览器下载 → 系统下载管理器 → Downloads 文件夹
        // Android 相册 App 会自动扫描 Downloads 文件夹中的图片
        const a = document.createElement("a");
        a.href = previewSrc;
        a.download = `Hlink_${Date.now()}.png`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        showToast("📥 图片已保存到「下载」文件夹\n在相册 → 下载 中可找到");
      } else {
        // 桌面/iOS：调用 Tauri 命令保存到 Pictures 目录
        try {
          const base64 = previewSrc.replace(/^data:image\/\w+;base64,/, "");
          const savedPath = await invoke<string>("save_image_to_gallery", { base64Data: base64 });
          const fileName = savedPath.split(/[/\\]/).pop() || "已保存";
          showToast(`💾 图片已保存\n${fileName}`);
        } catch (e) {
          // 降级到浏览器下载
          const a = document.createElement("a");
          a.href = previewSrc;
          a.download = `Hlink_${Date.now()}.png`;
          a.click();
          showToast("💾 图片已下载");
        }
      }
    } catch (e) {
      console.error("Save image failed:", e);
      showToast(`❌ 保存失败: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="section">
      <div className="section-header">
        <ClipboardList size={16} />
        <span>同步历史</span>
        <span className="badge">{history.length}</span>
      </div>

      {history.length === 0 ? (
        <div className="empty-state">
          <ClipboardList size={32} className="empty-icon" />
          <p>暂无同步记录</p>
          <p className="hint">复制内容后将自动同步到已连接设备</p>
        </div>
      ) : (
        <ul className="history-list">
          {history.map((entry, i) => {
            if (entry.kind === "image") {
              console.log(`[ClipHistory] image entry[${i}]: has_image=${entry.has_image}, ts=${entry.timestamp}`);
            }
            const isClickable = entry.kind === "image" && !!entry.has_image;
            return (
              <li
                key={i}
                className={`history-item kind-${entry.kind}${isClickable ? " clickable" : ""}`}
                style={isClickable ? { touchAction: "manipulation" } : undefined}
                onClick={isClickable ? () => handleViewImage(entry) : undefined}
              >
                <span className="history-kind-icon">
                  <KindIcon kind={entry.kind} />
                </span>
                <div className="history-content">
                  <span className="history-preview">{entry.preview}</span>
                  <span className="history-meta">
                    <span className="history-from">{entry.from.slice(0, 8)}…</span>
                    <span className="history-time">
                      <Clock size={10} />
                      {formatTime(entry.timestamp)}
                    </span>
                    {entry.kind === "image" && entry.has_image && (
                      <span style={{ fontSize: "10px", color: "var(--accent)", marginLeft: "8px" }}>
                        <Eye size={10} style={{ display: "inline", marginRight: "2px" }} />
                        点击放大
                      </span>
                    )}
                  </span>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {/* 本地 toast */}
      {toast && (
        <div style={{
          position: "fixed",
          bottom: "80px",
          left: "50%",
          transform: "translateX(-50%)",
          background: "rgba(0,0,0,0.8)",
          color: "#fff",
          borderRadius: "8px",
          padding: "8px 16px",
          fontSize: "13px",
          zIndex: 2000,
          pointerEvents: "none",
          textAlign: "center",
          maxWidth: "80vw",
        }}>
          {toast}
        </div>
      )}

      {/* 加载遮罩 */}
      {loading && createPortal(
        <div style={{
          position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
          zIndex: 9998, display: "flex", alignItems: "center", justifyContent: "center",
        }}>
          <Loader size={40} color="#fff" style={{ animation: "spin 1s linear infinite" }} />
        </div>,
        document.body
      )}

      {/* 图片全屏预览（Portal 挂到 body，避免父级 stacking context 问题）*/}
      {previewSrc && createPortal(
        <div
          style={{
            position: "fixed", inset: 0,
            background: "rgba(0,0,0,0.92)",
            zIndex: 9999,
            display: "flex", alignItems: "center", justifyContent: "center",
            paddingTop: "env(safe-area-inset-top, 0)",
          }}
          onClick={() => setPreviewSrc(null)}
        >
          {/* 关闭按钮 */}
          <div
            style={{
              position: "absolute",
              top: "max(16px, env(safe-area-inset-top, 16px))",
              right: "16px",
              width: "40px", height: "40px",
              borderRadius: "50%",
              background: "rgba(255,255,255,0.2)",
              display: "flex", alignItems: "center", justifyContent: "center",
              color: "#fff", cursor: "pointer", zIndex: 10000,
            }}
            onClick={(e) => { e.stopPropagation(); setPreviewSrc(null); }}
          >
            <X size={22} />
          </div>

          <img
            src={previewSrc}
            alt="图片预览"
            style={{
              maxWidth: "calc(100% - 32px)",
              maxHeight: "75vh",
              borderRadius: "8px",
              objectFit: "contain",
              background: "#ffffff",
              boxShadow: "0 0 0 1px rgba(255,255,255,0.1)",
            }}
            onClick={(e) => e.stopPropagation()}
          />

          {/* 底部保存按钮 */}
          <div
            style={{
              position: "absolute",
              bottom: "max(32px, env(safe-area-inset-bottom, 32px))",
              left: "50%",
              transform: "translateX(-50%)",
              zIndex: 10000,
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              style={{
                display: "flex", alignItems: "center", gap: "8px",
                background: saving ? "rgba(255,255,255,0.15)" : "rgba(255,255,255,0.25)",
                border: "1px solid rgba(255,255,255,0.4)",
                color: "#fff",
                borderRadius: "24px",
                padding: "10px 24px",
                fontSize: "15px",
                cursor: saving ? "not-allowed" : "pointer",
                touchAction: "manipulation",
                WebkitTapHighlightColor: "transparent",
              }}
              onClick={handleSaveImage}
              disabled={saving}
            >
              {saving
                ? <Loader size={18} style={{ animation: "spin 1s linear infinite" }} />
                : <Download size={18} />
              }
              {saving ? "保存中…" : "保存到相册"}
            </button>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}
