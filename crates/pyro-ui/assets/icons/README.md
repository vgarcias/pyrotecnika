# Iconos

`lucide-subset.ttf` es un subconjunto de la fuente de iconos de
[Lucide](https://lucide.dev) v0.460.0 (`lucide-static/font/lucide.ttf`),
licencia ISC (ver `LICENSE-LUCIDE`). Se usa la fuente en vez de los SVG para
no depender de `resvg` (MPL-2.0, que la política de `deny.toml` no admite).

| Código | Icono Lucide | Uso |
|---|---|---|
| U+E125 | `move` | Mover |
| U+E1CB | `square-dashed` | Marco |
| U+E4B5 | `circle-dashed` | Elipse |
| U+E1CE | `lasso` | Lazo |
| U+E35B | `wand-sparkles` | Varita mágica |
| U+E2E7 | `paintbrush` | Pincel |
| U+E28F | `eraser` | Borrador |
| U+E3BF | `stamp` | Tampón de clonar |
| U+E0B8 | `droplet` | Desenfocar |
| U+E2E6 | `paint-bucket` | Bote de pintura |
| U+E13F | `pipette` | Cuentagotas |
| U+E082 | `circle-help` | Ayuda de navegación |
| U+E193 | `triangle-alert` | Avisos |
| U+E088 | `circle-x` | Errores |

Los códigos salen de `lucide-static@0.460.0/font/lucide.css`. Para añadir un
icono, búscalo ahí, añádelo a `--unicodes` y regenera el subconjunto:

```sh
pyftsubset lucide.ttf --output-file=lucide-subset.ttf --layout-features='' --no-hinting \
  --unicodes=U+E125,U+E1CB,U+E4B5,U+E1CE,U+E35B,U+E2E7,U+E28F,U+E3BF,U+E0B8,U+E2E6,U+E13F,U+E082,U+E193,U+E088
```
