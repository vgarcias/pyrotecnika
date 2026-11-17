# Fase 1 — Sistema de diseño neutro

Fecha: 2026-11-16 · Estado: propuesta para aprobar · Hoja de ruta: `docs/ARQUITECTURA.md` §9.1, fase 1

## Intención

Pyrotecnika usa hoy el tema por defecto de egui, con 14 colores fijos repartidos por el código y sin tipografía propia. La fase 1 le da un sistema de diseño propio y coherente en tres temas, para que la interfaz sea **fácil de usar y de aprender**, con el nivel de acabado de Macromedia Fireworks y Paint Shop Pro (las referencias del autor, elegidas por su facilidad de uso y curva de aprendizaje, "nada en particular visualmente").

En la ronda de dirección visual de Impeccable (semilla `5ffb0d79`) el autor eligió **el estándar de la categoría**: un editor de escritorio convencional, ejecutado con todo el oficio, sin rarezas de identidad. Después precisó la paleta: **solo neutros (grises, blanco y negro)**; el logo no es referencia para la interfaz y no hay naranjas ni rojos. Lo activo se marca por **inversión**. La maqueta aprobada está en `.impeccable/mocks/fase1/sistema-neutro.html`.

## Objetivos

- Tokens semánticos neutros para los temas **oscuro**, **claro** y **alto contraste**, en un solo módulo; ningún widget usa colores literales.
- Tema por defecto: **seguir al sistema operativo**; se puede fijar desde el menú.
- **Inter** (interfaz) y **JetBrains Mono** (valores numéricos) incluidas en el binario.
- **Escala de la interfaz** de 80 a 200 %, independiente del zoom de la imagen.
- Estados de componente coherentes (reposo, hover, pulsado, seleccionado, foco, deshabilitado), con el foco de teclado siempre visible y nada indicado solo por color.
- **Capturas de referencia** de la ventana principal y de los diálogos en los tres temas, aprobadas por el autor (criterio de salida de la fase 1).

## No objetivos

- Widgets nuevos (deslizador con campo editable, paneles acoplables, pestañas, inspector de propiedades): enfoque B, siguiente paso.
- Preferencias persistentes: la fase 2 trae `settings.toml`. En la fase 1 el tema y la escala elegidos duran hasta cerrar la app.
- Diseño adaptable de §4.7 (paneles que se colapsan por ancho): fase 3.
- Cambios de comportamiento de herramientas o de flujos.

## Antiobjetivos

- Que se vea "como cualquier editor oscuro" genérico, es decir, descuidado. El estándar se diferencia por el acabado: alineación, espaciado y tipografía consistentes, estados claros y textos legibles.
- Cualquier tinte de color en la interfaz o alrededor del lienzo (altera la percepción del color de la imagen).
- Colores del logo, naranjas o rojos en la interfaz.
- Severidad (aviso, error) indicada solo por color.

## Restricciones

- Rust + egui/eframe 0.31, backend wgpu. Todo el código específico de SO sigue en `pyro-platform`.
- Licencias: solo MIT, Apache-2.0, BSD, ISC, Zlib, OFL, etc. (`deny.toml`); nada copyleft. Inter y JetBrains Mono son OFL-1.1 (ya permitida).
- Rendimiento (PRODUCT.md): primer frame < 500 ms, 0 % de CPU en reposo. Las fuentes se cargan una vez al arrancar.
- WCAG 2.2 AA: texto ≥ 4.5:1, componentes y foco ≥ 3:1, objetivos de clic ≥ 24×24 px.

## Enfoques considerados

| Enfoque | Qué es | Contra |
|---|---|---|
| **A. Tokens + estilo global de egui** ✔ | Módulo de tokens; desde él se configura `egui::Style`/`Visuals` por tema, más los pocos widgets que egui no hace como se quiere | — |
| B. A + juego completo de widgets propios | Además: deslizador con campo numérico, paneles acoplables, pestañas, inspector | ~2× el trabajo; se mezcla con las fases 2 y 3 |
| C. Solo cambiar los colores de egui | Sin módulo de tokens | Rompe la regla de §4 ("ningún widget usa colores literales") y no deja base para el alto contraste |

