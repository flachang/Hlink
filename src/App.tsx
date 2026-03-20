import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Link2, ToggleLeft, ToggleRight, RefreshCw, PlusCircle, X } from "lucide-react";
import DeviceList from "./components/DeviceList";
import ClipHistory from "./components/ClipHistory";
import QrConnect from "./components/QrConnect";
import { PeerDevice, HistoryEntry } from "./types";
import "./App.css";

import { ClipPayload } from "./types";

interface LocalInfo { ip: string; port: number; }

export default function App() {
  const [devices, setDevices]       = useState<PeerDevice[]>([]);
  const [history, setHistory]       = useState<HistoryEntry[]>([]);
  const [syncEnabled, setSyncEnabled] = useState(true);
  const [loading, setLoading]       = useState(false);
  const [toast, setToast]           = useState<string | null>(null);
  const [localInfo, setLocalInfo]   = useState<LocalInfo | null>(null);
  const [showManual, setShowManual] = useState(false);
  const [showQr, setShowQr]         = useState(false);
  const [manualAddr, setManualAddr] = useState("");
  const [isMobile, setIsMobile]     = useState(false);
  const deviceId = useRef<string>(crypto.randomUUID());
  const lastClip = useRef<string>("");

  // ── 初始化：检测平台 ──────────────────────────────────────
  useEffect(() => {
    invoke<boolean>("is_mobile").then(setIsMobile).catch(() => setIsMobile(false));
  }, []);

  // ── 定时刷新设备列表和历史 ────────────────────────────────
  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [devs, hist, info] = await Promise.all([
        invoke<PeerDevice[]>("get_devices").catch(() => [] as PeerDevice[]),
        invoke<HistoryEntry[]>("get_clip_history").catch(() => [] as HistoryEntry[]),
        invoke<LocalInfo>("get_local_info").catch(() => null),
      ]);
      setDevices(devs);
      setHistory(hist);
      if (info) setLocalInfo(info);
    } catch (e) {
      console.error("Refresh failed:", e);
      // 即使出错也设置空数组，避免 UI 显示错误
      setDevices([]);
      setHistory([]);
    }
    finally { setLoading(false); }
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 2000);
    return () => clearInterval(id);
  }, [refresh]);

  // ── 移动端：监听 clip-received 事件，写入手机剪贴板 ───────
  useEffect(() => {
    if (!isMobile) return;
    const unlisten = listen<ClipPayload>("clip-received", async (event) => {
      const p = event.payload;
      try {
        if (p.type === "text" && p.payload) {
          await writeText(p.payload);
          showToast("📋 文本已同步到剪贴板");
        } else if (p.type === "image" && p.payload && p.width && p.height) {
          console.log("Received image:", { width: p.width, height: p.height, payloadLength: p.payload.length });
          
          // 移动端图片写入：优先使用 Web Clipboard API，降级到保存到相册
          try {
            // 方法 1：尝试使用 Web Clipboard API（现代浏览器/WebView 支持）
            if (navigator.clipboard && navigator.clipboard.write) {
              try {
                // 解码 base64 数据
                const binaryString = atob(p.payload);
                const bytes = new Uint8Array(binaryString.length);
                for (let i = 0; i < binaryString.length; i++) {
                  bytes[i] = binaryString.charCodeAt(i);
                }
                
                // 创建 Blob（假设是 PNG 格式）
                const blob = new Blob([bytes], { type: "image/png" });
                const item = new ClipboardItem({ "image/png": blob });
                await navigator.clipboard.write([item]);
                showToast("🖼️ 图片已同步到剪贴板");
                console.log("Image written to clipboard via Web API");
                return; // 成功，退出
              } catch (webError) {
                console.warn("Web Clipboard API failed, trying fallback:", webError);
              }
            }
            
            // 方法 2：降级到 Tauri 命令（桌面端支持）
            try {
              await invoke("write_image_mobile", {
                base64Data: p.payload,
                width: p.width,
                height: p.height,
              });
              showToast("🖼️ 图片已同步到剪贴板");
              console.log("Image written to clipboard via Tauri command");
              return; // 成功，退出
            } catch (tauriError) {
              console.warn("Tauri image write failed:", tauriError);
              // 继续尝试保存到相册
            }
            
            // 方法 3：保存到相册（移动端）或文件系统
            try {
              console.log("Attempting to save image to gallery...");
              const savedPath = await invoke<string>("save_image_to_gallery", {
                base64Data: p.payload,
              });
              const fileName = savedPath.split(/[/\\]/).pop() || "已保存";
              
              // 检查文件路径，判断是否保存到相册目录
              const isGalleryPath = savedPath.includes("/DCIM/") || savedPath.includes("/Pictures/");
              const message = isGalleryPath 
                ? `📷 图片已保存到相册\n${fileName}\n（如未显示，请刷新相册）`
                : `💾 图片已保存\n${fileName}`;
              
              showToast(message);
              console.log("Image saved to gallery:", savedPath);
              
              // 保存后自动打开图片（移动端）
              if (isMobile) {
                try {
                  // 动态导入 opener 插件
                  const { openPath } = await import("@tauri-apps/plugin-opener");
                  await openPath(savedPath);
                } catch (openError) {
                  console.warn("Failed to open image:", openError);
                  // 如果打开失败，不显示错误，因为图片已经保存了
                }
              }
            } catch (saveError) {
              console.error("Save image to gallery failed:", saveError);
              // 方法 4：最终降级 - 提供下载链接
              try {
                // 创建下载链接
                const binaryString = atob(p.payload);
                const bytes = new Uint8Array(binaryString.length);
                for (let i = 0; i < binaryString.length; i++) {
                  bytes[i] = binaryString.charCodeAt(i);
                }
                const blob = new Blob([bytes], { type: "image/png" });
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a");
                a.href = url;
                a.download = `hlink_image_${Date.now()}.png`;
                document.body.appendChild(a);
                a.click();
                document.body.removeChild(a);
                URL.revokeObjectURL(url);
                showToast("💾 图片已下载");
                console.log("Image downloaded as fallback");
              } catch (downloadError) {
                console.error("Download failed:", downloadError);
                showToast("⚠️ 图片已接收，但无法自动保存");
              }
            }
          } catch (e) {
            console.error("Image processing failed:", e);
            showToast("❌ 图片同步失败");
          }
        }
      } catch (e) {
        console.error("Clipboard write failed", e);
        showToast("❌ 同步失败");
      }
    });
    return () => { unlisten.then(f => f()); };
  }, [isMobile]);

  // ── 移动端：轮询手机剪贴板，有变化就广播给桌面 ────────────
  useEffect(() => {
    if (!isMobile) return;
    const id = setInterval(async () => {
      if (!syncEnabled) return;
      try {
        const text = await readText();
        if (text && text !== lastClip.current) {
          lastClip.current = text;
          await invoke("broadcast_text", {
            text,
            from: deviceId.current,
          });
        }
      } catch (_) { /* 读取失败时忽略 */ }
    }, 500);
    return () => clearInterval(id);
  }, [isMobile, syncEnabled]);

  // ── 操作 ─────────────────────────────────────────────────
  const handleToggleSync = async () => {
    const next = !syncEnabled;
    setSyncEnabled(next);
    await invoke("toggle_sync", { enabled: next });
    showToast(next ? "同步已开启" : "同步已暂停");
  };

  const handleConnect = async (addr: string) => {
    try {
      await invoke("connect_peer", { addr });
      showToast(`已连接到 ${addr}`);
    } catch (e) { showToast(`连接失败: ${e}`); }
  };

  const handleManualConnect = async () => {
    if (!manualAddr.trim()) return;
    await handleConnect(manualAddr.trim());
    setManualAddr("");
    setShowManual(false);
  };

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2500);
  };

  return (
    <div className="app">
      {/* ── 顶栏 ── */}
      <header className="topbar">
        <div className="logo">
          <Link2 size={20} />
          <span>Hlink</span>
        </div>
        <div className="topbar-actions">
          {/* {localInfo && localInfo.port > 0 && (
            <button className="btn-icon" onClick={() => setShowQr(true)} title="显示二维码让手机扫码连接">
              <QrCode size={16} />
            </button>
          )} */}
          <button className="btn-icon" onClick={() => setShowManual(v => !v)} title="手动输入 IP 连接">
            <PlusCircle size={16} />
          </button>
          <button className={`btn-icon ${loading ? "spinning" : ""}`} onClick={refresh} title="刷新">
            <RefreshCw size={16} />
          </button>
          <button className={`btn-toggle ${syncEnabled ? "on" : "off"}`} onClick={handleToggleSync}>
            {syncEnabled ? <ToggleRight size={24} /> : <ToggleLeft size={24} />}
            <span>{syncEnabled ? "同步中" : "已暂停"}</span>
          </button>
        </div>
      </header>

      {/* ── 手动连接面板 ── */}
      {showManual && (
        <div className="manual-panel">
          <span className="manual-label">输入对端 IP:端口</span>
          {localInfo && (
            <span className="manual-hint">本机地址：{localInfo.ip}:{localInfo.port}</span>
          )}
          <div className="manual-row">
            <input
              className="manual-input"
              placeholder="192.168.1.x:端口"
              value={manualAddr}
              onChange={e => setManualAddr(e.target.value)}
              onKeyDown={e => e.key === "Enter" && handleManualConnect()}
              autoFocus
            />
            <button className="btn-connect" onClick={handleManualConnect}>连接</button>
            <button className="btn-icon" onClick={() => setShowManual(false)}><X size={14} /></button>
          </div>
        </div>
      )}

      {/* ── 状态指示条 ── */}
      <div className={`status-bar ${syncEnabled ? "active" : "paused"}`}>
        {syncEnabled
          ? `剪贴板同步已开启 · 已发现 ${devices.length} 台设备`
          : "剪贴板同步已暂停"}
        {localInfo && localInfo.port > 0 && (
          <span className="status-ip"> · {localInfo.ip}:{localInfo.port}</span>
        )}
      </div>

      {/* ── 主体 ── */}
      <main className="main">
        <DeviceList devices={devices} onConnect={handleConnect} />
        <ClipHistory history={history} isMobile={isMobile} />
      </main>

      {/* ── 二维码弹窗 ── */}
      {showQr && localInfo && (
        <QrConnect ip={localInfo.ip} port={localInfo.port} onClose={() => setShowQr(false)} />
      )}

      {/* ── Toast ── */}
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
