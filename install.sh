#!/bin/sh
set -eu

REPO="windsornguyen/microvm"
INSTALL_DIR="${MICROVM_INSTALL_DIR:-$HOME/.local/bin}"

main() {
    platform="$(uname -s)-$(uname -m)"
    case "$platform" in
        Darwin-arm64) ;;
        *) echo "error: microvm requires macOS on Apple Silicon (got $platform)" >&2; exit 1 ;;
    esac

    version="${1:-latest}"
    if [ "$version" = "latest" ]; then
        url="https://github.com/$REPO/releases/latest/download/microvm-darwin-arm64.tar.gz"
    else
        url="https://github.com/$REPO/releases/download/$version/microvm-darwin-arm64.tar.gz"
    fi

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    echo "downloading microvm..."
    curl -fsSL "$url" -o "$tmp/microvm.tar.gz"
    tar xzf "$tmp/microvm.tar.gz" -C "$tmp"

    # Re-sign locally (ad-hoc signatures aren't portable across machines).
    codesign --sign - --entitlements "$tmp/entitlements.plist" --force "$tmp/microvm"

    mkdir -p "$INSTALL_DIR"
    mv "$tmp/microvm" "$INSTALL_DIR/microvm"
    chmod +x "$INSTALL_DIR/microvm"

    echo "installed: $INSTALL_DIR/microvm"
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo "add to PATH: export PATH=\"$INSTALL_DIR:\$PATH\""
    fi
    "$INSTALL_DIR/microvm" version
}

main "$@"
