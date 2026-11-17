# Fase 1 — Sistema de diseño neutro: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sustituir el tema por defecto de egui por un sistema de diseño neutro (grises, blanco y negro) con temas oscuro, claro y alto contraste, Inter/JetBrains Mono, escala de interfaz y capturas de referencia aprobadas.

**Architecture:** Un módulo nuevo `crates/pyro-ui/src/design/` concentra tokens, fuentes, estilo de egui por tema y los pocos widgets propios; `app/` deja de usar colores literales y consume ese módulo. Las capturas se hacen con `egui_kittest` en local (Windows) y el CI solo dibuja las mismas escenas sin comparar píxeles.

**Tech Stack:** Rust 1.85+, egui/eframe 0.31 (wgpu), egui_kittest 0.31 (`wgpu`, `snapshot`, `eframe`), fontTools (Python) para recortar fuentes.

**Spec:** `docs/superpowers/specs/2026-11-16-fase1-sistema-diseno-design.md` (léelo entero antes de empezar; las tablas de tokens y contrastes viven allí). Maqueta aprobada: `.impeccable/mocks/fase1/sistema-neutro.html`.

## Global Constraints

- Paleta solo neutra: grises, blanco y negro. Nada de naranjas, rojos ni colores del logo en la interfaz.
- Lo activo/seleccionado/principal se marca por **inversión** (`active` de fondo, `on_active` de texto e icono).
- Ningún widget usa colores literales fuera de `crates/pyro-ui/src/design/`; excepciones: `Color32::WHITE` como tinte neutro de `painter.image` y `Color32::TRANSPARENT`.
- Contraste WCAG 2.2 AA: texto ≥ 4.5:1, bordes de control y foco ≥ 3:1. El texto deshabilitado está exento.
- Severidad nunca solo por color: icono Lucide + texto semibold.
- Licencias: solo las de `deny.toml` (Inter y JetBrains Mono son OFL-1.1). `cargo deny check` debe pasar.
- `#[cfg(target_os)]` solo dentro de `crates/pyro-platform/` (regla del CI).
- Idioma de la interfaz: español. Textos de menú exactos: «Tema», «Seguir al sistema», «Claro», «Oscuro», «Alto contraste», «Tamaño de la interfaz», «Aumentar», «Reducir», «Restablecer (100 %)».
- Escala de interfaz: 80–200 % en pasos de 10 %, solo desde el menú (sin atajo).
- `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios en cada commit. Rust no está en el PATH: `export PATH="/c/Users/v_gar/.cargo/bin:$PATH"`.

## Review Focus

1. **Escala 200 % en la ventana mínima (900×560):** los paneles no deben solaparse ni dejar el lienzo con ancho cero; la app no entra en pánico. → prueba en la tarea 7.
2. **Cambiar de tema en caliente (Sistema → Alto contraste → Sistema):** al volver no debe quedar el estilo de alto contraste en el tema oscuro. → prueba en la tarea 3.
3. **Foco de teclado sobre un elemento invertido:** el anillo es del mismo color que el relleno; debe quedar fuera del elemento. → prueba en la tarea 4.
4. **Sistema que no informa su tema (Linux):** «Seguir al sistema» debe usar el oscuro, no el estilo por defecto de egui. → prueba en la tarea 3.
5. **Nombres largos (capa, archivo, aviso) a 150–200 %:** se recortan con «…» y tooltip, sin empujar los paneles. → escena con nombre largo en la tarea 7.

---

### Task 1: Tokens de color

**Files:**
- Create: `crates/pyro-ui/src/design/mod.rs`, `crates/pyro-ui/src/design/tokens.rs`
- Modify: `crates/pyro-ui/src/lib.rs` (añadir `mod design;`)

**Interfaces:**
- Produces: `pub(crate) struct Palette { bg_app, bg_panel, bg_raised, bg_hover, bg_pressed, bg_canvas, border_subtle, border_control, text_primary, text_secondary, text_disabled, active, on_active, focus_ring: Color32, stroke_width: f32 }`; constantes `pub(crate) const DARK: Palette`, `LIGHT`, `HIGH_CONTRAST` con los valores exactos de la tabla del spec §1 (`stroke_width` 1.0, 1.0, 2.0); constantes `pub(crate) const OVERLAY_LIGHT: Color32 = WHITE`, `OVERLAY_DARK = BLACK`, `CHECKER_A = from_gray(204)`, `CHECKER_B = from_gray(255)`; `pub(crate) fn contrast(a: Color32, b: Color32) -> f32` (razón WCAG).

- [ ] **Step 1: Write the failing tests** en `tokens.rs` (`#[cfg(test)] mod tests`):

