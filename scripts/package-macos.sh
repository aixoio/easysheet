#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo 'This packaging script requires macOS. Use cargo build --release on other platforms.' >&2
  exit 1
fi
cargo build --release --locked
bundle="target/release/bundle/EasySheet.app"
mkdir -p "$bundle/Contents/MacOS"
cp target/release/easysheet "$bundle/Contents/MacOS/easysheet"
cp packaging/macos/Info.plist "$bundle/Contents/Info.plist"
printf 'Built %s\n' "$PWD/$bundle"
