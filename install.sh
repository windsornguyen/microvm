#!/bin/sh
set -eu

REPO="windsornguyen/microvm"
INSTALL_DIR="${MICROVM_INSTALL_DIR:-$HOME/.local/bin}"

main() {
    check_platform
    check_deps

    version="${1:-latest}"
    if [ "$version" = "latest" ]; then
        base="https://github.com/$REPO/releases/latest/download"
    else
        base="https://github.com/$REPO/releases/download/$version"
    fi

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    echo "downloading microvm..."
    curl -fsSL "$base/microvm-darwin-arm64.tar.gz" -o "$tmp/microvm.tar.gz"
    curl -fsSL "$base/microvm-darwin-arm64.tar.gz.sha256" -o "$tmp/expected.sha256"

    actual="$(shasum -a 256 "$tmp/microvm.tar.gz" | cut -d' ' -f1)"
    expected="$(cat "$tmp/expected.sha256" | tr -d '[:space:]')"
    if [ "$actual" != "$expected" ]; then
        echo "error: checksum mismatch" >&2
        echo "  expected: $expected" >&2
        echo "  actual:   $actual" >&2
        exit 1
    fi

    tar xzf "$tmp/microvm.tar.gz" -C "$tmp"

    # Ad-hoc signatures aren't portable; re-sign locally.
    codesign --sign - --entitlements "$tmp/entitlements.plist" --force "$tmp/microvm"

    mkdir -p "$INSTALL_DIR"
    mv "$tmp/microvm" "$INSTALL_DIR/microvm"
    chmod +x "$INSTALL_DIR/microvm"

    ensure_path
    echo "installed: $("$INSTALL_DIR/microvm" version)"
}

check_platform() {
    case "$(uname -s)-$(uname -m)" in
        Darwin-arm64) ;;
        *) echo "error: microvm requires macOS on Apple Silicon" >&2; exit 1 ;;
    esac
}

check_deps() {
    for cmd in curl tar codesign shasum; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            echo "error: $cmd is required but not found" >&2
            exit 1
        fi
    done
}

ensure_path() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) return ;;
    esac

    line="export PATH=\"$INSTALL_DIR:\$PATH\""

    for rc in "$HOME/.zshrc" "$HOME/.bashrc"; do
        if [ -f "$rc" ] && ! grep -qF "$INSTALL_DIR" "$rc"; then
            echo "$line" >> "$rc"
            echo "added $INSTALL_DIR to PATH in $(basename "$rc")"
        fi
    done

    # Apply for current session.
    export PATH="$INSTALL_DIR:$PATH"
}

main "$@"
