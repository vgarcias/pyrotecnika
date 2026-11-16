"""Genera latest.json, el manifiesto que consulta Ayuda › Buscar actualizaciones.

Uso:  python packaging/updater/make_latest_json.py DIST TAG REPO > latest.json

DIST contiene los paquetes del release y sus firmas `.sig` (las crea
`cargo packager` cuando está definida CARGO_PACKAGER_SIGN_PRIVATE_KEY).
Solo entran los formatos que se actualizan solos (docs/ARQUITECTURA.md §8.3):

    windows-x86_64            *_x64-setup.exe        nsis
    macos-aarch64/x86_64      *_universal.app.tar.gz app   (binario universal)
    linux-x86_64              *_x86_64.AppImage      appimage

El formato es el de cargo-packager-updater, que es el que lee
crates/pyro-platform/src/updates.rs.
"""

import json
import sys
from datetime import datetime, timezone
from pathlib import Path

PLATFORMS = [
    # (claves de plataforma, patrón del paquete, formato)
    (["windows-x86_64"], "*_x64-setup.exe", "nsis"),
    (["macos-aarch64", "macos-x86_64"], "*_universal.app.tar.gz", "app"),
    (["linux-x86_64"], "*_x86_64.AppImage", "appimage"),
]


def main() -> int:
    dist, tag, repo = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    version = tag.removeprefix("v")
    platforms = {}
    for keys, pattern, fmt in PLATFORMS:
        matches = sorted(dist.glob(pattern))
        if len(matches) != 1:
            print(f"error: se esperaba un {pattern} en {dist}, hay {len(matches)}", file=sys.stderr)
            return 1
        package = matches[0]
        sig = package.with_name(package.name + ".sig")
        if not sig.is_file():
            print(f"error: falta la firma {sig.name}", file=sys.stderr)
            return 1
        entry = {
            "url": f"https://github.com/{repo}/releases/download/{tag}/{package.name}",
            "signature": sig.read_text(encoding="utf-8").strip(),
            "format": fmt,
        }
        for key in keys:
            platforms[key] = entry
    manifest = {
        "version": version,
        "notes": f"Novedades: https://github.com/{repo}/releases/tag/{tag}",
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    json.dump(manifest, sys.stdout, indent=2, ensure_ascii=False)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
