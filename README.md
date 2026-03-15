# Hlink

> 局域网跨设备剪贴板共享工具 — 让手机与电脑之间复制粘贴像本地一样流畅。

---

## ✨ 功能特性

- **文本同步**：一端复制，所有已连接设备自动同步到剪贴板
- **图片同步**：支持截图、图片直接在设备间传输
- **同步历史**：保留最近 50 条记录，点击图片可全屏预览并保存
- **局域网发现**：UDP 广播自动发现同网设备，无需手动配置
- **二维码连接**：桌面端生成二维码，手机扫码即可快速连接
- **跨平台**：支持 macOS、Windows、Linux、Android

---

## 🖥️ 平台支持

| 平台 | 状态 |
|------|------|
| macOS | ✅ 支持 |
| Windows | ✅ 支持 |
| Linux | ✅ 支持 |
| Android | ✅ 支持 |
| iOS | 🚧 计划中 |

---

## 🚀 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.75+
- [Tauri CLI](https://tauri.app/start/) v2

### 桌面端开发

```bash
npm install
npm run tauri dev
```

### Android 开发

需要额外安装：

- JDK 17+（推荐 [Azul Zulu](https://www.azul.com/downloads/)）
- [Android Studio](https://developer.android.com/studio)（安装 NDK 26+、SDK 34+）

设置环境变量：

```bash
export JAVA_HOME=/path/to/jdk
export ANDROID_HOME=$HOME/Library/Android/sdk   # macOS
export NDK_HOME=$ANDROID_HOME/ndk/<版本号>
```

运行 Android 调试版：

```bash
# 连接手机（开启 USB 调试）后
npm run tauri android dev

# 或打包 Debug APK 安装到手机
npm run android:build
adb install src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

### 桌面端打包

```bash
npm run tauri build
```

---

## 📱 使用方式

1. 在电脑和手机上分别运行 Hlink，确保连接同一 Wi-Fi
2. 电脑端点击右上角 **二维码图标**，手机扫码连接（或手动输入 IP:端口）
3. 连接成功后，任意一端复制内容，另一端自动同步

### 手机图片接收

- 桌面复制图片 → 手机同步历史中显示 **「点击放大」**
- 点击历史记录 → 全屏预览
- 预览界面底部点击 **「保存到相册」** → 图片保存到手机「下载」文件夹（可在相册「下载」分类中查看）

---

## 🏗️ 技术栈

| 层 | 技术 |
|----|------|
| 前端 | React 18 + TypeScript + Vite |
| 后端 | Rust + Tauri 2.0 |
| 通信 | WebSocket (Axum) + UDP Broadcast |
| 剪贴板 | arboard (桌面) / tauri-plugin-clipboard-manager (移动) |
| 图片传输 | RGBA Base64 编码 + Canvas 渲染 |

---

## 📂 项目结构

```
Hlink/
├── src/                    # 前端 React 源码
│   ├── App.tsx             # 主界面
│   ├── components/
│   │   ├── ClipHistory.tsx # 同步历史（含图片预览）
│   │   ├── DeviceList.tsx  # 局域网设备列表
│   │   └── QrConnect.tsx   # 二维码连接
│   └── App.css
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Tauri 命令入口
│   │   ├── sync.rs         # 剪贴板同步 & 历史管理
│   │   ├── server.rs       # WebSocket 服务器
│   │   ├── clipboard.rs    # 剪贴板读写
│   │   └── discovery.rs    # UDP 设备发现
│   └── gen/android/        # Android 工程（自动生成）
└── scripts/                # 打包脚本
```

---

## ⚠️ 已知限制

- **Android 后台剪贴板**：Android 10+ 限制后台 App 读取剪贴板，需将 Hlink 保持在前台使用
- **Android 图片保存**：图片保存至「下载」文件夹，而非直接写入相册（Android 10+ 系统限制）
- **iOS**：暂未正式支持，功能待完善

---

## 📄 License

MIT
