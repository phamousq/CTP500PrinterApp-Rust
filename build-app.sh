#!/bin/bash
set -e

APP_NAME="CTP500 Printer"
BUNDLE_DIR="${APP_NAME}.app"
BINARY_NAME="ctp500"

echo "Building release binary..."
cargo build --release

echo "Creating app bundle..."
mkdir -p "${BUNDLE_DIR}/Contents/MacOS"
mkdir -p "${BUNDLE_DIR}/Contents/Resources"

cp "target/release/${BINARY_NAME}" "${BUNDLE_DIR}/Contents/MacOS/${BINARY_NAME}"

cat > "${BUNDLE_DIR}/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>CTP500 Printer</string>
    <key>CFBundleDisplayName</key>
    <string>CTP500 Printer</string>
    <key>CFBundleIdentifier</key>
    <string>com.ctp500.printer</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleExecutable</key>
    <string>ctp500</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>CTP500 Printer needs Bluetooth to discover and connect to your thermal printer.</string>
    <key>NSBluetoothPeripheralUsageDescription</key>
    <string>CTP500 Printer needs Bluetooth to discover and connect to your thermal printer.</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>
</dict>
</plist>
EOF

echo "Done: ${BUNDLE_DIR}"
echo "Run with: open '${BUNDLE_DIR}'"