```rust
#[test]
fn contraste_de_referencia() {
    assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
    assert!((contrast(Color32::from_rgb(0xED, 0xED, 0xED), Color32::from_rgb(0x1F, 0x1F, 0x1F)) - 14.08).abs() < 0.05);
}

#[test]
fn las_tres_paletas_cumplen_wcag() {
    for (name, p) in [("oscuro", DARK), ("claro", LIGHT), ("alto contraste", HIGH_CONTRAST)] {
        for (par, ratio, min) in [
            ("texto/panel", contrast(p.text_primary, p.bg_panel), 4.5),
            ("secundario/panel", contrast(p.text_secondary, p.bg_panel), 4.5),
            ("secundario/raised", contrast(p.text_secondary, p.bg_raised), 4.5),
            ("on_active/active", contrast(p.on_active, p.active), 4.5),
            ("active/panel", contrast(p.active, p.bg_panel), 3.0),
            ("borde/raised", contrast(p.border_control, p.bg_raised), 3.0),
            ("foco/panel", contrast(p.focus_ring, p.bg_panel), 3.0),
        ] {
            assert!(ratio >= min, "{name}: {par} = {ratio:.2} < {min}");
        }
    }
}

#[test]
fn la_interfaz_es_neutra() {
    for p in [DARK, LIGHT, HIGH_CONTRAST] {
        for c in [p.bg_app, p.bg_panel, p.bg_raised, p.bg_hover, p.bg_pressed, p.bg_canvas,
                  p.border_subtle, p.border_control, p.text_primary, p.text_secondary,
                  p.text_disabled, p.active, p.on_active, p.focus_ring] {
            assert!(c.r() == c.g() && c.g() == c.b(), "{c:?} no es gris");
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pyro-ui design::tokens`
Expected: error de compilación (`Palette`, `contrast` no existen).

- [ ] **Step 3: Implement `tokens.rs`** con las constantes del spec y `contrast` (luminancia relativa sRGB: canal/255, ≤0.03928 → /12.92, si no ((c+0.055)/1.055)^2.4; L = 0.2126R+0.7152G+0.0722B; razón (L1+0.05)/(L2+0.05)). `design/mod.rs` declara `pub(crate) mod tokens;` y documenta que es el único sitio con colores literales.

- [ ] **Step 4: Run tests** — `cargo test -p pyro-ui design::tokens` → 3 passed.

- [ ] **Step 5: Commit** — `git commit -m "feat(design): neutral color tokens with WCAG contrast tests"`

---

### Task 2: Fuentes y escala tipográfica

**Files:**
- Create: `crates/pyro-ui/assets/fonts/{Inter-Regular.ttf, Inter-SemiBold.ttf, JetBrainsMono-Regular.ttf, OFL-Inter.txt, OFL-JetBrainsMono.txt, README.md}`, `crates/pyro-ui/src/design/fonts.rs`
- Modify: `crates/pyro-ui/src/app/icons.rs` (quitar `install`; `FAMILY` pasa a `pub(crate)`), `crates/pyro-ui/src/app/mod.rs:93-96` (llamar a `design::fonts::install_fonts(&cc.egui_ctx)` en vez de `icons::install`)

**Interfaces:**
- Consumes: `icons::FAMILY` y el TTF de Lucide.
- Produces: `pub(crate) fn fonts() -> egui::FontDefinitions`; `pub(crate) fn install_fonts(ctx: &egui::Context)` (= `ctx.set_fonts(fonts())`); `pub(crate) const SEMIBOLD: &str = "semibold"`; `pub(crate) fn text_styles() -> BTreeMap<egui::TextStyle, egui::FontId>`; `pub(crate) fn title_style() -> egui::TextStyle` (= `TextStyle::Name("title".into())`).

