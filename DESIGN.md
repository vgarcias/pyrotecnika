---
name: Pyrotecnika
description: Editor de imágenes nativo de escritorio (Rust + egui 0.31). Interfaz neutra, estándar de la categoría, lo activo por inversión.
platform: native-desktop-egui
source: crates/pyro-ui/src/design/
colors:
  # Tema oscuro (respaldo cuando el SO no informa su tema). Claro y alto contraste en la sección Color y en .impeccable/design.json.
  bg-app: "#161616"
  bg-panel: "#1F1F1F"
  bg-raised: "#2A2A2A"
  bg-hover: "#303030"
  bg-pressed: "#333333"
  bg-canvas: "#121212"
  border-subtle: "#333333"
  border-control: "#7A7A7A"
  text-primary: "#EDEDED"
  text-secondary: "#A6A6A6"
  text-disabled: "#6B6B6B"
  active: "#EDEDED"
  on-active: "#121212"
  focus-ring: "#FFFFFF"
  overlay-light: "#FFFFFF"
  overlay-dark: "#000000"
  checker-a: "#CCCCCC"
  checker-b: "#FFFFFF"
typography:
  caption:
    fontFamily: Inter
    fontSize: 11px
    fontWeight: 400
  body:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: 400
  label:
    fontFamily: Inter SemiBold
    fontSize: 13px
    fontWeight: 600
  title:
    fontFamily: Inter SemiBold
    fontSize: 15px
    fontWeight: 600
  heading:
    fontFamily: Inter SemiBold
    fontSize: 18px
    fontWeight: 600
  mono:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: 400
rounded:
  control: 4px
  menu: 6px
  focus-ring: 6px
  window: 8px
spacing:
  item-x: 8px
  item-y: 4px
  button-x: 12px
  button-y: 4px
  control-height: 24px
  menu-margin: 6px
  window-margin: 16px
  tool-gap: 4px
  tool-button: 32px
components:
  button:
    backgroundColor: "{colors.bg-raised}"
    textColor: "{colors.text-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: 4px 12px
    height: 24px
  button-primary:
    backgroundColor: "{colors.active}"
    textColor: "{colors.on-active}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: 4px 12px
  tool-button:
    backgroundColor: transparent
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
    size: 32px
  tool-button-active:
    backgroundColor: "{colors.active}"
    textColor: "{colors.on-active}"
  layer-row-active:
    backgroundColor: "{colors.active}"
    textColor: "{colors.on-active}"
    height: 24px
    rounded: "{rounded.control}"
  window:
    backgroundColor: "{colors.bg-raised}"
    rounded: "{rounded.window}"
    padding: 16px
---

# Sistema de diseño: Pyrotecnika

> **App nativa, no web.** Pyrotecnika es Rust + egui 0.31 con backend wgpu; no hay HTML, CSS ni DOM. Los nombres de token de este documento corresponden a **campos de Rust** del `struct Palette` en `crates/pyro-ui/src/design/tokens.rs` (`bg-panel` ⇔ `Palette::bg_panel`, `on-active` ⇔ `Palette::on_active`…), no a variables CSS. El estilo de egui se genera desde esos campos en `design/style.rs`; los widgets propios están en `design/widgets.rs`. Fuera de `crates/pyro-ui/src/design/` el CI rechaza colores `Color32` literales (salvo `Color32::WHITE` como tinte de `painter.image` y `Color32::TRANSPARENT`).
>
> Fuente de verdad: el código construido y las capturas aprobadas en `crates/pyro-ui/tests/snapshots/`. Dirección: `docs/superpowers/specs/2026-11-16-fase1-sistema-diseno-design.md`.

## 1. Visión general

**Estrella polar: «el lienzo manda».** Un editor de imagen de escritorio convencional, ejecutado con oficio y sin rarezas de identidad. La interfaz es **estrictamente neutra** (grises, blanco y negro) para no alterar la percepción del color de la imagen. La vara es la facilidad de uso y la curva de aprendizaje de Macromedia Fireworks y Paint Shop Pro: su comportamiento, nunca su aspecto. El logotipo de `logo/` es solo el icono de la app y no es referencia visual.

Estructura de la ventana principal (captura `principal-oscuro.png`): barra de menús arriba; columna de herramientas a la izquierda (botones de 32×32 en dos columnas, agrupados por separadores); lienzo centrado sobre `bg-canvas`; panel de propiedades a la derecha (opciones de la herramienta y Capas, 280 px por defecto, redimensionable); barra de estado abajo con valores en monoespaciada.

