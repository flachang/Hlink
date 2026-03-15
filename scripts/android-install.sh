#!/usr/bin/env bash
# scripts/android-install.sh
# 通过 USB 连接安装 APK 到手机

set -e

# ── 1. 确保环境变量到位 ───────────────────────────────────────
export PATH="$HOME/.cargo/bin:$PATH"

if [ -z "$ANDROID_HOME" ]; then
  export ANDROID_HOME="$HOME/Library/Android/sdk"
fi

export PATH="$ANDROID_HOME/platform-tools:$PATH"

# ── 2. 检查 adb 设备 ──────────────────────────────────────────
echo "🔍 检测连接的 Android 设备..."
DEVICES=$(adb devices | grep -v "List" | grep "device$" | wc -l | tr -d ' ')

if [ "$DEVICES" -eq 0 ]; then
  echo "❌ 未检测到已连接的 Android 设备"
  echo ""
  echo "请确保："
  echo "1. 手机已通过 USB 连接到电脑"
  echo "2. 手机已开启 USB 调试（设置 → 开发者选项 → USB 调试）"
  echo "3. 手机上已授权此电脑的 USB 调试"
  exit 1
fi

echo "✅ 检测到 $DEVICES 台设备"
adb devices
echo ""

# ── 3. 查找可用的 APK ──────────────────────────────────────────
# 优先使用 Debug APK（已签名），Release APK 未签名无法直接安装
DEBUG_APK=$(find src-tauri/gen/android/app/build/outputs/apk -name "*debug*.apk" 2>/dev/null | head -1)
RELEASE_APK=$(find src-tauri/gen/android/app/build/outputs/apk/universal/release -name "*.apk" 2>/dev/null | head -1)

if [ -n "$DEBUG_APK" ]; then
  APK_PATH="$DEBUG_APK"
  APK_TYPE="Debug (已签名)"
elif [ -n "$RELEASE_APK" ]; then
  echo "⚠️  检测到未签名的 Release APK"
  echo ""
  echo "Release APK 未签名，无法直接安装到手机"
  echo ""
  echo "解决方案：使用已签名的 Debug APK"
  echo ""
  echo "请运行以下命令生成已签名的 Debug APK："
  echo "  npm run android:dev"
  echo ""
  echo "这会构建并自动安装 Debug APK 到手机"
  exit 1
else
  echo "❌ 未找到任何 APK 文件"
  echo ""
  echo "请先运行构建命令："
  echo "  npm run android:dev     # Debug APK（已签名，推荐）"
  echo "  或"
  echo "  npm run android:build   # Release APK（未签名，需签名）"
  exit 1
fi

APK_SIZE=$(ls -lh "$APK_PATH" | awk '{print $5}')
echo "📦 使用 APK: $APK_PATH ($APK_SIZE)"
echo "   类型: $APK_TYPE"
echo ""

# ── 5. 安装 APK ───────────────────────────────────────────────
echo "📱 正在安装到手机..."
adb install -r "$APK_PATH"

if [ $? -eq 0 ]; then
  echo ""
  echo "✅ 安装成功！"
  echo ""
  echo "现在可以在手机上打开 Hlink 应用了"
else
  echo ""
  echo "❌ 安装失败"
  echo ""
  echo "如果使用的是 Release APK，请尝试："
  echo "  1. 运行 'npm run android:dev' 生成已签名的 Debug APK"
  echo "  2. 或者手动签名 Release APK"
  exit 1
fi
