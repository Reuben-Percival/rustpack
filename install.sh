#!/bin/bash
set -e

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

skip_build=false
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=true ;;
        -h|--help)
            cat <<'EOF'
Usage: install.sh [--skip-build]
  --skip-build  Install existing target/release/rustpack without rebuilding.
EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            exit 1
            ;;
    esac
done

echo "Installing rustpack..."

# Build rustpack unless explicitly skipped
if ! $skip_build; then
    echo "Building release binary with cargo..."
    if [ "$EUID" -eq 0 ] && [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != "root" ]; then
        if ! sudo -u "$SUDO_USER" -H bash -lc "command -v cargo >/dev/null 2>&1"; then
            echo "Error: cargo not found for user '$SUDO_USER'. Install Rust toolchain for that user first."
            exit 1
        fi
        SCRIPT_DIR_Q="$(printf '%q' "$SCRIPT_DIR")"
        sudo -u "$SUDO_USER" -H bash -lc "cd $SCRIPT_DIR_Q && cargo build --release"
    elif [ "$EUID" -eq 0 ]; then
        echo "Error: cannot build as root without SUDO_USER."
        echo "Run 'cargo build --release' as your normal user, then run sudo ./install.sh --skip-build"
        exit 1
    else
        if ! command -v cargo >/dev/null 2>&1; then
            echo "Error: cargo not found in PATH. Install Rust toolchain first."
            exit 1
        fi
        cargo build --release
    fi
else
    echo "Skipping build (--skip-build)."
fi

if [ ! -f "target/release/rustpack" ]; then
    echo "Error: build completed but target/release/rustpack was not found."
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

# Install man page
install -d /usr/local/share/man/man8
install -m 0644 docs/man/rustpack.8 /usr/local/share/man/man8/rustpack.8

# Install shell completions
install -d /usr/share/bash-completion/completions
install -m 0644 completions/rustpack.bash /usr/share/bash-completion/completions/rustpack

install -d /usr/share/zsh/site-functions
install -m 0644 completions/_rustpack /usr/share/zsh/site-functions/_rustpack

install -d /usr/share/fish/vendor_completions.d
install -m 0644 completions/rustpack.fish /usr/share/fish/vendor_completions.d/rustpack.fish

echo "rustpack installed successfully to /usr/local/bin/rustpack"
echo ""
echo "Also installed:"
echo "  man page: /usr/local/share/man/man8/rustpack.8"
echo "  bash completion: /usr/share/bash-completion/completions/rustpack"
echo "  zsh completion: /usr/share/zsh/site-functions/_rustpack"
echo "  fish completion: /usr/share/fish/vendor_completions.d/rustpack.fish"
echo ""
echo "Usage:"
echo "  rustpack -Ss firefox      # Search packages"
echo "  sudo rustpack -S firefox  # Install packages"
echo "  rustpack -Q               # List installed"
echo "  sudo rustpack -Syu        # System upgrade"