- [ ] **Step 1: Obtener y recortar las fuentes.** Inter 4.x estática (Regular, SemiBold; `extras/ttf/` del zip de la release en github.com/rsms/inter) y JetBrains Mono 2.304 Regular (`fonts/ttf/` de la release en github.com/JetBrains/JetBrainsMono), descargadas a una carpeta nueva del scratchpad. Recortar cada una con `python -I -m fontTools.subset <ttf> --unicodes="U+0020-007E,U+00A0-00FF,U+0152-0153,U+2013-2014,U+2018-201E,U+2022,U+2026,U+2030,U+2039-203A,U+20AC,U+2190-2193,U+2212,U+00D7" --layout-features='*' --output-file=<destino>`. Copiar los textos de licencia OFL. El README documenta versión, origen y el comando, como `assets/icons/README.md`.

- [ ] **Step 2: Write the failing tests** en `fonts.rs`:

```rust
#[test]
fn familias_registradas() {
    let f = fonts();
    for fam in [FontFamily::Proportional, FontFamily::Monospace,
                FontFamily::Name(SEMIBOLD.into()), FontFamily::Name(icons::FAMILY.into())] {
        assert!(f.families.get(&fam).is_some_and(|v| !v.is_empty()), "{fam:?}");
    }
    assert_eq!(f.families[&FontFamily::Proportional][0], "inter");
}

#[test]
fn escala_tipografica_del_spec() {
    let s = text_styles();
    assert_eq!(s[&TextStyle::Small].size, 11.0);
    assert_eq!(s[&TextStyle::Body].size, 13.0);
    assert_eq!(s[&TextStyle::Button].size, 13.0);
    assert_eq!(s[&title_style()].size, 15.0);
    assert_eq!(s[&TextStyle::Heading].size, 18.0);
    assert_eq!(s[&TextStyle::Monospace].size, 12.0);
    assert_eq!(s[&TextStyle::Button].family, FontFamily::Name(SEMIBOLD.into()));
}
```

- [ ] **Step 3: Run tests** → falla al compilar.
- [ ] **Step 4: Implement `fonts()` y `text_styles()`.** Claves de `font_data`: `"inter"`, `"inter-semibold"`, `"jetbrains-mono"`, `icons::FAMILY`. Proportional = `["inter"]` seguido de las fuentes por defecto de egui como respaldo; Monospace = `["jetbrains-mono"]` + respaldo. Title/Heading y Button usan la familia semibold.
- [ ] **Step 5: Run tests** → pasan. `cargo build -p pyrotecnika` y comprobar que el binario release no crece más de 1 MB (`ls -la target/release/pyrotecnika.exe` antes/después).
- [ ] **Step 6: Commit** — `feat(design): bundle Inter and JetBrains Mono (OFL) and the type scale`

---

### Task 3: Estilo de egui por tema y selección de tema

**Files:**
- Create: `crates/pyro-ui/src/design/style.rs`
- Modify: `crates/pyro-ui/src/design/mod.rs`, `crates/pyro-ui/src/app/mod.rs` (`PyroApp::new` llama a `design::install(&ctx)`), `crates/pyro-ui/src/app/guard.rs:79-115` (avisos neutros)

**Interfaces:**
- Consumes: `Palette`, `DARK`, `LIGHT`, `HIGH_CONTRAST` (tarea 1); `fonts()`, `text_styles()` (tarea 2).
- Produces: `pub(crate) fn style(p: &Palette) -> egui::Style`; `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub(crate) enum ThemeChoice { System, Light, Dark, HighContrast }`; `pub(crate) fn apply_theme(ctx: &egui::Context, choice: ThemeChoice)`; `pub(crate) fn install(ctx: &egui::Context)` (`install_fonts` + `apply_theme(System)`; sustituye a la llamada de la tarea 2 en `PyroApp::new`); `pub(crate) fn palette(ctx: &egui::Context) -> &'static Palette` (la paleta del estilo vigente; los widgets y el lienzo la usan para leer tokens).

- [ ] **Step 1: Write the failing tests** en `style.rs`:

