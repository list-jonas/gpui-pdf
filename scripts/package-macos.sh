#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app_dir="$project_dir/dist/GPUI PDF.app"
contents_dir="$app_dir/Contents"

cd "$project_dir"
cargo build -p gpui-pdf --release --locked

mkdir -p "$contents_dir/MacOS" "$contents_dir/Resources"
cp "$project_dir/target/release/gpui-pdf" "$contents_dir/MacOS/gpui-pdf"
cp "$project_dir/packaging/macos/Info.plist" "$contents_dir/Info.plist"
chmod 755 "$contents_dir/MacOS/gpui-pdf"

plutil -lint "$contents_dir/Info.plist"
codesign --force --deep --sign - "$app_dir"
codesign --verify --deep --strict --verbose=2 "$app_dir"

echo "$app_dir"
