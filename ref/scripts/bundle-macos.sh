#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
    echo "The app bundle must be built on macOS." >&2
    exit 1
fi

cargo build --release --locked
app=target/release/smolbrowse.app
mkdir -p "$app/Contents/MacOS"
cp target/release/smolbrowse "$app/Contents/MacOS/smolbrowse.new"
mv "$app/Contents/MacOS/smolbrowse.new" "$app/Contents/MacOS/smolbrowse"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>smolbrowse</string>
    <key>CFBundleDisplayName</key><string>smolbrowse</string>
    <key>CFBundleExecutable</key><string>smolbrowse</string>
    <key>CFBundleIdentifier</key><string>dev.smolbrowse.browser</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app"
echo "Built $app"