```rust
#[test]
fn estados_desde_los_tokens() {
    let s = style(&DARK);
    let w = &s.visuals.widgets;
    assert_eq!(w.inactive.bg_fill, DARK.bg_raised);
    assert_eq!(w.hovered.bg_fill, DARK.bg_hover);
    assert_eq!(w.active.bg_fill, DARK.bg_pressed);
    assert_eq!(w.inactive.bg_stroke.color, DARK.border_control);
    assert_eq!(s.visuals.panel_fill, DARK.bg_panel);
    assert_eq!(s.visuals.window_fill, DARK.bg_raised);
    assert_eq!(s.visuals.warn_fg_color, DARK.text_primary);
    assert_eq!(s.visuals.error_fg_color, DARK.text_primary);
    assert_eq!(s.visuals.selection.stroke.color, DARK.on_active);
    assert!(s.visuals.dark_mode);
    assert!(!style(&LIGHT).visuals.dark_mode);
    assert_eq!(style(&HIGH_CONTRAST).visuals.widgets.inactive.bg_stroke.width, 2.0);
}

#[test]
fn espaciado_y_forma() {
    let s = style(&DARK);
    assert_eq!(s.spacing.item_spacing, egui::vec2(8.0, 4.0));
    assert_eq!(s.spacing.button_padding, egui::vec2(12.0, 4.0));
    assert_eq!(s.visuals.widgets.inactive.corner_radius, egui::CornerRadius::same(4));
    assert_eq!(s.visuals.window_corner_radius, egui::CornerRadius::same(8));
    assert_eq!(s.visuals.menu_corner_radius, egui::CornerRadius::same(6));
    assert_eq!(s.animation_time, 0.12);
}

#[test]
fn alto_contraste_y_vuelta_al_sistema() {
    let ctx = egui::Context::default();
    apply_theme(&ctx, ThemeChoice::HighContrast);
    assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::Dark);
    assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, HIGH_CONTRAST.bg_panel);
    apply_theme(&ctx, ThemeChoice::System);
    assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::System);
    assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, DARK.bg_panel);
    assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, LIGHT.bg_panel);
}

#[test]
fn sin_tema_del_sistema_usa_el_oscuro() {
    let ctx = egui::Context::default();
    apply_theme(&ctx, ThemeChoice::System);
    assert_eq!(ctx.options(|o| o.fallback_theme), egui::Theme::Dark);
    let _ = ctx.run(Default::default(), |_| {});
    assert_eq!(ctx.style().visuals.panel_fill, DARK.bg_panel);
}
```

- [ ] **Step 2: Run tests** → fallan.
- [ ] **Step 3: Implement `style`, `apply_theme`, `install`, `palette`.** `style` parte de `egui::Style::default()` (con `Visuals::dark()`/`light()` según `p.active == DARK.active`/claro), asigna `text_styles()`, los fondos y los cinco estados de `widgets` (`noninteractive`: `bg_panel`, borde `border_subtle`, texto `text_secondary`; `open` = `hovered`), `selection.bg_fill = p.active` con alfa 0.35 y `selection.stroke.color = p.on_active`, `extreme_bg_color = p.bg_raised`, sombras de ventana y popup neutras (`Color32::from_black_alpha(90)`), `stroke_width` en todos los bordes, y los radios/espaciados del test. `palette(ctx)` compara `ctx.style().visuals.panel_fill` contra las tres paletas. En `guard.rs`, `tone_icon` deja el color en `ui.visuals().text_color()` (ya es `text_primary`) y el texto de aviso/error va en `FontFamily::Name(SEMIBOLD)`.
- [ ] **Step 4: Run tests** → pasan; `cargo run -p pyrotecnika` abre con el tema del sistema.
- [ ] **Step 5: Commit** — `feat(design): egui style per theme, theme switching, neutral notices`

---

### Task 4: Widgets propios e inversión de lo activo

**Files:**
- Create: `crates/pyro-ui/src/design/widgets.rs`
- Modify: `crates/pyro-ui/src/app/panels.rs:131-143` (`tool_button`), `panels.rs:231-300` (`layers_ui`), `crates/pyro-ui/src/app/dialogs.rs:201,331,359` (Aplicar, Guardar…, Crear), `crates/pyro-ui/src/app/updates.rs` (Descargar e instalar, Reiniciar ahora), `crates/pyro-ui/src/app/guard.rs` (Exportar primero… sigue normal; Cancelar normal), `crates/pyro-ui/src/app/mod.rs` (`update` llama a `design::paint_focus_ring(ctx)` al final)

