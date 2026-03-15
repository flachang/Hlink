#!/usr/bin/env bash
# scripts/android-dev.sh
# 启动 Android 开发调试（自动清理旧进程 → 构建 → 安装到手机）

set -e

# ── 1. 清理占用端口的旧进程 ────────────────────────────────────
echo "🧹 清理旧进程..."
lsof -ti:1420,1421 | xargs kill -9 2>/dev/null || true
pkill -f "tauri android dev" 2>/dev/null || true
pkill -f "target/debug/hlink" 2>/dev/null || true
sleep 1

# ── 2. 确保环境变量到位 ───────────────────────────────────────
export PATH="$HOME/.cargo/bin:$PATH"

if [ -z "$ANDROID_HOME" ]; then
  export ANDROID_HOME="$HOME/Library/Android/sdk"
fi
if [ -z "$NDK_HOME" ]; then
  # 取最新 NDK 版本
  NDK_DIR="$ANDROID_HOME/ndk"
  if [ -d "$NDK_DIR" ]; then
    LATEST_NDK=$(ls "$NDK_DIR" | sort -V | tail -1)
    export NDK_HOME="$NDK_DIR/$LATEST_NDK"
  fi
fi
if [ -z "$JAVA_HOME" ]; then
  # 常见 Android Studio JDK 路径
  STUDIO_JDK="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  if [ -d "$STUDIO_JDK" ]; then
    export JAVA_HOME="$STUDIO_JDK"
  fi
fi

export PATH="$ANDROID_HOME/platform-tools:$PATH"

echo "📱 ANDROID_HOME = $ANDROID_HOME"
echo "🔨 NDK_HOME     = $NDK_HOME"
echo "☕ JAVA_HOME    = $JAVA_HOME"

# ── 3. 检查 adb 设备 ──────────────────────────────────────────
echo ""
echo "🔍 检测连接的 Android 设备..."
adb devices
echo ""

# ── 4. 启动 tauri android dev ─────────────────────────────────
echo "🚀 启动 tauri android dev ..."
npm run tauri android dev
