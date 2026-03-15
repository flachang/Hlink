import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// 处理 hlink://connect?addr=ip:port 深度链接（移动端扫码后触发）
async function handleDeepLink() {
  try {
    // Tauri 2 mobile deep link 通过 window.__TAURI_INTERNALS__ 传入
    const url = (window as any).__HLINK_DEEP_LINK_URL__ as string | undefined;
    if (url) {
      const parsed = new URL(url);
      if (parsed.protocol === "hlink:" && parsed.pathname === "//connect") {
        const addr = parsed.searchParams.get("addr");
        if (addr) {
          const { invoke } = await import("@tauri-apps/api/core");
          try {
            await invoke("connect_peer", { addr });
            console.log("[Hlink] Deep link connected to", addr);
          } catch (e) {
            console.error("[Hlink] Failed to connect via deep link:", e);
          }
        }
      }
    }
  } catch (e) {
    // deep link 不存在或解析失败时忽略
    console.debug("[Hlink] Deep link not available:", e);
  }
}

handleDeepLink();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
