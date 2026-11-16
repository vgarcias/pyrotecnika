"""Genera los íconos de la app a partir del logo original.

El logo (`logo/logo pyrotecnica.jpg`) es un JPG con fondo blanco. Este script
recorta el cuadrado rojo redondeado, deja transparente todo lo de fuera
(con antialias) y escribe los formatos que usan los instaladores:

    icon.png        1024x1024, fuente para Linux y para la ventana de la app
    32x32.png, 64x64.png, 128x128.png, 256x256.png, 512x512.png
    icon.ico        Windows (16-256 px)
    icon.icns       macOS (16-1024 px)

Uso:  python packaging/icons/make_icons.py
Requiere Pillow. Si cambias el logo, ajusta SQUARE y RADIUS (medidos sobre el
original de 1024x1024).
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "logo" / "logo pyrotecnica.jpg"
OUT = Path(__file__).resolve().parent

SQUARE = (61, 61, 962, 962)  # bordes del cuadrado rojo, inclusive
RADIUS = 152  # radio de las esquinas
INSET = 1  # recorta 1 px para no arrastrar el halo blanco del JPG
SUPERSAMPLE = 4


def rounded_mask(size: int) -> Image.Image:
    big = size * SUPERSAMPLE
    mask = Image.new("L", (big, big), 0)
    x0, y0, x1, y1 = (c * SUPERSAMPLE for c in SQUARE)
    inset = INSET * SUPERSAMPLE
    ImageDraw.Draw(mask).rounded_rectangle(
        (x0 + inset, y0 + inset, x1 + SUPERSAMPLE - 1 - inset, y1 + SUPERSAMPLE - 1 - inset),
        radius=RADIUS * SUPERSAMPLE,
        fill=255,
    )
    return mask.resize((size, size), Image.LANCZOS)


def main() -> None:
    src = Image.open(SRC).convert("RGB")
    if src.size != (1024, 1024):
        src = src.resize((1024, 1024), Image.LANCZOS)
    icon = src.convert("RGBA")
    icon.putalpha(rounded_mask(1024))

    icon.save(OUT / "icon.png", optimize=True)
    for size in (32, 64, 128, 256, 512):
        icon.resize((size, size), Image.LANCZOS).save(OUT / f"{size}x{size}.png", optimize=True)

    icon.save(OUT / "icon.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    icon.save(OUT / "icon.icns", sizes=[(s, s) for s in (16, 32, 64, 128, 256, 512, 1024)])
    print(f"Íconos escritos en {OUT}")


if __name__ == "__main__":
    main()