**Interfaces:**
- Consumes: `palette(ctx)`, `Palette`.
- Produces:
  - `pub(crate) fn tool_button(ui: &mut egui::Ui, glyph: char, selected: bool, name: &str, shortcut: &str) -> egui::Response` — 32×32, icono 18 px, invertido si `selected`; tooltip `"{name} · {shortcut}"`; nombre accesible `name` (sin atajo).
  - `pub(crate) fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response` — botón invertido (`active`/`on_active`), semibold.
  - `pub(crate) fn active_row(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response` — fila seleccionable de ancho completo, invertida si `selected`; el texto se recorta con «…» y el completo va en el tooltip.
  - `pub(crate) fn focus_ring_rect(widget: egui::Rect) -> egui::Rect` = `widget.expand(3.0)` (2 px de separación + medio trazo de 2 px).
  - `pub(crate) fn paint_focus_ring(ctx: &egui::Context)` — si hay widget con foco y el último evento de entrada fue de teclado (Tab/flechas), pinta en `Order::Foreground` un `rect_stroke` de 2 px en `focus_ring` sobre `focus_ring_rect(rect)`; el rect se guarda con `ctx.data_mut` desde cada widget que pide foco (helper `pub(crate) fn remember_focus(resp: &egui::Response)` llamado por los tres widgets y por `ui.button` vía un trait de extensión `ResponseExt::with_focus_ring(self) -> Self` para los botones normales).
  - `Tool::shortcut(self) -> &'static str` en `app/tools.rs` (hoy el atajo está dentro de `label()`, p. ej. "Pincel (B)"); `label()` pasa a devolver solo el nombre.

- [ ] **Step 1: Write the failing tests** (`widgets.rs`, usan `egui_kittest`, ver tarea 7 para añadir la dependencia; en esta tarea añádela ya como dev-dependency `egui_kittest = { version = "0.31", features = ["wgpu", "snapshot", "eframe"] }` en `crates/pyro-ui/Cargo.toml`):

```rust
#[test]
fn el_anillo_de_foco_queda_fuera_del_widget() {
    let r = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(32.0, 32.0));
    let ring = focus_ring_rect(r);
    assert!(ring.contains_rect(r));
    assert_eq!(ring.min, egui::pos2(7.0, 7.0));
}

#[test]
fn boton_de_herramienta_accesible_sin_atajo() {
    let mut harness = egui_kittest::Harness::new_ui(|ui| {
        crate::design::install(ui.ctx());
        tool_button(ui, '\u{e2e7}', true, "Pincel", "B");
    });
    harness.run();
    let node = harness.get_by_label("Pincel");
    assert_eq!(node.is_selected(), Some(true));
}
```

- [ ] **Step 2: Run tests** → fallan.
- [ ] **Step 3: Implement los widgets** y sustituir los usos listados en **Files**. Además, los valores numéricos visibles pasan a `TextStyle::Monospace` (spec §2): tamaño, dureza, opacidad y tolerancia en `tool_options`, dimensiones, zoom y coordenadas en `status_bar`, y tamaños de archivo en el diálogo de exportar. En `layers_ui`, cada fila es `[casilla visible] + active_row(...)`; la casilla lleva nombre accesible «Visible: {nombre}». Los botones de acción de capa siguen como botones normales.
- [ ] **Step 4: Run tests** → pasan. `cargo run`: el pincel activo se ve blanco sobre gris (oscuro), Tab recorre los botones de un diálogo con el anillo visible, también sobre «Aplicar».
- [ ] **Step 5: Commit** — `feat(design): inverted active state, primary button, keyboard focus ring`

---

### Task 5: Menú Ver › Tema y Tamaño de la interfaz

**Files:**
- Modify: `crates/pyro-ui/src/app/mod.rs` (campos `theme: ThemeChoice`, `ui_scale: f32`; variantes de `Action`), `crates/pyro-ui/src/app/panels.rs` (menú Ver), `crates/pyro-ui/src/design/style.rs` (función de escala)

**Interfaces:**
- Consumes: `ThemeChoice`, `apply_theme` (tarea 3).
- Produces: `pub(crate) fn step_scale(current: f32, delta: i32) -> f32` (pasos de 0.1, límites 0.8 y 2.0, redondeo a 1 decimal); `Action::SetTheme(ThemeChoice)`, `Action::UiScaleUp`, `Action::UiScaleDown`, `Action::UiScaleReset`.

