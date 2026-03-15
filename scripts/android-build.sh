#!/usr/bin/env bash
# scripts/android-build.sh
# 打包 Android Release APK（不依赖 USB / adb，生成 .apk 文件）

set -e

# ── 1. 确保环境变量到位 ───────────────────────────────────────
export PATH="$HOME/.cargo/bin:$PATH"

if [ -z "$ANDROID_HOME" ]; then
  export ANDROID_HOME="$HOME/Library/Android/sdk"
fi
if [ -z "$NDK_HOME" ]; then
  NDK_DIR="$ANDROID_HOME/ndk"
  if [ -d "$NDK_DIR" ]; then
    LATEST_NDK=$(ls "$NDK_DIR" | sort -V | tail -1)
    export NDK_HOME="$NDK_DIR/$LATEST_NDK"
  fi
fi
if [ -z "$JAVA_HOME" ]; then
  STUDIO_JDK="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  if [ -d "$STUDIO_JDK" ]; then
    export JAVA_HOME="$STUDIO_JDK"
  fi
fi

export PATH="$ANDROID_HOME/platform-tools:$PATH"

echo "📱 ANDROID_HOME = $ANDROID_HOME"
echo "🔨 NDK_HOME     = $NDK_HOME"
echo "☕ JAVA_HOME    = $JAVA_HOME"

# ── 2. 构建 Release APK ───────────────────────────────────────
echo ""
echo "🔨 构建 Android Release APK ..."
npm run tauri android build -- --apk

# ── 3. 输出 APK 路径 ──────────────────────────────────────────
echo ""
echo "✅ 构建完成！APK 路径："
echo ""
echo "📦 Release APK (用于分发):"
find src-tauri/gen/android/app/build/outputs/apk/universal/release -name "*.apk" 2>/dev/null | while read apk; do
  size=$(ls -lh "$apk" | awk '{print $5}')
  echo "   $apk ($size)"
done
echo ""
echo "🔧 Debug APK (开发测试用，可忽略):"
find src-tauri/gen/android/app/build/outputs/apk -name "*debug*.apk" 2>/dev/null | while read apk; do
  size=$(ls -lh "$apk" | awk '{print $5}')
  echo "   $apk ($size)"
done
