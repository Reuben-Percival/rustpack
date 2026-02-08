#!/bin/bash
set -e

echo "Uninstalling rustpack..."

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Error: Please run as root (use sudo)"
    exit 1
fi

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
declare -A seen
for p in "${paths[@]}"; do
    [ -z "$p" ] && continue
    if [ -n "${seen[$p]}" ]; then
        continue
    fi
    seen["$p"]=1
    if [ -e "$p" ]; then
        rm -f "$p"
        echo "Removed: $p"
        removed=$((removed + 1))
    fi
done

# Optional deep scan if requested
if [ "$1" = "--find" ]; then
    echo "Performing deep scan (this may take a while)..."
    while IFS= read -r p; do
        if [ -e "$p" ]; then
            rm -f "$p"
            echo "Removed: $p"
            removed=$((removed + 1))
        fi
    done < <(find / -type f -o -type l -name rustpack 2>/dev/null | awk '!seen[$0]++')
fi

if [ $removed -eq 0 ]; then
    echo "No rustpack binaries found to remove."
else
    echo "rustpack uninstalled successfully"
fi
