# Fase 2: comandos, atajos y preferencias. Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un catálogo único de comandos con los atajos de Fireworks CS6 de fábrica, configurables desde Preferencias, una paleta Ctrl+K, recientes y preferencias persistentes en TOML.

**Architecture:** Un módulo nuevo `crates/pyro-ui/src/commands/` contiene la combinación de teclas (`Chord`), el catálogo (`CATALOG`), el mapa vigente (`Keymap`) y el almacenamiento (`Store`, `Settings`, `KeymapFile`). `app/` deja de tener atajos fijos: el teclado, los menús, la paleta y el editor leen del catálogo y del mapa. Todo comando acaba en `PyroApp::run(Action)`. Lo específico de cada SO (carpeta de configuración, Mostrar en carpeta) vive en `pyro-platform`.

**Tech Stack:** Rust, egui/eframe 0.31, egui_kittest 0.31. Dependencias nuevas: `serde` (derive), `toml` y `tempfile` (dev) en `pyro-ui`; `directories` en `pyro-platform`.

**Spec:** `docs/superpowers/specs/2026-11-17-fase2-comandos-preferencias-design.md`. Léelo entero; la tabla de atajos de fábrica está en su §6.1.

## Global Constraints

- Atajos de fábrica: exactamente la tabla del spec §6.1. `Ctrl` es `Modifiers::COMMAND` (⌘ en macOS).
- Formato de combinación en disco: `Ctrl`, `Alt`, `Shift` y el nombre de la tecla de `egui::Key::name()`, unidos con `+` (p. ej. `Ctrl+Shift+R`, `Ctrl+Plus`, `OpenBracket`). En pantalla, en Windows y Linux: `Ctrl`, `Alt`, `Mayús`, `Supr`, `Retroceso`, `+`, `=`, `-`, `[`, `]`; en macOS: `⌘⌥⇧` seguidos de la tecla.
- Una combinación sin modificadores solo vale para herramientas y para los comandos que la tienen de fábrica (Supr, Retroceso, `[`, `]`). Mayús cuenta como modificador.
- Archivos: `settings.toml` y `keymap.toml`, ambos con `schema = 1`, en `pyro_platform::paths::config_dir()`. Escritura atómica. Un archivo ilegible se renombra a `*.toml.bak`; un `schema` mayor se copia a `*.toml.bak` antes de la primera sobrescritura. En los dos casos se avisa.
- Recientes: máximo 10, el más reciente primero, sin duplicados; solo archivos abiertos con éxito.
- Interfaz en español. Textos exactos: menú «Capas», «Abrir reciente», «Limpiar lista», «Mostrar en carpeta», «Preferencias…», «Paleta de comandos», secciones «Apariencia» y «Atajos», botones «+ Añadir», «Restablecer», «Restablecer todos a Fireworks», «Exportar…», «Importar…», «Reasignar», «Cancelar».
- Sistema de diseño de la fase 1: ningún `Color32` literal fuera de `crates/pyro-ui/src/design/`; la acción principal usa `widgets::primary_button`; todo widget propio lleva nombre accesible.
- `cfg(target_os)` solo en `crates/pyro-platform/`. Licencias según `deny.toml` (`cargo deny check`).
- Ningún archivo de más de ~600 líneas.
- `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios en cada commit. Los literales de ancho de trazo y similares llevan `_f32` (el rustc del CI rechaza el *fallback* de flotantes). Rust no está en el PATH: `export PATH="/c/Users/v_gar/.cargo/bin:$PATH"`.
- Las pruebas nunca tocan la carpeta de configuración real: usan `tempfile::tempdir()` o un `Store` sin carpeta.

## Review Focus

1. **Teclado español:** Ctrl con la tecla `+` llega como `Key::Plus` y debe acercar igual que `Ctrl+=`. Un carácter AltGr escrito en un campo de texto (`€` = AltGr+E) no debe combinar capas. → prueba en la tarea 5.
2. **`keymap.toml` editado a mano con basura** (tecla inexistente, id desconocido, dos comandos con la misma combinación): la app arranca, aplica lo válido y avisa. → prueba en la tarea 3.
3. **Sin carpeta de configuración o sin permiso de escritura:** la app funciona; al fallar el guardado avisa una vez y no se cierra. → prueba en la tarea 4.
4. **Un reciente que ya no existe:** al elegirlo, aviso en la barra de estado y se quita de la lista, sin tocar el documento abierto. → prueba en la tarea 5.
5. **Esc durante la captura de un atajo** cancela la captura sin cerrar Preferencias; la captura no ejecuta el comando que ya tenía esa combinación. → prueba en la tarea 8.

---

### Task 1: Combinación de teclas (`Chord`)

**Files:**
- Create: `crates/pyro-ui/src/commands/mod.rs`, `crates/pyro-ui/src/commands/chord.rs`
- Modify: `crates/pyro-ui/src/lib.rs` (`mod commands;`)

**Interfaces:**
- Produces: `#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)] pub(crate) struct Chord { pub ctrl: bool, pub alt: bool, pub shift: bool, pub key: egui::Key }`.
  - `Chord::parse(s: &str) -> Result<Chord, String>`.
  - `Chord::to_config(&self) -> String`.
  - `Chord::label(&self, mac: bool) -> String`.
  - `Chord::modifiers(&self) -> egui::Modifiers`, con `ctrl` como `COMMAND`.
  - `Chord::has_modifier(&self) -> bool`; Mayús cuenta.
  - `Chord::pressed(&self, i: &egui::InputState) -> bool`: la tecla se pulsó en este cuadro y `i.modifiers.matches_exact(self.modifiers())`.
  - `pub(crate) fn is_mac() -> bool` en `commands/mod.rs`, basado en `pyro_platform::IS_MAC` (constante nueva en `pyro-platform/src/lib.rs`, la única con `cfg`).

