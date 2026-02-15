#!/bin/bash
set -euo pipefail

show_help() {
    cat <<'EOF'
Usage: uninstall.sh [--find] [--dry-run] [--purge] [--yes] [--force-unknown]
  --find     Deep scan the filesystem for rustpack binaries (slow).
  --dry-run  Show what would be removed, but do not delete anything.
  --purge    Also remove rustpack config/cache data (if known).
  --yes      Skip confirmation prompts.
  --force-unknown  Remove unverified binaries named rustpack (dangerous).
EOF
}

deep_scan=false
dry_run=false
purge=false
assume_yes=false
force_unknown=false

for arg in "$@"; do
    case "$arg" in
        --find) deep_scan=true ;;
        --dry-run) dry_run=true ;;
        --purge) purge=true ;;
        --yes) assume_yes=true ;;
        --force-unknown) force_unknown=true ;;
        -h|--help) show_help; exit 0 ;;
        *)
            echo "Unknown option: $arg"
            show_help
            exit 1
            ;;
    esac
done

echo "Uninstalling rustpack..."

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Error: Please run as root (use sudo)"
    exit 1
fi

# "Known-safe" targets this project installs to or commonly uses.
trusted_targets=(
    "/usr/local/bin/rustpack"
    "/usr/bin/rustpack"
    "/bin/rustpack"
    "/usr/local/sbin/rustpack"
    "/usr/sbin/rustpack"
    "/sbin/rustpack"
    "$HOME/.local/bin/rustpack"
    "$HOME/bin/rustpack"
    "/usr/local/share/man/man8/rustpack.8"
    "/usr/share/bash-completion/completions/rustpack"
    "/usr/share/zsh/site-functions/_rustpack"
    "/usr/share/fish/vendor_completions.d/rustpack.fish"
)

is_trusted_target() {
    local candidate="$1"
    local target
    for target in "${trusted_targets[@]}"; do
        if [ "$candidate" = "$target" ]; then
            return 0
        fi
    done
    return 1
}

looks_like_rustpack_binary() {
    local candidate="$1"
    local resolved
    resolved="$(readlink -f -- "$candidate" 2>/dev/null || printf '%s' "$candidate")"
    [ -f "$resolved" ] || return 1
    grep -a -Eq \
        "rustpack - A Rust-based package manager for Arch Linux|rustpack history show <id>|--asexplicit" \
        "$resolved" 2>/dev/null
}

remove_candidate() {
    local candidate="$1"
    local trusted=false
    local verified=false

    if is_trusted_target "$candidate"; then
        trusted=true
    fi
    if looks_like_rustpack_binary "$candidate"; then
        verified=true
    fi

    if ! $trusted && ! $verified && ! $force_unknown; then
        echo "Skipping unverified candidate: $candidate"
        skipped=$((skipped + 1))
        return
    fi

    if ! $trusted && ! $verified && $force_unknown; then
        echo "Warning: removing unverified candidate due to --force-unknown: $candidate"
    fi

    if $dry_run; then
        echo "Would remove: $candidate"
    else
        rm -f "$candidate"
        echo "Removed: $candidate"
    fi
    removed=$((removed + 1))
}

# Candidate locations (common + PATH + /opt + whereis/type)
paths=(
    "/usr/local/bin/rustpack"
    "/usr/bin/rustpack"
    "/bin/rustpack"
    "/usr/local/sbin/rustpack"
    "/usr/sbin/rustpack"
    "/sbin/rustpack"
    "$HOME/.local/bin/rustpack"
    "$HOME/bin/rustpack"
    "/usr/local/share/man/man8/rustpack.8"
    "/usr/share/bash-completion/completions/rustpack"
    "/usr/share/zsh/site-functions/_rustpack"
    "/usr/share/fish/vendor_completions.d/rustpack.fish"
)

mapfile -t found_paths < <(type -a -p rustpack 2>/dev/null | awk '!seen[$0]++')
paths+=("${found_paths[@]}")

mapfile -t whereis_paths < <(whereis -b rustpack 2>/dev/null | awk '{for (i=2;i<=NF;i++) print $i}')
paths+=("${whereis_paths[@]}")

IFS=':' read -r -a path_dirs <<< "$PATH"
for d in "${path_dirs[@]}"; do
    paths+=("$d/rustpack")
done
for d in /opt/*/bin /opt/*/sbin; do
    [ -d "$d" ] && paths+=("$d/rustpack")
done

# Deduplicate and remove
removed=0
skipped=0
declare -A seen
for p in "${paths[@]}"; do
    [ -z "$p" ] && continue
    if [ -n "${seen[$p]+x}" ]; then
        continue
    fi
    seen["$p"]=1
    if [ -e "$p" ]; then
        remove_candidate "$p"
    fi
done

# Optional deep scan if requested
if $deep_scan; then
    if ! $assume_yes; then
        read -r -p "Deep scan can be slow. Continue? [y/N] " ans
        case "${ans,,}" in
            y|yes) ;;
            *) echo "Aborted."; exit 1 ;;
        esac
    fi
    echo "Performing deep scan (this may take a while)..."
    while IFS= read -r p; do
        if [ -e "$p" ]; then
            remove_candidate "$p"
        fi
    done < <(find / \( -type f -o -type l \) -name rustpack 2>/dev/null | awk '!seen[$0]++')
fi

if $purge; then
    purge_paths=(
        "/etc/rustpack"
        "/var/lib/rustpack"
        "/var/cache/rustpack"
        "$HOME/.config/rustpack"
        "$HOME/.cache/rustpack"
        "$HOME/.local/share/rustpack"
    )
    for p in "${purge_paths[@]}"; do
        [ -z "$p" ] && continue
        if [ -e "$p" ]; then
            if $dry_run; then
                echo "Would remove: $p"
            else
                rm -rf "$p"
                echo "Removed: $p"
            fi
            removed=$((removed + 1))
        fi
    done
fi

if [ $removed -eq 0 ]; then
    echo "No rustpack binaries found to remove."
else
    if $dry_run; then
        echo "Dry run complete."
    else
        echo "rustpack uninstalled successfully"
    fi
fi

if [ $skipped -gt 0 ]; then
    echo "Skipped $skipped unverified candidate(s)."
    echo "Re-run with --force-unknown to remove unverified matches."
fi