**Elegido: A** (por el autor, 2026-11-16). Lo de B que más ayuda a la facilidad de uso (valores numéricos editables, opciones siempre visibles) queda como siguiente paso.

## Diseño

### 1. Tokens de color

En `crates/pyro-ui/src/design/tokens.rs`. Un `struct Palette` con un campo por token y tres constantes (`DARK`, `LIGHT`, `HIGH_CONTRAST`).

| Token | Oscuro | Claro | Alto contraste | Uso |
|---|---|---|---|---|
| `bg_app` | `#161616` | `#EAEAEA` | `#000000` | Fondo de la ventana |
| `bg_panel` | `#1F1F1F` | `#F7F7F7` | `#000000` | Paneles, menú, barra de estado |
| `bg_raised` | `#2A2A2A` | `#FFFFFF` | `#000000` | Controles, campos, tooltips, menús emergentes |
| `bg_hover` | `#303030` | `#EDEDED` | `#1A1A1A` | Fondo en hover |
| `bg_pressed` | `#333333` | `#DCDCDC` | `#333333` | Fondo al pulsar |
| `bg_canvas` | `#121212` | `#D6D6D6` | `#000000` | Alrededor del lienzo (gris neutro puro) |
| `border_subtle` | `#333333` | `#DCDCDC` | `#FFFFFF` | Divisores |
| `border_control` | `#7A7A7A` | `#858585` | `#FFFFFF` | Bordes de controles |
| `text_primary` | `#EDEDED` | `#181818` | `#FFFFFF` | Texto e iconos |
| `text_secondary` | `#A6A6A6` | `#575757` | `#FFFFFF` | Etiquetas, ayudas, iconos en reposo |
| `text_disabled` | `#6B6B6B` | `#9A9A9A` | `#8C8C8C` | Deshabilitado |
| `active` | `#EDEDED` | `#181818` | `#FFFFFF` | Relleno de lo activo (inversión) |
| `on_active` | `#121212` | `#FFFFFF` | `#000000` | Texto e icono sobre lo activo |
| `focus_ring` | `#FFFFFF` | `#000000` | `#FFFFFF` | Anillo de foco de teclado |

Grosor de bordes: 1 px; **2 px en alto contraste**.

Contraste medido (WCAG): texto principal 14.1 / 16.6 / 21:1; secundario 6.8 / 6.8 / 21:1 (5.9 / 7.2 sobre `bg_raised`); texto sobre lo activo 16 / 17.8 / 21:1; lo activo contra el panel 14.1 / 16.6 / 21:1; bordes de control 3.3 / 3.7 / 21:1 (mín. 3:1); foco 16.5 / 19.6 / 21:1. El texto deshabilitado está exento (WCAG 1.4.3).

**Superposiciones sobre la imagen** (no son interfaz; deben verse sobre cualquier foto): siempre en par blanco + negro, como las hormigas de la selección que ya existen. Pasan a ese par la guía de forma (hoy azul), la línea de la recta con Mayús (hoy azul) y la marca de origen del tampón (hoy naranja). El tablero de transparencia sigue en grises 204/255. Se definen como constantes en `tokens.rs` (`OVERLAY_LIGHT`, `OVERLAY_DARK`, `CHECKER_A`, `CHECKER_B`), no por tema.

**Avisos y errores**: sin color. Icono Lucide (`triangle-alert` para aviso, `circle-x` para error) en `text_primary` y texto en semibold. `Visuals::warn_fg_color` y `error_fg_color` se igualan a `text_primary`.

### 2. Tipografía