- [ ] **Step 1: Write the failing tests** en `chord.rs`:

```rust
#[test]
fn ida_y_vuelta() {
    for s in ["Ctrl+Shift+R", "Ctrl+Alt+0", "Shift+L", "M", "Delete", "Ctrl+Plus", "Ctrl+Equals", "OpenBracket"] {
        assert_eq!(Chord::parse(s).unwrap().to_config(), s);
    }
}
#[test]
fn orden_y_mayusculas_indiferentes() {
    assert_eq!(Chord::parse("shift+ctrl+r").unwrap(), Chord::parse("Ctrl+Shift+R").unwrap());
}
#[test]
fn invalidas() {
    for s in ["", "Ctrl+", "Ctrl+Nada", "Hyper+R", "Ctrl+R+T"] {
        assert!(Chord::parse(s).is_err(), "{s}");
    }
}
#[test]
fn etiquetas() {
    let c = Chord::parse("Ctrl+Shift+R").unwrap();
    assert_eq!(c.label(false), "Ctrl+Mayús+R");
    assert_eq!(c.label(true), "⌘⇧R");
    assert_eq!(Chord::parse("Delete").unwrap().label(false), "Supr");
    assert_eq!(Chord::parse("Backspace").unwrap().label(false), "Retroceso");
    assert_eq!(Chord::parse("Ctrl+Plus").unwrap().label(false), "Ctrl++");
    assert_eq!(Chord::parse("Ctrl+Alt+0").unwrap().label(false), "Ctrl+Alt+0");
}
#[test]
fn mayus_es_modificador() {
    assert!(Chord::parse("Shift+L").unwrap().has_modifier());
    assert!(!Chord::parse("M").unwrap().has_modifier());
}
```

- [ ] **Step 2: Run** `cargo test -p pyro-ui --lib commands::chord`. Expected: FAIL (no compila: `Chord` no existe).
- [ ] **Step 3: Implement** `Chord`. La tecla se analiza con `egui::Key::from_name`. Usa `matches_exact`, nunca `consume_key`, que admite Mayús de más.
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat(commands): key chord parsing and labels`.

### Task 2: Catálogo de comandos

**Files:**
- Create: `crates/pyro-ui/src/commands/catalog.rs`
- Modify: `crates/pyro-ui/src/app/mod.rs`. `Action` gana `#[derive(Clone, Copy, PartialEq, Debug)]` y estas variantes:
  - `LayerNew`, `LayerDuplicate`, `LayerDelete`, `LayerUp`, `LayerDown`, `LayerMergeDown`;
  - `SelectTool(Tool)`, `BrushSmaller`, `BrushBigger`;
  - `OpenPalette`, `OpenPreferences`;
  - `OpenRecent(usize)`, `ClearRecent`, `RevealInFolder`.
- Modify: `crates/pyro-ui/src/app/tools.rs` (`TOOLS` pierde la tecla; queda `(Tool, &str)`).

**Interfaces:**
- Produces: `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub(crate) enum Category { File, Edit, Select, Layers, Filters, View, Tools, Brush, Help }` con `fn label(self) -> &'static str` («Archivo», «Editar», «Selección», «Capas», «Filtros», «Ver», «Herramientas», «Pincel», «Ayuda»).
- Produces: `pub(crate) struct CommandDef { pub id: &'static str, pub name: &'static str, pub category: Category, pub action: Action, pub defaults: &'static [&'static str] }`.
- Produces: `pub(crate) static CATALOG: &[CommandDef]`.
  - Contiene todos los comandos de las tablas §6.1 y §6.2 del spec.
  - Los ids siguen `categoría.acción`: `file.new`, `file.open`, `file.export`, `file.quit`, `file.reveal`, `file.clear_recent`, `edit.undo`, `edit.redo`, `edit.clear`, `edit.crop`, `edit.palette`, `edit.preferences`, `select.all`, `select.none`, `select.invert`, `layer.new`, `layer.duplicate`, `layer.delete`, `layer.up`, `layer.down`, `layer.merge_down`, `filter.<nombre>`, `view.zoom_in`, `view.zoom_out`, `view.fit`, `view.actual_size`, `view.theme_<x>`, `view.ui_scale_<x>`, `tool.<nombre>`, `brush.smaller`, `brush.bigger`, `help.updates`.
  - Herramientas en el orden de `TOOLS`; `tool.marquee` y `tool.ellipse` llevan `M` los dos.
  - Abrir reciente no es un comando del catálogo: lo construyen el menú y la paleta (tareas 5 y 7).
- Produces: `pub(crate) fn find(id: &str) -> Option<&'static CommandDef>`, `pub(crate) fn by_action(a: Action) -> Option<&'static CommandDef>`.
- Produces: `pub(crate) const PLAIN_KEY_DEFAULTS: &[&str] = &["Delete", "Backspace", "OpenBracket", "CloseBracket"]` (sin modificador, fuera de las herramientas).

- [ ] **Step 1: Write the failing tests** en `catalog.rs`:

