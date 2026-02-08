#!/bin/bash
set -e

echo "Installing rustpack..."

# Check if rustpack binary exists
if [ ! -f "target/release/rustpack" ]; then
    echo "Error: rustpack binary not found. Please run 'cargo build --release' first."
    exit 1
fi

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Error: Please run as root (use sudo)"
    exit 1
fi

# Show any existing rustpack binaries in PATH
mapfile -t existing_paths < <(type -a -p rustpack 2>/dev/null | awk '!seen[$0]++')
if [ ${#existing_paths[@]} -gt 0 ]; then
    echo "Warning: existing rustpack binaries found in PATH:"
    for p in "${existing_paths[@]}"; do
        echo "  $p"
    done
    echo "This install will overwrite /usr/local/bin/rustpack only."
fi

# Copy binary to /usr/local/bin
cp target/release/rustpack /usr/local/bin/rustpack
chmod +x /usr/local/bin/rustpack

echo "rustpack installed successfully to /usr/local/bin/rustpack"
echo ""
echo "Usage:"
echo "  rustpack -Ss firefox      # Search packages"
echo "  sudo rustpack -S firefox  # Install packages"
echo "  rustpack -Q               # List installed"
echo "  sudo rustpack -Syu        # System upgrade"
