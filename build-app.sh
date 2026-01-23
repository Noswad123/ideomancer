#!/usr/bin/env bash
set -euo pipefail

# ---- config ----
APP_NAME="Ideomancer"
BUNDLE_ID="com.local.ideomancer"
VERSION="0.1.0"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
APP_DIR="$BUILD_DIR/$APP_NAME.app"
BIN_PATH="$BUILD_DIR/$APP_NAME"

# ---- build Go binary ----
echo "🔨 Building GUI binary..."
mkdir -p "$BUILD_DIR"
go build -o "$BIN_PATH" ./cmd/ideomancer

# ---- create .app structure ----
echo "📦 Creating app bundle..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$APP_DIR/Contents/MacOS/$APP_NAME"

# ---- Info.plist ----
cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>

  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>

  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>

  <key>CFBundleVersion</key>
  <string>$VERSION</string>

  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>

  <key>CFBundleExecutable</key>
  <string>$APP_NAME</string>

  <key>CFBundlePackageType</key>
  <string>APPL</string>

  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>

  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

echo "✅ App built at: $APP_DIR"
echo "👉 Run with: open \"$APP_DIR\""