```rust
#[test]
fn atajos_de_fabrica_de_fireworks() {
    let want: &[(&str, &[&str])] = &[
        ("file.new", &["Ctrl+N"]), ("file.open", &["Ctrl+O"]), ("file.export", &["Ctrl+Shift+R"]),
        ("file.quit", &["Ctrl+Q"]), ("edit.undo", &["Ctrl+Z"]), ("edit.redo", &["Ctrl+Shift+Z"]),
        ("edit.clear", &["Delete", "Backspace"]), ("edit.palette", &["Ctrl+K"]),
        ("select.all", &["Ctrl+A"]), ("select.none", &["Ctrl+D"]), ("select.invert", &["Ctrl+Shift+I"]),
        ("layer.new", &["Shift+L"]), ("layer.duplicate", &["Ctrl+Alt+D"]), ("layer.up", &["Ctrl+Shift+F"]),
        ("layer.down", &["Ctrl+Shift+B"]), ("layer.merge_down", &["Ctrl+Alt+E"]),
        ("view.zoom_in", &["Ctrl+Equals", "Ctrl+Plus"]), ("view.zoom_out", &["Ctrl+Minus"]),
        ("view.fit", &["Ctrl+Alt+0"]), ("view.actual_size", &["Ctrl+1"]),
        ("tool.move", &["V"]), ("tool.marquee", &["M"]), ("tool.ellipse", &["M"]), ("tool.lasso", &["L"]),
        ("tool.wand", &["W"]), ("tool.brush", &["B"]), ("tool.eraser", &["E"]), ("tool.clone", &["S"]),
        ("tool.blur", &["R"]), ("tool.bucket", &["G"]), ("tool.eyedropper", &["I"]),
        ("brush.smaller", &["OpenBracket"]), ("brush.bigger", &["CloseBracket"]),
    ];
    for (id, chords) in want {
        assert_eq!(find(id).unwrap_or_else(|| panic!("falta {id}")).defaults, *chords, "{id}");
    }
    for c in CATALOG.iter().filter(|c| !want.iter().any(|(id, _)| id == &c.id)) {
        assert!(c.defaults.is_empty(), "{} no tiene atajo de fábrica en el spec", c.id);
    }
}
#[test]
fn ids_unicos_y_atajos_validos() {
    let mut seen = std::collections::HashSet::new();
    for c in CATALOG {
        assert!(seen.insert(c.id), "id repetido {}", c.id);
        for d in c.defaults { Chord::parse(d).unwrap_or_else(|e| panic!("{}: {e}", c.id)); }
    }
}
#[test]
fn todas_las_herramientas_y_filtros_estan() {
    for (tool, _) in crate::app::tools::TOOLS { assert!(by_action(Action::SelectTool(tool)).is_some()); }
    assert!(CATALOG.iter().filter(|c| c.category == Category::Filters).count() == 7);
}
```

- [ ] **Step 2: Run** `cargo test -p pyro-ui --lib commands::catalog`. Expected: FAIL (no compila).
- [ ] **Step 3: Implement** el catálogo con los nombres visibles de los menús actuales («Nuevo…», «Exportar…», «Combinar abajo», «Desenfoque gaussiano…»…). Añade las variantes de `Action` con su brazo en `run()`:
  - las de capa usan la misma secuencia que los botones de hoy (`before = doc.clone()`, la operación, `history.push(&before)` si cambió, `canvas_dirty`), extraída a `fn layer_op(&mut self, f: fn(&mut Document) -> bool)`;
  - `SelectTool`, `BrushSmaller` y `BrushBigger` (±2.0, acotado a 0.5–250) mueven aquí la lógica de `input.rs`;
  - el resto queda con `todo!()` solo hasta su tarea.
  - **No** cambies todavía `input.rs` ni los menús.
- [ ] **Step 4: Run** `cargo test -p pyro-ui`. Expected: PASS, sin regresiones.
- [ ] **Step 5: Commit** `feat(commands): command catalog with Fireworks CS6 defaults`.

### Task 3: Mapa de atajos (`Keymap`) y `keymap.toml`

**Files:**
- Create: `crates/pyro-ui/src/commands/keymap.rs`

**Interfaces:**
- Consumes: `Chord`, `CATALOG`, `find`, `PLAIN_KEY_DEFAULTS`.
- Produces:
  - `#[derive(Clone, PartialEq, Debug)] pub(crate) struct Keymap`, con el mapa interno `id → Vec<Chord>`.
  - `Keymap::fireworks() -> Keymap`.
  - `Keymap::chords(&self, id: &str) -> &[Chord]`.
  - `Keymap::label(&self, id: &str, mac: bool) -> String`: la primera combinación, o `""`.
  - `Keymap::allowed(id: &str, c: Chord) -> bool`: la regla de la tecla sin modificador.
  - `Keymap::conflict(&self, id: &str, c: Chord) -> Option<&'static str>`: el id que ya la usa. Dos herramientas con la misma tecla sin modificador no cuentan como conflicto.
  - `Keymap::assign(&mut self, id: &str, c: Chord)`: se la quita a quien la tuviera, salvo herramienta con herramienta.
  - `Keymap::remove(&mut self, id: &str, c: Chord)`, `Keymap::reset(&mut self, id: &str)`, `Keymap::is_default(&self, id: &str) -> bool`.
  - `Keymap::resolve(&self, c: Chord, active: Tool) -> Option<Action>`: con la alternancia del spec §7.2.
  - `#[derive(Serialize, Deserialize, Default, PartialEq, Debug)] pub(crate) struct KeymapFile { pub schema: u32, pub bindings: BTreeMap<String, Vec<String>> }`.
  - `Keymap::to_file(&self) -> KeymapFile`: solo las diferencias con fábrica; una lista vacía significa «sin atajo».
  - `Keymap::from_file(f: &KeymapFile) -> (Keymap, Vec<String>)`: parte de fábrica; reemplaza los ids presentes, ignora y avisa de ids desconocidos, combinaciones inválidas y no permitidas; en un conflicto dentro del archivo gana el primero en orden del catálogo y se avisa.

