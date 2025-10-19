#!/bin/bash
# Build script for cross-compilation

set -e

echo "Building for Linux..."
cargo build --release

echo ""
echo "Installing mingw-w64 for Windows cross-compilation..."
sudo apt-get update -qq
sudo apt-get install -y mingw-w64

echo ""
echo "Building for Windows..."
cargo build --release --target x86_64-pc-windows-gnu

echo ""
echo "Build complete!"
echo "Linux binary: target/release/audio-transcribe-cli"
echo "Windows binary: target/x86_64-pc-windows-gnu/release/audio-transcribe-cli.exe"