- **Inter** (OFL-1.1) en pesos Regular y SemiBold, y **JetBrains Mono** (OFL-1.1) Regular, como TTF estáticos en `crates/pyro-ui/assets/fonts/` con sus licencias. Se recortan a latín ampliado (español, inglés y símbolos de interfaz: ×, •, «», →, …) con `fontTools.subset` para no inflar el binario; el README de la carpeta documenta el comando, como el de los iconos.
- Las tres fuentes y la de iconos se registran juntas en **un solo** `FontDefinitions` (hoy `icons::install` reemplaza las fuentes; pasa a integrarse en `design::install`). Familias: `Proportional` → Inter Regular; `Name("semibold")` → Inter SemiBold; `Monospace` → JetBrains Mono; `Name("iconos")` → Lucide.
- Escala (a 100 %): `caption 11` · `body 13` · `label 13 semibold` · `title 15 semibold` · `heading 18 semibold`, mapeada a los `TextStyle` de egui (`Small`, `Body`, `Button`, `Name("title")`, `Heading`) más `Monospace 12`.
- Los valores numéricos visibles (tamaño, dureza, opacidad, coordenadas, zoom, tamaños de archivo) usan `Monospace`.

### 3. Espaciado, forma y elevación

- Retícula de 4 px: `item_spacing` 8×4, márgenes de panel 12, de ventana 16, botones con relleno 12×4 y alto mínimo 24.
- Radios: 4 (controles), 6 (tooltips, menús, popovers), 8 (ventanas modales).
- Paneles planos. Solo los elementos flotantes (tooltips, menús, modales) llevan una sombra neutra suave; ninguna sombra de color.
- Botones de herramienta: 32×32 con icono de 18 px (como hoy).

### 4. Estados

Se configuran en `Visuals::widgets` (`inactive`, `hovered`, `active`, `open`, `noninteractive`) desde los tokens:

| Estado | Aspecto |
|---|---|
| Reposo | `bg_raised`, borde `border_control`, texto `text_primary` |
| Hover | `bg_hover` |
| Pulsado | `bg_pressed` |
| Seleccionado / principal | **inversión**: relleno `active`, texto e icono `on_active` |
| Foco de teclado | anillo de 2 px en `focus_ring`, separado 2 px del elemento (visible también sobre lo invertido) |
| Deshabilitado | texto `text_disabled`, borde `border_subtle`, sin hover |

Widgets que egui no resuelve así y se escriben en `crates/pyro-ui/src/design/widgets.rs`:

- `tool_button`: botón de herramienta invertido cuando está activo, con tooltip "Nombre · atajo" y nombre accesible sin el atajo (corrige la observación de la crítica de que el atajo forma parte del nombre).
- `primary_button`: botón invertido para la acción principal de cada diálogo (Exportar, Aplicar, Crear).
- Fila de capa activa invertida en el panel de capas.
- Anillo de foco: egui 0.31 no dibuja uno configurable; se pinta tras el cuadro alrededor del widget con foco de teclado (`ctx.memory(|m| m.focused())` + su rect), solo cuando el foco llegó por teclado.

`Visuals::selection` (texto seleccionado en campos) usa `active`/`on_active` con transparencia suficiente para leer el texto.

### 5. Temas y escala de la interfaz

- Al arrancar: `ThemePreference::System`, con `set_style_of(Theme::Dark, DARK)` y `set_style_of(Theme::Light, LIGHT)`. Si el sistema no informa su tema (frecuente en Linux), egui usa el oscuro.
- Menú **Ver › Tema**: Seguir al sistema · Claro · Oscuro · Alto contraste (opciones de radio).
- *Decisión del documento:* **Alto contraste es un único tema fijo** (blanco sobre negro), independiente del tema del sistema: al elegirlo se pone `ThemePreference::Dark` y se registra el estilo de alto contraste como estilo oscuro. Al volver a Seguir al sistema, Claro u Oscuro se registran de nuevo `DARK` y `LIGHT` y la preferencia correspondiente. No hay variante clara de alto contraste.
- Menú **Ver › Tamaño de la interfaz**: Aumentar · Reducir · Restablecer (100 %), en pasos de 10 % entre 80 y 200 %, con `ctx.set_zoom_factor`. *Decisión del documento:* **solo desde el menú, sin atajo** en esta fase. §4.2 proponía Ctrl+`+`/`-` según dónde esté el puntero, pero esos atajos ya hacen zoom de la imagen y cambiar su significado según el foco va contra la facilidad de uso; y Ctrl+Alt no sirve porque en Windows equivale a AltGr, que en el teclado español escribe caracteres. Los atajos configurables llegan en la fase 2. Se actualiza §4.2.
- Sin persistencia hasta la fase 2.

