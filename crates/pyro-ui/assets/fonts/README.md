# Fuentes de la interfaz

| Archivo | Fuente | Versión | Origen | Licencia |
|---|---|---|---|---|
| `Inter-Regular.ttf`, `Inter-SemiBold.ttf` | Inter (estática) | 4.1 | `extras/ttf/` de https://github.com/rsms/inter/releases/tag/v4.1 | SIL OFL 1.1 (`OFL-Inter.txt`) |
| `JetBrainsMono-Regular.ttf` | JetBrains Mono | 2.304 | `fonts/ttf/` de https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304 | SIL OFL 1.1 (`OFL-JetBrainsMono.txt`) |

Se incluyen recortadas al latín ampliado (español, inglés y los símbolos que usa la
interfaz) para no inflar el binario. Para regenerarlas o añadir caracteres:

```sh
python -m fontTools.subset <original.ttf> \
  --unicodes="U+0020-007E,U+00A0-00FF,U+0152-0153,U+2013-2014,U+2018-201E,U+2022,U+2026,U+2030,U+2039-203A,U+20AC,U+2190-2193,U+2212,U+00D7" \
  --layout-features='*' --output-file=<archivo de esta carpeta>
```

Los caracteres que falten se dibujan con las fuentes de respaldo de egui.