Tres temas: **Seguir al sistema** (por defecto; si el SO no informa, oscuro), **Claro**, **Oscuro** y **Alto contraste** (un único tema fijo, blanco sobre negro, registrado como estilo oscuro). Se eligen en Ver › Tema. Sin persistencia hasta la fase 2.

## 2. Color

Todos los tokens son grises puros (r = g = b); lo comprueba la prueba `la_interfaz_es_neutra`. No hay color de acento, ni naranjas ni rojos, en ningún tema.

| Token (`Palette::…`) | Oscuro | Claro | Alto contraste | Uso |
|---|---|---|---|---|
| `bg_app` | `#161616` | `#EAEAEA` | `#000000` | Fondo de la ventana |
| `bg_panel` | `#1F1F1F` | `#F7F7F7` | `#000000` | Paneles, menú, barra de estado (`panel_fill`) |
| `bg_raised` | `#2A2A2A` | `#FFFFFF` | `#000000` | Controles en reposo, campos, ventanas, menús emergentes |
| `bg_hover` | `#303030` | `#EDEDED` | `#1A1A1A` | Hover |
| `bg_pressed` | `#333333` | `#DCDCDC` | `#333333` | Pulsado |
| `bg_canvas` | `#121212` | `#D6D6D6` | `#000000` | Alrededor del lienzo |
| `border_subtle` | `#333333` | `#DCDCDC` | `#FFFFFF` | Divisores, borde de ventana, contorno del lienzo |
| `border_control` | `#7A7A7A` | `#858585` | `#FFFFFF` | Bordes de controles |
| `text_primary` | `#EDEDED` | `#181818` | `#FFFFFF` | Texto e iconos; también avisos y errores |
| `text_secondary` | `#A6A6A6` | `#575757` | `#FFFFFF` | Etiquetas, ayudas, iconos de herramienta en reposo (= texto «débil» de egui) |
| `text_disabled` | `#6B6B6B` | `#9A9A9A` | `#8C8C8C` | Reservado para deshabilitado (hoy egui atenúa por su cuenta; exento de WCAG 1.4.3) |
| `active` | `#EDEDED` | `#181818` | `#FFFFFF` | Relleno de lo activo (inversión) |
| `on_active` | `#121212` | `#FFFFFF` | `#000000` | Texto e icono sobre `active` |
| `focus_ring` | `#FFFFFF` | `#000000` | `#FFFFFF` | Anillo de foco de teclado |
| `stroke_width` | 1 | 1 | **2** | Grosor de bordes y divisores |

**Constantes fuera de los temas** (no son interfaz, se dibujan sobre la imagen): `OVERLAY_LIGHT` blanco y `OVERLAY_DARK` negro, siempre en par; `CHECKER_A` gris 204 y `CHECKER_B` gris 255 para el tablero de transparencia (casillas de 8 px de pantalla a cualquier zoom).

**Contraste garantizado por prueba** (`las_tres_paletas_cumplen_wcag`): texto principal y secundario ≥ 4.5:1 sobre `bg_panel` y `bg_raised`; `on_active`/`active` ≥ 4.5:1; `active` contra panel, `border_control` sobre `bg_raised` y `focus_ring` sobre panel ≥ 3:1.

### Regla de inversión
Lo activo (herramienta, capa, acción principal de un diálogo) se marca **invirtiendo** fondo y tinta: relleno `active`, texto e icono `on_active`. Nunca con un matiz.

### Regla de severidad sin color
`Visuals::warn_fg_color` y `error_fg_color` son `text_primary`. Un aviso lleva el icono Lucide `triangle-alert`; un error, `circle-x`; ambos con el texto en Inter SemiBold. La información (`Tone::Info`) va sin icono y en regular.

### Regla de superposición en par
Toda forma dibujada sobre la imagen (guía de marco/elipse, recta con Mayús, contorno de la selección al alejar, marca del tampón) usa `overlay_stroke`: trazo negro de `w + 2` debajo y blanco de `w` encima, legible sobre cualquier foto. El anillo del pincel es un círculo blanco de 1 px con otro negro de 1 px justo fuera; las hormigas de la selección alternan tramos negros y blancos de 4 px.

## 3. Tipografía