- [ ] **Step 1: Write the failing test** en `style.rs`:

```rust
#[test]
fn escala_de_la_interfaz() {
    assert_eq!(step_scale(1.0, 1), 1.1);
    assert_eq!(step_scale(1.0, -1), 0.9);
    assert_eq!(step_scale(2.0, 1), 2.0);
    assert_eq!(step_scale(0.8, -1), 0.8);
    assert_eq!(step_scale(1.3, -3), 1.0);
}
```

- [ ] **Step 2: Run** → falla.
- [ ] **Step 3: Implement.** Submenú «Tema» con `ui.radio_value` para las cuatro opciones; submenú «Tamaño de la interfaz» con «Aumentar», «Reducir», «Restablecer (100 %)» y una etiqueta débil con el porcentaje actual. `run()` aplica `apply_theme` o `ctx.set_zoom_factor(step_scale(...))`. Estas acciones se permiten aunque haya un filtro abierto (añadirlas a la lista de excepciones de `run`, junto al zoom). Sin atajos de teclado.
- [ ] **Step 4: Run** → pasa; en la app, los cuatro temas y la escala cambian en caliente.
- [ ] **Step 5: Commit** — `feat(ui): View menu theme and interface size`

---

### Task 6: Superposiciones del lienzo y regla de colores literales

**Files:**
- Modify: `crates/pyro-ui/src/app/canvas.rs:65-145`, `crates/pyro-ui/src/app/view.rs:61-66,88`, `.github/workflows/ci.yml` (job `lint`)

**Interfaces:**
- Consumes: `palette(ctx).bg_canvas`, `OVERLAY_LIGHT`, `OVERLAY_DARK`, `CHECKER_A`, `CHECKER_B`.
- Produces: `pub(crate) fn overlay_stroke(painter: &egui::Painter, shape: impl Fn(egui::Stroke) -> egui::Shape)` en `design/widgets.rs` — dibuja la forma dos veces: trazo negro de 3 px y encima blanco de 1 px (legible sobre cualquier imagen).

- [ ] **Step 1: Añadir la regla al CI** (job `lint`, tras la de `cfg`):

```yaml
      - name: Sin colores literales fuera del sistema de diseño
        run: |
          if grep -rnE 'Color32::(from_|[A-Z]{3,})' --include='*.rs' crates app \
             | grep -v '^crates/pyro-ui/src/design/' \
             | grep -vE 'Color32::(WHITE|TRANSPARENT)\b'; then
            echo "::error::Usa los tokens de crates/pyro-ui/src/design"; exit 1
          fi
```

- [ ] **Step 2: Ejecutar la regla en local** (mismo pipeline en Git Bash) → lista los usos de `canvas.rs` y `view.rs` (falla).
- [ ] **Step 3: Implement.** Fondo del lienzo: `palette.bg_canvas`. Borde del documento: `border_subtle`. Guía de forma, recta y rectángulo de selección a zoom < 1: `overlay_stroke`. Marca del tampón: dos `line_segment` con `overlay_stroke`. Anillo del pincel: ya es blanco + negro, pasa a las constantes. Tablero: `CHECKER_A/B`. Hormigas: `OVERLAY_DARK/LIGHT`.
- [ ] **Step 4: Verificar** → la regla no imprime nada; `cargo test --workspace` pasa (la prueba existente de `selection_overlay` sigue igual).
- [ ] **Step 5: Commit** — `refactor(ui): canvas overlays from design tokens; CI rule against literal colors`

---

### Task 7: Escenas y capturas de referencia

**Files:**
- Create: `crates/pyro-ui/src/app/scenes.rs` (`#[cfg(test)]`), `crates/pyro-ui/tests/snapshots/*.png` (las genera la prueba)
- Modify: `crates/pyro-ui/src/app/mod.rs` (`#[cfg(test)] mod scenes;` y un constructor `#[cfg(test)] pub(super) fn for_tests(ctx: &egui::Context) -> Self` que no llama a `pyro_platform`)

