#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --release
codesign --sign - --entitlements entitlements.plist --force target/release/microvm
cp target/release/microvm ~/.local/bin/microvm
echo "installed: $(microvm version)"
