#!/usr/bin/env bash
# Construye macuse y lo empaqueta como macuse.app con Info.plist.
# El bundle ID estable permite que macOS conserve el permiso de Accesibilidad
# entre rebuilds. Sin esto el permiso se invalida cada `cargo build`.
set -euo pipefail

cd "$(dirname "$0")"

echo ">>> cargo build --release"
cargo build --release

APP="macuse.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

cp target/release/macuse "$APP/Contents/MacOS/macuse"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>     <string>en</string>
    <key>CFBundleDisplayName</key>           <string>macuse</string>
    <key>CFBundleExecutable</key>            <string>macuse</string>
    <key>CFBundleIdentifier</key>            <string>com.macuse.app</string>
    <key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
    <key>CFBundleName</key>                  <string>macuse</string>
    <key>CFBundlePackageType</key>           <string>APPL</string>
    <key>CFBundleShortVersionString</key>    <string>0.1.0</string>
    <key>CFBundleVersion</key>               <string>1</string>
    <key>LSMinimumSystemVersion</key>        <string>11.0</string>
    <key>NSPrincipalClass</key>              <string>NSApplication</string>
    <key>NSHighResolutionCapable</key>       <true/>
</dict>
</plist>
PLIST

# Firma ad-hoc - mejora la persistencia del permiso de Accesibilidad
codesign --force --deep --sign - "$APP" 2>/dev/null || true

echo ""
echo ">>> Bundle listo: $(pwd)/$APP"
echo ""
echo "Siguiente paso:"
echo "  1) Abre Ajustes del Sistema -> Privacidad y Seguridad -> Accesibilidad"
echo "  2) Quita cualquier entrada antigua de 'macuse' o 'target/release/macuse'"
echo "  3) Arrastra '$APP' a la lista (o usa +) y activa el switch"
echo "  4) Lanza la app:  open $APP"