- [ ] **Step 1: Write the failing tests** en `keymap.rs`. Usa `fn ch(s) -> Chord` como atajo de prueba.

```rust
#[test]
fn alternancia_de_herramientas() {
    let k = Keymap::fireworks();
    assert_eq!(k.resolve(ch("M"), Tool::Brush), Some(Action::SelectTool(Tool::Marquee)));
    assert_eq!(k.resolve(ch("M"), Tool::Marquee), Some(Action::SelectTool(Tool::Ellipse)));
    assert_eq!(k.resolve(ch("M"), Tool::Ellipse), Some(Action::SelectTool(Tool::Marquee)));
    let mut k2 = k.clone();
    k2.remove("tool.marquee", ch("M"));
    k2.assign("tool.marquee", ch("Q"));
    assert_eq!(k2.resolve(ch("M"), Tool::Ellipse), Some(Action::SelectTool(Tool::Ellipse)));
    assert_eq!(k2.resolve(ch("Q"), Tool::Brush), Some(Action::SelectTool(Tool::Marquee)));
}
#[test]
fn modificadores_exactos() {
    let k = Keymap::fireworks();
    assert_eq!(k.resolve(ch("Ctrl+Z"), Tool::Brush), Some(Action::Undo));
    assert_eq!(k.resolve(ch("Ctrl+Shift+Z"), Tool::Brush), Some(Action::Redo));
    assert_eq!(k.resolve(ch("Ctrl+Y"), Tool::Brush), None);
    assert_eq!(k.resolve(ch("Ctrl+E"), Tool::Brush), None);
}
#[test]
fn conflictos_y_reasignar() {
    let mut k = Keymap::fireworks();
    assert_eq!(k.conflict("file.export", ch("Ctrl+K")), Some("edit.palette"));
    assert_eq!(k.conflict("tool.lasso", ch("M")), None);
    assert_eq!(k.conflict("file.export", ch("M")), Some("tool.marquee"));
    k.assign("file.export", ch("Ctrl+K"));
    assert!(k.chords("edit.palette").is_empty());
    assert_eq!(k.resolve(ch("Ctrl+K"), Tool::Brush), Some(Action::Export));
}
#[test]
fn tecla_sola_solo_para_herramientas() {
    assert!(Keymap::allowed("tool.lasso", ch("Q")));
    assert!(Keymap::allowed("edit.clear", ch("Delete")));
    assert!(!Keymap::allowed("file.export", ch("X")));
    assert!(Keymap::allowed("file.export", ch("Shift+X")));
}
#[test]
fn archivo_solo_con_diferencias_y_reemplazo() {
    let mut k = Keymap::fireworks();
    k.assign("file.export", ch("Ctrl+E"));
    let f = k.to_file();
    assert_eq!(f.bindings.len(), 1);
    assert_eq!(f.bindings["file.export"], vec!["Ctrl+Shift+R".to_string(), "Ctrl+E".into()]);
    let (back, warns) = Keymap::from_file(&f);
    assert_eq!(back, k);
    assert!(warns.is_empty());
    // Importar reemplaza: un archivo vacío devuelve todo a fábrica.
    assert_eq!(Keymap::from_file(&KeymapFile { schema: 1, ..Default::default() }).0, Keymap::fireworks());
}
#[test]
fn archivo_editado_a_mano_con_errores() {
    let f: KeymapFile = toml::from_str(r#"
        schema = 1
        [bindings]
        "nada.de.nada" = ["Ctrl+J"]
        "file.export" = ["Ctrl+Nada", "X", "Ctrl+J"]
        "file.open" = ["Ctrl+J"]
    "#).unwrap();
    let (k, warns) = Keymap::from_file(&f);
    assert_eq!(warns.len(), 4); // id desconocido, tecla inválida, tecla sola, conflicto
    assert_eq!(k.chords("file.open"), &[ch("Ctrl+J")]); // file.open va antes en el catálogo
    assert!(k.chords("file.export").is_empty());
}
```

- [ ] **Step 2: Run** `cargo test -p pyro-ui --lib commands::keymap`. Expected: FAIL (no compila).
- [ ] **Step 3: Implement** `Keymap` y `KeymapFile`. Dependencias en `pyro-ui`: `serde = { version = "1", features = ["derive"] }`, `toml = "0.8"`.
- [ ] **Step 4: Run** the same tests y `cargo deny check`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(commands): configurable keymap with conflicts, tool cycling and keymap.toml`.

### Task 4: Preferencias, recientes y almacenamiento

**Files:**
- Create: `crates/pyro-ui/src/commands/store.rs`, `crates/pyro-platform/src/paths.rs`
- Modify: `crates/pyro-platform/src/lib.rs` (`pub mod paths;`), `crates/pyro-platform/Cargo.toml` (`directories = "5"`), `crates/pyro-ui/Cargo.toml` (dev: `tempfile = "3"`)

**Interfaces:**
- Produces in `pyro-platform`: `pub fn config_dir() -> Option<PathBuf>`, que devuelve `directories::ProjectDirs::from("io", "", "Pyrotecnika")?.config_dir()`.
- Produces in `store.rs`:
  - `#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)] pub(crate) struct Settings { pub schema: u32, pub appearance: Appearance, pub recent: Recent }`, con `#[serde(default)]` en todo.
  - `Appearance { theme: ThemeChoice, ui_scale: f32 }`: `ThemeChoice` gana `Serialize` y `Deserialize` con nombres `system`, `light`, `dark` y `high-contrast`.
  - `Recent { files: Vec<PathBuf> }`.
  - `Recent::push(&mut self, p: PathBuf)`, `Recent::remove(&mut self, p: &Path)`, `Recent::clear(&mut self)`. Tope 10; los duplicados se detectan con la ruta normalizada (`dunce`-like: `std::fs::canonicalize` si se puede, si no la ruta tal cual).
  - `Settings::sanitize(&mut self)`: acota `ui_scale` a 0.8–2.0 con `design::style::step_scale(v, 0)`.
  - `pub(crate) struct Store { dir: Option<PathBuf>, settings_newer: bool, keymap_newer: bool, save_failed: bool }`.
  - `Store::new(dir: Option<PathBuf>) -> Store`.
  - `Store::load_settings(&mut self) -> (Settings, Vec<String>)` y `Store::load_keymap(&mut self) -> (Keymap, Vec<String>)`.
  - `Store::save_settings(&mut self, s: &Settings) -> Result<(), String>` y `Store::save_keymap(&mut self, k: &Keymap) -> Result<(), String>`.
  - `pub(crate) fn write_atomic(path: &Path, text: &str) -> std::io::Result<()>`: archivo temporal en la misma carpeta y después renombrar.