### 6. Movimiento

`style.animation_time` = 0.12 s (hover y pulsado). Nada se anima en el lienzo. La opción "Reducir movimiento" llega con las preferencias de la fase 2.

### 7. Capturas de referencia

- `egui_kittest` 0.31 (MIT OR Apache-2.0) con las features `wgpu` y `snapshot`, como dependencia de desarrollo de `pyro-ui`. Dibuja la interfaz sin abrir ventanas ni simular ratón o teclado.
- Pruebas en `crates/pyro-ui/tests/snapshots.rs`: ventana principal (1280×800, documento de ejemplo sintético con dos capas y una selección), diálogo de exportar, diálogo de filtro, confirmación de cambios sin exportar y ventana de actualizaciones, en los tres temas, más la ventana principal al 150 %. Las imágenes se guardan en `crates/pyro-ui/tests/snapshots/`.
- `PyroApp` necesita un constructor de prueba que no dependa de `CreationContext` ni de la plataforma, y una forma de abrir cada diálogo en un estado determinista.
- *Decisión del documento:* **las capturas se generan y se comparan en local, en Windows**, y el autor las aprueba mirándolas. Las pruebas de capturas llevan `#[ignore]` y se ejecutan a propósito (`cargo test -p pyro-ui --test snapshots -- --ignored`; con `UPDATE_SNAPSHOTS=1` para regenerarlas). El CI no compara píxeles: sus runners no tienen GPU y el renderizado por software no coincide píxel a píxel con una GPU real, así que una comparación allí fallaría por ruido. Las mismas escenas sí se construyen y se dibujan en el CI como prueba normal (sin comparar imágenes), para detectar pánicos o diálogos rotos en los tres SO. Así no hace falta `cfg(target_os)` fuera de `pyro-platform`.

### 8. Limpieza de colores fijos

Los 14 usos actuales de `Color32` en `canvas.rs` y `view.rs` pasan a tokens o a las constantes de superposición. Se añade una regla al job de arquitectura del CI: fuera de `crates/pyro-ui/src/design/` no puede aparecer `Color32::from_rgb`, `from_gray`, `from_rgba*` ni los colores con nombre, salvo `Color32::WHITE` como tinte neutro de `painter.image` y `Color32::TRANSPARENT`.

### 9. Documentación

- `docs/ARQUITECTURA.md` §4: se sustituye la tabla de §4.1 (que tenía acento naranja) por la de este documento, y se actualizan §4.2 (atajos de escala) y §4.5 (inversión).
- `PRODUCT.md`: se corrige "no hay logotipo" (sí lo hay, `logo/`, solo para el icono de la app), y se añaden como compromisos de marca la paleta neutra, el estándar de la categoría y Fireworks / Paint Shop Pro como vara de facilidad de uso.
- Al terminar, Impeccable genera `DESIGN.md` y `.impeccable/design.json` a partir de lo construido.

## Lo que decide quien lo construya

- El valor exacto de la tolerancia de las capturas y el contenido del documento de ejemplo.
- Cómo se nombra cada `TextStyle` personalizado y la API interna de `design::widgets`.
- Los pasos exactos de `bg_hover`/`bg_pressed` si al verlos en la app no se distinguen bien (manteniendo los contrastes de la tabla).