Fuentes incluidas en el binario (subconjuntos latinos, OFL-1.1) en `crates/pyro-ui/assets/fonts/`, registradas en un solo `FontDefinitions` (`design/fonts.rs`) con las de egui como respaldo:

| Familia egui | Fuente |
|---|---|
| `Proportional` | Inter Regular |
| `Name("semibold")` | Inter SemiBold |
| `Monospace` | JetBrains Mono Regular |
| `Name("iconos")` | Subconjunto de Lucide (ISC) |

Escala a 100 % (`text_styles()`):

| Rol | `TextStyle` | Tamaño | Familia | Dónde |
|---|---|---|---|---|
| caption | `Small` | 11 | Inter | Textos auxiliares |
| body | `Body` | 13 | Inter | Texto corriente, filas de capa, mensajes |
| label | `Button` | 13 | Inter SemiBold | Botones y entradas de la barra de menús |
| title | `Name("title")` | 15 | Inter SemiBold | Títulos de sección del panel («Pincel», «Capas») |
| heading | `Heading` | 18 | Inter SemiBold | Título de ventanas y modales |
| mono | `Monospace` | 12 | JetBrains Mono | Todo valor numérico visible |

**Regla de números en monoespaciada:** tamaños, porcentajes, coordenadas, zoom, dimensiones, tamaños de archivo y códigos hexadecimales van en `Monospace`; `drag_value_text_style` es `Monospace`, así que los campos numéricos de los deslizadores también.

## 4. Espaciado, forma y elevación

Retícula de 4 px, en `design/style.rs`:

- `item_spacing` 8 × 4; `button_padding` 12 × 4; alto mínimo de control (`interact_size.y`) 24 (objetivo de clic WCAG 2.5.8).
- `window_margin` 16; `menu_margin` 6. Los paneles laterales (herramientas y propiedades) usan `side_panel_frame`, con margen interior de 12 (`PANEL_MARGIN`); la columna de herramientas mide 2 × 32 + 4 + 2 × 12 = 92.
- Columna de herramientas: botones de 32×32 con 4 px de separación, dos por fila; ancho fijo = 2 × 32 + 4 + 2 × `PANEL_MARGIN`. Grupos (mover · selección · pintura y retoque · relleno y muestreo) separados por divisores.
- Espacios verticales en diálogos: 4, 6, 8 y 12.

Radios: **4** controles, botones de herramienta y filas; **6** menús, tooltips, popovers y el anillo de foco; **8** ventanas y modales.

Elevación: paneles planos, sin sombra. Solo lo flotante (ventanas, modales, menús, tooltips) lleva una sombra neutra: desplazamiento (0, 4), desenfoque 14, negro al 35 % de opacidad (alfa 90). Ventanas con borde `border_subtle`.

## 5. Componentes y estados

Estados de los widgets de egui (`Visuals::widgets`), en los tres temas:

| Estado | Relleno | Borde | Tinta |
|---|---|---|---|
| Reposo (`inactive`) | `bg_raised` | `border_control` × `stroke_width` | `text_primary` |
| Hover (`hovered`, también `open`) | `bg_hover` | igual | igual |
| Pulsado (`active`) | `bg_pressed` | igual | igual |
| No interactivo | `bg_panel` | `border_subtle` | `text_primary` |
| Seleccionado / principal | `active` | `active` | `on_active` |
| Foco de teclado | anillo de 2 px en `focus_ring`, 3 px fuera del widget, radio 6 | | |

Sin expansión al hover. Texto seleccionado en campos: `active` al 25 % con trazo `focus_ring`. Cursor de texto: 2 px `text_primary`. Deslizadores con relleno del tramo recorrido (`slider_trailing_fill`), porque el riel vacío no se ve sobre el fondo de las ventanas.

Widgets propios (`design/widgets.rs`):

