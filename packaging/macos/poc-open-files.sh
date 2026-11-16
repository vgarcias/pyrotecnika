#!/usr/bin/env bash
# Prueba de concepto de RF-19 en macOS: abrir archivos desde el Finder.
# Empaqueta el ejemplo `open_files_probe` como .app, le envía un archivo al
# arrancar (en frío) y otro con la app ya abierta (en caliente), y comprueba
# que los dos llegan. Uso: packaging/macos/poc-open-files.sh
set -euo pipefail

cd "$(dirname "$0")/../.."
cargo build -p pyrotecnika --example open_files_probe

APP="target/PyroProbe.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/debug/examples/open_files_probe "$APP/Contents/MacOS/"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>io.pyrotecnika.probe</string>
    <key>CFBundleName</key><string>PyroProbe</string>
    <key>CFBundleExecutable</key><string>open_files_probe</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>0.1.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>Imagen</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key>
            <array><string>public.png</string><string>public.image</string></array>
        </dict>
    </array>
</dict>
</plist>
PLIST
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP"

WORK="$(mktemp -d)"
printf 'x' > "$WORK/en-frio.png"
printf 'x' > "$WORK/en-caliente.png"
OUT="$HOME/pyro-open-files-probe.txt"
rm -f "$OUT"

open -a "$PWD/$APP" "$WORK/en-frio.png"
sleep 8
open -a "$PWD/$APP" "$WORK/en-caliente.png"
sleep 5

echo "--- Rutas recibidas ---"
cat "$OUT" 2>/dev/null || echo "(ninguna)"
grep -q "en-frio.png" "$OUT" && echo "OK: archivo recibido al arrancar" || { echo "FALLO: no llegó el archivo al arrancar"; FAIL=1; }
grep -q "en-caliente.png" "$OUT" && echo "OK: archivo recibido con la app abierta" || { echo "FALLO: no llegó el archivo con la app abierta"; FAIL=1; }
exit "${FAIL:-0}"