- [ ] **Step 1: Write the failing tests** en `store.rs`, cada una con `tempfile::tempdir()`:

```rust
#[test]
fn ida_y_vuelta() {
    let d = tempdir().unwrap();
    let mut st = Store::new(Some(d.path().into()));
    let mut s = Settings::default();
    s.appearance.theme = ThemeChoice::HighContrast; s.appearance.ui_scale = 1.5;
    s.recent.push(d.path().join("a.png"));
    st.save_settings(&s).unwrap();
    assert_eq!(st.load_settings(), (s, vec![]));
}
#[test]
fn sin_archivos_valores_de_fabrica() {
    let d = tempdir().unwrap();
    let (s, w) = Store::new(Some(d.path().into())).load_settings();
    assert_eq!(s, Settings::default()); assert!(w.is_empty());
    assert_eq!(s.appearance.theme, ThemeChoice::System); assert_eq!(s.appearance.ui_scale, 1.0);
}
#[test]
fn archivo_danado_se_aparta() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("settings.toml"), "esto = [no es toml").unwrap();
    let (s, w) = Store::new(Some(d.path().into())).load_settings();
    assert_eq!(s, Settings::default()); assert_eq!(w.len(), 1);
    assert!(d.path().join("settings.toml.bak").exists());
}
#[test]
fn claves_desconocidas_y_escala_fuera_de_rango() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("settings.toml"), "schema = 1\nfuturo = 3\n[appearance]\nui_scale = 7.0\n").unwrap();
    let (s, _) = Store::new(Some(d.path().into())).load_settings();
    assert_eq!(s.appearance.ui_scale, 2.0);
}
#[test]
fn version_mas_nueva_se_respalda_al_escribir() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("settings.toml"), "schema = 9\n").unwrap();
    let mut st = Store::new(Some(d.path().into()));
    let (s, w) = st.load_settings();
    assert_eq!(w.len(), 1);
    assert!(!d.path().join("settings.toml.bak").exists());
    st.save_settings(&s).unwrap();
    assert_eq!(std::fs::read_to_string(d.path().join("settings.toml.bak")).unwrap(), "schema = 9\n");
}
#[test]
fn recientes_tope_orden_y_duplicados() {
    let mut r = Recent::default();
    for i in 0..12 { r.push(format!("f{i}.png").into()); }
    r.push("f5.png".into());
    assert_eq!(r.files.len(), 10);
    assert_eq!(r.files[0], PathBuf::from("f5.png"));
    assert_eq!(r.files.iter().filter(|p| **p == PathBuf::from("f5.png")).count(), 1);
}
#[test]
fn sin_carpeta_no_guarda_ni_falla() {
    let mut st = Store::new(None);
    assert_eq!(st.load_settings().0, Settings::default());
    assert!(st.save_settings(&Settings::default()).is_ok());
}
#[test]
fn carpeta_sin_permiso_avisa() {
    let d = tempdir().unwrap();
    let blocked = d.path().join("archivo");
    std::fs::write(&blocked, "").unwrap();
    // La «carpeta» es un archivo: crear dentro falla en los tres SO.
    let mut st = Store::new(Some(blocked.join("sub")));
    assert!(st.save_settings(&Settings::default()).is_err());
}
```