- **`button`**: botón de egui con anillo de foco. Es el botón por defecto de toda acción no principal.
- **`primary_button`**: invertido. Uno por diálogo, para la acción que lo cierra con éxito: Aplicar, Guardar…, Crear, Descargar e instalar, Reiniciar ahora. La confirmación de descartar cambios **no** lleva botón principal: sus tres opciones son normales y el foco inicial va a Cancelar, para que Intro o Espacio nunca descarten.
- **`tool_button`**: 32×32, icono Lucide de 18 px. Reposo: sin relleno, icono `text_secondary`. Hover o foco: `bg_hover`, icono `text_primary`. Activo: invertido. Tooltip «Nombre · atajo»; nombre accesible solo «Nombre», con estado de selección para AccessKit.
- **`active_row`**: fila de ancho completo, 24 de alto, texto con sangría de 8; invertida si es la capa activa, `bg_hover` al pasar el puntero. Recorta con «…» y entonces muestra el nombre completo en el tooltip.
- **Anillo de foco** (`paint_focus_ring`): una pasada al final de cada cuadro pinta 2 px en `focus_ring`, 3 px fuera del widget con foco (propio o de egui), en la capa de ese widget y recortado a su zona visible. Los campos de texto con foco llevan además borde en `focus_ring` (`selection.stroke`).
- **Selección de egui**: relleno `active` al 25 % y texto en `focus_ring`; texto encima ≥ 4.5:1 en los tres temas (prueba `la_seleccion_se_lee_y_el_campo_con_foco_tiene_borde`).
- **Paleta de comandos** (`app/palette.rs`): ventana modal de 520 de ancho anclada arriba al 15 % del alto, con campo de búsqueda y filas de 24: nombre a la izquierda, categoría y atajo en monoespaciada a la derecha. La fila elegida va invertida; las no disponibles, en `text_disabled` y sin poder elegirse.
- **Preferencias** (`app/prefs.rs`, `app/shortcuts.rs`): ventana modal de 640 con pestañas «Apariencia» y «Atajos» hechas con `active_row`. En Atajos, la lista agrupa por categoría con el atajo en monoespaciada («—» si no tiene); lo cambiado lleva «(modificado)» tras el nombre. El aviso de conflicto es texto con «⚠» y los botones Reasignar (principal) y Cancelar. «Cerrar» es el botón principal de la ventana.
- **Tarea en curso** (`app/tasks.rs`): en la barra de estado, antes del mensaje: el nombre de la tarea («Abriendo «x»…», «Aplicando «filtro»…»), una barra de progreso de 160 de ancho (relleno `selection.bg_fill`, es decir `active` al 25 %; animada si aún no hay porcentaje) y el botón «Cancelar»; Esc también cancela si no hay otra ventana abierta. Mientras dura, el panel de herramientas y capas se ve desactivado. En Exportar, la misma barra animada con «Calculando…» y «Cancelar» dentro del diálogo.
- **Avisos** (`app/guard.rs`): icono de severidad del tamaño del texto `Body` más texto semibold, en la barra de estado o en el diálogo; una línea recortada con el texto completo en tooltip.

Iconografía: Lucide como fuente tipográfica (no SVG), coloreada con los tokens de texto; todo icono tiene tooltip y nombre accesible.

## 6. Movimiento y escala

- `animation_time` 0,12 s (hover y pulsado). Nada se anima en el lienzo.
- **Tamaño de la interfaz** de 80 a 200 % en pasos de 10 % (`step_scale`, `ctx.set_zoom_factor`), **solo** desde Ver › Tamaño de la interfaz (Aumentar · Reducir · Restablecer). Ctrl/⌘ + `+`/`-`/`0` son el zoom de la imagen; `zoom_with_keyboard` de egui está desactivado.

## 7. Qué hacer y qué no

**Hacer**
- Leer los colores con `design::style::palette(ctx)` y usar sus campos; añadir un token nuevo a `Palette` en los tres temas antes de usarlo.
- Marcar lo activo por inversión; un único `primary_button` por diálogo.
- Poner los números en `Monospace`.
- Dibujar todo lo que va sobre la imagen en par blanco + negro (`overlay_stroke`), nunca con un token de tema.
- Dar nombre accesible a todo widget interactivo propio; el anillo de foco lo pone `paint_focus_ring` sin hacer nada más.
- Comprobar cualquier cambio visual con las capturas de referencia en los tres temas (`UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored` las regenera; las escenas están en `crates/pyro-ui/src/app/scenes.rs`).

**No hacer**
- Ningún matiz en la interfaz ni alrededor del lienzo: ni acento, ni naranja, ni rojo, ni colores tomados del logotipo.
- Indicar severidad, selección o estado solo con color.
- Escribir `Color32::from_rgb`/`from_gray`/`from_rgba*` o colores con nombre fuera de `crates/pyro-ui/src/design/`.
- Sombras en paneles fijos, o sombras de color.
- Atajos de teclado para el tamaño de la interfaz.
- Imitar el aspecto de Fireworks, Paint Shop Pro u otro producto: se toma su facilidad de uso, no su apariencia.
