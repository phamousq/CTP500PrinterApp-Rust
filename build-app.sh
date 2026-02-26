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

echo "Done: ${BUNDLE_DIR}"
echo "Run with: open '${BUNDLE_DIR}'"