- [ ] **Step 2: Run** `cargo test -p pyro-ui --lib commands::store`. Expected: FAIL (no compila).
- [ ] **Step 3: Implement** `Store`, que crea la carpeta al guardar. Para `keymap.toml` aplica las mismas reglas de archivo dañado y `schema` mayor.
- [ ] **Step 4: Run** `cargo test -p pyro-ui -p pyro-platform` y `cargo deny check`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(commands): settings, recent files and keymap persisted as TOML`.

### Task 5: La app usa el catálogo: teclado, menús, recientes y persistencia

**Files:**
- Modify:
  - `crates/pyro-ui/src/app/input.rs`: quita las tablas fijas y resuelve con `keymap.resolve`.
  - `crates/pyro-ui/src/app/panels.rs`: menús desde el catálogo, menú Capas nuevo, tooltips de los botones de capa y de herramienta.
  - `crates/pyro-ui/src/app/mod.rs`: estado nuevo y `run()`.
  - `crates/pyro-ui/src/app/scenes.rs`.
- Create: `crates/pyro-ui/src/app/menus.rs`, si `panels.rs` pasa de ~450 líneas.

**Interfaces:**
- Consumes: `CATALOG`, `Keymap`, `Store`, `Settings`, `is_mac()`.
- Produces:
  - campos de `PyroApp`: `keymap: Keymap`, `settings: Settings`, `store: Store`;
  - `PyroApp::with_config(cc: &eframe::CreationContext<'_>, dir: Option<PathBuf>) -> Self`. `PyroApp::new(cc)` llama a `with_config(cc, pyro_platform::paths::config_dir())`, y `scenes.rs` pasa `None` o una carpeta temporal;
  - `PyroApp::save_settings(&mut self)`, que si falla muestra un único aviso en la barra de estado;
  - `pub(super) fn menu_item(&mut self, ui, id: &str, enabled: bool) -> Option<Action>`, que pinta nombre y atajo vigentes del catálogo.
- Comportamiento:
  - al arrancar se cargan `settings.toml` y `keymap.toml` con `store`, se aplican el tema y la escala guardados, y los avisos de carga van a la barra de estado;
  - `SetTheme` y `UiScale*` guardan los ajustes;
  - `open_path` correcto hace `settings.recent.push(path)` y guarda;
  - `OpenRecent(i)` pide `Pending::Open` si el archivo existe; si no, avisa «Ya no existe «nombre»; se quitó de recientes.», quita la entrada y guarda, sin tocar el documento;
  - menú Archivo: Nuevo…, Abrir…, Abrir reciente ▸ (nombres con la ruta en el tooltip, separador, Limpiar lista), Exportar…, Mostrar en carpeta (por ahora deshabilitado, hasta la tarea 6), separador, Salir;
  - menú Editar: Deshacer, Rehacer, separador, Borrar selección, Recortar a la selección, separador, Paleta de comandos, Preferencias…;
  - menú Capas: Nueva capa, Duplicar capa, Eliminar capa, separador, Subir capa, Bajar capa, separador, Combinar abajo;
  - Ver y Ayuda como hoy, con las entradas del catálogo;
  - los botones de capa ejecutan su `Action` y llevan el tooltip «Nombre · atajo»; el tooltip de la herramienta usa `keymap.label`.

- [ ] **Step 1: Write the failing tests** (kittest, en `scenes.rs` o en un `app/tests_commands.rs` con `#[cfg(test)]`):

```rust
#[test]
fn exportar_con_el_atajo_de_fireworks() {
    let mut h = scene(Scene::Main, ThemeChoice::Dark, 1.0, egui::vec2(1280.0, 800.0));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::R);
    h.run_steps(2);
    assert!(h.state().export_dialog.is_some());
}
#[test]
fn ctrl_e_ya_no_exporta() { /* Ctrl+E → export_dialog sigue None */ }
#[test]
fn ctrl_mas_del_teclado_espanol_acerca() { /* zoom tras Ctrl+Plus > zoom inicial; igual con Ctrl+Equals */ }
#[test]
fn m_alterna_marco_y_elipse() { /* M, M → tool == Ellipse; M → Marquee */ }
#[test]
fn ningun_atajo_con_un_campo_de_texto_enfocado() {
    /* enfoca «Nombre» del panel de capas, pulsa Ctrl+Alt+E y la tecla E:
       el número de capas no cambia y la herramienta sigue siendo Pincel */
}
#[test]
fn el_menu_muestra_el_atajo_vigente() {
    /* tras h.state_mut().keymap.assign("file.export", ch("Ctrl+E")), abrir Archivo:
       existe un nodo con la etiqueta «Exportar…» y otro con «Ctrl+E» */
}
#[test]
fn reciente_que_ya_no_existe() {
    /* con un Recent que apunta a <tempdir>/borrado.png, run(OpenRecent(0)):
       recent vacío, status Warn, el documento sigue con las mismas dimensiones */
}
#[test]
fn tema_y_escala_se_recuerdan() {
    /* with_config(tempdir): SetTheme(Light) y UiScaleUp; una app nueva con la misma
       carpeta arranca con Light y 1.1 */
}
```

  Los cuerpos marcados con `/* */` son la aserción exacta que debe escribirse; la forma de llegar al estado (helpers de kittest) queda al implementador.
- [ ] **Step 2: Run** `cargo test -p pyro-ui`. Expected: FAIL. Las pruebas nuevas no compilan o fallan.
- [ ] **Step 3: Implement** según «Comportamiento». `handle_input` conserva su prioridad actual: confirmación abierta o `ctx.wants_keyboard_input()` → nada; con el diálogo de filtro abierto, `run()` ya filtra.
- [ ] **Step 4: Run** `cargo test -p pyro-ui`. Expected: PASS. Revisa la escena principal: `UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored` cambia solo por el menú Capas (que no se ve cerrado) o no cambia. Si cambia algo más, investígalo.
- [ ] **Step 5: Commit** `feat(ui): menus, keyboard and layer buttons from the command catalog; persisted theme, scale and recent files`.

### Task 6: Mostrar en carpeta

**Files:**
- Create: `crates/pyro-platform/src/reveal.rs`
- Modify: `crates/pyro-platform/src/lib.rs`, `crates/pyro-ui/src/app/mod.rs` (`RevealInFolder`), `panels.rs`/`menus.rs` (habilitar si `source_path.is_some()`)

**Interfaces:**
- Produces:
  - `pub fn reveal(path: &Path) -> Result<(), String>`.
  - `pub(crate) fn reveal_commands(path: &Path) -> Vec<(String, Vec<OsString>)>`, intentos en orden según el SO:
    - Windows: `explorer` con `/select,<ruta>`;
    - macOS: `open -R <ruta>`;
    - Linux: `dbus-send --session --dest=org.freedesktop.FileManager1 --type=method_call /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems array:string:file://<ruta> string:""` y, si falla, `xdg-open <carpeta>`.
  - `reveal` lanza cada intento con `std::process::Command` y para en el primero que arranca y sale con éxito. Ojo: `explorer` devuelve 1 aunque funcione, así que en Windows basta con que arranque.

- [ ] **Step 1: Write the failing test** en `reveal.rs` (sin `cfg`: comprueba el plan del SO actual):

```rust
#[test]
fn plan_para_este_so() {
    let p = std::env::temp_dir().join("a b.png");
    let cmds = reveal_commands(&p);
    assert!(!cmds.is_empty());
    let all: String = cmds.iter().flat_map(|(_, a)| a.iter().map(|s| s.to_string_lossy().into_owned())).collect();
    assert!(all.contains("a b.png") || all.contains("a%20b.png"));
}
```

  Además, en `pyro-ui`: `run(RevealInFolder)` sin `source_path` deja un aviso y no lanza nada.
- [ ] **Step 2: Run** `cargo test -p pyro-platform reveal`. Expected: FAIL (no compila).
- [ ] **Step 3: Implement.** En la URI `file://` de Linux, codifica los espacios y caracteres no ASCII. Si falla, avisa en la barra de estado: «No se pudo abrir la carpeta: …».
- [ ] **Step 4: Run** `cargo test -p pyro-platform -p pyro-ui`. Expected: PASS. A mano en Windows: `cargo run`, abre un archivo, Archivo › Mostrar en carpeta selecciona el archivo en el Explorador.
- [ ] **Step 5: Commit** `feat(platform): show the current file in the system file manager`.

### Task 7: Paleta de comandos

**Files:**
- Create: `crates/pyro-ui/src/app/palette.rs`

**Interfaces:**
- Produces:
  - `pub(super) struct Palette { query: String, selected: usize }`.
  - `PyroApp.palette: Option<Palette>`, con `OpenPalette` → `Some`.
  - `pub(crate) fn normalize(s: &str) -> String`: minúsculas, sin tildes ni diéresis (á→a, é→e, í→i, ó→o, ú→u, ü→u, ñ→n), sin «…».
  - `pub(super) fn matches(query: &str, entries: &[Entry]) -> Vec<usize>`, con `pub(super) struct Entry { label: String, category: &'static str, shortcut: String, available: bool, action: Action }`. Puntuación: prefijo de palabra > subcadena > iniciales (cada carácter de la consulta abre una palabra, en orden). Dentro de cada grupo, primero los disponibles; el empate se resuelve por orden del catálogo. Una consulta vacía devuelve todos.
  - `pub(super) fn palette_window(&mut self, ctx)`: una `egui::Area` o `Window` sin título, centrada arriba (y = 15 % del alto), 520 px de ancho, con fondo `bg_raised` y el campo con foco al abrir.
    - Lista con nombre, categoría en texto débil y atajo en monoespaciada; los comandos no disponibles van deshabilitados.
    - ↑/↓ mueven la selección, Intro ejecuta con `self.run(action)` y cierra, Esc o un clic fuera cierra.
    - Incluye «Abrir reciente: nombre» por cada reciente, con `OpenRecent(i)`.
    - La disponibilidad sale de la misma función que los menús, `fn available(&self, a: Action) -> bool` en `mod.rs`, que también usan los menús.

- [ ] **Step 1: Write the failing tests** en `palette.rs`:

```rust
#[test]
fn sin_tildes_ni_mayusculas() {
    assert_eq!(normalize("Desenfoque Gaussiano…"), "desenfoque gaussiano");
    assert_eq!(normalize("Selección"), "seleccion");
}
#[test]
fn busqueda() {
    let e = entries_for_test(); // catálogo completo, todos disponibles
    let first = |q: &str| e[matches(q, &e)[0]].label.clone();
    assert_eq!(first("gaus"), "Desenfoque gaussiano…");
    assert_eq!(first("seleccion"), "Seleccionar todo");
    assert_eq!(first("ca"), "Combinar abajo"); // iniciales
    assert!(matches("zzz", &e).is_empty());
}
#[test]
fn no_disponibles_al_final() { /* con «Deseleccionar» no disponible, «desel» lo devuelve igualmente pero detrás de cualquier disponible que coincida */ }
```

  Y con kittest: `ctrl_k_busca_y_ejecuta`. Ctrl+K, escribir «gaus», Intro: `filter_dialog.is_some()`, `palette.is_none()`. También `esc_cierra_la_paleta`.
- [ ] **Step 2: Run** `cargo test -p pyro-ui --lib app::palette`. Expected: FAIL.
- [ ] **Step 3: Implement.** Llama a `palette_window` en `update()` después de `update_window` y antes de `confirm_window`.
- [ ] **Step 4: Run** `cargo test -p pyro-ui`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): command palette (Ctrl+K)`.

### Task 8: Preferencias y editor de atajos

**Files:**
- Create: `crates/pyro-ui/src/app/prefs.rs` (ventana y sección Apariencia), `crates/pyro-ui/src/app/shortcuts.rs` (sección Atajos)

**Interfaces:**
- Produces:
  - `pub(super) struct PrefsDialog { section: Section, search: String, selected: Option<&'static str>, capture: Capture, pending: Option<(&'static str, Chord)> }`, con `enum Section { Appearance, Shortcuts }` y `enum Capture { Idle, Waiting }`.
  - `PyroApp.prefs: Option<PrefsDialog>`, con `OpenPreferences` → `Some`.
  - `pub(super) fn prefs_window(&mut self, ctx)`: `egui::Modal`, 640×480 como máximo, con pestañas «Apariencia» y «Atajos».
    - **Apariencia**: radios del tema y botones de la escala; escriben lo mismo que el menú Ver y guardan.
    - **Atajos**: búsqueda por nombre o por etiqueta de atajo, lista agrupada por `Category::label`.
      - Cada fila: nombre y atajos vigentes. Las modificadas van en semibold y con «(modificado)» en el nombre accesible.
      - Bajo la lista, para el comando elegido: chips con sus atajos (× quita), «+ Añadir», «Restablecer».
      - Pie: «Exportar…», «Importar…», «Restablecer todos a Fireworks».
      - Los atajos se guardan con `store.save_keymap` en cada cambio.
  - Captura:
    - en `Waiting`, el siguiente evento `Key` con su modificador forma el `Chord`;
    - Esc vuelve a `Idle` y el modal no se cierra;
    - una combinación no permitida da el aviso «Esta tecla sola solo vale para herramientas; añade Ctrl, Alt o Mayús.»;
    - si `conflict` devuelve un id, se guarda en `pending` y se muestra «⚠ {atajo} ya lo usa «{nombre}».» con [Reasignar] [Cancelar];
    - mientras captura o con el modal abierto, `handle_input` no resuelve atajos. Ya ocurre porque el modal está abierto: añade `self.prefs.is_some()` a la condición de salida de `handle_input`.
  - Exportar e Importar usan `rfd::FileDialog` con el filtro «Atajos de Pyrotecnika» `toml`. Importar avisa cuántas entradas se ignoraron, con los mensajes de `from_file`.

- [ ] **Step 1: Write the failing tests** (kittest). `reasignar_un_atajo` es el flujo de `ARQUITECTURA.md` §7:

```rust
#[test]
fn reasignar_un_atajo() {
    // Preferencias › Atajos › «Exportar…» › + Añadir › Ctrl+Shift+E
    // → el menú Archivo muestra «Ctrl+Mayús+E» junto a Exportar…, y la pulsación abre el diálogo de exportar.
}
#[test]
fn conflicto_pide_confirmacion() {
    // + Añadir sobre «Exportar…» y Ctrl+K → aparece el texto «ya lo usa «Paleta de comandos»»;
    // Cancelar → keymap sin cambios; repetir y Reasignar → Ctrl+K exporta y la paleta queda sin atajo.
}
#[test]
fn esc_cancela_la_captura_sin_cerrar() {
    // + Añadir, Esc → prefs.is_some(), capture == Idle, keymap sin cambios.
}
#[test]
fn la_captura_no_ejecuta_el_atajo() {
    // + Añadir sobre «Exportar…», Ctrl+Z → no se deshace nada (history sin cambios) y aparece el conflicto con «Deshacer».
}
#[test]
fn restablecer_todos() {
    // tras dos cambios, «Restablecer todos a Fireworks» → keymap == Keymap::fireworks() y keymap.toml no tiene bindings.
}
```

- [ ] **Step 2: Run** `cargo test -p pyro-ui`. Expected: FAIL.
- [ ] **Step 3: Implement** con los widgets del sistema de diseño: `active_row` para la fila elegida y `primary_button` para «Reasignar».
- [ ] **Step 4: Run** `cargo test -p pyro-ui`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(ui): preferences window with appearance and shortcut editor`.