*Decisión del plan:* las escenas viven dentro del crate (`#[cfg(test)]`) y no en `tests/`, porque necesitan abrir diálogos y fijar estado privado de `PyroApp`; las imágenes sí se guardan en `crates/pyro-ui/tests/snapshots/`, como dice el spec.

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: `fn scene(name: Scene, theme: ThemeChoice, scale: f32, size: egui::Vec2) -> egui_kittest::Harness<'static, PyroApp>` y `enum Scene { Main, Export, Filter, Confirm, Updates, LongNames }`. Documento sintético: 800×533, dos capas («Fondo» con degradado cielo/tierra, «Retoque cielo» activa al 85 %), selección rectangular 230×150 en (120, 90).

- [ ] **Step 1: Write the smoke test (no ignorado, corre en el CI de los tres SO):**

```rust
#[test]
fn todas_las_escenas_se_dibujan() {
    for theme in [ThemeChoice::Dark, ThemeChoice::Light, ThemeChoice::HighContrast] {
        for s in Scene::ALL {
            let mut h = scene(s, theme, 1.0, egui::vec2(1280.0, 800.0));
            h.run();
        }
    }
}

#[test]
fn ventana_minima_al_200_por_ciento() {
    let mut h = scene(Scene::Main, ThemeChoice::Dark, 2.0, egui::vec2(900.0, 560.0));
    h.run();
    let canvas = h.get_by_label("Lienzo");
    assert!(canvas.bounds().unwrap().width() > 0.0);
}
```

(Para la segunda, el área del lienzo en `canvas_ui` necesita `widget_info` con nombre «Lienzo».)

- [ ] **Step 2: Write the snapshot tests (ignorados):** una por escena y tema con `#[ignore = "capturas de referencia: cargo test -p pyro-ui -- --ignored"]`, nombre de archivo `{escena}-{tema}` (p. ej. `main-oscuro`), más `main-oscuro-150`; usan `harness.snapshot_options(name, &SnapshotOptions::new().threshold(<valor del implementador>))`.
- [ ] **Step 3: Implement `scenes.rs` y `for_tests`.** Cada escena abre su diálogo con el mismo estado que produciría el uso real (p. ej. `Export`: `run(Action::Export)` y esperar a que termine el trabajo de codificación con `harness.run_steps` hasta que `export_dialog.bytes.is_some()`; `Confirm`: `history` modificado + `request(Pending::Quit)`; `Updates`: `update = Some(UpdateState::UpToDate)`; `LongNames`: capa «Retoque del cielo con nubes de tormenta sobre el cenote, versión final 3» y aviso largo en la barra de estado).
- [ ] **Step 4: Run** `cargo test -p pyro-ui` → smoke tests pasan. Luego `UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored` → genera las PNG; `cargo test -p pyro-ui -- --ignored` sin la variable → pasan.
- [ ] **Step 5: Revisar las capturas.** Abrir cada PNG y comparar con la maqueta; corregir lo que no cuadre en la tarea correspondiente y regenerar. Después **mostrárselas al autor** para su aprobación (criterio de salida de la fase); no continuar a la tarea 8 sin ella.
- [ ] **Step 6: Commit** — `test(ui): reference snapshots for the design system`

---

### Task 8: Documentación

**Files:**
- Modify: `docs/ARQUITECTURA.md` §4.1 (tabla del spec en lugar de la del acento naranja, con contrastes), §4.2 (escala solo por menú, motivo de AltGr), §4.5 (inversión), §9.1 (fase 1 ✔); `PRODUCT.md` (hay logo en `logo/`, solo para el icono de la app; compromisos de marca: paleta neutra, estándar de la categoría, Fireworks y Paint Shop Pro como vara de facilidad de uso; quitar "No hay logotipo")
- Create (vía Impeccable): `DESIGN.md`, `.impeccable/design.json`

- [ ] **Step 1:** Editar `ARQUITECTURA.md` y `PRODUCT.md` como se indica.
- [ ] **Step 2:** Generar `DESIGN.md` y `.impeccable/design.json` con el documentador de Impeccable (`impeccable-documenter`, entradas: raíz del proyecto, `crates/pyro-ui/src/design/`, el spec como contrato de dirección, `PRODUCT.md`, límite de escritura: esos dos archivos).
- [ ] **Step 3:** Verificación final: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, regla de colores del CI; todo limpio.
- [ ] **Step 4: Commit** — `docs: neutral design system in architecture, product and DESIGN.md`
