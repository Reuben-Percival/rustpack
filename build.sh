#!/bin/bash
set -e

echo "Building rustpack..."

# Check if on Arch Linux
if [ ! -f /etc/arch-release ]; then
    echo "Warning: This package manager is designed for Arch Linux"
fi

# Check dependencies
echo "Checking dependencies..."
if ! command -v pacman &> /dev/null; then
    echo "Error: pacman not found. This tool requires Arch Linux."
    exit 1
fi

# Build release version
cargo build --release

echo ""
echo "Build complete!"
echo "Install with: sudo cp target/release/rustpack /usr/local/bin/"
echo ""
echo "Usage examples (pacman syntax):"
echo "  rustpack -Ss firefox          # Search for packages"
echo "  sudo rustpack -S firefox      # Install packages"
echo "  rustpack -Q                   # List installed"
echo "  rustpack -Qi firefox          # Package info"
echo "  sudo rustpack -R firefox      # Remove packages"
echo "  sudo rustpack -Syu            # System upgrade"