### Task 9: Capturas, documentación y verificación final

**Files:**
- Modify:
  - `crates/pyro-ui/src/app/scenes.rs`: escenas `Palette` («gaus» escrito) y `Prefs` (Atajos con «Exportar…» elegido y el aviso de conflicto visible). Ambas en los tres temas, más `PrefsAppearance` solo en oscuro.
  - `docs/ARQUITECTURA.md`:
    - §1.5: recientes en `settings.toml`; SQLite pospuesto hasta que haya presets.
    - §5.1: RF-13 a RF-16 a ✔ con el alcance del spec, y RF-16 con nota de qué preferencias llegan en qué fase.
    - §9.1: fase 2 ✔.
  - `DESIGN.md`: paleta y Preferencias como componentes.
- Create: `docs/ATAJOS.md`, la tabla de atajos de fábrica generada del catálogo.
- Test: `commands::catalog::tests::tabla_de_atajos_al_dia`. Genera el markdown a partir de `CATALOG` con `fn shortcuts_markdown() -> String` (tabla «Categoría | Comando | Atajo», etiquetas de Windows) y lo compara con `docs/ATAJOS.md` usando `include_str!`. Si no coincide, el mensaje dice cómo regenerarlo: `PYRO_WRITE_ATAJOS=1 cargo test -p pyro-ui tabla_de_atajos` lo escribe.

- [ ] **Step 1:** Escribe `tabla_de_atajos_al_dia` y comprueba que falla sin `docs/ATAJOS.md`.
- [ ] **Step 2:** Genera `docs/ATAJOS.md` con la variable de entorno y comprueba que la prueba pasa.
- [ ] **Step 3:** Añade las escenas y genera las capturas con `UPDATE_SNAPSHOTS=1 cargo test -p pyro-ui -- --ignored`. Revisa cada una (inspección acotada: una ronda y, como mucho, otra de arreglos) y **enséñaselas al autor** antes del commit.
- [ ] **Step 4:** Actualiza `ARQUITECTURA.md` y `DESIGN.md`.
- [ ] **Step 5:** Verificación final. Todo debe salir limpio:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `cargo test --workspace`;
  - `cargo deny check`;
  - la regla de colores del CI.
- [ ] **Step 6: Commit** `docs: phase 2 shortcuts table, architecture and design updates; reference snapshots`.
